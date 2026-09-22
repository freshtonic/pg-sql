//! Fast, oracle-free validation of the frozen differential baseline.

// This target uses only part of the shared baseline module.
#[allow(dead_code)]
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

#[test]
fn every_version_baseline_is_complete_and_comes_from_its_pinned_oracle() {
    // Loading checks the corpus identity, the pin, the per-file oracle
    // outcomes and the order and ranges of the expected version gaps.
    for feature in baseline::version_features() {
        let version = baseline::VersionBaseline::of(feature);
        assert_eq!(version.feature, feature);
        assert_eq!(
            (version.oracle_ref.as_str(), version.oracle_commit.as_str()),
            baseline::oracle_pin(feature)
        );
    }
    assert_eq!(
        baseline::VersionBaseline::active().feature,
        pg_sql::TARGET_VERSION.feature()
    );
}

#[test]
fn the_pg17_version_baseline_agrees_with_the_frozen_legacy_baseline() {
    let legacy = baseline::Baseline::pinned();
    let pg17 = baseline::VersionBaseline::of("pg17");
    for file in legacy.file_names() {
        assert_eq!(
            pg17.expected_outcomes(file),
            legacy.file(file).outcomes,
            "{file}: the 17.11 oracle gives the frozen 17.9 outcome counts"
        );
    }
    assert_eq!(pg17.expected_totals(), legacy.totals());
    assert!(
        pg17.gaps().is_empty(),
        "pg17 is the complete grammar; it has no expected version gaps"
    );
}
