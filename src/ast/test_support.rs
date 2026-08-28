//! Shared test helpers for the relocated `legacy_tests` test bodies.
//!
//! The legacy module defined three private helpers (`parse_stmt`,
//! `reparse_stable`, `roundtrip`) that many tests relied on. After the
//! relocation the tests live in dozens of per-file `mod tests` blocks
//! scattered across `ast/<area>/<file>.rs`, so the helpers need a single
//! shared home; duplicating them per file would violate DRY and bloat
//! the diff. Each test mod simply imports from
//! `crate::ast::test_support::*` to keep the original bodies verbatim.

#![cfg(test)]

use recursa::{FormatTokens, Parse};

use crate::formatter::format_tokens_sql;

/// `parse_stmt(src)` parses `src` as the given statement type and asserts
/// that the whole input was consumed, returning the typed AST.
pub fn parse_stmt<T: Parse<'static>>(src: &'static str) -> T {
    let mut input = crate::tokens::test_input(src);
    let stmt = T::parse(&mut input).unwrap();
    assert!(input.is_empty(), "unconsumed input after parsing {src:?}");
    stmt
}

/// Asserts the formatter output is a fixed point: parsing `src`, formatting
/// it, then re-parsing and re-formatting yields the same text. This proves
/// the modelled statement round-trips without exact-whitespace coupling
/// (the differential test is the structural-equality oracle against PG).
pub fn reparse_stable<T>(src: &'static str)
where
    T: Parse<'static> + FormatTokens,
{
    let mut input = crate::tokens::test_input(src);
    let stmt = T::parse(&mut input).unwrap();
    assert!(input.is_empty(), "unconsumed input after parsing {src:?}");
    let once = format_tokens_sql(&stmt, recursa::fmt::FormatStyle::default());
    let leaked: &'static str = Box::leak(once.clone().into_boxed_str());
    let mut reinput = crate::tokens::test_input(leaked);
    let restmt = T::parse(&mut reinput).unwrap();
    assert!(
        reinput.is_empty(),
        "unconsumed input after re-parsing {once:?}"
    );
    let twice = format_tokens_sql(&restmt, recursa::fmt::FormatStyle::default());
    assert_eq!(once, twice, "formatter output is not a fixed point");
}

/// Parse `src` as `T`, format the result back to SQL, and return the
/// formatted text. Used by tests asserting a fixed formatter shape.
pub fn roundtrip<T>(src: &'static str) -> String
where
    T: Parse<'static> + FormatTokens,
{
    let mut input = crate::tokens::test_input(src);
    let stmt = T::parse(&mut input).unwrap();
    assert!(input.is_empty(), "unconsumed input after parsing {src:?}");
    format_tokens_sql(&stmt, recursa::fmt::FormatStyle::default())
}
