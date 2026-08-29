//! CHECKPOINT statement.

use recursa_diagram::railroad;

use crate::tokens::keyword::*;

/// `CHECKPOINT` — force a transaction log checkpoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum CheckpointStmt { #[tok(CHECKPOINT)] Value, }
