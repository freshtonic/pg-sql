//! DECLARE cursor — see `cursor/fetch.rs` for FETCH/MOVE/CLOSE.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::literal;

// --- Cursor operations ---

/// A single cursor option in `DECLARE ... cursor_options CURSOR`.
///
/// Postgres' `cursor_options` is a repeatable, order-free list. `NO SCROLL`
/// (2 tokens) is declared before bare `SCROLL` so longest-match-wins picks
/// it; the rest have disjoint first-sets.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CursorOption {
    NoScroll((NO, SCROLL)),
    Scroll(SCROLL),
    Binary(BINARY),
    Asensitive(ASENSITIVE),
    Insensitive(INSENSITIVE),
}

/// `{ WITH | WITHOUT } HOLD` cursor-hold clause (`opt_hold` in `gram.y`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CursorHold {
    With((WITH, HOLD)),
    Without((WITHOUT, HOLD)),
}

/// ```sql
/// DECLARE name [BINARY] [ASENSITIVE | INSENSITIVE] [[NO] SCROLL]
///   CURSOR [{WITH | WITHOUT} HOLD] FOR query
/// ```
///
/// `query` is `Subquery` — Postgres' `SelectStmt`, which already covers
/// `SELECT`, set operations, `VALUES`, `TABLE`, and `WITH`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct DeclareStmt<'input> {
    pub declare: DECLARE,
    pub name: literal::AliasName<'input>,
    pub options: Vec<CursorOption>,
    pub cursor: CURSOR,
    pub hold: Option<CursorHold>,
    pub r#for: FOR,
    pub query: Box<crate::ast::dml::values::Subquery<'input>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn declare_plain_cursor_is_modelled() {
        let stmt: DeclareStmt = parse_stmt("DECLARE c CURSOR FOR SELECT 1");
        assert_eq!(stmt.name.text(), "c");
        assert!(stmt.options.is_empty());
        assert!(stmt.hold.is_none());
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE c CURSOR FOR SELECT 1"),
            "DECLARE c CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_scroll_cursor_keeps_option() {
        let stmt: DeclareStmt = parse_stmt("DECLARE foo1 SCROLL CURSOR FOR SELECT a FROM t");
        assert_eq!(stmt.options.len(), 1);
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo1 SCROLL CURSOR FOR SELECT a FROM t"),
            "DECLARE foo1 SCROLL CURSOR FOR SELECT a FROM t"
        );
    }

    #[test]
    fn declare_no_scroll_cursor_roundtrips() {
        let stmt: DeclareStmt = parse_stmt("DECLARE foo24 NO SCROLL CURSOR FOR SELECT 1");
        assert_eq!(stmt.options.len(), 1);
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo24 NO SCROLL CURSOR FOR SELECT 1"),
            "DECLARE foo24 NO SCROLL CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_binary_cursor_roundtrips() {
        let _stmt: DeclareStmt = parse_stmt("DECLARE bc BINARY CURSOR FOR SELECT 1");
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE bc BINARY CURSOR FOR SELECT 1"),
            "DECLARE bc BINARY CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_insensitive_cursor_roundtrips() {
        let _stmt: DeclareStmt = parse_stmt("DECLARE c1 INSENSITIVE CURSOR FOR SELECT 1");
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE c1 INSENSITIVE CURSOR FOR SELECT 1"),
            "DECLARE c1 INSENSITIVE CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_cursor_with_hold_keeps_hold() {
        let stmt: DeclareStmt = parse_stmt("DECLARE foo25 SCROLL CURSOR WITH HOLD FOR SELECT 1");
        assert!(stmt.hold.is_some());
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo25 SCROLL CURSOR WITH HOLD FOR SELECT 1"),
            "DECLARE foo25 SCROLL CURSOR WITH HOLD FOR SELECT 1"
        );
    }

    #[test]
    fn declare_no_scroll_cursor_with_hold_roundtrips() {
        let _stmt: DeclareStmt =
            parse_stmt("DECLARE foo25ns NO SCROLL CURSOR WITH HOLD FOR SELECT 1");
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo25ns NO SCROLL CURSOR WITH HOLD FOR SELECT 1"),
            "DECLARE foo25ns NO SCROLL CURSOR WITH HOLD FOR SELECT 1"
        );
    }
}
