//! DECLARE cursor — see `cursor/fetch.rs` for FETCH/MOVE/CLOSE.

use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::literal;

// --- Cursor operations ---

/// A single cursor option in `DECLARE ... cursor_options CURSOR`.
///
/// Postgres' `cursor_options` is a repeatable, order-free list. `NO SCROLL`
/// (2 tokens) is declared before bare `SCROLL` so longest-match-wins picks
/// it; the rest have disjoint first-sets.
#[derive(recursa::Node, Debug, Clone)]
pub enum CursorOption {
    #[tok(NO, SCROLL)] NoScroll,
    #[tok(SCROLL)] Scroll,
    #[tok(BINARY)] Binary,
    #[tok(ASENSITIVE)] Asensitive,
    #[tok(INSENSITIVE)] Insensitive,
}

/// `{ WITH | WITHOUT } HOLD` cursor-hold clause (`opt_hold` in `gram.y`).
#[derive(recursa::Node, Debug, Clone)]
pub enum CursorHold {
    #[tok(WITH, HOLD)] With,
    #[tok(WITHOUT, HOLD)] Without,
}

/// ```sql
/// DECLARE name [BINARY] [ASENSITIVE | INSENSITIVE] [[NO] SCROLL]
///   CURSOR [{WITH | WITHOUT} HOLD] FOR query
/// ```
///
/// `query` is `Subquery` — Postgres' `SelectStmt`, which already covers
/// `SELECT`, set operations, `VALUES`, `TABLE`, and `WITH`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DeclareStmt<'input> {
    #[tok(DECLARE, this)]
    pub name: literal::AliasName<'input>,
    pub options: Vec<CursorOption>,
    #[tok(CURSOR, this)]
    pub hold: Option<CursorHold>,
    #[tok(FOR, this)]
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
