//! Public strict PostgreSQL-statement seam.
//!
//! Issue #9 restores this one-statement entry point before document framing,
//! psql behavior, formatting provenance, or recovery are introduced.

use pg_sql::{
    LexErrorCode,
    ast::{Statement, dml::select::SelectBody, dml::values::Subquery},
    lex,
};
use recursa::{ParseErrorKind, Span};

#[test]
fn parses_one_complete_semantically_typed_statement() {
    let lexed = lex("SELECT 1");
    assert!(lexed.errors().next().is_none());

    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).expect("strict statement");

    assert!(input.is_eof(), "strict parsing must consume the statement");
    assert!(matches!(
        parsed.into_ast(),
        Statement::Query(query)
            if matches!(
                query.as_ref(),
                Subquery::Body(body) if matches!(&body.body, SelectBody::Select(_))
            )
    ));
}

#[test]
fn query_statement_owns_every_postgresql_query_prefix() {
    for source in [
        "SELECT 1",
        "VALUES (1)",
        "TABLE example",
        "WITH example AS (SELECT 1) SELECT * FROM example",
        "WITH RECURSIVE recursive AS (SELECT 1) SELECT * FROM recursive",
        "WITH example AS (SELECT 1), recursive AS (SELECT 2) SELECT * FROM recursive",
    ] {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );

        let mut input = lexed.input();
        let parsed = Statement::parse(&mut input)
            .unwrap_or_else(|error| panic!("strict query {source:?}: {error}"));

        assert!(input.is_eof(), "strict query left input for {source:?}");
        assert!(
            matches!(parsed.into_ast(), Statement::Query(_)),
            "query prefix selected a non-query statement for {source:?}"
        );
    }
}

#[test]
fn recursive_immediately_after_with_commits_to_the_clause_modifier() {
    let lexed = lex("WITH recursive AS (SELECT 1) SELECT 1");
    assert!(lexed.errors().next().is_none());

    let mut input = lexed.input();
    assert!(
        Statement::parse(&mut input).is_err(),
        "PostgreSQL treats the immediate RECURSIVE as the modifier, not as a CTE name"
    );
}

#[test]
fn parses_explain_without_optional_settings_as_a_guarded_statement() {
    let lexed = lex("EXPLAIN SELECT 1");
    assert!(lexed.errors().next().is_none());

    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).expect("strict EXPLAIN statement");

    assert!(input.is_eof(), "strict parsing must consume EXPLAIN's body");
    let Statement::Explain(explain) = parsed.into_ast() else {
        panic!("EXPLAIN must select the semantically typed statement variant");
    };
    assert!(explain.options().is_none());
    assert!(matches!(
        explain.statement(),
        pg_sql::ast::utility::explain::ExplainableStmt::Query(query)
            if matches!(
                query.as_ref(),
                Subquery::Body(body) if matches!(&body.body, SelectBody::Select(_))
            )
    ));
}

#[test]
fn lexical_failure_has_a_stable_code_region_and_anchor() {
    let lexed = lex("SELECT /* unterminated");
    let errors = lexed.errors().collect::<Vec<_>>();

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].diagnostic_code(),
        LexErrorCode::UnterminatedNested
    );
    assert_eq!(
        errors[0].span(),
        Span::new(7, 22).expect("valid error span")
    );
    assert_eq!(errors[0].anchor(), Span::new(7, 9).expect("valid anchor"));
}

#[test]
fn strict_parse_failure_does_not_construct_a_statement() {
    let lexed = lex("SELECT FROM");
    assert!(lexed.errors().next().is_none());

    let mut input = lexed.input();
    let original_cursor = input.cursor();
    let original_source = input.source();
    let error = Statement::parse(&mut input).expect_err("invalid strict statement");

    // PostgreSQL permits an empty SELECT target list, so FROM is committed
    // before the missing relation is diagnosed at end of input.
    assert_eq!(error.kind(), ParseErrorKind::UnexpectedEof);
    assert_eq!(error.code(), "RCA4101");
    assert_eq!(error.span(), Span::new(11, 11).expect("valid EOF span"));
    assert_eq!(error.anchor(), Span::new(11, 11).expect("valid EOF anchor"));
    assert_eq!(error.found(), None);
    assert_eq!(
        input.cursor(),
        original_cursor,
        "strict failure must restore the exact public input cursor"
    );
    assert_eq!(
        input.source(),
        original_source,
        "strict failure must preserve the public input source"
    );
}

#[test]
fn strict_parse_rejects_a_terminated_empty_from_list() {
    let lexed = lex("SELECT FROM;");
    assert!(lexed.errors().next().is_none());

    let mut input = lexed.input();
    assert!(
        Statement::parse(&mut input).is_err(),
        "PostgreSQL requires a from-list item before the terminator"
    );
}

#[test]
fn strict_parse_rejects_a_trailing_comma_from_list() {
    let lexed = lex("SELECT FROM emp,");
    assert!(lexed.errors().next().is_none());

    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input);
    assert!(
        parsed.is_err() || !input.is_eof(),
        "PostgreSQL requires a from-list item after the comma"
    );
}

/// A Pratt extender that PostgreSQL's shift preference makes unreachable as a
/// bare output alias must extend the target expression, not end it.
///
/// `target_el: a_expr BareColLabel` lists AND, OR, NOT, IN, COLLATE and the
/// rest of the extender keywords in `bare_label_keyword`, but bison always
/// shifts them into `a_expr`, so no PostgreSQL statement can reach them as a
/// bare alias. `SelectBareAliasName` therefore excludes them, which removes
/// the caller-FOLLOW overlap that made the Pratt loop report `RCA4102`.
#[test]
fn select_targets_extend_on_every_operator_keyword() {
    for source in [
        "SELECT a OR b",
        "SELECT a AND b",
        "SELECT a COLLATE \"C\"",
        "SELECT a NOT LIKE 'b'",
        "SELECT a IN (1, 2)",
        "SELECT 'abc' LIKE 'a%'",
        "SELECT 'abc' ILIKE 'A%'",
        "SELECT 'abc' SIMILAR TO 'a%'",
        "SELECT 1 BETWEEN 0 AND 2",
        "SELECT t AT TIME ZONE 'GMT'",
        "SELECT a ISNULL",
        "SELECT a NOTNULL",
        "SELECT bool 't' or bool 'f' AS true",
    ] {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );
        let mut input = lexed.input();
        Statement::parse(&mut input)
            .unwrap_or_else(|error| panic!("strict statement {source:?}: {error}"));
        assert!(input.is_eof(), "strict parse left input for {source:?}");
    }
}

/// A bare output alias still accepts every keyword PostgreSQL can reach there.
#[test]
fn select_targets_keep_every_reachable_bare_alias() {
    for source in [
        "SELECT a alias",
        "SELECT a value",
        "SELECT a true",
        "SELECT a AS between",
        "SELECT a AS collate",
    ] {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );
        let mut input = lexed.input();
        Statement::parse(&mut input)
            .unwrap_or_else(|error| panic!("strict statement {source:?}: {error}"));
        assert!(input.is_eof(), "strict parse left input for {source:?}");
    }
}

/// `index_elem` and `part_elem` take a column, a windowless function call, or a
/// parenthesized expression — never a bare `a_expr` — so `COLLATE` belongs to
/// the element rather than to an expression that would swallow it.
#[test]
fn index_shaped_elements_take_a_restricted_target() {
    for source in [
        "CREATE TABLE t (a text) PARTITION BY RANGE (a COLLATE \"POSIX\")",
        "INSERT INTO t VALUES (0) ON CONFLICT (fruit COLLATE \"C\" text_pattern_ops) DO NOTHING",
    ] {
        let lexed = lex(source);
        assert!(
            lexed.errors().next().is_none(),
            "lexical errors in {source:?}"
        );
        let mut input = lexed.input();
        Statement::parse(&mut input)
            .unwrap_or_else(|error| panic!("strict statement {source:?}: {error}"));
        assert!(input.is_eof(), "strict parse left input for {source:?}");
    }
}
