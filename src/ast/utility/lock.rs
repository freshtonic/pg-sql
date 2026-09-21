//! LOCK statement.

// --- LOCK ---

recursa::ast_node! {
    /// A `LOCK` lock-mode name — Postgres' `lock_type`.
    ///
    /// Variant ordering: the three-word `SHARE ROW EXCLUSIVE` /
    /// `SHARE UPDATE EXCLUSIVE` forms precede the two-word `ROW EXCLUSIVE` and the
    /// bare `SHARE`, so longest-match-wins picks the most specific spelling.
    #[derive(Debug)]
    pub enum LockType {
        #[tok(ACCESS, SHARE)]
        AccessShare,
        #[tok(ACCESS, EXCLUSIVE)]
        AccessExclusive,
        #[tok(SHARE, ROW, EXCLUSIVE)]
        ShareRowExclusive,
        #[tok(SHARE, UPDATE, EXCLUSIVE)]
        ShareUpdateExclusive,
        #[tok(ROW, SHARE)]
        RowShare,
        #[tok(ROW, EXCLUSIVE)]
        RowExclusive,
        #[tok(SHARE)]
        Share,
        #[tok(EXCLUSIVE)]
        Exclusive,
    }
}

recursa::ast_node! {
    /// The `IN lock_type MODE` clause on a `LOCK` statement — Postgres' `opt_lock`.
    #[derive(Debug)]
    pub struct LockMode {
        #[tok(IN, this, MODE)]
        pub lock_type: LockType,
    }
}

recursa::ast_node! {
    /// ```sql
    /// LOCK [TABLE] name [, ...] [IN mode MODE] [NOWAIT]
    /// ```
    #[derive(Debug)]
    pub struct LockStmt {
        #[tok(LOCK, this)]
        #[presence(TABLE)]
        pub table: bool,
        #[sep(COMMA)]
        pub relations: one_or_many!(crate::ast::shared::names::RelationExpr),
        pub mode: Option<LockMode>,
        #[presence(NOWAIT)]
        pub nowait: bool,
    }
}
