//! Public strict PostgreSQL-document seam (issue #10).
//!
//! `document::parse_sql` accepts zero or more semicolon-separated PostgreSQL
//! statements with an optional final semicolon and no psql-only syntax. The
//! tests cover the reviewed happy-path matrix, the exact source-ownership
//! partition, the strict rejection of invalid input, and psql rejection.

use pg_sql::ast::{Statement, dml::select::SelectBody, dml::values::Subquery};
use pg_sql::document::{self, SqlParseError};
use recursa::Span;

/// Proves the ordered spans own every source byte exactly once.
fn assert_owned_once(source: &str, spans: impl IntoIterator<Item = Span>) {
    let mut cursor = 0_usize;
    let mut rendered = String::new();
    for span in spans {
        assert_eq!(
            span.start() as usize,
            cursor,
            "part starts where previous ended"
        );
        rendered.push_str(&source[span.range()]);
        cursor = span.end() as usize;
    }
    assert_eq!(cursor, source.len(), "parts reach end of input");
    assert_eq!(rendered, source, "concatenated parts reproduce the source");
}

fn parse(source: &str) -> document::SqlDocument<'_> {
    document::parse_sql(source)
        .unwrap_or_else(|error| panic!("strict document {source:?}: {error}"))
}

// --- Happy paths -----------------------------------------------------------

#[test]
fn empty_document_is_complete_with_no_statements() {
    let doc = parse("");
    assert_eq!(doc.source(), "");
    assert_eq!(doc.render_exact(), "");
    assert_eq!(doc.statements().len(), 0);
    assert_eq!(doc.items().count(), 0);
    assert_owned_once("", doc.part_spans());
}

#[test]
fn trivia_only_and_comment_only_documents_are_complete() {
    for source in [
        " \t\r\n",
        "-- comment only",
        "/* block comment; with a semicolon spelling */",
        " \r\n-- c\r\n",
        "/* outer /* nested; */ still outer */\n",
    ] {
        let doc = parse(source);
        assert_eq!(doc.render_exact(), source);
        assert_eq!(doc.statements().len(), 0, "no statements in {source:?}");
        assert_eq!(doc.items().count(), 0, "no items in {source:?}");
        assert_eq!(doc.part_spans().count(), 1, "one gap part in {source:?}");
        assert_owned_once(source, doc.part_spans());
    }
}

#[test]
fn one_statement_with_final_semicolon() {
    let doc = parse("SELECT 1;");
    assert_eq!(doc.statements().len(), 1);
    assert_eq!(doc.items().count(), 1);
    assert_owned_once("SELECT 1;", doc.part_spans());
}

#[test]
fn bare_final_statement_without_semicolon_is_complete() {
    let doc = parse("SELECT 1");
    assert_eq!(doc.statements().len(), 1);
    assert_eq!(doc.items().count(), 1);
    assert_eq!(doc.render_exact(), "SELECT 1");
    assert_owned_once("SELECT 1", doc.part_spans());
}

#[test]
fn multi_statement_document_keeps_source_order() {
    let source = "SELECT 1;\nCREATE TABLE t (a int);\nSELECT 2;";
    let doc = parse(source);
    assert_eq!(doc.items().count(), 3);
    let kinds = doc
        .statements()
        .iter()
        .map(|statement| match statement {
            Statement::Query(_) => "query",
            Statement::CreateTable(_) => "create-table",
            _ => "other",
        })
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["query", "create-table", "query"]);
    assert_owned_once(source, doc.part_spans());
}

#[test]
fn statements_are_semantically_typed() {
    let doc = parse("SELECT 1;");
    let statement = doc.statements().first().expect("one statement");
    assert!(matches!(
        statement,
        Statement::Query(query)
            if matches!(
                query.as_ref(),
                Subquery::Body(body) if matches!(&body.body, SelectBody::Select(_))
            )
    ));
}

#[test]
fn optional_final_semicolon_parses_the_same_statement_list() {
    for source in ["SELECT 1", "SELECT 1;", "SELECT 1;\n-- trailing\n"] {
        let doc = parse(source);
        assert_eq!(doc.statements().len(), 1, "one statement in {source:?}");
        assert_eq!(doc.render_exact(), source);
        assert_owned_once(source, doc.part_spans());
    }
}

// --- Empty statements ------------------------------------------------------

#[test]
fn empty_statements_stay_out_of_the_semantic_list() {
    // (source, items, statements)
    for (source, items, statements) in [
        (";", 1, 0),
        (";;", 2, 0),
        (";;;", 3, 0),
        (";SELECT 1;", 2, 1),
        ("SELECT 1;;SELECT 2;", 3, 2),
        ("  ;  ; SELECT 1;", 3, 1),
        (";SELECT 1", 2, 1),
    ] {
        let doc = parse(source);
        assert_eq!(doc.items().count(), items, "items in {source:?}");
        assert_eq!(
            doc.statements().len(),
            statements,
            "statements in {source:?}"
        );
        assert_eq!(doc.render_exact(), source);
        assert_owned_once(source, doc.part_spans());
    }
}

#[test]
fn empty_statements_remain_provenance_occurrences() {
    let source = ";SELECT 1;";
    let doc = parse(source);
    // Both items own their exact island extent, boundary included: the
    // empty statement stays a source occurrence in the checked partition.
    let island_texts = doc
        .parts()
        .filter_map(|part| match part {
            pg_sql::CompletePart::Island(island) => Some(&source[island.span().range()]),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(island_texts, [";", "SELECT 1;"]);
    // The non-empty item additionally carries captured parse provenance.
    let items = doc.items().collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[1].source().expect("statement island provenance"),
        "SELECT 1;"
    );
    assert_eq!(doc.statements().len(), 1);
}

// --- Provenance and trivia ownership ---------------------------------------

#[test]
fn islands_carry_absolute_bounded_provenance() {
    let source = "SELECT 1;\nSELECT 22;";
    let doc = parse(source);
    let items = doc.items().collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].source().expect("first island source"), "SELECT 1;");
    assert_eq!(
        items[1].source().expect("second island source"),
        "SELECT 22;"
    );
    let bounds = items[1].source_bounds().expect("second island bounds");
    assert_eq!(bounds.start(), 10, "absolute document offsets");
    assert_eq!(bounds.end() as usize, source.len());
}

#[test]
fn inter_item_trivia_attaches_to_the_right_item() {
    let source = "SELECT 1;\n-- belongs to two\nSELECT 2;";
    let doc = parse(source);
    let spans = doc.part_spans().collect::<Vec<_>>();
    // Island, right-owned gap, island: the comment gap immediately precedes
    // the second island, so it belongs to that item, not to the first.
    assert_eq!(spans.len(), 3);
    assert_eq!(&source[spans[0].range()], "SELECT 1;");
    assert_eq!(&source[spans[1].range()], "\n-- belongs to two\n");
    assert_eq!(&source[spans[2].range()], "SELECT 2;");
    assert_eq!(doc.eof_trivia(), None);
}

#[test]
fn eof_trivia_belongs_to_the_document_root() {
    let source = "SELECT 1; -- tail comment";
    let doc = parse(source);
    assert_eq!(doc.eof_trivia(), Some(" -- tail comment"));
    assert_owned_once(source, doc.part_spans());

    assert_eq!(parse("SELECT 1;").eof_trivia(), None);
    assert_eq!(parse("").eof_trivia(), None);
}

// --- Exact rendering and byte ownership ------------------------------------

#[test]
fn exact_rendering_preserves_crlf_missing_newline_and_utf8() {
    for source in [
        "SELECT 'é';\r\nSELECT 'β';\r\n",
        "SELECT 1;\nSELECT 2",
        "-- naïve comment\nSELECT 'proseçção';",
        "SELECT '🦀';",
        "SELECT 1;\r\n;\r\n",
    ] {
        let doc = parse(source);
        assert_eq!(doc.render_exact(), source);
        assert_owned_once(source, doc.part_spans());
    }
}

// --- Strict rejection --------------------------------------------------------

fn reject(source: &str) -> document::SqlRejection<'_> {
    match document::parse_sql(source) {
        Ok(_) => panic!("{source:?} must be rejected"),
        Err(SqlParseError::Rejected(rejection)) => rejection,
        Err(other) => panic!("{source:?} must reject as invalid input, got {other}"),
    }
}

#[test]
fn invalid_input_is_rejected_at_its_first_failing_statement() {
    let source = "SELECT FROM;";
    let rejection = reject(source);
    assert_eq!(rejection.source(), source);
    assert_eq!(&source[rejection.island().range()], "SELECT FROM;");
    assert!(
        !rejection.diagnostics().is_empty(),
        "the failed statement carries its strict diagnostics"
    );
    assert!(rejection.framing().is_none(), "a syntax failure has no framing cause");
    let failure = rejection.span();
    assert!(failure.start() >= rejection.island().start());
    assert!(failure.end() <= rejection.island().end());
}

#[test]
fn a_later_valid_statement_does_not_rescue_an_earlier_failure() {
    let source = "SELECT FROM;\nSELECT 1;";
    let rejection = reject(source);
    assert_eq!(
        &source[rejection.island().range()],
        "SELECT FROM;",
        "the rejection names the first failing statement only"
    );
    assert!(!rejection.diagnostics().is_empty());
}

#[test]
fn rejection_diagnostics_are_stable_across_parses() {
    let source = "SELECT FROM;\nSELECT 1 1;";
    let collect = |rejection: &document::SqlRejection<'_>| {
        rejection
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.kind(),
                    diagnostic.span(),
                    diagnostic.message().to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    let diagnostics = collect(&reject(source));
    assert!(!diagnostics.is_empty(), "stable diagnostics exist");
    assert_eq!(diagnostics, collect(&reject(source)));
}

#[test]
fn lexical_failures_and_unclosed_regions_are_rejected() {
    for source in [
        "SELECT 'unterminated;",
        "SELECT /* never closed;",
        "SELECT $tag$never closed;",
        "SELECT (1;",
        "SELECT 'é side of an unterminated multibyte literal;",
    ] {
        let rejection = reject(source);
        assert_eq!(rejection.source(), source, "source retained for {source:?}");
        assert!(
            !rejection.diagnostics().is_empty(),
            "strict diagnostics exist for {source:?}"
        );
    }
}

#[test]
fn missing_boundary_between_statements_fails_closed() {
    // `SELECT` is a bare-label keyword, so the second `SELECT` is absorbed
    // as a column alias before the strict failure at `2`: the whole
    // candidate is one rejected statement rather than a guessed split, and
    // the complete prefix before `2` makes the failure a missing boundary.
    let source = "SELECT 1 SELECT 2";
    let rejection = reject(source);
    assert_eq!(rejection.island(), Span::new(0, source.len()).unwrap());
    assert_eq!(
        rejection.framing().map(pg_sql::FrameDiagnostic::code),
        Some("RCA5002"),
        "ambiguous ownership fails closed"
    );
    assert_eq!(&source[rejection.diagnostics()[0].span().range()], "2");

    // A reserved keyword cannot continue the statement, so the statement
    // parses completely and the leftover token names the missing boundary.
    let source = "SELECT 1 CREATE TABLE t (a int)";
    let rejection = reject(source);
    assert_eq!(rejection.island(), Span::new(0, source.len()).unwrap());
    let framing = rejection
        .framing()
        .expect("a missing boundary names its framing cause");
    assert_eq!(framing.code(), "RCA5002", "the missing boundary fails closed");
    assert_eq!(&source[framing.span().range()], "CREATE");
    assert_eq!(&source[rejection.diagnostics()[0].span().range()], "CREATE");
}

// --- Psql rejections --------------------------------------------------------

fn assert_strict_rejection(source: &str) {
    match document::parse_sql(source) {
        Ok(_) => panic!("{source:?} is psql-only and must be rejected"),
        Err(SqlParseError::Rejected(rejection)) => {
            assert_eq!(rejection.source(), source);
            assert!(
                !rejection.diagnostics().is_empty() || rejection.framing().is_some(),
                "{source:?} names its strict or framing cause"
            );
        }
        Err(other) => panic!("{source:?} must reject as invalid input, got {other}"),
    }
}

#[test]
fn psql_directives_are_rejected() {
    for source in ["\\d users", "\\set foo 1", "\\connect mydb", "\\timing on"] {
        assert_strict_rejection(source);
    }
}

#[test]
fn psql_send_commands_are_rejected() {
    for source in [
        "SELECT 1 \\gset",
        "SELECT 1 \\g output.txt",
        "SELECT 1 \\gx",
        "SELECT 1 \\gexec",
        "SELECT 1 \\crosstabview",
    ] {
        assert_strict_rejection(source);
    }
}

#[test]
fn psql_query_buffer_escapes_are_rejected() {
    for source in [
        "SELECT 1 \\; SELECT 2;",
        "\\e",
        "\\p",
        "\\r",
        "\\w file.sql",
    ] {
        assert_strict_rejection(source);
    }
}

#[test]
fn psql_interpolation_is_rejected() {
    for source in [
        "SELECT :var;",
        "SELECT :'var';",
        "SELECT :\"var\";",
        "SELECT * FROM t WHERE a = :filter;",
        "COPY t FROM :'filename';",
        "SELECT bigint :'txid';",
    ] {
        match document::parse_sql(source) {
            Ok(_) => panic!("{source:?} is psql interpolation and must be rejected"),
            Err(SqlParseError::Psql(_)) => {}
            Err(other) => panic!("{source:?} must reject as psql syntax, got {other}"),
        }
    }
}

#[test]
fn psql_rejection_names_the_offending_statement() {
    let source = "SELECT 1;\nSELECT :x;";
    let Err(SqlParseError::Psql(psql)) = document::parse_sql(source) else {
        panic!("interpolation in the second statement must be rejected");
    };
    assert_eq!(&source[psql.span().range()], "SELECT :x;");
}

#[test]
fn array_slices_are_not_psql_interpolation() {
    for source in [
        "SELECT arr[:2] FROM t;",
        "SELECT arr[:] FROM t;",
        "SELECT arr[1:2] FROM t;",
        "SELECT arr[:(1 + 1)] FROM t;",
    ] {
        let doc = parse(source);
        assert_eq!(doc.statements().len(), 1, "one statement in {source:?}");
    }
}

#[test]
fn copy_from_stdin_header_is_ordinary_sql() {
    let doc = parse("COPY t FROM STDIN;");
    assert_eq!(doc.statements().len(), 1);
    assert!(matches!(
        doc.statements().first().expect("copy statement"),
        Statement::Copy(_)
    ));
}

#[test]
fn copy_payload_text_is_rejected() {
    for source in [
        "COPY t FROM STDIN;\n1\tone\n2\ttwo\n\\.\n",
        "COPY t FROM STDIN;\n\\.\n",
        "\\.\n",
    ] {
        assert_strict_rejection(source);
    }
}

#[test]
fn semicolons_inside_atomic_lexical_regions_do_not_split_islands() {
    // A boundary spelling inside every closed lexical region stays inside
    // one island: plain, escape, unicode, dollar-quoted strings, quoted
    // identifiers, line comments, and nested block comments.
    for source in [
        "SELECT 'a;b';",
        "SELECT E'a\\';b';",
        "SELECT U&'a;b';",
        "SELECT $$a;b$$;",
        "SELECT $tag$a;$notag$;b$tag$;",
        "SELECT 1 AS \"col;name\";",
        "SELECT 1 -- trailing; comment\n;",
        "SELECT /* inner; semi */ 1;",
        "SELECT /* outer /* nested; */ still; */ 1;",
    ] {
        let doc = parse(source);
        assert_eq!(doc.statements().len(), 1, "one statement in {source:?}");
        assert_eq!(doc.render_exact(), source);
        assert_owned_once(source, doc.part_spans());
    }
}
