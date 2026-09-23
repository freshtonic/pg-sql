//! `ALTER SYSTEM` — PostgreSQL's `AlterSystemStmt`.

recursa::ast_node! {
    /// `RESET generic_reset` — the reset half of `AlterSystemStmt`.
    ///
    /// `ALTER SYSTEM` takes `generic_reset`, not the `reset_rest` of a
    /// top-level `RESET`, so `ALTER SYSTEM RESET TIME ZONE`,
    /// `... RESET SESSION AUTHORIZATION` and
    /// `... RESET TRANSACTION ISOLATION LEVEL` are syntax errors.
    #[derive(Debug)]
    pub struct AlterSystemReset {
        #[tok(RESET, this)]
        pub target: crate::ast::session::set_reset::GenericReset,
    }
}

recursa::ast_node! {
    /// The two halves of PostgreSQL's `AlterSystemStmt`: `SET generic_set`
    /// and `RESET generic_reset`.
    ///
    /// `SET` takes `generic_set` only, so none of the `set_rest_more` special
    /// forms (`TIME ZONE`, `ROLE`, `SESSION AUTHORIZATION`, `XML OPTION`,
    /// `SCHEMA`, `NAMES`, `FROM CURRENT`) and neither `LOCAL` nor `SESSION`
    /// is an `ALTER SYSTEM` target.
    ///
    /// Variant ordering: `Set` before `SetNull` for the reason given on
    /// [`crate::ast::session::set_reset::VariableSetRest`] — the value list of
    /// `Set` cannot hold `NULL`, so the two are disjoint after `TO` or `=`.
    #[derive(Debug)]
    pub enum AlterSystemAction {
        Set(crate::ast::session::set_reset::SetStmt),
        /// Added in 19: `generic_set: var_name TO NULL_P | var_name '=' NULL_P`
        /// (gram.y b73d13c:1727, 1737; research PostgreSQL 19, "Changes to
        /// existing statements", commit ff4597acd).
        #[cfg(feature = "since-pg19")]
        SetNull(crate::ast::session::set_reset::SetNullStmt),
        Reset(AlterSystemReset),
    }
}

recursa::ast_node! {
    /// `ALTER SYSTEM { SET generic_set | RESET generic_reset }` — PostgreSQL's
    /// `AlterSystemStmt`. Every target version has the same two forms; only
    /// `generic_set` moves, with the `NULL` value that 19 adds.
    #[derive(Debug)]
    pub struct AlterSystemStmt {
        #[tok(ALTER, SYSTEM, this)]
        pub action: AlterSystemAction,
    }
}
