//! FETCH / MOVE / CLOSE cursor statements.

use crate::tokens::literal;

recursa::ast_node! {
    /// `FROM` or `IN` cursor-source keyword in FETCH/MOVE.
    #[derive(Debug)]
    pub enum FetchSource {
        #[tok(FROM)]
        From,
        #[tok(IN)]
        In,
    }
}

recursa::ast_node! {
    /// `ABSOLUTE n` form. `n` is a `SignedIconst` per gram.y's
    /// `fetch_args: ABSOLUTE_P SignedIconst opt_from_in cursor_name` — so a
    /// leading sign (e.g. `ABSOLUTE -1`) is accepted.
    #[derive(Debug)]
    pub struct FetchAbsolute {
        #[tok(ABSOLUTE, this)]
        pub count: crate::ast::shared::numbers::SignedIconst,
    }
}

recursa::ast_node! {
    /// `RELATIVE n` form. `n` is a `SignedIconst` (see [`FetchAbsolute`]).
    #[derive(Debug)]
    pub struct FetchRelative {
        #[tok(RELATIVE, this)]
        pub count: crate::ast::shared::numbers::SignedIconst,
    }
}

recursa::ast_node! {
    /// `FORWARD [n|ALL]` form.
    #[derive(Debug)]
    #[tok(FORWARD, this)]
    pub struct FetchForward {
        pub count: Option<FetchCountOrAll>,
    }
}

recursa::ast_node! {
    /// `BACKWARD [n|ALL]` form.
    #[derive(Debug)]
    #[tok(BACKWARD, this)]
    pub struct FetchBackward {
        pub count: Option<FetchCountOrAll>,
    }
}

recursa::ast_node! {
    /// A count or `ALL` marker following `FORWARD`/`BACKWARD`.
    ///
    /// The count is a `SignedIconst`, per gram.y's
    /// `fetch_args: FORWARD SignedIconst opt_from_in cursor_name` (REL_17_11
    /// 7477) and `BACKWARD SignedIconst opt_from_in cursor_name` (7504), so
    /// `FORWARD -1` and `BACKWARD +2` are accepted.
    #[derive(Debug)]
    pub enum FetchCountOrAll {
        #[tok(ALL)]
        All,
        Count(crate::ast::shared::numbers::SignedIconst),
    }
}

recursa::ast_node! {
    /// FETCH/MOVE direction clause.
    ///
    /// Variant ordering: multi-token forms (`ABSOLUTE n`, `RELATIVE n`,
    /// `FORWARD [...]`, `BACKWARD [...]`) before single-keyword directions.
    /// `Count` listed last since it has no keyword prefix. It is a
    /// `SignedIconst`, per gram.y's
    /// `fetch_args: SignedIconst opt_from_in cursor_name` (REL_17_11 7450),
    /// so `FETCH -5 c` and `MOVE +5 c` are accepted.
    #[derive(Debug)]
    pub enum FetchDirection {
        Absolute(FetchAbsolute),
        Relative(FetchRelative),
        Forward(FetchForward),
        Backward(FetchBackward),
        #[tok(NEXT)]
        Next,
        #[tok(PRIOR)]
        Prior,
        #[tok(FIRST)]
        First,
        #[tok(LAST)]
        Last,
        #[tok(ALL)]
        All,
        Count(crate::ast::shared::numbers::SignedIconst),
    }
}

recursa::ast_node! {
    /// ```sql
    /// FETCH [direction] [FROM|IN] cursor_name
    /// ```
    #[derive(Debug)]
    #[tok(FETCH, this)]
    pub struct FetchStmt {
        pub direction: Option<FetchDirection>,
        pub source: Option<FetchSource>,
        /// gram.y `cursor_name: name`, a `ColId`.
        pub cursor: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Target of a `CLOSE` statement: a named cursor or `ALL`.
    ///
    /// Variant ordering: `All` (the `ALL` keyword) before `Cursor` so the
    /// reserved word is not swallowed as a cursor name.
    #[derive(Debug)]
    pub enum CloseTarget {
        #[tok(ALL)]
        All,
        Cursor(literal::Ident),
    }
}

recursa::ast_node! {
    /// ```sql
    /// CLOSE { cursor_name | ALL }
    /// ```
    #[derive(Debug)]
    pub struct CloseStmt {
        #[tok(CLOSE, this)]
        pub target: CloseTarget,
    }
}

recursa::ast_node! {
    /// ```sql
    /// MOVE [direction] [FROM|IN] cursor_name
    /// ```
    #[derive(Debug)]
    #[tok(MOVE, this)]
    pub struct MoveStmt {
        pub direction: Option<FetchDirection>,
        pub source: Option<FetchSource>,
        /// gram.y `cursor_name: name`, a `ColId`.
        pub cursor: crate::tokens::ColId,
    }
}
