//! CHECKPOINT statement.

/// `CHECKPOINT` — force a transaction log checkpoint.
#[derive(recursa::Node, Debug)]
pub enum CheckpointStmt {
    #[tok(CHECKPOINT)]
    Value,
}
