//! CHECKPOINT statement.

recursa::ast_node! {
    /// `CHECKPOINT [(option [, ...])]` — force a transaction log checkpoint.
    /// gram.y `CheckPointStmt` (b73d13c:2100).
    #[derive(Debug)]
    pub enum CheckpointStmt {
        /// Added in 19: `CHECKPOINT opt_utility_option_list` (research
        /// PostgreSQL 19, "Changes to existing statements").
        #[config(since = pg19)]
        Options(CheckpointWithOptions),
        #[tok(CHECKPOINT)]
        Value,
    }
}

#[cfg(feature = "since-pg19")]
recursa::ast_node! {
    /// `CHECKPOINT (option [, ...])`, added in 19 (b73d13c:2100).
    #[derive(Debug)]
    pub struct CheckpointWithOptions {
        #[tok(CHECKPOINT, this)]
        pub options: crate::ast::utility::explain::UtilityOptionList,
    }
}
