//! TRUNCATE statement.

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;

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
    /// A single relation reference in a `TRUNCATE` statement — Postgres'
    /// `relation_expr`.
    ///
    /// `ONLY name` excludes inheritance children; a trailing `*` makes the
    /// (default) inheritance behaviour explicit. The `ONLY (name)` parenthesised
    /// form is not exercised by any TRUNCATE corpus statement, so it is not
    /// modelled (matches the `LockRelation` shape).
    #[derive(Debug)]
    pub struct TruncateRelation {
        #[presence(ONLY)]
        pub only: bool,
        pub name: QualifiedName,
        #[presence(STAR)]
        pub star: bool,
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
        pub relations: one_or_many!(TruncateRelation),
        pub restart_seqs: Option<RestartSeqs>,
        pub behavior: Option<DropBehavior>,
    }
}
