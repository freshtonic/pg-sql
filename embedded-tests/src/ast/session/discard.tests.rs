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
