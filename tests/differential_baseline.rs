//! Fast, oracle-free validation of the frozen differential baseline.

#[path = "support/baseline.rs"]
mod baseline;

#[test]
fn accepted_legacy_gap_contract_has_one_identity_per_frozen_skip() {
    let gaps = baseline::AcceptedLegacyGaps::pinned();

    assert_eq!(gaps.entries().len(), 18);
    assert_eq!(
        gaps.entries()
            .iter()
            .filter(|gap| matches!(
                &gap.outcome,
                baseline::AcceptedLegacyGapOutcome::Diagnostic(_)
            ))
            .count(),
        1
    );
}
