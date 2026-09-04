//! Lexical arbitration witnesses.
//!
//! `dump_lex_records` writes one deterministic line per lexical record. The
//! adversarial inputs below carry a checked-in expectation, so any change to
//! longest-match arbitration, operator-fence suppression, closed-region
//! extents, or diagnostic anchors shows up as a diff. The corpus pass lexes
//! every PostgreSQL regression file and every stress fixture, and can dump its
//! records for a cross-generation comparison.
//!
//! Regenerate the expectation with `PG_SQL_LEX_BLESS=1`, and dump the complete
//! corpus with `PG_SQL_LEX_DUMP=<path>`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Adversarial inputs whose arbitration is not exercised by the corpus.
const EDGE_CASES: &[&str] = &[
    "a+--b",
    "x*/*c*/y",
    "x||/*c*/y",
    "x||/*c",
    "SELECT $$unterminated",
    "SELECT $tag$body$tag$",
    "SELECT $tag$body$other$",
    "SELECT /* unterminated",
    "SELECT /* a /* b */ c */ 1",
    "SELECT ?",
    "SELECT 1e+",
    "SELECT 1e+1",
    "SELECT x.y",
    "SELECT 1..2",
    "SELECT 1.",
    "SELECT .1",
    "SELECT 0x1f, 0o17, 0b101",
    "SELECT 1_000",
    "SELECT 1abc",
    "SELECT 0x",
    "a <<>> b",
    "a <> b",
    "a ! b",
    "a !! b",
    "a +- b",
    "a +-~ b",
    "a ~+- b",
    "a @-@ b",
    "a ||/ 9",
    "a |/ 9",
    "select 1 -- trailing\nselect 2",
    "select 1 --trailing",
    "select 1 -- trailing\r\nselect 2",
    "u&'\\0441'",
    "U&\"d\\0061t\\+000061\"",
    "e'\\n'",
    "E'a''b'",
    "b'0101'",
    "x'1f'",
    "'it''s'",
    "'unterminated",
    "\"unterminated",
    "$1 $2 $12a",
    "\\gexec \\g \\gx \\gset \\crosstabview \\; \\",
    "SELECT 1 \u{00e9} 2",
    "SELECT '\u{00e9}'",
    "",
    " ",
    "\t\r\n\u{000c}",
    "---",
    "/*/",
    "/**/",
    "/*",
    "*/",
    "+",
    "-",
    "*",
    "/",
    "a#-b",
    "a^@b",
    "a#>>b",
    "a?||b",
    "a!~~*b",
    "a~<=~b",
];

/// Renders one lexical record stream as a deterministic dump.
fn dump_lex_records(label: &str, source: &str, out: &mut String) {
    let lexed = pg_sql::lex(source);
    let mut rows: Vec<(u32, u32, String)> = Vec::new();
    for token in lexed.tokens() {
        let span = token.span();
        rows.push((
            span.start(),
            span.end(),
            format!("T {:?} {:?}", token.kind(), token.text()),
        ));
    }
    for error in lexed.errors() {
        let span = error.span();
        let anchor = error.anchor();
        rows.push((
            span.start(),
            span.end(),
            format!(
                "E {} {}..{} {:?}",
                error.code(),
                anchor.start(),
                anchor.end(),
                error.text()
            ),
        ));
    }
    rows.sort_by_key(|(start, end, _)| (*start, *end));
    writeln!(out, "## {label} ({} bytes, {} records)", source.len(), rows.len())
        .expect("string write");
    for (start, end, body) in rows {
        writeln!(out, "{start}..{end} {body}").expect("string write");
    }
}

/// Collects every `.sql` file under `root`, sorted by path.
fn sql_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// Checked-in expectation for the adversarial arbitration inputs.
const EDGE_EXPECTATION: &str = include_str!("fixtures/lex_edge_cases.txt");

#[test]
fn adversarial_inputs_keep_their_pinned_arbitration() {
    let mut out = String::new();
    for source in EDGE_CASES {
        dump_lex_records(&format!("edge {source:?}"), source, &mut out);
    }

    let expectation = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lex_edge_cases.txt");
    if std::env::var_os("PG_SQL_LEX_BLESS").is_some() {
        fs::create_dir_all(expectation.parent().expect("fixture directory"))
            .expect("create fixture directory");
        fs::write(&expectation, &out).expect("write lexical expectation");
        return;
    }
    assert_eq!(
        out, EDGE_EXPECTATION,
        "lexical arbitration changed; rerun with PG_SQL_LEX_BLESS=1 to inspect the new expectation",
    );
}

#[test]
fn the_complete_corpus_lexes_without_a_panic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = String::new();

    for directory in [
        root.join("fixtures/stress"),
        root.join("vendor/postgres/src/test/regress/sql"),
    ] {
        for path in sql_files(&directory) {
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            dump_lex_records(&name, &source, &mut out);
        }
    }

    if let Ok(destination) = std::env::var("PG_SQL_LEX_DUMP") {
        let mut edges = String::new();
        for source in EDGE_CASES {
            dump_lex_records(&format!("edge {source:?}"), source, &mut edges);
        }
        edges.push_str(&out);
        fs::write(&destination, &edges).expect("write lexical dump");
    }

    assert!(!out.is_empty(), "the corpus must produce lexical records");
}
