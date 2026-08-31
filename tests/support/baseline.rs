//! Frozen PostgreSQL 17.9 differential expectations.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::OnceLock;

const PINNED_BASELINE: &str = include_str!("../../baselines/postgresql-17.9.json");
const PINNED_STATEMENTS: &str = include_str!("../../baselines/postgresql-17.9-statements.json");
const PINNED_ACCEPTED_LEGACY_GAPS: &str =
    include_str!("../../baselines/postgresql-17.9-accepted-legacy-gaps.json");
const LEGACY_COMMIT: &str = "1e71421d66baac15c8c5264e8f29b5f80122f50e";
const LEGACY_TREE: &str = "f3191ab707c8a957d1bb5fe142e74fc624fe6661";
const LEGACY_PG_SQL_TREE: &str = "50e1376d16796e5f05db88d99dab42252a9f78a4";
const POSTGRES_GITLINK: &str = "6d396980fc5aed4f1a525e0bd75cb16b25ed40ca";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutcomeCounts {
    pub pass: usize,
    pub skip: usize,
    pub fail: usize,
}

impl OutcomeCounts {
    pub fn total(self) -> usize {
        self.pass + self.skip + self.fail
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileExpectation {
    pub statements: usize,
    pub outcomes: OutcomeCounts,
}

#[derive(Debug)]
pub struct Baseline {
    files: BTreeMap<String, FileExpectation>,
    totals: OutcomeCounts,
}

impl Baseline {
    pub fn pinned() -> &'static Self {
        static BASELINE: OnceLock<Baseline> = OnceLock::new();
        BASELINE.get_or_init(Self::load)
    }

    fn load() -> Self {
        let document: serde_json::Value =
            serde_json::from_str(PINNED_BASELINE).expect("parse pinned differential baseline");

        assert_eq!(document["schema_version"].as_u64(), Some(1));
        assert_eq!(document["name"].as_str(), Some("postgresql-17.9"));
        assert_eq!(document["legacy"]["commit"].as_str(), Some(LEGACY_COMMIT));
        assert_eq!(document["legacy"]["tree"].as_str(), Some(LEGACY_TREE));
        assert_eq!(
            document["legacy"]["pg_sql_tree"].as_str(),
            Some(LEGACY_PG_SQL_TREE)
        );
        assert_eq!(document["postgres"]["release"].as_str(), Some("17.9"));
        assert_eq!(
            document["postgres"]["gitlink"].as_str(),
            Some(POSTGRES_GITLINK)
        );

        let corpus = document["corpus"]
            .as_object()
            .expect("baseline corpus object");
        let totals = parse_outcomes(&corpus["totals"]);
        let total_statements = as_usize(&corpus["total_statements"], "total_statements");
        assert_eq!(totals.total(), total_statements);

        let included = corpus["included"]
            .as_array()
            .expect("baseline included array")
            .iter()
            .map(|file| file.as_str().expect("included file name").to_owned())
            .collect::<BTreeSet<_>>();
        let files = corpus["files"]
            .as_array()
            .expect("baseline files array")
            .iter()
            .map(|file| {
                let name = file["file"]
                    .as_str()
                    .expect("baseline file name")
                    .to_owned();
                let expectation = FileExpectation {
                    statements: as_usize(&file["statements"], "file statements"),
                    outcomes: parse_outcomes(&file["outcomes"]),
                };
                assert_eq!(
                    expectation.statements,
                    expectation.outcomes.total(),
                    "outcome count mismatch for {name}"
                );
                (name, expectation)
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(files.keys().cloned().collect::<BTreeSet<_>>(), included);
        assert_eq!(files.len(), 222);
        assert_eq!(
            files.values().map(|file| file.statements).sum::<usize>(),
            total_statements
        );
        assert_eq!(
            OutcomeCounts {
                pass: files.values().map(|file| file.outcomes.pass).sum(),
                skip: files.values().map(|file| file.outcomes.skip).sum(),
                fail: files.values().map(|file| file.outcomes.fail).sum(),
            },
            totals
        );

        Self { files, totals }
    }

    pub fn file(&self, name: &str) -> FileExpectation {
        *self
            .files
            .get(name)
            .unwrap_or_else(|| panic!("{name} is absent from the pinned differential baseline"))
    }

    pub fn file_names(&self) -> BTreeSet<&str> {
        self.files.keys().map(String::as_str).collect()
    }

    pub fn totals(&self) -> OutcomeCounts {
        self.totals
    }
}

#[derive(Debug)]
pub struct FrozenFile {
    pub source_git_blob: String,
    pub source_bytes: usize,
    ranges: Vec<Range<usize>>,
    legacy_item_kinds: Vec<LegacyItemKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyItemKind {
    Statement,
    ParseError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaselineOutcome {
    Pass,
    Skip,
}

impl LegacyItemKind {
    pub fn expected_outcome(self, postgres_accepts: bool) -> BaselineOutcome {
        match (self, postgres_accepts) {
            (Self::ParseError, true) => BaselineOutcome::Skip,
            (Self::Statement | Self::ParseError, false) | (Self::Statement, true) => {
                BaselineOutcome::Pass
            }
        }
    }
}

impl FrozenFile {
    pub fn statements<'source>(&self, source: &'source str) -> Result<Vec<&'source str>, String> {
        if source.len() != self.source_bytes {
            return Err(format!(
                "fixture has {} bytes; frozen source has {}",
                source.len(),
                self.source_bytes
            ));
        }
        self.ranges
            .iter()
            .map(|range| {
                source.get(range.clone()).ok_or_else(|| {
                    format!("invalid UTF-8 byte range {}:{}", range.start, range.end)
                })
            })
            .collect()
    }

    pub fn legacy_item_kinds(&self) -> &[LegacyItemKind] {
        &self.legacy_item_kinds
    }

    pub fn ranges(&self) -> &[Range<usize>] {
        &self.ranges
    }
}

#[derive(Debug)]
pub struct FrozenStatements {
    files: BTreeMap<String, FrozenFile>,
    total_statements: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenDiagnostic {
    pub code: String,
    pub region: Range<usize>,
    pub anchor: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptedLegacyGapOutcome {
    Pass,
    Diagnostic(FrozenDiagnostic),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedLegacyGap {
    pub file: String,
    pub statement_index: usize,
    pub byte_range: Range<usize>,
    pub outcome: AcceptedLegacyGapOutcome,
}

#[derive(Debug)]
pub struct AcceptedLegacyGaps {
    entries: Vec<AcceptedLegacyGap>,
}

impl AcceptedLegacyGaps {
    pub fn pinned() -> &'static Self {
        static GAPS: OnceLock<AcceptedLegacyGaps> = OnceLock::new();
        GAPS.get_or_init(Self::load)
    }

    fn load() -> Self {
        let document: serde_json::Value = serde_json::from_str(PINNED_ACCEPTED_LEGACY_GAPS)
            .expect("parse pinned accepted legacy gaps");
        assert_eq!(document["schema_version"].as_u64(), Some(1));
        assert_eq!(document["name"].as_str(), Some("postgresql-17.9"));
        assert_eq!(document["legacy_commit"].as_str(), Some(LEGACY_COMMIT));
        assert_eq!(
            document["postgres_gitlink"].as_str(),
            Some(POSTGRES_GITLINK)
        );
        assert_eq!(
            document["verification"]["oracle"].as_str(),
            Some("PostgreSQL 17.9 raw_parser")
        );

        let baseline = Baseline::pinned();
        let statements = FrozenStatements::pinned();
        let mut identities = BTreeSet::new();
        let mut entries_per_file = BTreeMap::<String, usize>::new();
        let mut previous_identity = None;
        let entries = document["entries"]
            .as_array()
            .expect("accepted legacy gap entries")
            .iter()
            .map(|entry| {
                let file = entry["file"]
                    .as_str()
                    .expect("accepted legacy gap file")
                    .to_owned();
                let statement_index =
                    as_usize(&entry["statement_index"], "accepted gap statement index");
                let byte_range = parse_range(
                    entry["byte_range"]
                        .as_str()
                        .expect("accepted legacy gap byte range"),
                );
                let identity = (file.clone(), statement_index);
                assert!(
                    identities.insert(identity.clone()),
                    "duplicate accepted legacy gap identity {file}:{statement_index}"
                );
                assert!(
                    previous_identity.as_ref().is_none_or(|previous| previous < &identity),
                    "accepted legacy gaps are not in file/index order at {file}:{statement_index}"
                );
                previous_identity = Some(identity);

                let frozen_file = statements.file(&file);
                assert_eq!(
                    frozen_file.legacy_item_kinds().get(statement_index),
                    Some(&LegacyItemKind::ParseError),
                    "accepted legacy gap {file}:{statement_index} was not a legacy parse error"
                );
                assert_eq!(
                    frozen_file.ranges().get(statement_index),
                    Some(&byte_range),
                    "accepted legacy gap {file}:{statement_index} byte range changed"
                );
                *entries_per_file.entry(file.clone()).or_default() += 1;

                let outcome = match entry["outcome"]
                    .as_str()
                    .expect("accepted legacy gap outcome")
                {
                    "pass" => {
                        assert!(
                            entry.get("diagnostic").is_none(),
                            "passing accepted legacy gap {file}:{statement_index} has a diagnostic"
                        );
                        AcceptedLegacyGapOutcome::Pass
                    }
                    "diagnostic" => {
                        let diagnostic = entry["diagnostic"]
                            .as_object()
                            .expect("diagnostic accepted legacy gap contract");
                        let code = diagnostic["code"]
                            .as_str()
                            .expect("accepted legacy gap diagnostic code")
                            .to_owned();
                        assert!(
                            code.starts_with("RCA")
                                && code.len() == 7
                                && code[3..].bytes().all(|byte| byte.is_ascii_digit()),
                            "invalid diagnostic code for {file}:{statement_index}"
                        );
                        let region = parse_range(
                            diagnostic["region"]
                                .as_str()
                                .expect("accepted legacy gap diagnostic region"),
                        );
                        let anchor = parse_range(
                            diagnostic["anchor"]
                                .as_str()
                                .expect("accepted legacy gap diagnostic anchor"),
                        );
                        let statement_bytes = byte_range.end - byte_range.start;
                        for (name, range) in [("region", &region), ("anchor", &anchor)] {
                            assert!(
                                range.start <= range.end && range.end <= statement_bytes,
                                "accepted legacy gap {file}:{statement_index} {name} is outside the statement"
                            );
                        }
                        AcceptedLegacyGapOutcome::Diagnostic(FrozenDiagnostic {
                            code,
                            region,
                            anchor,
                        })
                    }
                    outcome => panic!(
                        "unsupported accepted legacy gap outcome {outcome:?} for {file}:{statement_index}"
                    ),
                };

                AcceptedLegacyGap {
                    file,
                    statement_index,
                    byte_range,
                    outcome,
                }
            })
            .collect::<Vec<_>>();

        let expected_per_file = baseline
            .files
            .iter()
            .filter(|(_, file)| file.outcomes.skip != 0)
            .map(|(name, file)| (name.clone(), file.outcomes.skip))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            entries_per_file, expected_per_file,
            "accepted legacy gap identities do not account for every frozen skip"
        );
        Self { entries }
    }

    pub fn entries(&self) -> &[AcceptedLegacyGap] {
        &self.entries
    }
}

impl FrozenStatements {
    pub fn pinned() -> &'static Self {
        static STATEMENTS: OnceLock<FrozenStatements> = OnceLock::new();
        STATEMENTS.get_or_init(Self::load)
    }

    fn load() -> Self {
        let document: serde_json::Value =
            serde_json::from_str(PINNED_STATEMENTS).expect("parse pinned statement spans");
        assert_eq!(document["schema_version"].as_u64(), Some(1));
        assert_eq!(document["name"].as_str(), Some("postgresql-17.9"));
        assert_eq!(document["legacy"]["commit"].as_str(), Some(LEGACY_COMMIT));
        assert_eq!(document["legacy"]["tree"].as_str(), Some(LEGACY_TREE));
        assert_eq!(
            document["legacy"]["pg_sql_tree"].as_str(),
            Some(LEGACY_PG_SQL_TREE)
        );
        assert_eq!(document["postgres"]["release"].as_str(), Some("17.9"));
        assert_eq!(
            document["postgres"]["gitlink"].as_str(),
            Some(POSTGRES_GITLINK)
        );
        assert_eq!(
            document["corpus_root"].as_str(),
            Some("vendor/postgres/src/test/regress/sql")
        );
        assert_eq!(
            document["encoding"].as_str(),
            Some("comma-separated-byte-ranges-v1")
        );
        assert_eq!(
            document["capture"]["method"].as_str(),
            Some("pinned legacy parse_sql_file_with_spans")
        );
        assert_eq!(
            document["capture"]["fixture"].as_str(),
            Some("migration-tool/fixtures/baseline/capture-statement-spans.rs")
        );
        let capture_sha = document["capture"]["fixture_sha256"]
            .as_str()
            .expect("capture fixture SHA-256");
        assert!(
            capture_sha.len() == 64 && capture_sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid capture fixture SHA-256"
        );

        let baseline = Baseline::pinned();
        let files = document["files"]
            .as_array()
            .expect("statement-span files array")
            .iter()
            .map(|file| {
                let name = file["file"]
                    .as_str()
                    .expect("statement-span file name")
                    .to_owned();
                let source_git_blob = file["source_git_blob"]
                    .as_str()
                    .expect("statement-span source Git blob")
                    .to_owned();
                assert!(
                    source_git_blob.len() == 40
                        && source_git_blob.bytes().all(|byte| byte.is_ascii_hexdigit()),
                    "invalid source Git blob for {name}"
                );
                let source_bytes = as_usize(&file["source_bytes"], "source bytes");
                let statement_count = as_usize(&file["statement_count"], "statement-span count");
                let ranges = parse_ranges(
                    file["byte_ranges"]
                        .as_str()
                        .expect("encoded statement byte ranges"),
                );
                let legacy_item_kinds = parse_legacy_item_kinds(
                    file["legacy_item_kinds"]
                        .as_str()
                        .expect("encoded legacy item kinds"),
                );
                assert_eq!(ranges.len(), statement_count, "span count for {name}");
                assert_eq!(
                    legacy_item_kinds.len(),
                    statement_count,
                    "legacy item kind count for {name}"
                );
                assert_eq!(
                    statement_count,
                    baseline.file(&name).statements,
                    "baseline count for {name}"
                );
                let mut previous_end = 0;
                for range in &ranges {
                    assert!(
                        range.start < range.end
                            && range.start >= previous_end
                            && range.end <= source_bytes,
                        "invalid statement range {}:{} for {name}",
                        range.start,
                        range.end
                    );
                    previous_end = range.end;
                }
                (
                    name,
                    FrozenFile {
                        source_git_blob,
                        source_bytes,
                        ranges,
                        legacy_item_kinds,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            files.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            baseline.file_names()
        );
        let total_statements = as_usize(&document["total_statements"], "total statements");
        assert_eq!(
            files.values().map(|file| file.ranges.len()).sum::<usize>(),
            total_statements
        );
        assert_eq!(total_statements, baseline.totals().total());
        Self {
            files,
            total_statements,
        }
    }

    pub fn file(&self, name: &str) -> &FrozenFile {
        self.files
            .get(name)
            .unwrap_or_else(|| panic!("{name} is absent from the pinned statement spans"))
    }

    pub fn file_names(&self) -> BTreeSet<&str> {
        self.files.keys().map(String::as_str).collect()
    }

    pub fn total_statements(&self) -> usize {
        self.total_statements
    }
}

fn as_usize(value: &serde_json::Value, field: &str) -> usize {
    value
        .as_u64()
        .unwrap_or_else(|| panic!("baseline {field} must be an unsigned integer"))
        .try_into()
        .expect("baseline count fits usize")
}

fn parse_outcomes(value: &serde_json::Value) -> OutcomeCounts {
    OutcomeCounts {
        pass: as_usize(&value["pass"], "pass outcome"),
        skip: as_usize(&value["skip"], "skip outcome"),
        fail: as_usize(&value["fail"], "fail outcome"),
    }
}

fn parse_ranges(encoded: &str) -> Vec<Range<usize>> {
    if encoded.is_empty() {
        return Vec::new();
    }
    encoded
        .split(',')
        .map(|range| {
            let (start, end) = range.split_once(':').expect("encoded byte range");
            start.parse().expect("byte-range start")..end.parse().expect("byte-range end")
        })
        .collect()
}

fn parse_range(encoded: &str) -> Range<usize> {
    let (start, end) = encoded.split_once(':').expect("encoded byte range");
    start.parse().expect("byte-range start")..end.parse().expect("byte-range end")
}

fn parse_legacy_item_kinds(encoded: &str) -> Vec<LegacyItemKind> {
    encoded
        .bytes()
        .map(|kind| match kind {
            b'S' => LegacyItemKind::Statement,
            b'E' => LegacyItemKind::ParseError,
            _ => panic!("invalid legacy item kind {kind:?}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_baseline_is_complete_and_self_consistent() {
        let baseline = Baseline::pinned();

        assert_eq!(baseline.file_names().len(), 222);
        assert_eq!(
            baseline.totals(),
            OutcomeCounts {
                pass: 43_456,
                skip: 18,
                fail: 0,
            }
        );
        assert_eq!(
            baseline.file("with.sql"),
            FileExpectation {
                statements: 306,
                outcomes: OutcomeCounts {
                    pass: 302,
                    skip: 4,
                    fail: 0,
                },
            }
        );
        assert_eq!(
            baseline
                .files
                .iter()
                .filter(|(_, file)| file.outcomes.skip != 0)
                .map(|(name, file)| (name.as_str(), file.outcomes.skip))
                .collect::<Vec<_>>(),
            [
                ("amutils.sql", 1),
                ("create_index.sql", 1),
                ("create_view.sql", 2),
                ("join.sql", 6),
                ("returning.sql", 2),
                ("rules.sql", 1),
                ("select.sql", 1),
                ("with.sql", 4),
            ]
        );
    }

    #[test]
    fn pinned_statement_spans_match_every_frozen_file_and_count() {
        let statements = FrozenStatements::pinned();

        assert_eq!(statements.file_names(), Baseline::pinned().file_names());
        assert_eq!(statements.total_statements(), 43_474);
        assert_eq!(statements.file("with.sql").ranges.len(), 306);
        let file = statements.file("advisory_lock.sql");
        assert_eq!(file.source_git_blob.len(), 40);
        assert_eq!(file.legacy_item_kinds().len(), 23);
        assert!(
            file.legacy_item_kinds()
                .iter()
                .all(|kind| *kind == LegacyItemKind::Statement)
        );
        let legacy_gap = statements.file("amutils.sql").legacy_item_kinds()[6];
        assert_eq!(legacy_gap.expected_outcome(true), BaselineOutcome::Skip);
        assert_eq!(legacy_gap.expected_outcome(false), BaselineOutcome::Pass);
        let synthetic_source = " ".repeat(file.source_bytes);
        assert_eq!(file.statements(&synthetic_source).unwrap().len(), 23);
    }
}
