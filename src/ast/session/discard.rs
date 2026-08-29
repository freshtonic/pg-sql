//! DISCARD statement.

use recursa_diagram::railroad;

use crate::tokens::keyword::*;

// --- DISCARD ---

/// Target of a `DISCARD` statement.
///
/// Variant ordering: `TEMPORARY` (longer) before `TEMP` so the longer keyword
/// wins longest-match disambiguation; the rest have disjoint first-sets.
#[derive(recursa::Node, Debug, Clone)]
pub enum DiscardTarget {
    #[tok(ALL)] All,
    #[tok(PLANS)] Plans,
    #[tok(SEQUENCES)] Sequences,
    #[tok(TEMPORARY)] Temporary,
    #[tok(TEMP)] Temp,
}

/// DISCARD { ALL | PLANS | SEQUENCES | TEMP | TEMPORARY }
#[derive(recursa::Node, Debug, Clone)]
pub struct DiscardStmt {
    #[tok(DISCARD, this)]
    pub target: DiscardTarget,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn discard_all_roundtrips() {
        assert_eq!(roundtrip::<DiscardStmt>("DISCARD ALL"), "DISCARD ALL");
    }

    #[test]
    fn discard_temp_roundtrips() {
        assert_eq!(roundtrip::<DiscardStmt>("DISCARD TEMP"), "DISCARD TEMP");
    }

    #[test]
    fn discard_plans_roundtrips() {
        assert_eq!(roundtrip::<DiscardStmt>("DISCARD PLANS"), "DISCARD PLANS");
    }

    #[test]
    fn discard_sequences_roundtrips() {
        assert_eq!(
            roundtrip::<DiscardStmt>("DISCARD SEQUENCES"),
            "DISCARD SEQUENCES"
        );
    }
}
