use crate::support::Stmt;
use pg_oracle::{Equal, parse_equal, parse_ok};
use std::fmt;
use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String), // human-readable reason
    Skip(String), // grammar gap reason
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrictDiagnostic {
    pub code: String,
    pub region: Range<usize>,
    pub anchor: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct StrictStatementFailure {
    diagnostic: Option<StrictDiagnostic>,
    summary: String,
}

impl StrictStatementFailure {
    pub fn diagnostic(&self) -> Option<&StrictDiagnostic> {
        self.diagnostic.as_ref()
    }
}

impl fmt::Display for StrictStatementFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.summary)
    }
}

/// Lex one frozen legacy statement slice while excluding its document-level
/// semicolon from the significant tokens presented to `Statement`.
///
/// The legacy extractor deliberately leaves trivia after the terminator in the
/// statement's source range. Rebuilding the token stream keeps that complete
/// source and every retained token span unchanged; only the final significant
/// `SEMI` record is omitted. Lexically invalid input is returned verbatim so
/// its mechanism-specific diagnostic and anchor are not weakened by rebuilding.
fn lex_statement_source(source: &str) -> pg_sql::LexResult<'_> {
    let lexed = pg_sql::lex(source);
    if lexed.errors().next().is_some() {
        return lexed;
    }

    let terminator = lexed
        .tokens()
        .last()
        .filter(|token| token.kind() == pg_sql::TokenKind::SEMI)
        .map(|token| token.span());
    let Some(terminator) = terminator else {
        return lexed;
    };

    let mut statement = pg_sql::LexBuilder::new(source);
    for token in lexed.tokens() {
        if token.span() != terminator {
            statement
                .append(token.kind(), token.span())
                .expect("tokens copied from pg-sql lexing retain valid ordered spans");
        }
    }
    statement.finish()
}

/// Strictly parse one extracted statement with pg-sql and return its formatted
/// SQL or a stable diagnostic summary for the grammar gap.
pub(crate) fn pgsql_format(source: &str) -> Result<String, StrictStatementFailure> {
    use pg_sql::ast::Statement;

    let lexed = lex_statement_source(source);
    if let Some(error) = lexed.errors().next() {
        let diagnostic = StrictDiagnostic {
            code: error.code().to_owned(),
            region: error.span().range(),
            anchor: error.anchor().range(),
        };
        return Err(StrictStatementFailure {
            summary: format!(
                "pg-sql lexical failure {} at {:?}, anchor {:?}",
                diagnostic.code, diagnostic.region, diagnostic.anchor
            ),
            diagnostic: Some(diagnostic),
        });
    }
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).map_err(|error| {
        let expected = error.expected().collect::<Vec<_>>().join(", ");
        let diagnostic = StrictDiagnostic {
            code: error.code().to_owned(),
            region: error.span().range(),
            anchor: error.anchor().range(),
        };
        StrictStatementFailure {
            summary: format!(
                "pg-sql parse failure {} ({:?}) at {:?}, anchor {:?}, found {:?}, expected [{}]",
                diagnostic.code,
                error.kind(),
                diagnostic.region,
                diagnostic.anchor,
                error.found(),
                expected
            ),
            diagnostic: Some(diagnostic),
        }
    })?;
    if !input.is_eof() {
        let cursor = input.cursor();
        let trailing = lexed
            .tokens()
            .nth(cursor)
            .expect("a non-EOF public cursor addresses a significant token");
        return Err(StrictStatementFailure {
            diagnostic: None,
            summary: format!(
                "pg-sql parse left trailing input {} at {:?}, token cursor {cursor}",
                trailing.kind(),
                trailing.span().range()
            ),
        });
    }
    let ast = parsed.into_ast();
    Ok(pg_sql::formatter::format_tokens_sql(
        &ast,
        recursa::PrettyConfig::default(),
    ))
}

pub fn check_statement(stmt: &Stmt) -> Outcome {
    let src = &stmt.source;

    if parse_ok(src) {
        // PostgreSQL accepts the input.
        let formatted = match pgsql_format(src) {
            Ok(formatted) => formatted,
            Err(diagnostic) => return Outcome::Skip(diagnostic.to_string()),
        };
        // (1) pg-sql must re-parse its own output.
        if let Err(diagnostic) = pgsql_format(&formatted) {
            return Outcome::Fail(format!(
                "pg-sql cannot re-parse its own output: {diagnostic}"
            ));
        }
        // (2) PostgreSQL must see the trees as identical.
        match parse_equal(src, &formatted) {
            Equal::Equal => Outcome::Pass,
            Equal::Differ => Outcome::Fail(format!(
                "reformat changed the parse tree\n  in:  {src}\n  out: {formatted}"
            )),
            Equal::ErrorRight => {
                Outcome::Fail(format!("PostgreSQL rejects pg-sql's output: {formatted}"))
            }
            Equal::ErrorLeft => unreachable!("parse_ok said the input is valid"),
        }
    } else {
        // PostgreSQL rejects the input. pg-sql must not reformat it into
        // something PostgreSQL accepts.
        if let Ok(formatted) = pgsql_format(src)
            && parse_ok(&formatted)
        {
            return Outcome::Fail(format!(
                "over-permissive: pg-sql turned PG-rejected SQL into \
                 PG-accepted SQL\n  in:  {src}\n  out: {formatted}"
            ));
        }
        Outcome::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(sql: &str) -> Outcome {
        super::check_statement(&Stmt {
            source: sql.to_string(),
        })
    }

    #[test]
    fn faithful_statement_passes() {
        assert_eq!(check("SELECT 1 AS one"), Outcome::Pass);
    }

    #[test]
    fn pg_rejected_input_passes_when_pgsql_also_rejects() {
        // PostgreSQL rejects trailing junk; pg-sql must not "fix" it into
        // valid SQL.
        assert_eq!(check("SELECT 123abc"), Outcome::Pass);
    }

    #[test]
    fn invalid_statement_reports_stable_parse_diagnostics() {
        // The former fixture here (`DELETE ... USING t1 JOIN t2 USING (a)`)
        // resolved when suffix-proving optional viability landed; an invalid
        // statement now carries the stable-diagnostic contract instead.
        let failure = pgsql_format("DELETE FROM t3 USING t1 WHERE;")
            .expect_err("an empty WHERE expression must fail");
        assert_eq!(
            failure.diagnostic(),
            Some(&StrictDiagnostic {
                code: "RCA4101".into(),
                region: 30..30,
                anchor: 30..30,
            })
        );
    }

    #[test]
    fn grammar_gap_reports_stable_lexical_diagnostics() {
        let diagnostic =
            pgsql_format("SELECT /* unterminated").expect_err("unterminated comment must fail");
        assert_eq!(
            diagnostic.to_string(),
            "pg-sql lexical failure RCA4002 at 7..22, anchor 7..9"
        );

        let after_terminator = pgsql_format("SELECT 1; /* unterminated")
            .expect_err("a post-terminator lexical failure must not be discarded");
        assert_eq!(
            after_terminator.to_string(),
            "pg-sql lexical failure RCA4002 at 10..25, anchor 10..12"
        );
    }

    #[test]
    fn final_terminator_before_owned_line_comment_is_document_framing() {
        pgsql_format("SELECT 1;\n\n-- advisory_lock cleanup")
            .expect("the legacy slice's final semicolon must not enter Statement parsing");
    }

    #[test]
    fn final_terminator_before_owned_block_comment_is_document_framing() {
        pgsql_format("SELECT 1 <> 2; /* legacy-owned ; block comment */")
            .expect("operators and comment text must not obscure the final SQL terminator");
    }

    #[test]
    fn semicolons_inside_dollar_strings_are_not_statement_terminators() {
        pgsql_format("SELECT $$body; -- still body$$;\n-- legacy-owned comment")
            .expect("only the significant semicolon after the dollar string is framing");
        pgsql_format("SELECT $$body; -- still body$$")
            .expect("an interior semicolon must remain part of its dollar string");
    }

    #[test]
    fn terminator_exclusion_preserves_owned_source_and_token_spans() {
        let source = " \nSELECT 1;\n\n-- advisory_lock cleanup";
        let lexed = lex_statement_source(source);
        let tokens = lexed.tokens().collect::<Vec<_>>();

        assert_eq!(lexed.source(), source);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text(), "SELECT");
        assert_eq!(tokens[0].span().range(), 2..8);
        assert_eq!(tokens[1].text(), "1");
        assert_eq!(tokens[1].span().range(), 9..10);
    }

    #[test]
    fn nonfinal_statement_separator_remains_significant() {
        let lexed = lex_statement_source("SELECT 1; SELECT 2;");
        let semicolons = lexed
            .tokens()
            .filter(|token| token.kind() == pg_sql::TokenKind::SEMI)
            .collect::<Vec<_>>();

        assert_eq!(semicolons.len(), 1);
        assert_eq!(semicolons[0].span().range(), 8..9);
    }

    // The frozen baseline contains 18 PostgreSQL-accepted statements that the
    // legacy grammar skipped. The corpus driver accounts for those skips per
    // file and rejects every new skip.
}
