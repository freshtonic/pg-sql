//! Exact current outcomes for the PostgreSQL-accepted legacy grammar gaps.

mod support;

use std::collections::BTreeSet;

use pg_oracle::parse_ok;
use support::baseline::{
    AcceptedLegacyGapOutcome, AcceptedLegacyGaps, FrozenStatements, LegacyItemKind, VersionBaseline,
};
use support::diff_check::{Outcome, StrictDiagnostic, check_statement, pgsql_format};

#[test]
fn frozen_accepted_legacy_gap_contracts_are_exact() {
    let frozen = FrozenStatements::pinned();
    let gaps = AcceptedLegacyGaps::pinned();
    let mut derived_identities = BTreeSet::new();

    for file_name in frozen.file_names() {
        let file = frozen.file(file_name);
        let source = file.source();
        let statements = file
            .statements(&source)
            .unwrap_or_else(|error| panic!("cannot load {file_name}: {error}"));
        for (statement_index, (statement, kind)) in statements
            .into_iter()
            .zip(file.legacy_item_kinds())
            .enumerate()
        {
            if *kind == LegacyItemKind::ParseError && parse_ok(statement) {
                let range = &file.ranges()[statement_index];
                derived_identities.insert((
                    file_name.to_owned(),
                    statement_index,
                    range.start,
                    range.end,
                ));
            }
        }
    }

    let contracted_identities = gaps
        .entries()
        .iter()
        .map(|gap| {
            (
                gap.file.clone(),
                gap.statement_index,
                gap.byte_range.start,
                gap.byte_range.end,
            )
        })
        .collect::<BTreeSet<_>>();
    // The ledger is the contract of the PostgreSQL 17 oracle. The oracle of
    // another target version can reject a ledger statement, and it can
    // accept a legacy parse error that 17 rejects. A build of that version
    // then has the 17 grammar, so each such statement must be an expected
    // version gap (docs/differential-baseline.md).
    let version = VersionBaseline::active();
    let version_gaps = version
        .gaps()
        .iter()
        .map(|gap| (gap.file.clone(), gap.statement_index))
        .collect::<BTreeSet<_>>();
    let accepted_contracts = contracted_identities
        .iter()
        .filter(|(file, index, _, _)| version.oracle_accepts(file)[*index])
        .cloned()
        .collect::<BTreeSet<_>>();
    let rejected_contracts = contracted_identities
        .difference(&accepted_contracts)
        .collect::<Vec<_>>();
    assert!(
        version.feature != "pg17" || rejected_contracts.is_empty(),
        "the PostgreSQL 17 oracle rejects accepted legacy gaps: {rejected_contracts:?}"
    );
    let not_contracted = derived_identities
        .difference(&accepted_contracts)
        .filter(|(file, index, _, _)| !version_gaps.contains(&(file.clone(), *index)))
        .collect::<Vec<_>>();
    assert!(
        version.feature != "pg17" || not_contracted.is_empty(),
        "legacy parse errors that the PostgreSQL 17 oracle accepts are not in the \
         accepted legacy gap ledger: {not_contracted:?}"
    );
    // Another version's oracle can accept a legacy parse error that 17
    // rejects. Unless it is an expected version gap, pg-sql must pass it.
    for (file, index, _, _) in not_contracted {
        let source = frozen.file(file).source();
        let statement = frozen
            .file(file)
            .statements(&source)
            .unwrap_or_else(|error| panic!("cannot load {file}: {error}"))[*index];
        assert_eq!(
            check_statement(&support::Stmt {
                source: statement.to_owned(),
            }),
            Outcome::Pass,
            "{file}:{index} is accepted by the {} oracle but is not an expected version gap",
            version.feature
        );
    }
    assert!(
        accepted_contracts.is_subset(&derived_identities),
        "the recorded {} oracle outcomes disagree with the live oracle",
        version.feature
    );

    for gap in gaps.entries() {
        if !version.oracle_accepts(&gap.file)[gap.statement_index]
            || version_gaps.contains(&(gap.file.clone(), gap.statement_index))
        {
            // The target version rejects the statement, or its build has an
            // expected version gap there. The differential test checks both.
            continue;
        }
        let source = frozen.file(&gap.file).source();
        let statement = frozen
            .file(&gap.file)
            .statements(&source)
            .unwrap_or_else(|error| panic!("cannot load {}: {error}", gap.file))
            [gap.statement_index];
        assert!(
            parse_ok(statement),
            "{}:{} is no longer accepted by the pinned PostgreSQL oracle",
            gap.file,
            gap.statement_index
        );

        match &gap.outcome {
            AcceptedLegacyGapOutcome::Pass => assert_eq!(
                check_statement(&support::Stmt {
                    source: statement.to_owned(),
                }),
                Outcome::Pass,
                "{}:{} regressed from a resolved legacy gap",
                gap.file,
                gap.statement_index
            ),
            AcceptedLegacyGapOutcome::Diagnostic(expected) => {
                let failure = match pgsql_format(statement) {
                    Ok(_) => panic!(
                        "{}:{} unexpectedly resolved; review the accepted-gap ledger",
                        gap.file, gap.statement_index
                    ),
                    Err(failure) => failure,
                };
                assert_eq!(
                    failure.diagnostic(),
                    Some(&StrictDiagnostic {
                        code: expected.code.clone(),
                        region: expected.region.clone(),
                        anchor: expected.anchor.clone(),
                    }),
                    "{}:{} diagnostic contract changed",
                    gap.file,
                    gap.statement_index
                );
            }
        }
    }
}
