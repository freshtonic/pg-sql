//! Seed of the style equivalence gate (#69): a handful of corpus statements
//! lexed once and parsed by both named parsers of the grammar, which must
//! produce equal values and, on invalid input, fail at the same token.
//!
//! The grammar must declare `parsers(rd = recursive_descent, lr =
//! table_driven)` in `src/lib.rs` for `pg_sql::parsers::lr` to exist. That
//! declaration is still not in place. recursa refuses to generate the
//! table-driven parser while any conflict remains (`RCA9105`), and the
//! construction reports 31: 9,643 before the first pass, 522 after it, 33
//! after the second, 31 now.
//!
//! recursa #129 was implemented and then rejected: two grammar types sharing
//! one LR nonterminal cannot retire a reduce/reduce between themselves, since
//! the merged goto entry serves both and the reduce has nothing left to
//! choose by. So the remaining conflicts are pg-sql grammar work, judged
//! against the nonterminal each node mirrors (CLAUDE.md principle 9). What is
//! left, and what each needs:
//!
//! - 18, `SelectClause` against `DirectSelectClause`. Unifying them into the
//!   one `select_clause` gram.y has (gram.y:12757) is correct and was
//!   measured: conflicts fall 32 -> 16, and the analysis reports no
//!   `RCA0200`, so recursive descent handles the unified form. It is blocked
//!   by recursa #130. `Expr`'s only precedence-grouping witness is the
//!   `Expr::Parenthesized` atom, and recursa discards a candidate atom whose
//!   subtree is cyclic. `DirectParenthesizedSet` carried a *required*
//!   `SetOpCombiner`, and a required non-fixed sibling stops the traversal,
//!   so the cycle stayed hidden. One `select_clause` self-cycles as
//!   `Subquery => '(' Subquery ')'` -- gram.y's own `select_with_parens: '('
//!   select_with_parens ')'` (gram.y:12684) -- and `RCA3101` then claims the
//!   grammar "has no atom that encloses Self between fixed tokens", which is
//!   false. Splitting the query out of `ParenContent` into its own atom, as
//!   gram.y's separate `c_expr` productions do (gram.y:15391), clears
//!   `RCA3101` but makes `Expr` derive `( Subquery )` and three enums then
//!   report `RCA0200`. There is no pg-sql-side route around it.
//!
//! - 8, pg-sql's psql-variable extension. `TypeCastValue::PsqlVar` admits
//!   `:'x'` after a fixed type-name keyword, so `SELECT int :'x'` is a typed
//!   literal while `int :` also begins a `JSON_OBJECT` key/value entry. psql
//!   substitutes `:'x'` textually before the server lexes, so gram.y never
//!   meets the choice, and no amount of lookahead settles it: both readings
//!   stay live past the colon. pg-sql already made the opposite call for
//!   identifier-spelled type names (see `TypeCastFunc`), and making the same
//!   call here would retire all 8; the cost is `numeric :'txid_current'` in
//!   psql scripts. The differential never sees it -- "statements containing
//!   psql variable interpolation are not standalone SQL" is a frozen corpus
//!   rule -- so this is a product decision, not a parity one.
//!
//! - 3, the two `func_arg_list` spellings. Mirroring gram.y exactly (one
//!   `func_arg_list`, gram.y:16539, with `VARIADIC` as the `',' VARIADIC
//!   func_arg_expr` tail of `func_application`, gram.y:15517) was tried and
//!   reverted: it removes the two competing list nonterminals but nets zero,
//!   because recursa inlines an optional only when it can begin with an
//!   admitted keyword and this one begins with `COMMA`, so the 2
//!   reduce/reduce become shift/reduce on the `opt(...)` epsilon. It also
//!   breaks `f(1, VARIADIC xs ORDER BY 1)` under recursive descent, whose
//!   `list1` eats the comma and then demands a `FuncArg`. The real blocker is
//!   that `FunctionCallTail`'s two variants must share every nonterminal up
//!   to `')'`, as `func_application` and `AexprConst` do, and pg-sql's do
//!   not.
//!
//! - 1, `SET SESSION . CHARACTERISTICS`. gram.y shifts the scope keyword as a
//!   literal in `VariableSetStmt: SET set_rest | SET LOCAL set_rest | SET
//!   SESSION set_rest` (gram.y:1617), so nothing reduces before
//!   `CHARACTERISTICS` decides. pg-sql routes it through the `SetScope`
//!   nonterminal, whose reduce stands against that shift. Retiring it means
//!   merging `SetStmt`, `SetRoleStmt`, `SetSessionAuthStmt`,
//!   `SetTimeZoneStmt` and `SetTransactionStmt` into one `VariableSetStmt`
//!   over one `set_rest`, with the scope keywords inlined.
//!
//! - 1, `UESCAPE` after a `U&'...'` literal. gram.y has no production for it
//!   at all: `UESCAPE` appears only in the unreserved and bare-label keyword
//!   lists (gram.y:17826, 18474), and `scan.l` produces one token for the
//!   whole `U&'...' [UESCAPE 'c']` form. pg-sql lexes the two separately, so
//!   the grammar must choose between attaching the escape and reading
//!   `UESCAPE` as a column label. The fix is lexical, not grammatical.
//!
//! The `table-driven` cargo feature therefore stays. It is not dead weight to
//! be removed "now that both parsers exist": they do not both exist, and an
//! unconditional test would fail to compile on every build. The feature comes
//! off, and this becomes #69's gate, in the commit that adds the
//! `parsers(...)` declaration.

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
