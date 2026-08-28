//! CHECKPOINT statement.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;

/// `CHECKPOINT` — force a transaction log checkpoint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct CheckpointStmt {
    pub checkpoint: CHECKPOINT,
}
