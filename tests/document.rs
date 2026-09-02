//! Public strict PostgreSQL-document seam (issue #10).
//!
//! `document::parse_sql` accepts zero or more semicolon-separated PostgreSQL
//! statements with an optional final semicolon and no psql-only syntax. The
//! tests cover the reviewed happy-path matrix, the exact source-ownership
//! partition, the grammar-erased recovery projection, and psql rejection.

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

// --- Recovery projection ----------------------------------------------------

fn recover(source: &str) -> document::SqlRecovery<'_> {
    match document::parse_sql(source) {
        Ok(_) => panic!("{source:?} must be rejected"),
        Err(SqlParseError::Recovered(recovery)) => recovery,
        Err(other) => panic!("{source:?} must reject through recovery, got {other}"),
    }
}

#[test]
fn invalid_input_returns_a_nonempty_source_covering_recovery() {
    let source = "SELECT FROM;";
    let recovery = recover(source);
    assert_eq!(recovery.source(), source);
    assert_eq!(recovery.render_exact(), source);
    assert!(recovery.parts().count() >= 1, "nonempty projection");
    assert_eq!(recovery.islands().count(), 1);
    let island = recovery.islands().next().expect("one recovered island");
    assert!(
        !island.diagnostics().is_empty(),
        "the failed island carries diagnostics"
    );
    assert_owned_once(source, recovery.parts().map(|part| part.span()));
}

#[test]
fn recovery_is_grammar_erased_and_covers_later_valid_islands() {
    let source = "SELECT FROM;\nSELECT 1;";
    let recovery = recover(source);
    let islands = recovery.islands().collect::<Vec<_>>();
    assert_eq!(islands.len(), 2, "every island is erased after one failure");
    assert!(
        islands
            .iter()
            .all(|island| island.root().schema().name() == "SqlDocumentItem"),
        "recovered islands expose only the erased grammar projection"
    );
    assert!(
        !islands[0].diagnostics().is_empty(),
        "the failed island keeps its diagnostics"
    );
    assert!(
        islands[1].diagnostics().is_empty(),
        "a strictly valid erased island gains no spurious diagnostic"
    );
    assert!(
        islands
            .iter()
            .all(|island| island.progress_trace().is_within_bound()),
        "every replay keeps hard bounded progress"
    );
    assert_owned_once(source, recovery.parts().map(|part| part.span()));
}

#[test]
fn recovery_diagnostics_are_stable_across_parses() {
    let source = "SELECT FROM;\nSELECT 1 1;";
    let first = recover(source);
    let second = recover(source);
    let collect = |recovery: &document::SqlRecovery<'_>| {
        recovery
            .islands()
            .flat_map(|island| {
                island
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
            })
            .collect::<Vec<_>>()
    };
    let diagnostics = collect(&first);
    assert!(!diagnostics.is_empty(), "stable diagnostics exist");
    assert_eq!(diagnostics, collect(&second));
}

#[test]
fn lexical_failures_and_unclosed_regions_recover() {
    for source in [
        "SELECT 'unterminated;",
        "SELECT /* never closed;",
        "SELECT $tag$never closed;",
        "SELECT (1;",
        "SELECT 'é side of an unterminated multibyte literal;",
    ] {
        let recovery = recover(source);
        assert_eq!(recovery.render_exact(), source, "exact for {source:?}");
        assert_owned_once(source, recovery.parts().map(|part| part.span()));
    }
}

#[test]
fn missing_boundary_between_statements_fails_closed() {
    // `SELECT` is a bare-label keyword, so the second `SELECT` is absorbed
    // as a column alias before the strict failure at `2`: the whole
    // candidate stays one fail-closed island rather than guessing a split.
    let source = "SELECT 1 SELECT 2";
    let recovery = recover(source);
    assert_eq!(recovery.render_exact(), source);
    let codes = recovery
        .diagnostics()
        .iter()
        .map(pg_sql::FrameDiagnostic::code)
        .collect::<Vec<_>>();
    assert_eq!(codes, ["RCA5002"], "ambiguous ownership fails closed");
    assert_eq!(recovery.islands().count(), 1);
    assert!(
        !recovery
            .parts()
            .any(|part| matches!(part, pg_sql::RecoveredPart::Unresolved(_))),
        "no restart evidence, so recovery owns the whole candidate"
    );
    assert_owned_once(source, recovery.parts().map(|part| part.span()));

    // A token that can only begin a new statement is generated restart
    // evidence: framing retains the suffix explicitly instead of guessing.
    let source = "SELECT 1 CREATE TABLE t (a int)";
    let recovery = recover(source);
    assert_eq!(recovery.render_exact(), source);
    let codes = recovery
        .diagnostics()
        .iter()
        .map(pg_sql::FrameDiagnostic::code)
        .collect::<Vec<_>>();
    assert_eq!(codes, ["RCA5002"], "the missing boundary fails closed");
    assert!(
        recovery
            .parts()
            .any(|part| matches!(part, pg_sql::RecoveredPart::Unresolved(_))),
        "the unassignable suffix is retained explicitly"
    );
    assert_owned_once(source, recovery.parts().map(|part| part.span()));
}

// --- Psql rejections --------------------------------------------------------

fn assert_recovered_rejection(source: &str) {
    match document::parse_sql(source) {
        Ok(_) => panic!("{source:?} is psql-only and must be rejected"),
        Err(SqlParseError::Recovered(recovery)) => {
            assert_eq!(recovery.render_exact(), source);
            assert_owned_once(source, recovery.parts().map(|part| part.span()));
        }
        Err(other) => panic!("{source:?} must reject through recovery, got {other}"),
    }
}

#[test]
fn psql_directives_are_rejected() {
    for source in ["\\d users", "\\set foo 1", "\\connect mydb", "\\timing on"] {
        assert_recovered_rejection(source);
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
        assert_recovered_rejection(source);
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
        assert_recovered_rejection(source);
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
        assert_recovered_rejection(source);
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
