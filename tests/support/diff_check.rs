use crate::support::Stmt;
use pg_oracle::{Equal, parse_equal, parse_ok};
use recursa::Input;

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail(String), // human-readable reason
    Skip(String), // grammar gap reason
}

/// Parse `source` with pg-sql; return the reformatted SQL, or `None` if
/// pg-sql cannot parse it structurally (raw COPY data, parse-error fallback,
/// or whole-file parse failure).
fn pgsql_format(source: &str) -> Option<String> {
    use pg_sql::ast::{FileItem, parse_sql_file};
    let lexed = pg_sql::tokens::pg_lex(source);
    let mut input = Input::new(source, &lexed);
    let items = parse_sql_file(&mut input).ok()?;
    let item = items.first()?;
    match item {
        FileItem::Command(cmd) => Some(pg_sql::formatter::format_tokens_sql(
            cmd,
            recursa::fmt::FormatStyle::default(),
        )),
        // `RawLines` is non-SQL (e.g. COPY data); `ParseError` is a single
        // SQL statement pg-sql failed to model. Neither has a reformatted
        // form — `check_statement` interprets `None` as a grammar gap (Skip
        // when PG accepts, Pass when PG also rejects).
        FileItem::RawLines(_) | FileItem::ParseError { .. } => None,
    }
}

pub fn check_statement(stmt: &Stmt) -> Outcome {
    let src = &stmt.source;

    if parse_ok(src) {
        // PostgreSQL accepts the input.
        let Some(formatted) = pgsql_format(src) else {
            return Outcome::Skip("pg-sql cannot parse it".into());
        };
        // (1) pg-sql must re-parse its own output.
        if pgsql_format(&formatted).is_none() {
            return Outcome::Fail("pg-sql cannot re-parse its own output".into());
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
        if let Some(formatted) = pgsql_format(src)
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

    // The plan's `grammar_gap_is_skip_not_fail` placeholder is intentionally
    // omitted: at implementation time, pg-sql's grammar parses every advanced
    // PostgreSQL 17.9 construct probed (MERGE, JSON_TABLE, XMLTABLE, recursive
    // CTEs, GROUPING SETS, etc.), so no statement reaches the `Skip` arm. A
    // vacuous no-op test asserts nothing; a contrived "gap" would not reflect
    // reality. The `Skip` arm is exercised honestly by the Task 9 corpus
    // driver if a real grammar gap is ever hit.
}
