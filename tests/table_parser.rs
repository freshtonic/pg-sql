//! Recursa parser smoke gate. The PostgreSQL corpus differential is the
//! exhaustive acceptance gate; these cases keep common successes and failures
//! cheap to diagnose.

use pg_sql::{ast::Statement, formatter::format_tokens_sql, lex};
use recursa::PrettyConfig;

const ACCEPTED: &[&str] = &[
    "SELECT 1",
    "SELECT a, b AS c FROM t WHERE a = 1 AND b NOT LIKE 'x%' ORDER BY b LIMIT 10",
    "SELECT * FROM a JOIN b ON a.id = b.id LEFT JOIN c USING (id)",
    "SELECT count(*) FILTER (WHERE x > 0) OVER (PARTITION BY y) FROM t",
    "SELECT s.t.column, s.t.* FROM s.t",
    "SELECT 1 + 2 / ANY (SELECT z FROM u), a[1:2], (SELECT 1)",
    "WITH RECURSIVE r AS (SELECT 1 UNION ALL SELECT n + 1 FROM r) SELECT * FROM r",
    "SELECT ((SELECT 2) UNION SELECT 2)",
    "SELECT (((SELECT 2)) UNION SELECT 2)",
    "SELECT * FROM ((SELECT 1) UNION SELECT 2) AS u",
    "SELECT * FROM ((SELECT 1 ORDER BY 1) UNION ALL (SELECT 2 ORDER BY 1)) AS u",
    "INSERT INTO t (a, b) VALUES (1, 'x') ON CONFLICT (a) DO NOTHING RETURNING *",
    "CREATE TABLE t (a int NOT NULL DEFAULT 1 CHECK (a > 0), b text UNIQUE)",
    "CREATE FUNCTION f(a int, OUT b text) RETURNS text LANGUAGE sql RETURN 'x'",
    "EXPLAIN (ANALYZE, FORMAT JSON) SELECT x FROM t WHERE x IS NOT NULL",
    "EXPLAIN (COSTS OFF) SELECT string_agg(DISTINCT f1, ',') FILTER (WHERE length(f1) > 1) FROM varchar_tbl",
    "EXPLAIN (COSTS OFF) SELECT string_agg(DISTINCT f1::varchar(2), ',') FILTER (WHERE length(f1) > 1) FROM varchar_tbl",
    "SELECT aggfstr(DISTINCT a, b, c) FROM t",
    "SELECT jsonb_object_agg(DISTINCT 'a', 'abc')",
    "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY, DEFERRABLE",
];

const REJECTED: &[&str] = &[
    "SELECT FROM",
    "SELECT a FROM t WHERE",
    "INSERT INTO t VALUES (1,",
    "CREATE TABLE t (a int,)",
];

#[test]
fn recursa_parser_accepts_representative_statements() {
    for source in ACCEPTED {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );
        let mut input = lexed.input();
        Statement::parse(&mut input)
            .unwrap_or_else(|error| panic!("Recursa parser rejects {source:?}: {error}"));
        assert!(input.is_eof(), "Recursa parser left input for {source:?}");
    }
}

#[test]
fn recursa_parser_rejects_representative_invalid_statements() {
    for source in REJECTED {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );
        let mut input = lexed.input();
        assert!(
            Statement::parse(&mut input).is_err(),
            "Recursa parser accepts {source:?}"
        );
    }
}

/// PostgreSQL's `quotecontinue` scanner rule requires a newline with only
/// non-newline spaces or line comments around it. A block comment breaks that
/// lexical continuation; accepting it would let formatting turn a PostgreSQL
/// error into a valid newline-adjacent string sequence.
#[test]
fn string_continuation_rejects_block_comments_and_preserves_newlines() {
    let valid = "SELECT 'first line'\n' - next line'\n\t' - third line' AS lines";
    let lexed = lex(valid);
    assert!(
        lexed.errors().next().is_none(),
        "lexical errors in {valid:?}"
    );
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input)
        .unwrap_or_else(|error| panic!("Recursa parser rejects {valid:?}: {error}"));
    assert!(input.is_eof(), "Recursa parser left input for {valid:?}");
    let formatted = format_tokens_sql(parsed.ast(), PrettyConfig::default());
    assert!(
        formatted.contains("'first line'")
            && formatted.contains("\n' - next line'")
            && formatted.contains("\n\t' - third line'"),
        "continuation formatting must retain its physical hard breaks: {formatted:?}"
    );

    let invalid =
        "SELECT 'first line'\n' - next line' /* block comment */\n' - third line' AS lines";
    let lexed = lex(invalid);
    assert!(
        lexed.errors().next().is_none(),
        "lexical errors in {invalid:?}"
    );
    let mut input = lexed.input();
    let result = Statement::parse(&mut input);
    assert!(
        result.is_err() || !input.is_eof(),
        "Recursa parser must not consume a block-comment-separated string continuation"
    );
}
