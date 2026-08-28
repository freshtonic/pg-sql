//! DISCARD statement.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;

// --- DISCARD ---

/// Target of a `DISCARD` statement.
///
/// Variant ordering: `TEMPORARY` (longer) before `TEMP` so the longer keyword
/// wins longest-match disambiguation; the rest have disjoint first-sets.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DiscardTarget {
    All(ALL),
    Plans(PLANS),
    Sequences(SEQUENCES),
    Temporary(TEMPORARY),
    Temp(TEMP),
}

/// DISCARD { ALL | PLANS | SEQUENCES | TEMP | TEMPORARY }
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct DiscardStmt {
    pub discard: DISCARD,
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
