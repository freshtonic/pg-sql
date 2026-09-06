//! The psql client grammar, substitution, and the source map.
//!
//! The recognition tests are written against
//! `vendor/postgres/src/fe_utils/psqlscan.l`: an interpolation is recognised
//! only in psql's `INITIAL` state, so every lexical shell that scanner
//! enters — a string, a dollar-quoted body, a quoted identifier, a comment —
//! must swallow a colon whole.
//!
//! The rejection tests that used to live in `pg-sql`'s `tests/document.rs`
//! moved here with the behaviour: pg-sql no longer knows what psql is, so
//! "psql interpolation is rejected" became "psql interpolation is rendered,
//! and then pg-sql accepts the result".

use pg_psql::{Origin, PsqlItem, Variables};

fn variables(pairs: &[(&str, &str)]) -> Variables {
    pairs.iter().copied().collect()
}

/// Renders with no bindings at all, for the recognition tests: an unbound
/// variable is left verbatim, so an unchanged rendering means the source held
/// no interpolation, and a changed one means it did.
fn render_unbound(source: &str) -> String {
    pg_psql::render(source, &Variables::new())
        .unwrap_or_else(|error| panic!("{source:?} must parse as psql: {error}"))
        .sql()
        .to_owned()
}

fn interpolation_count(source: &str) -> usize {
    pg_psql::parse(source)
        .unwrap_or_else(|error| panic!("{source:?} must parse as psql: {error}"))
        .items
        .iter()
        .filter(|item| matches!(item, PsqlItem::Interpolation(_)))
        .count()
}

// --- Recognition ------------------------------------------------------------

#[test]
fn interpolation_is_recognised_in_every_position() {
    let bindings = variables(&[("v", "1"), ("f", "/tmp/x.csv"), ("lib", "regresslib")]);
    for (source, expected) in [
        // A bare target-list expression.
        ("SELECT :v", "SELECT 1"),
        ("SELECT :'v'", "SELECT '1'"),
        ("SELECT :\"v\"", "SELECT \"1\""),
        // A function argument.
        ("SELECT f(:v)", "SELECT f(1)"),
        // A predicate operand.
        (
            "SELECT * FROM t WHERE a = :'v'",
            "SELECT * FROM t WHERE a = '1'",
        ),
        // The typed-literal payload that cost pg-sql 8 LALR conflicts.
        ("SELECT numeric :'v'", "SELECT numeric '1'"),
        ("SELECT int :'v'", "SELECT int '1'"),
        // A COPY target.
        ("COPY t FROM :'f'", "COPY t FROM '/tmp/x.csv'"),
        // A function body.
        (
            "CREATE FUNCTION g() RETURNS int AS :'lib'",
            "CREATE FUNCTION g() RETURNS int AS 'regresslib'",
        ),
        // A statement of its own, and inside a parenthesised subquery.
        ("SELECT (SELECT :v)", "SELECT (SELECT 1)"),
        // Immediately after punctuation and at the very start of input.
        (":v", "1"),
        ("(:v)", "(1)"),
    ] {
        let rendered = pg_psql::render(source, &bindings)
            .unwrap_or_else(|error| panic!("{source:?} must parse as psql: {error}"));
        assert_eq!(rendered.sql(), expected, "rendering {source:?}");
    }
}

#[test]
fn interpolation_is_not_recognised_inside_a_string() {
    // psqlscan.l:538-543 `xq`/`xus`, :492-495 `xe`, :464-467 `xb`/`xh` — the
    // colon is ordinary content in every string state.
    for source in [
        "SELECT ':v'",
        "SELECT ':''v'",
        "SELECT E':v'",
        "SELECT E'\\\\:v'",
        "SELECT U&':v'",
        "SELECT B'0:1'",
        "SELECT X'0:1'",
        "SELECT 'a :v b'",
    ] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} holds no interpolation"
        );
        assert_eq!(
            interpolation_count(source),
            0,
            "{source:?} holds no interpolation"
        );
    }
}

#[test]
fn interpolation_is_not_recognised_inside_a_dollar_quoted_body() {
    // psqlscan.l:382-383 — "Dollar quoted strings are totally opaque, and no
    // escaping is done on them."
    for source in [
        "SELECT $$ :v $$",
        "SELECT $tag$ :v $tag$",
        // psqlscan.l:576-591: a delimiter that does not match the opener is
        // more opaque content, not a nested quote.
        "SELECT $a$ :v $b$ :v $a$",
        "DO $$ BEGIN PERFORM :v; END $$",
    ] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} holds no interpolation"
        );
        assert_eq!(
            interpolation_count(source),
            0,
            "{source:?} holds no interpolation"
        );
    }
}

#[test]
fn interpolation_is_not_recognised_inside_a_quoted_identifier() {
    // psqlscan.l:622-624 `xd`/`xui`.
    for source in ["SELECT \":v\"", "SELECT \"a :v b\"", "SELECT U&\":v\""] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} holds no interpolation"
        );
        assert_eq!(
            interpolation_count(source),
            0,
            "{source:?} holds no interpolation"
        );
    }
}

#[test]
fn interpolation_is_not_recognised_inside_a_comment() {
    // psqlscan.l:163-165 folds `--` comments into `{whitespace}`; :423-458
    // `xc` handles block comments, and they nest.
    for source in [
        "SELECT 1 -- :v\n",
        "SELECT 1 -- :'v'",
        "SELECT /* :v */ 1",
        "SELECT /* a /* :v */ b */ 1",
    ] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} holds no interpolation"
        );
        assert_eq!(
            interpolation_count(source),
            0,
            "{source:?} holds no interpolation"
        );
    }
}

#[test]
fn the_cast_and_assignment_operators_are_not_interpolation() {
    // psqlscan.l:288 `typecast` and :290 `colon_equals` are matched first.
    for source in [
        "SELECT a::int",
        "SELECT a::b::c",
        "SELECT x := 1",
        "SELECT a:::v",
    ] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} renders unchanged"
        );
    }
    // `a:::v` is `::` then `:v` — one interpolation, exactly as flex's
    // longest-match gives psql.
    assert_eq!(interpolation_count("SELECT a:::v"), 1);
    assert_eq!(interpolation_count("SELECT a::int"), 0);
    assert_eq!(interpolation_count("SELECT x := 1"), 0);
}

#[test]
fn an_incomplete_form_falls_back_to_a_bare_colon() {
    // psqlscan.l:775-796 throws back everything but the colon with
    // `yyless(1)`, because a space is not a `variable_char`.
    for source in [
        "SELECT :'a b'",
        "SELECT :\"a b\"",
        "SELECT : 'v'",
        "SELECT :",
    ] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} renders unchanged"
        );
        assert_eq!(
            interpolation_count(source),
            0,
            "{source:?} holds no interpolation"
        );
    }
}

// --- Substitution -----------------------------------------------------------

#[test]
fn the_three_substitution_forms() {
    let bindings = variables(&[("v", "a b")]);
    let rendered = pg_psql::render("SELECT :v, :'v', :\"v\"", &bindings).unwrap();
    // `:name` is raw text, `:'name'` a string literal, `:"name"` a quoted
    // identifier — psqlscan.l:710, :756, :761.
    assert_eq!(rendered.sql(), "SELECT a b, 'a b', \"a b\"");
}

#[test]
fn literal_substitution_quotes_like_pqescapeliteral() {
    // fe-exec.c:4371-4375 doubles `'` and `\`; :4334-4345 switches to
    // ` E'...'` when the value contains a backslash.
    let bindings = variables(&[("q", "it's"), ("b", "a\\b"), ("plain", "x")]);
    let rendered = pg_psql::render("SELECT :'q', :'b', :'plain'", &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT 'it''s',  E'a\\\\b', 'x'");
}

#[test]
fn identifier_substitution_quotes_like_pqescapeidentifier() {
    // The same routine with `as_ident`: `"` is doubled, and a backslash is an
    // ordinary character inside an identifier.
    let bindings = variables(&[("q", "a\"b"), ("b", "a\\b")]);
    let rendered = pg_psql::render("SELECT :\"q\", :\"b\"", &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT \"a\"\"b\", \"a\\b\"");
}

#[test]
fn an_unbound_variable_is_left_verbatim() {
    // psqlscan.l:744-751 for `:name`, :1561-1565 for the quoted forms.
    // Substitution never invents a value, so the SQL parse fails on the
    // surviving colon and the condition stays diagnosable.
    let bindings = variables(&[("bound", "1")]);
    let rendered =
        pg_psql::render("SELECT :bound, :loose, :'loose', :\"loose\"", &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT 1, :loose, :'loose', :\"loose\"");

    let names: Vec<&str> = rendered.unbound().iter().map(|u| u.name.as_str()).collect();
    assert_eq!(names, ["loose", "loose", "loose"]);
    // The first unbound token is `:loose` at source offset 15.
    assert_eq!(rendered.unbound()[0].source, 15..21);
    assert_eq!(
        &"SELECT :bound, :loose, :'loose', :\"loose\""[15..21],
        ":loose"
    );

    // pg-sql then rejects the rendering, which is the point.
    assert!(rendered.parse_sql().is_err());
}

#[test]
fn the_existence_test_substitutes_true_or_false() {
    // psqlscan.l:766 and :1568-1591 — the one form with no unbound case,
    // because psql answers the question rather than passing it through.
    let bindings = variables(&[("set", "")]);
    let rendered = pg_psql::render("SELECT :{?set}, :{?unset}", &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT TRUE, FALSE");
}

#[test]
fn send_commands_render_as_a_statement_boundary() {
    // A send command submits the query buffer but is client syntax the server
    // never sees.
    for (source, expected) in [
        (r"SELECT 1 \gset", "SELECT 1 ;"),
        (r"SELECT 1 \gexec", "SELECT 1 ;"),
        (r"SELECT 1 \gx", "SELECT 1 ;"),
        (r"SELECT 1 \g", "SELECT 1 ;"),
        (r"SELECT 1 \crosstabview", "SELECT 1 ;"),
    ] {
        let rendered = pg_psql::render(source, &Variables::new()).unwrap();
        assert_eq!(rendered.sql(), expected, "rendering {source:?}");
        assert_eq!(rendered.parse_sql().unwrap().statements().len(), 1);
    }
}

#[test]
fn the_batch_separator_renders_as_one_semicolon() {
    // psqlscan.l:697-702 — `\;` is not a submission boundary; it contributes
    // a semicolon to the query buffer, so the server receives one byte where
    // the user wrote two.
    let rendered = pg_psql::render(r"SELECT 1 \; SELECT 2;", &Variables::new()).unwrap();
    assert_eq!(rendered.sql(), "SELECT 1 ; SELECT 2;");
    assert_eq!(rendered.parse_sql().unwrap().statements().len(), 2);
}

#[test]
fn an_escaped_colon_is_not_an_interpolation() {
    // psqlscan.l:697-702's rule is `"\\"[;:]`, and its body emits
    // `yytext + 1`. `\:` is therefore how a psql user writes a colon that
    // must not be interpolated -- the escape has to beat the interpolation
    // rule, and it does because it is one token.
    let bindings = variables(&[("x", "SUBSTITUTED")]);
    for (source, expected) in [
        (r"SELECT \:x", "SELECT :x"),
        (r"SELECT \:'x'", "SELECT :'x'"),
    ] {
        let rendered = pg_psql::render(source, &bindings)
            .unwrap_or_else(|error| panic!("{source:?} must parse as psql: {error}"));
        assert_eq!(rendered.sql(), expected, "rendering {source:?}");
        assert!(
            !rendered.sql().contains("SUBSTITUTED"),
            "{source:?} must not interpolate an escaped colon",
        );
    }
    // The unescaped spelling still interpolates, so the escape is doing the
    // work rather than the variable being unreachable.
    assert_eq!(
        pg_psql::render("SELECT :x", &bindings).unwrap().sql(),
        "SELECT SUBSTITUTED",
    );
    // The escape covers exactly one colon, because `"\\"[;:]` matches two
    // characters and flex takes the longest match from there: `\::x` is the
    // escape followed by an ordinary interpolation.
    assert_eq!(
        pg_psql::render(r"SELECT \::x", &bindings).unwrap().sql(),
        "SELECT :SUBSTITUTED",
    );
}

#[test]
fn an_unmodelled_meta_command_stays_text() {
    // `\set` and its kin belong to psqlscanslash.l and to pg-sql#11, #12,
    // #13. They are forwarded verbatim rather than silently dropped, so the
    // SQL parse fails and the gap is visible.
    for source in [r"\set foo 1", r"\d users", r"\timing on"] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} renders unchanged"
        );
    }
    let rendered = pg_psql::render(r"\set foo 1", &Variables::new()).unwrap();
    assert!(rendered.parse_sql().is_err());
}

#[test]
fn a_send_command_name_is_read_whole() {
    // psqlscanslash.l reads a whole command name before looking it up, so
    // `\gsetfoo` is not `\gset` followed by `foo`, and `\getenv` is not
    // `\g` followed by `etenv`. The catch-all meta-command token matches
    // the longer name and wins on length, so neither becomes a statement
    // boundary; both are forwarded verbatim.
    for source in [
        r"SELECT 1 \gsetfoo",
        r"SELECT 1 \gexecx",
        r"\getenv abs_srcdir",
        r"\gdesc",
    ] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} renders unchanged"
        );
        assert_eq!(
            pg_psql::parse(source)
                .unwrap()
                .items
                .iter()
                .filter(|item| matches!(item, PsqlItem::Terminator(_)))
                .count(),
            0,
            "{source:?} holds no terminator",
        );
    }
}

// --- The source map ---------------------------------------------------------

#[test]
fn the_source_map_translates_inside_and_outside_a_substituted_region() {
    let bindings = variables(&[("x", "424242")]);
    let source = "SELECT :'x' , 1";
    let rendered = pg_psql::render(source, &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT '424242' , 1");

    // Outside, before the region: the two texts run in step.
    assert_eq!(rendered.map().origin(0), Origin::Verbatim(0));
    assert_eq!(rendered.map().origin(6), Origin::Verbatim(6));

    // Inside the region: the substituted bytes never appeared in the source,
    // so the interpolation's own extent is the whole available truth.
    let interpolation = 7..11;
    assert_eq!(&source[interpolation.clone()], ":'x'");
    for offset in 7..15 {
        assert_eq!(
            rendered.map().origin(offset),
            Origin::Substituted {
                start: interpolation.start,
                end: interpolation.end
            },
            "rendered offset {offset} lies in the substituted region",
        );
    }

    // Outside, after the region: shifted by the region's length difference.
    // The `,` sits at rendered 16 and source 12.
    assert_eq!(rendered.sql().as_bytes()[16], b',');
    assert_eq!(source.as_bytes()[12], b',');
    assert_eq!(rendered.map().origin(16), Origin::Verbatim(12));
    // One past the end continues the final verbatim run.
    assert_eq!(
        rendered.map().origin(rendered.sql().len()),
        Origin::Verbatim(source.len())
    );
}

#[test]
fn a_variable_bound_to_the_empty_string_keeps_the_map_exact() {
    // `:name` substitutes raw text, so an empty value produces a rewrite
    // that renders nothing. The region is real but zero-length, and every
    // verbatim byte after it must still name the byte that produced it.
    let bindings = variables(&[("e", "")]);
    let source = "SELECT :e, 1";
    let rendered = pg_psql::render(source, &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT , 1");
    assert_eq!(rendered.map().regions().count(), 1);

    for (offset, byte) in rendered.sql().bytes().enumerate() {
        if let Origin::Verbatim(origin) = rendered.map().origin(offset) {
            assert_eq!(
                source.as_bytes()[origin],
                byte,
                "rendered byte {offset} came from source byte {origin}",
            );
        }
    }
}

#[test]
fn the_source_map_is_exact_across_several_regions() {
    let bindings = variables(&[("a", "1"), ("b", "22")]);
    let source = "SELECT :a, :'b', :c;";
    let rendered = pg_psql::render(source, &bindings).unwrap();
    // `:c` is unbound, so it is not a region at all — it stays verbatim.
    assert_eq!(rendered.sql(), "SELECT 1, '22', :c;");
    assert_eq!(rendered.map().regions().count(), 2);

    // Every verbatim byte must map to the byte that produced it.
    for (offset, byte) in rendered.sql().bytes().enumerate() {
        if let Origin::Verbatim(origin) = rendered.map().origin(offset) {
            assert_eq!(
                source.as_bytes()[origin],
                byte,
                "rendered byte {offset} came from source byte {origin}",
            );
        }
    }
}

#[test]
fn the_source_map_carries_a_sql_diagnostic_back_to_the_psql_source() {
    // This is what `tests/document.rs::psql_rejection_names_the_offending_statement`
    // used to check inside pg-sql: a psql document whose second statement is
    // the problem. Now the diagnostic comes from the SQL parse of the
    // rendering, and the map names the psql byte it belongs to.
    let source = "SELECT 1;\nSELECT :x;";
    let rendered = pg_psql::render(source, &Variables::new()).unwrap();
    // Unbound, so it survives into the SQL and is rejected there.
    assert_eq!(rendered.sql(), source);
    let unbound = &rendered.unbound()[0];
    assert_eq!(&source[unbound.source.clone()], ":x");
    assert!(rendered.parse_sql().is_err());
}

// --- The pipeline end to end ------------------------------------------------

#[test]
fn select_int_quoted_x_parses_as_psql_substitutes_and_then_parses_as_sql() {
    // The exact case that made the SQL grammar ambiguous: `int :'x'` is a
    // typed literal under one reading and the start of a `JSON_OBJECT` entry
    // under the other. psql never puts the server in that position, because
    // it substitutes first.
    let bindings = variables(&[("x", "42")]);
    let source = "SELECT int :'x'";

    // 1. It is a psql document with one interpolation.
    let document = pg_psql::parse(source).expect("psql parse");
    assert_eq!(
        document
            .items
            .iter()
            .filter(|item| matches!(item, PsqlItem::Interpolation(_)))
            .count(),
        1,
    );

    // 2. Substitution renders server SQL.
    let rendered = pg_psql::render(source, &bindings).unwrap();
    assert_eq!(rendered.sql(), "SELECT int '42'");

    // 3. Only then does the SQL grammar see it, and it is ordinary SQL.
    let parsed = rendered.parse_sql().expect("rendered SQL must parse");
    assert_eq!(parsed.statements().len(), 1);
}

#[test]
fn the_documents_pg_sql_used_to_reject_now_render_and_parse() {
    // Moved from `tests/document.rs::psql_interpolation_is_rejected`, which
    // asserted `SqlParseError::Psql`. pg-sql has no such variant now: these
    // are psql documents, and this crate is what reads them.
    let bindings = variables(&[
        ("var", "1"),
        ("filter", "2"),
        ("filename", "/tmp/t.csv"),
        ("txid", "3"),
    ]);
    for (source, expected) in [
        ("SELECT :var;", "SELECT 1;"),
        ("SELECT :'var';", "SELECT '1';"),
        ("SELECT :\"var\";", "SELECT \"1\";"),
        (
            "SELECT * FROM t WHERE a = :filter;",
            "SELECT * FROM t WHERE a = 2;",
        ),
        ("COPY t FROM :'filename';", "COPY t FROM '/tmp/t.csv';"),
        ("SELECT numeric :'txid';", "SELECT numeric '3';"),
    ] {
        let rendered = pg_psql::render(source, &bindings)
            .unwrap_or_else(|error| panic!("{source:?} must parse as psql: {error}"));
        assert_eq!(rendered.sql(), expected, "rendering {source:?}");
        rendered
            .parse_sql()
            .unwrap_or_else(|error| panic!("{expected:?} must parse as SQL: {error}"));
    }
}

#[test]
fn an_empty_document_is_valid_and_source_preserving() {
    for source in ["", "   ", "-- just a comment\n", "/* only this */"] {
        assert_eq!(
            render_unbound(source),
            source,
            "{source:?} renders unchanged"
        );
    }
    assert!(pg_psql::parse("").unwrap().items.is_empty());
}

#[test]
fn rendering_preserves_every_byte_outside_a_rewrite() {
    // The strongest statement of the rendering contract: with nothing bound
    // and no send command, the output is the input.
    let source = "-- header\nSELECT $$a$$, 'b', \"c\", 1.5, a::int, x[1:2] /* t */;\n";
    assert_eq!(render_unbound(source), source);
}

// --- The real corpus ---------------------------------------------------------

/// Every PostgreSQL regression script is a psql document.
///
/// These files really are psql scripts — that is why psql runs first — so
/// this is the honest measure of whether the grammar reads what psql reads.
/// It also pins the rendering contract at corpus scale: with nothing bound,
/// the only bytes that may change are the two client spellings the server
/// never sees, and every one of them must be a recorded region.
#[test]
fn every_regression_script_is_a_psql_document() {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../vendor/postgres/src/test/regress/sql"
    );
    let mut scripts: Vec<_> = std::fs::read_dir(directory)
        .expect("the vendored PostgreSQL submodule must be checked out")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect();
    scripts.sort();

    let mut read = 0;
    for path in &scripts {
        let name = path.file_name().expect("a file name").to_string_lossy();
        // One script is deliberately not UTF-8 (the encoding tests). psql
        // scans bytes; this grammar takes `&str`, so such a script is out of
        // scope by construction rather than by omission.
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        read += 1;
        let rendered = pg_psql::render(&source, &Variables::new())
            .unwrap_or_else(|error| panic!("{name} must parse as a psql document: {error}"));

        // With nothing bound the only rewrites allowed are client syntax
        // the server never sees -- a send command or `\;` -- and `:{?name}`,
        // the one interpolation psql answers rather than passes through
        // (psqlscan.l:1568-1591, `FALSE` when unset). A `:name`, `:'name'`
        // or `:"name"` must survive untouched.
        for (_, origin) in rendered.map().regions() {
            let text = &source[origin.clone()];
            assert!(
                text.starts_with('\\') || text.starts_with(":{?"),
                "{name}: rewrote {text:?} at {origin:?} with nothing bound",
            );
        }
        // And the rendering differs from the source only where it did so.
        if rendered.map().regions().count() == 0 {
            assert_eq!(rendered.sql(), source, "{name} must render unchanged");
        }
    }

    assert_eq!(read, 225, "the vendored regression corpus changed size");
}
