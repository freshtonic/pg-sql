//! `ConstraintAttributeSpec`'s own conflict checks, reproduced as Recursa
//! declared-rejection rules (`docs/declared-rejections.md` in Recursa;
//! `freshtonic/recursa#138`, `freshtonic/pg-sql#86`).
//!
//! `processCASbits`'s action (REL_17_11 gram.y 6098-6125, `ConstraintAttributeSpec`)
//! rejects two conflicts among the deferrability entries of a constraint
//! attribute list: the specific message for `NOT DEFERRABLE` beside
//! `INITIALLY DEFERRED` (REL_17_11 gram.y 6114), and the generic message for
//! `NOT DEFERRABLE` beside `DEFERRABLE` or `INITIALLY IMMEDIATE` beside
//! `INITIALLY DEFERRED` (REL_17_11 gram.y 6121). This suite proves the
//! declared rejection reports the right SQLSTATE, the right message, and a
//! span over the item that completed the pair — never the whole list — and
//! that the value, arena and flat parse entry points agree.

use pg_sql::ast::Statement;
use pg_sql::lex;
use recursa::{ArenaParse, ParseError};

/// Parses one complete statement and returns the error, through whichever
/// parse entry point `entry` names.
fn parse_err(source: &'static str, entry: Entry) -> ParseError {
    let lexed = lex(source);
    assert_eq!(lexed.errors().count(), 0, "lex errors in {source:?}");
    match entry {
        Entry::Value => {
            let mut input = lexed.input();
            Statement::parse(&mut input).expect_err("statement must be rejected")
        }
        Entry::Arena => {
            let mut input = lexed.input();
            Statement::parse_arena(&mut input).expect_err("statement must be rejected")
        }
        Entry::Flat => {
            let mut input = lexed.input();
            match Statement::parse_flat(&mut input) {
                Ok(_) => panic!("{source:?} must be rejected"),
                Err(error) => error,
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Entry {
    Value,
    Arena,
    Flat,
}

/// Asserts that every parse entry point rejects `source` with the same
/// declared rejection: runtime code `RCA4105`, the rule's `code`, `message`,
/// and a span over exactly `expect_span_text` within `source`.
fn assert_declared_rejection(
    source: &'static str,
    expect_code: &str,
    expect_message: &str,
    #[cfg_attr(not(feature = "spans"), allow(unused_variables))] expect_span_text: &str,
) {
    for entry in [Entry::Value, Entry::Arena, Entry::Flat] {
        let error = parse_err(source, entry);
        assert_eq!(
            error.code(),
            "RCA4105",
            "a declared rejection reports RCA4105 for {source:?}"
        );
        let note = error
            .rejection()
            .unwrap_or_else(|| panic!("{source:?} must fail as a declared rejection, got {error}"));
        assert_eq!(note.code(), Some(expect_code), "rule code for {source:?}");
        assert_eq!(
            note.message(),
            expect_message,
            "rule message for {source:?}"
        );
        assert_eq!(
            error.to_string(),
            expect_message,
            "a declared rejection's Display is the rule's message"
        );

        #[cfg(feature = "spans")]
        {
            let start = source
                .find(expect_span_text)
                .unwrap_or_else(|| panic!("{expect_span_text:?} not found in {source:?}"));
            let end = start + expect_span_text.len();
            let expected_span = recursa::Span::new(start, end).unwrap();
            assert_eq!(
                error.span(),
                expected_span,
                "rejection span for {source:?} must cover {expect_span_text:?}"
            );
        }
    }
}

#[test]
fn not_deferrable_beside_initially_deferred_is_the_specific_rule() {
    assert_declared_rejection(
        "CREATE TABLE t (a int, UNIQUE (a) NOT DEFERRABLE INITIALLY DEFERRED)",
        "42601",
        "constraint declared INITIALLY DEFERRED must be DEFERRABLE",
        "INITIALLY DEFERRED",
    );
}

#[test]
fn deferrable_beside_not_deferrable_is_the_generic_rule() {
    assert_declared_rejection(
        "CREATE TABLE t (a int, UNIQUE (a) DEFERRABLE NOT DEFERRABLE)",
        "42601",
        "conflicting constraint properties",
        "NOT DEFERRABLE",
    );
}
