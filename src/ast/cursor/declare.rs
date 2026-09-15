//! DECLARE cursor — see `cursor/fetch.rs` for FETCH/MOVE/CLOSE.

use crate::tokens::literal;

// --- Cursor operations ---

recursa::ast_node! {
    /// A single cursor option in `DECLARE ... cursor_options CURSOR`.
    ///
    /// Postgres' `cursor_options` is a repeatable, order-free list. `NO SCROLL`
    /// (2 tokens) is declared before bare `SCROLL` so longest-match-wins picks
    /// it; the rest have disjoint first-sets.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `{ WITH | WITHOUT } HOLD` cursor-hold clause (`opt_hold` in `gram.y`).
    #[derive(Debug)]
    pub enum CursorHold {
        #[tok(WITH, HOLD)]
        With,
        #[tok(WITHOUT, HOLD)]
        Without,
    }
}

recursa::ast_node! {
    /// ```sql
    /// DECLARE name [BINARY] [ASENSITIVE | INSENSITIVE] [[NO] SCROLL]
    ///   CURSOR [{WITH | WITHOUT} HOLD] FOR query
    /// ```
    ///
    /// `query` is `Subquery` — Postgres' `SelectStmt`, which already covers
    /// `SELECT`, set operations, `VALUES`, `TABLE`, and `WITH`.
    #[derive(Debug)]
    pub struct DeclareStmt {
        #[tok(DECLARE, this)]
        pub name: literal::AliasName,
        pub options: zero_or_many!(CursorOption),
        pub cursor: CursorKeyword,
        pub hold: Option<CursorHold>,
        #[tok(FOR, this)]
        pub query: boxed!(crate::ast::dml::values::Subquery),
    }
}

recursa::ast_node! {
    /// Required `CURSOR` marker between declaration options and hold mode.
    #[derive(Debug)]
    pub enum CursorKeyword {
        #[tok(CURSOR)]
        Cursor,
    }
}
