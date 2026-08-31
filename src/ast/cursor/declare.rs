//! DECLARE cursor — see `cursor/fetch.rs` for FETCH/MOVE/CLOSE.

use crate::tokens::literal;

// --- Cursor operations ---

/// A single cursor option in `DECLARE ... cursor_options CURSOR`.
///
/// Postgres' `cursor_options` is a repeatable, order-free list. `NO SCROLL`
/// (2 tokens) is declared before bare `SCROLL` so longest-match-wins picks
/// it; the rest have disjoint first-sets.
#[derive(recursa::Node, Debug, Clone)]
pub enum CursorOption {
    #[tok(NO, SCROLL)]
    NoScroll,
    #[tok(SCROLL)]
    Scroll,
    #[tok(BINARY)]
    Binary,
    #[tok(ASENSITIVE)]
    Asensitive,
    #[tok(INSENSITIVE)]
    Insensitive,
}

/// `{ WITH | WITHOUT } HOLD` cursor-hold clause (`opt_hold` in `gram.y`).
#[derive(recursa::Node, Debug, Clone)]
pub enum CursorHold {
    #[tok(WITH, HOLD)]
    With,
    #[tok(WITHOUT, HOLD)]
    Without,
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
    pub cursor: CursorKeyword,
    pub hold: Option<CursorHold>,
    #[tok(FOR, this)]
    pub query: Box<crate::ast::dml::values::Subquery<'input>>,
}

/// Required `CURSOR` marker between declaration options and hold mode.
#[derive(recursa::Node, Debug, Clone)]
pub enum CursorKeyword {
    #[tok(CURSOR)]
    Cursor,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/cursor/declare.tests.rs"
));
