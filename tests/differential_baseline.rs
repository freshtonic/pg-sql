//! Fast, oracle-free validation of the frozen differential baseline.

#[path = "support/baseline.rs"]
mod baseline;

#[test]
fn accepted_legacy_gap_contract_has_one_identity_per_frozen_skip() {
    let gaps = baseline::AcceptedLegacyGaps::pinned();

    assert_eq!(gaps.entries().len(), 18);
    // The last diagnostic-outcome gap (join.sql:171, `USING t1 JOIN t2 USING`)
    // resolved when suffix-proving optional viability landed in Recursa.
    assert_eq!(
        gaps.entries()
            .iter()
            .filter(|gap| matches!(
                &gap.outcome,
                baseline::AcceptedLegacyGapOutcome::Diagnostic(_)
            ))
            .count(),
        0
    );
}
