//! TRUNCATE statement.

use crate::ast::shared::flags::DropBehavior;

// --- TRUNCATE ---

recursa::ast_node! {
    /// `{ RESTART | CONTINUE } IDENTITY` — Postgres' `opt_restart_seqs`.
    #[derive(Debug)]
    pub enum RestartSeqs {
        #[tok(RESTART, IDENTITY)]
        Restart,
        #[tok(CONTINUE, IDENTITY)]
        Continue,
    }
}

recursa::ast_node! {
    /// ```sql
    /// TRUNCATE [TABLE] [ONLY] name [*] [, ...]
    ///     [ { RESTART | CONTINUE } IDENTITY ]
    ///     [ CASCADE | RESTRICT ]
    /// ```
    #[derive(Debug)]
    #[tok(TRUNCATE, optional(TABLE), this)]
    pub struct TruncateStmt {
        #[sep(COMMA)]
        pub relations: one_or_many!(crate::ast::shared::names::RelationExpr),
        pub restart_seqs: Option<RestartSeqs>,
        pub behavior: Option<DropBehavior>,
    }
}
