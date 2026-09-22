//! Nested/Flat AST parity over the pinned differential corpus.
//!
//! The Flat AST is a second concrete representation of the same `ast_node!`
//! declarations that produce the nested (arena) AST, selected by the `flat`
//! flag on the `recursa::grammar!` root. Recursa proves the two agree on small
//! fixture grammars; this suite proves it at pg-sql's scale — 1,463
//! declarations and the whole PostgreSQL 17.9 regression corpus.
//!
//! The statement membership is the one the differential suite pins
//! (`tests/support/baseline.rs`, `FrozenStatements::pinned()`), read straight
//! out of the vendored corpus under `vendor/postgres/src/test/regress/sql`, so
//! this suite and the differential suite cannot drift. No PostgreSQL oracle is
//! needed: the assertions compare pg-sql against itself.
//!
//! For every pinned statement the suite asserts:
//!
//! 1. **Recognition parity.** `Statement::parse_without_spans` and
//!    `Statement::parse_flat_without_spans` both accept or both reject, and on
//!    acceptance they agree on `is_eof()` — the same automaton, the same
//!    tables, only a different set of reduce actions.
//! 2. **Rendering parity.** `FlatAst::render` returns exactly what the detached
//!    nested AST of the same input renders. Neither value carries occurrence
//!    provenance, so both emit the canonical spelling of every fixed token.
//! 3. **Post-order.** `check_post_order` walks `children` over every Node of
//!    every typed pool and finds no same-pool child that follows its parent
//!    (invariant I1 of `docs/flat-ast-design.md` in Recursa).
//!
//! A second test, behind the `spans` feature, parses with `parse_flat`, the
//! span-capturing entry point a consumer runs, asserts it gives the verdict
//! `parse_flat_without_spans` gives, and asserts the root span covers the
//! whole statement.
//!
//! Run with `cargo test -p pg-sql --test flat_parity` and, for the span half,
//! `cargo test -p pg-sql --features spans --test flat_parity`.

// Only part of the support module is used here, and rustc drops the module's
// own `#[test]` functions in this target, which strands its test imports.
#[allow(dead_code, unused_imports)]
#[path = "support/mod.rs"]
mod support;

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use pg_sql::ast::Statement;
use pg_sql::ast::shared;
use recursa::Pretty as _;
use support::baseline::FrozenStatements;
use support::diff_check::lex_statement_source;

/// The PostgreSQL regression SQL corpus, vendored as a submodule.
fn corpus_sql_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/postgres/src/test/regress/sql")
}

/// Every pinned statement of every frozen corpus file, as `(file, index, sql)`.
///
/// The statement text is the verbatim corpus slice, exactly what the
/// differential suite and the benchmark harness feed their engines.
fn pinned_statements() -> Vec<(String, usize, String)> {
    let frozen = FrozenStatements::pinned();
    let corpus_dir = corpus_sql_dir();
    let mut out = Vec::new();
    for name in frozen.file_names() {
        let path = corpus_dir.join(name);
        let text = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "read corpus file {} (is the vendor/postgres submodule checked out?): {error}",
                path.display()
            )
        });
        let statements = frozen
            .file(name)
            .statements(&text)
            .unwrap_or_else(|error| panic!("{name}: cannot load frozen statements: {error}"));
        for (index, source) in statements.iter().enumerate() {
            out.push((name.to_owned(), index, (*source).to_owned()));
        }
    }
    out
}

/// What one representation made of one statement: whether it parsed, whether it
/// consumed the statement whole, and what it rendered.
struct Outcome {
    accepted: bool,
    is_eof: bool,
    rendered: String,
}

/// The nested (arena) parse: accept/reject, cursor state, and the detached
/// render — `ArenaParsed::ast` borrows the parse-owned arena, and with
/// `parse_without_spans` it carries no occurrence provenance, which is what
/// makes its render comparable with the Flat AST's.
fn nested_outcome(sql: &str) -> Outcome {
    let lexed = lex_statement_source(sql);
    if lexed.errors().next().is_some() {
        return Outcome {
            accepted: false,
            is_eof: false,
            rendered: String::new(),
        };
    }
    let mut input = lexed.input();
    match Statement::parse_without_spans(&mut input) {
        Ok(parsed) => Outcome {
            accepted: true,
            is_eof: input.is_eof(),
            rendered: parsed.ast().render(),
        },
        Err(_) => Outcome {
            accepted: false,
            is_eof: false,
            rendered: String::new(),
        },
    }
}

/// The flat parse, through the same lex pass and the same automaton. Also
/// checks post-order over every typed pool, which a fresh parse always passes.
fn flat_outcome(sql: &str) -> Outcome {
    let lexed = lex_statement_source(sql);
    if lexed.errors().next().is_some() {
        return Outcome {
            accepted: false,
            is_eof: false,
            rendered: String::new(),
        };
    }
    let mut input = lexed.input();
    match Statement::parse_flat_without_spans(&mut input) {
        Ok(flat) => {
            pg_sql::check_post_order(&flat).unwrap_or_else(|violation| {
                panic!("flat pools are not in post-order: {violation}")
            });
            Outcome {
                accepted: true,
                is_eof: input.is_eof(),
                rendered: flat.render(),
            }
        }
        Err(_) => Outcome {
            accepted: false,
            is_eof: false,
            rendered: String::new(),
        },
    }
}

#[test]
fn a_flat_parse_recognizes_and_renders_every_pinned_statement_as_the_nested_parse_does() {
    let statements = pinned_statements();
    assert!(
        statements.len() > 10_000,
        "the pinned corpus is much larger than this: {}",
        statements.len(),
    );

    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut mismatches = String::new();
    let mut mismatch_count = 0usize;

    for (file, index, sql) in &statements {
        let nested = nested_outcome(sql);
        let flat = flat_outcome(sql);

        let mut fail = |what: &str, detail: String| {
            mismatch_count += 1;
            if mismatch_count <= 20 {
                let _ = writeln!(mismatches, "{file}:{index} {what}: {detail}");
            }
        };

        if nested.accepted != flat.accepted {
            fail(
                "recognition",
                format!(
                    "nested {}, flat {} — {:?}",
                    if nested.accepted {
                        "accepted"
                    } else {
                        "rejected"
                    },
                    if flat.accepted {
                        "accepted"
                    } else {
                        "rejected"
                    },
                    truncate(sql),
                ),
            );
            continue;
        }

        if !nested.accepted {
            rejected += 1;
            continue;
        }
        accepted += 1;

        if nested.is_eof != flat.is_eof {
            fail(
                "cursor",
                format!(
                    "nested is_eof={} flat is_eof={} — {:?}",
                    nested.is_eof,
                    flat.is_eof,
                    truncate(sql),
                ),
            );
        }
        if nested.rendered != flat.rendered {
            fail(
                "render",
                format!(
                    "\n  nested: {:?}\n  flat:   {:?}",
                    truncate(&nested.rendered),
                    truncate(&flat.rendered),
                ),
            );
        }
    }

    println!(
        "flat parity: {} pinned statements — {accepted} accepted by both, \
         {rejected} rejected by both, {mismatch_count} mismatches",
        statements.len(),
    );
    assert_eq!(
        mismatch_count, 0,
        "nested and flat parsing must agree on every pinned statement; first mismatches:\n{mismatches}",
    );
}

#[cfg(feature = "spans")]
#[test]
fn a_flat_parse_with_spans_gives_the_root_a_span_covering_the_statement() {
    let statements = pinned_statements();
    let mut spanned = 0usize;

    for (file, index, sql) in &statements {
        let lexed = lex_statement_source(sql);
        if lexed.errors().next().is_some() {
            continue;
        }
        // Recognition parity for the span-capturing entry point: `parse_flat`
        // is the parse a consumer such as pg-analyze runs, and it must give the
        // verdict `parse_flat_without_spans` gives, which the test above holds
        // to the nested parse. A divergence here is a defect in the spans path
        // of the generated parser, and it used to pass silently.
        let without_spans = flat_outcome(sql);
        let mut input = lexed.input();
        let parsed = Statement::parse_flat(&mut input);
        assert_eq!(
            parsed.is_ok(),
            without_spans.accepted,
            "{file}:{index}: `parse_flat` and `parse_flat_without_spans` disagree on acceptance — {:?}",
            truncate(sql),
        );
        let Ok(flat) = parsed else {
            continue;
        };
        assert_eq!(
            input.is_eof(),
            without_spans.is_eof,
            "{file}:{index}: `parse_flat` and `parse_flat_without_spans` disagree on is_eof — {:?}",
            truncate(sql),
        );
        if !input.is_eof() {
            continue;
        }
        assert!(
            flat.has_spans(),
            "{file}:{index}: `parse_flat` under the `spans` feature captures spans",
        );
        let root = flat
            .span(flat.root())
            .unwrap_or_else(|| panic!("{file}:{index}: the root of a captured parse has a span"));

        // The statement's own extent within `sql`: the lex pass drops a trailing
        // `;`, so the root span runs from the first retained token to the last.
        let ranges: Vec<_> = lexed.tokens().map(|token| token.span().range()).collect();
        let first = ranges
            .first()
            .unwrap_or_else(|| panic!("{file}:{index}: an accepted statement has a token"));
        let last = ranges.last().expect("a non-empty token list has a last");
        assert!(
            root.range().start <= first.start && root.range().end >= last.end,
            "{file}:{index}: root span {:?} must cover the statement extent {}..{}",
            root.range(),
            first.start,
            last.end,
        );
        spanned += 1;
    }

    println!("flat spans: {spanned} statements with a root span covering the statement");
    assert!(spanned > 10_000, "most pinned statements parse: {spanned}");
}

/// The flat type sizes and the flat type count, printed for the design record
/// (`docs/flat-ast-design.md` in Recursa). The pooled types are the hot set;
/// `FlatExpr` also carries a generated `max_size` assertion, so a growth in its
/// declaration fails to compile rather than silently widening the pool.
#[test]
fn reports_the_size_of_every_pooled_flat_node() {
    for (name, size) in [
        ("FlatExpr", size_of::<shared::expr::FlatExpr>()),
        ("FlatColumnRef", size_of::<shared::expr::FlatColumnRef>()),
        ("FlatFuncCall", size_of::<shared::expr::FlatFuncCall>()),
        (
            "FlatSelectStmt",
            size_of::<pg_sql::ast::dml::select::FlatSelectStmt>(),
        ),
        ("FlatStatement", size_of::<pg_sql::ast::FlatStatement>()),
    ] {
        println!("flat size {name} = {size}");
        assert!(size > 0, "{name} is a real flat Node type");
    }
}

/// Shortens a statement for a failure message; a corpus statement can be long.
fn truncate(sql: &str) -> String {
    let flat: String = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 160 {
        flat
    } else {
        let head: String = flat.chars().take(160).collect();
        format!("{head}…")
    }
}
