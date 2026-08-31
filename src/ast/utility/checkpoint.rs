//! CHECKPOINT statement.

/// `CHECKPOINT` — force a transaction log checkpoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum CheckpointStmt {
    #[tok(CHECKPOINT)]
    Value,
}
