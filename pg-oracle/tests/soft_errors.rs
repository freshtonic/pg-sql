//! The soft-error API of the oracle (issue #80).
//!
//! PostgreSQL's `errsave_start` records the error in the caller's
//! `ErrorSaveContext` and returns false, so the caller continues. The raw
//! parser uses this in `process_integer_literal` (`scan.l`): a literal that
//! does not fit in `int32` becomes an `FCONST`, not an error. The oracle
//! must do the same, or it rejects SQL that PostgreSQL accepts.
//!
//! The `ErrorSaveContext` path exists from PostgreSQL 16. Before 16 the same
//! function reaches the `FCONST` fallback through `strtoint` and `errno`, so
//! the token is an `FCONST` at every target version. Only the grammar rules
//! that take the token differ between versions, and a test that depends on
//! such a rule carries the version gate of that rule.

use pg_oracle::parse_ok;

/// One more than `INT32_MAX`: the smallest decimal literal that takes the
/// soft-error path.
#[test]
fn an_integer_literal_above_int32_is_a_float() {
    assert!(parse_ok("SELECT 2147483648"));
}

/// Far above `INT32_MAX`, and above `INT64_MAX` too.
#[test]
fn a_very_large_integer_literal_is_a_float() {
    assert!(parse_ok("SELECT 99999999999999999999"));
}

/// The literal is a float, so it reaches a grammar rule that takes a
/// `NumericOnly`, not only an `ICONST`. `createdb_opt_item` takes a
/// `NumericOnly` from 15; at 14 it takes a `SignedIconst`, so 14 rejects it.
#[cfg(feature = "since-pg15")]
#[test]
fn a_create_database_oid_above_int32_is_accepted() {
    assert!(parse_ok("CREATE DATABASE d OID = 3000000000"));
}

#[cfg(not(feature = "since-pg15"))]
#[test]
fn a_create_database_oid_above_int32_is_rejected_before_15() {
    assert!(parse_ok("CREATE DATABASE d OID = 2147483647"));
    assert!(!parse_ok("CREATE DATABASE d OID = 3000000000"));
}

/// The same fallback under a unary minus.
#[test]
fn a_negative_integer_literal_below_int32_is_a_float() {
    assert!(parse_ok("SELECT -2147483649"));
}

/// The token is `FCONST`, not `ICONST`. A grammar rule that takes only a
/// `SignedIconst` therefore rejects it, while the same rule accepts the
/// largest literal that fits in `int32`.
#[test]
fn a_literal_above_int32_is_an_fconst_not_an_iconst() {
    assert!(parse_ok("CREATE ROLE r SYSID 2147483647"));
    assert!(!parse_ok("CREATE ROLE r SYSID 2147483648"));
    assert!(parse_ok("ALTER TABLE t ALTER c SET STATISTICS 2147483647"));
    assert!(!parse_ok("ALTER TABLE t ALTER c SET STATISTICS 3000000000"));
}

/// Non-decimal literals arrived in 16 and take the same fallback.
#[cfg(feature = "since-pg16")]
#[test]
fn a_hexadecimal_literal_above_int32_is_a_float() {
    assert!(parse_ok("SELECT 0x100000000"));
    assert!(parse_ok("CREATE DATABASE d OID = 0xC0000000"));
    // The same `FCONST`/`ICONST` split as the decimal form.
    assert!(parse_ok("CREATE ROLE r SYSID 0x7fffffff"));
    assert!(!parse_ok("CREATE ROLE r SYSID 0x100000000"));
}

/// The hard-error path is unchanged: a soft error with no context, and every
/// ordinary syntax error, still rejects.
#[test]
fn the_hard_error_path_still_rejects() {
    assert!(!parse_ok("SELECT FROM FROM"));
    assert!(!parse_ok("SELECT 1 +"));
    // The scanner reports this one through yyerror, not through errsave.
    assert!(!parse_ok("SELECT $$"));
}

/// A soft error must not leave state behind: the next statement parses as it
/// would on its own.
#[test]
fn a_soft_error_does_not_disturb_the_next_parse() {
    assert!(parse_ok("SELECT 2147483648"));
    assert!(!parse_ok("SELECT FROM FROM"));
    assert!(parse_ok("SELECT 1"));
    assert!(parse_ok("SELECT 2147483648"));
}

/// From 18 the `$n` parameter path also has an `ErrorSaveContext`, but the
/// scanner turns the soft error into `yyerror("parameter number too large")`.
/// So it stays a rejection.
#[cfg(feature = "since-pg18")]
#[test]
fn a_parameter_number_above_int32_is_rejected() {
    assert!(parse_ok("SELECT $2147483647"));
    assert!(!parse_ok("SELECT $2147483648"));
}

/// Before 18 the parameter number goes through `atol` with no range check.
#[cfg(not(feature = "since-pg18"))]
#[test]
fn a_parameter_number_above_int32_is_accepted_before_18() {
    assert!(parse_ok("SELECT $2147483648"));
}
