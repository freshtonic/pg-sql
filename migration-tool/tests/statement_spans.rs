use pg_sql_migrate::baseline::Baseline;
use pg_sql_migrate::statement_spans::{
    ByteRange, StatementSpanBaseline, StatementSpanCapture, StatementSpanCommands,
    StatementSpanFile, parse_byte_ranges, to_canonical_statement_spans, validate_statement_spans,
};
use sha2::{Digest, Sha256};

fn outcomes() -> Baseline {
    serde_json::from_str(include_str!("../../baselines/postgresql-17.9.json"))
        .expect("parse differential baseline")
}

fn valid_spans(outcomes: &Baseline) -> StatementSpanBaseline {
    let files = outcomes
        .corpus
        .files
        .iter()
        .rev()
        .map(|file| StatementSpanFile {
            file: file.file.clone(),
            source_git_blob: "0123456789abcdef0123456789abcdef01234567".into(),
            source_bytes: file.statements * 2,
            statement_count: file.statements,
            byte_ranges: (0..file.statements)
                .map(|index| format!("{}:{}", index * 2, index * 2 + 1))
                .collect::<Vec<_>>()
                .join(","),
            legacy_item_kinds: "S".repeat(file.statements),
        })
        .collect();
    StatementSpanBaseline {
        schema_version: 1,
        name: outcomes.name.clone(),
        legacy: outcomes.legacy.clone(),
        postgres: outcomes.postgres.clone(),
        corpus_root: outcomes.corpus.root.clone(),
        encoding: "comma-separated-byte-ranges-v1".into(),
        capture: StatementSpanCapture {
            method: "pinned legacy parse_sql_file_with_spans".into(),
            fixture: "migration-tool/fixtures/baseline/capture-statement-spans.rs".into(),
            fixture_sha256: format!(
                "{:x}",
                Sha256::digest(
                    include_str!("../fixtures/baseline/capture-statement-spans.rs").as_bytes()
                )
            ),
        },
        files,
        total_statements: outcomes.corpus.total_statements,
        commands: StatementSpanCommands {
            review: "review".into(),
            update: "update".into(),
        },
    }
}

#[test]
fn byte_range_encoding_is_exact_and_rejects_malformed_rows() {
    assert_eq!(
        parse_byte_ranges("7:11,14:29").unwrap(),
        [
            ByteRange { start: 7, end: 11 },
            ByteRange { start: 14, end: 29 },
        ]
    );
    assert!(parse_byte_ranges("7-11").is_err());
    assert!(parse_byte_ranges("x:11").is_err());
}

#[test]
fn validation_ties_every_file_and_range_to_the_outcome_baseline() {
    let outcomes = outcomes();
    let spans = valid_spans(&outcomes);

    validate_statement_spans(&spans, &outcomes).unwrap();
}

#[test]
fn validation_rejects_overlap_and_provenance_drift() {
    let outcomes = outcomes();
    let mut spans = valid_spans(&outcomes);
    let file = spans
        .files
        .iter_mut()
        .find(|file| file.statement_count >= 2)
        .unwrap();
    let tail = file.byte_ranges.split(',').skip(2).collect::<Vec<_>>();
    file.byte_ranges = if tail.is_empty() {
        "0:2,1:3".into()
    } else {
        format!("0:2,1:3,{}", tail.join(","))
    };
    assert!(
        validate_statement_spans(&spans, &outcomes)
            .unwrap_err()
            .to_string()
            .contains("overlapping")
    );

    let mut spans = valid_spans(&outcomes);
    spans.postgres.gitlink = "wrong".into();
    assert!(
        validate_statement_spans(&spans, &outcomes)
            .unwrap_err()
            .to_string()
            .contains("provenance")
    );
}

#[test]
fn validation_requires_one_frozen_legacy_kind_per_statement() {
    let outcomes = outcomes();
    let mut spans = valid_spans(&outcomes);
    spans.files[0].legacy_item_kinds.pop();

    assert!(
        validate_statement_spans(&spans, &outcomes)
            .unwrap_err()
            .to_string()
            .contains("legacy item kind count")
    );
}

#[test]
fn canonical_serialization_sorts_files_and_is_byte_stable() {
    let outcomes = outcomes();
    let spans = valid_spans(&outcomes);

    let first = to_canonical_statement_spans(&spans, &outcomes).unwrap();
    let second = to_canonical_statement_spans(&spans, &outcomes).unwrap();

    assert_eq!(first, second);
    assert!(first.ends_with('\n'));
    assert!(!first.ends_with("\n\n"));
    assert!(first.find("advisory_lock.sql").unwrap() < first.find("xmlmap.sql").unwrap());
}
