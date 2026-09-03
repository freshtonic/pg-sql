//! DISCARD statement.

// --- DISCARD ---

/// Target of a `DISCARD` statement.
///
/// Variant ordering: `TEMPORARY` (longer) before `TEMP` so the longer keyword
/// wins longest-match disambiguation; the rest have disjoint first-sets.
#[derive(recursa::Node, Debug, Clone)]
pub enum DiscardTarget {
    #[tok(ALL)]
    All,
    #[tok(PLANS)]
    Plans,
    #[tok(SEQUENCES)]
    Sequences,
    #[tok(TEMPORARY)]
    Temporary,
    #[tok(TEMP)]
    Temp,
}

/// DISCARD { ALL | PLANS | SEQUENCES | TEMP | TEMPORARY }
#[derive(recursa::Node, Debug, Clone)]
pub struct DiscardStmt {
    #[tok(DISCARD, this)]
    pub target: DiscardTarget,
}
