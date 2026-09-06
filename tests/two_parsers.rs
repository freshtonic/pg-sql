//! Seed of the style equivalence gate (#69): a handful of corpus statements
//! lexed once and parsed by both named parsers of the grammar, which must
//! produce equal values and, on invalid input, fail at the same token.
//!
//! The grammar must declare `parsers(rd = recursive_descent, lr =
//! table_driven)` in `src/lib.rs` for `pg_sql::parsers::lr` to exist. That
//! declaration is not yet in place: the table-driven construction still
//! reports LR conflicts that only recursa changes can retire (recursa
//! #120 to #127), and recursa treats every conflict as a hard build error. The
//! `table-driven` cargo feature gates this test until the declaration lands;
//! it is the seed of #69's gate, not the gate itself.

use pg_sql::{ast::Statement, lex, parsers::lr};

/// Corpus statements every parser must accept with the same value.
const ACCEPTED: &[&str] = &[
    "SELECT 1",
    "SELECT a, b AS c FROM t WHERE a = 1 AND b NOT LIKE 'x%' ORDER BY b LIMIT 10",
    "SELECT * FROM a JOIN b ON a.id = b.id LEFT JOIN c USING (id)",
    "SELECT count(*) FILTER (WHERE x > 0) OVER (PARTITION BY y) FROM t",
    "SELECT 1 + 2 / ANY (SELECT z FROM u), a[1:2], (SELECT 1)",
    "WITH RECURSIVE r AS (SELECT 1 UNION ALL SELECT n + 1 FROM r) SELECT * FROM r",
    "INSERT INTO t (a, b) VALUES (1, 'x') ON CONFLICT (a) DO NOTHING RETURNING *",
    "CREATE TABLE t (a int NOT NULL DEFAULT 1 CHECK (a > 0), b text UNIQUE)",
    "CREATE FUNCTION f(a int, OUT b text) RETURNS text LANGUAGE sql RETURN 'x'",
    "EXPLAIN (ANALYZE, FORMAT JSON) SELECT x FROM t WHERE x IS NOT NULL",
];

/// Inputs every parser must reject at the same token.
const REJECTED: &[&str] = &[
    "SELECT FROM",
    "SELECT a FROM t WHERE",
    "INSERT INTO t VALUES (1,",
    "CREATE TABLE t (a int,)",
];

#[test]
fn both_parsers_produce_equal_values() {
    for source in ACCEPTED {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );

        let mut rd = lexed.input();
        let rd_parsed = Statement::parse(&mut rd)
            .unwrap_or_else(|error| panic!("recursive descent rejects {source:?}: {error}"));
        assert!(rd.is_eof(), "recursive descent left input for {source:?}");

        let mut table = lr::input(&lexed);
        let lr_parsed = Statement::parse(&mut table)
            .unwrap_or_else(|error| panic!("table-driven rejects {source:?}: {error}"));
        assert!(table.is_eof(), "table-driven left input for {source:?}");

        assert_eq!(
            format!("{:?}", rd_parsed.into_ast()),
            format!("{:?}", lr_parsed.into_ast()),
            "the parsers disagree on the value of {source:?}"
        );
    }
}

#[test]
fn both_parsers_fail_at_the_same_token() {
    for source in REJECTED {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );

        let mut rd = lexed.input();
        let rd_error =
            Statement::parse(&mut rd).expect_err(&format!("recursive descent accepts {source:?}"));
        let mut table = lr::input(&lexed);
        let lr_error =
            Statement::parse(&mut table).expect_err(&format!("table-driven accepts {source:?}"));

        assert_eq!(rd_error.kind(), lr_error.kind(), "error kind for {source:?}");
        assert_eq!(rd_error.span(), lr_error.span(), "failing span for {source:?}");
        assert_eq!(rd_error.found(), lr_error.found(), "failing token for {source:?}");
    }
}
