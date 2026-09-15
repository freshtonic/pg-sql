//! CHECKPOINT statement.

recursa::ast_node! {
    /// `CHECKPOINT` — force a transaction log checkpoint.
    #[derive(Debug)]
    pub enum CheckpointStmt {
        #[tok(CHECKPOINT)]
        Value,
    }
}
