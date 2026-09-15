//! DISCARD statement.

// --- DISCARD ---

recursa::ast_node! {
    /// Target of a `DISCARD` statement.
    ///
    /// Variant ordering: `TEMPORARY` (longer) before `TEMP` so the longer keyword
    /// wins longest-match disambiguation; the rest have disjoint first-sets.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// DISCARD { ALL | PLANS | SEQUENCES | TEMP | TEMPORARY }
    #[derive(Debug)]
    pub struct DiscardStmt {
        #[tok(DISCARD, this)]
        pub target: DiscardTarget,
    }
}
