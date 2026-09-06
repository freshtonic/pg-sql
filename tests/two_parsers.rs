//! Seed of the style equivalence gate (#69): a handful of corpus statements
//! lexed once and parsed by both named parsers of the grammar, which must
//! produce equal values and, on invalid input, fail at the same token.
//!
//! The grammar must declare `parsers(rd = recursive_descent, lr =
//! table_driven)` in `src/lib.rs` for `pg_sql::parsers::lr` to exist. That
//! declaration is still not in place. recursa refuses to generate the
//! table-driven parser while any conflict remains (`RCA9105`), and the
//! construction reports 17: 9,643 before the first pass, 522 after it, 33
//! after the second, 31 after the third, 23 once psql left the grammar, 17
//! now that `select_with_parens` is one nonterminal.
//!
//! recursa #129 was implemented and then rejected: two grammar types sharing
//! one LR nonterminal cannot retire a reduce/reduce between themselves, since
//! the merged goto entry serves both and the reduce has nothing left to
//! choose by. So the remaining conflicts are pg-sql grammar work, judged
//! against the nonterminal each node mirrors (CLAUDE.md principle 9). What is
//! left, and what each needs:
//!
//! - 12, `Subquery` against `DirectSubquery`, on lookaheads `ORDER`,
//!   `OFFSET`, `LIMIT`, `FOR`, `FETCH` and `RPAREN` in two states. Blocked by
//!   recursa #132, and not pg-sql grammar work.
//!
//!   The 18 that used to head this list are gone. They were `SelectClause`
//!   against `DirectSelectClause`, and the diagnosis in this note was wrong:
//!   `DirectSelectClause` was never a second copy of `select_clause`, it is
//!   `simple_select` (gram.y:12790). What the two really shared, each
//!   carrying its own copy, was `select_with_parens` (gram.y:12682). Giving
//!   that its own node, as gram.y does and for the reason gram.y states
//!   (gram.y:12658), retired all 18. Note that substituting `Subquery` for
//!   `DirectSubquery` wholesale -- the "unification" this note used to
//!   recommend -- reports `RCA0200` on five enums and is not the answer;
//!   recursa #130 did not change that.
//!
//!   What remains is the same duplication one level up. gram.y writes every
//!   position that admits both a parenthesized query and a parenthesized
//!   expression as two alternatives that each own their `(`: `in_expr:
//!   select_with_parens | '(' expr_list ')'` (gram.y:16715), and likewise
//!   `c_expr` (gram.y:15391), `table_ref` (gram.y:13492) and the
//!   `subquery_Op sub_type` operand (gram.y:15152). recursa cannot express
//!   that shape. Written directly it
//!   is `RCA0200`, witness `( -> ( -> SELECT -> U&'...' -> SELECT`,
//!   `lookahead=Some(5)->None`: balanced dispatch does not engage, because
//!   both alternatives close on the same `)` and their residuals after it are
//!   identical. bison has no trouble, keeping `%expect 0` by giving `')'`
//!   higher precedence than the `%prec UMINUS` on `c_expr:
//!   select_with_parens` (gram.y:896-898, rationale at gram.y:12658-12666).
//!
//!   So pg-sql takes the remedy `RCA0200` names and factors the `(` out to
//!   the position, which makes `DirectSubquery` a second spelling of
//!   `select_no_parens`. The construction then sees both after the same `(`
//!   and cannot tell which nonterminal owns it:
//!
//!   ```text
//!   path: QuantifiedComparisonOperand -> ParenthesizedOpen SimpleSelect . ORDER
//!     SelectClause   -> SimpleSelect .                  the `(` is SelectWithParens's
//!     DirectSubquery -> SimpleSelect . OrderByClause    the `(` is the position's
//!   ```
//!
//!   The grammar is unambiguous -- the `SelectWithParens` reading needs a set
//!   operation that never arrives -- but LALR(1) cannot see that far, and the
//!   shape that would avoid the conflict is the one `RCA0200` refuses. Do not
//!   try to widen an admission set out of this; it is a recursa gap.
//!
//! - 0, and retired: pg-sql's psql-variable extension used to cost 8.
//!   `TypeCastValue::PsqlVar` admitted `:'x'` after a fixed type-name
//!   keyword, so `SELECT int :'x'` was a typed literal while `int :` also
//!   begins a `JSON_OBJECT` key/value entry, and no lookahead settled it:
//!   both readings stayed live past the colon. gram.y never meets the
//!   choice, because psql substitutes `:'x'` textually before the server
//!   lexes. The product decision was to say the same thing in the same
//!   place: psql is its own grammar in the `pg-psql` crate, which parses a
//!   psql document, substitutes, and renders the SQL text this grammar then
//!   reads. 31 -> 23, and the conflict states fell from 14 to 6.
//!
//! - 3, the two `func_arg_list` spellings, on `ORDER`, `COMMA` and `RPAREN`
//!   in one state: `list1(FuncArg, sep=COMMA)` against
//!   `FunctionOrdinaryArgumentSequence`. Mirroring gram.y exactly (one
//!   `func_arg_list`, gram.y:16539, with `VARIADIC` as the `',' VARIADIC
//!   func_arg_expr` tail of `func_application`, gram.y:15517) was tried and
//!   reverted: it removes the two competing list nonterminals but nets zero,
//!   because recursa inlines an optional only when it can begin with an
//!   admitted keyword and this one begins with `COMMA`, so the 2
//!   reduce/reduce become shift/reduce on the `opt(...)` epsilon. It also
//!   breaks `f(1, VARIADIC xs ORDER BY 1)` under recursive descent, whose
//!   `list1` eats the comma and then demands a `FuncArg`. The real blocker is
//!   that `FunctionCallTail`'s two variants must share every nonterminal up
//!   to `')'`, as `func_application` and `AexprConst` (gram.y:17231) do, and
//!   pg-sql's do not. Merging them into one `open body close continuation`,
//!   parting on the `Sconst` after `')'`, would do it -- but `body` is
//!   `FunctionCallBody`, so the typed-literal form would then admit `*`,
//!   `DISTINCT`, `ALL` and `VARIADIC`, which gram.y rejects grammatically and
//!   not in a rule action. That is over-acceptance bought for 3 conflicts
//!   that cannot reach zero anyway, so it is not taken.
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
//!   `UESCAPE` as a column label. The fix is lexical, not grammatical: it is
//!   not expressible in an LALR grammar that sees the two tokens, because
//!   `UESCAPE` is a bare label and so may legitimately follow the literal.
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

        assert_eq!(
            rd_error.kind(),
            lr_error.kind(),
            "error kind for {source:?}"
        );
        assert_eq!(
            rd_error.span(),
            lr_error.span(),
            "failing span for {source:?}"
        );
        assert_eq!(
            rd_error.found(),
            lr_error.found(),
            "failing token for {source:?}"
        );
    }
}
