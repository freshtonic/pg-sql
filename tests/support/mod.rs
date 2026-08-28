//! Shared helpers for the differential parser test.

use recursa::Input;

pub mod diff_check;

/// One SQL statement extracted from a corpus file.
///
/// `source` is the verbatim original slice of the corpus file — never
/// pg-sql's reformatted text. The differential test reformats it itself and
/// compares against this original, so corrupting it here corrupts every
/// downstream comparison.
pub struct Stmt {
    pub source: String,
}

/// Extract the SQL statements from a corpus `.sql` file's text.
///
/// Returns the verbatim source slice of every SQL statement in the file —
/// both [`FileItem::Command`] (parsed structurally) and [`FileItem::ParseError`]
/// (failed to parse). The differential oracle reformats each slice and
/// compares it against PG, so ParseError statements still get tested: when
/// PG accepts them, the oracle records a Skip (pg-sql grammar gap); when PG
/// also rejects them, the oracle records a Pass.
///
/// psql directives (`\d`, `\set`, …) and COPY-from-stdin data blocks are
/// skipped — they are not SQL. Statements carrying psql variable
/// interpolation (`:'v'`, `:"v"`, `:var`) are not valid standalone SQL and
/// are skipped too.
pub fn extract_statements(sql_text: &str) -> Vec<Stmt> {
    use pg_sql::ast::{FileItem, PsqlCommand, parse_sql_file_with_spans};

    let lexed = pg_sql::tokens::pg_lex(sql_text);
    let mut input = Input::new(sql_text, &lexed);
    let items = match parse_sql_file_with_spans(&mut input) {
        Ok(items) => items,
        // Whole-file parse failure: nothing to extract.
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for (item, span) in &items {
        match item {
            // A parsed SQL statement — the common case.
            FileItem::Command(PsqlCommand::Statement(_)) => {}
            // A statement pg-sql could not model. Include it so the
            // differential oracle still exercises it (and surfaces Skip
            // vs Pass against PG).
            FileItem::ParseError { .. } => {}
            // Skip non-SQL items: psql `\`-directives and COPY data.
            _ => continue,
        }
        // Slice the ORIGINAL source — `parse_sql_file_with_spans` guarantees
        // the span covers exactly the statement and its terminator, with
        // leading whitespace/comments excluded.
        let source = &sql_text[span.clone()];
        // psql interpolates `:'v'` / `:"v"` / `:var` before the server ever
        // sees the SQL; the raw text is not valid standalone SQL.
        if has_psql_interpolation(source) {
            continue;
        }
        out.push(Stmt {
            source: source.to_string(),
        });
    }
    out
}

/// `true` if `sql` contains a psql variable interpolation: `:'v'`, `:"v"`, or
/// a bare `:var` reference.
///
/// The PostgreSQL cast operator `::` is *not* interpolation, so a `:`
/// immediately followed by another `:` is skipped.
fn has_psql_interpolation(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            match bytes.get(i + 1) {
                // `::` is the cast operator — consume both, not interpolation.
                Some(b':') => {
                    i += 2;
                    continue;
                }
                // `:'v'` and `:"v"` quoted interpolation.
                Some(b'\'') | Some(b'"') => return true,
                // `:var` — `:` followed by an identifier start.
                Some(&c) if c.is_ascii_alphabetic() || c == b'_' => return true,
                _ => {}
            }
        }
        i += 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_sql_skips_directives() {
        let text = "SELECT 1;\n\\d foo\nSELECT 2;\n";
        let stmts = extract_statements(text);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].source.contains("SELECT 1"));
        assert!(stmts[1].source.contains("SELECT 2"));
    }

    #[test]
    fn skips_psql_interpolation() {
        let text = "COPY t FROM :'filename';\n";
        assert_eq!(extract_statements(text).len(), 0);
    }
}
