//! Exact current outcomes for the PostgreSQL-accepted legacy grammar gaps.

mod support;

use std::collections::BTreeSet;

use pg_oracle::parse_ok;
use support::baseline::{
    AcceptedLegacyGapOutcome, AcceptedLegacyGaps, FrozenStatements, LegacyItemKind,
};
use support::diff_check::{Outcome, StrictDiagnostic, check_statement, pgsql_format};

#[test]
fn frozen_accepted_legacy_gap_contracts_are_exact() {
    let frozen = FrozenStatements::pinned();
    let gaps = AcceptedLegacyGaps::pinned();
    let corpus = format!(
        "{}/vendor/postgres/src/test/regress/sql",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut derived_identities = BTreeSet::new();

    for file_name in frozen.file_names() {
        let file = frozen.file(file_name);
        let path = format!("{corpus}/{file_name}");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
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
    assert_eq!(contracted_identities, derived_identities);

    for gap in gaps.entries() {
        let path = format!("{corpus}/{}", gap.file);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {path}: {error}"));
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
