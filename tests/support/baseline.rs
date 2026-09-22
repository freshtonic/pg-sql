//! Frozen differential expectations over the PostgreSQL 17.9 regression corpus,
//! and the version baseline of each target version over the same corpus.
//!
//! The corpus is frozen: every file is read by its Git blob ID from the
//! `vendor/postgres` object database, not from the checked-out tree. The
//! submodule pin (the oracle release) can therefore move to a later minor
//! release while the statement spans and legacy item kinds stay valid.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::OnceLock;

const PINNED_BASELINE: &str = include_str!("../../baselines/postgresql-17.9.json");
const PINNED_STATEMENTS: &str = include_str!("../../baselines/postgresql-17.9-statements.json");
const PINNED_ACCEPTED_LEGACY_GAPS: &str =
    include_str!("../../baselines/postgresql-17.9-accepted-legacy-gaps.json");
/// One version baseline for each target version (`docs/differential-baseline.md`).
const VERSION_BASELINES: &[(&str, &str)] = &[
    (
        "pg14",
        include_str!("../../baselines/target-versions/pg14.json"),
    ),
    (
        "pg15",
        include_str!("../../baselines/target-versions/pg15.json"),
    ),
    (
        "pg16",
        include_str!("../../baselines/target-versions/pg16.json"),
    ),
    (
        "pg17",
        include_str!("../../baselines/target-versions/pg17.json"),
    ),
    (
        "pg18",
        include_str!("../../baselines/target-versions/pg18.json"),
    ),
    (
        "pg19-beta",
        include_str!("../../baselines/target-versions/pg19-beta.json"),
    ),
];
/// The pin table of the oracle: `feature<TAB>ref<TAB>commit`.
const ORACLE_PINS: &str = include_str!("../../pg-oracle/pins.tsv");
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
    /// The frozen corpus source, read by its Git blob ID from the
    /// `vendor/postgres` object database. The pinned oracle release can be
    /// later than the corpus release, so the checked-out file can differ.
    pub fn source(&self) -> String {
        let repository = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/postgres");
        let output = std::process::Command::new("git")
            .args(["-C", repository, "cat-file", "blob", &self.source_git_blob])
            .output()
            .unwrap_or_else(|error| panic!("cannot run git cat-file: {error}"));
        assert!(
            output.status.success(),
            "cannot read frozen corpus blob {} from {repository} \
             (is the submodule checked out with full history?): {}",
            self.source_git_blob,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap_or_else(|_| panic!("frozen corpus blob {} is not UTF-8", self.source_git_blob))
    }

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
        // The entries name statements of the frozen 17.9 corpus; the pinned
        // oracle that verifies their outcomes is 17.11.
        assert_eq!(
            document["verification"]["oracle"].as_str(),
            Some("PostgreSQL 17.11 raw_parser")
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

/// The pg-sql outcome of a statement in an expected version gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GapOutcome {
    /// pg-sql cannot parse a statement that the oracle accepts.
    Skip,
    /// The differential check fails: pg-sql over-accepts, or its output
    /// changes the parse tree or is rejected.
    Fail,
}

impl GapOutcome {
    pub fn name(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Fail => "fail",
        }
    }
}

/// One statement where a build of the target version disagrees with that
/// version's oracle, because the grammar of the version is not complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionGap {
    pub file: String,
    pub statement_index: usize,
    pub byte_range: Range<usize>,
    pub pg_sql: GapOutcome,
    pub oracle_accepts: bool,
}

/// The differential baseline of one target version: the oracle outcome of
/// every frozen corpus statement, and the expected version gaps.
#[derive(Debug)]
pub struct VersionBaseline {
    pub feature: String,
    pub oracle_ref: String,
    pub oracle_commit: String,
    oracle_accepts: BTreeMap<String, Vec<bool>>,
    gaps: Vec<VersionGap>,
}

/// The pin of `feature` in `pg-oracle/pins.tsv`: `(ref, commit)`.
pub fn oracle_pin(feature: &str) -> (&'static str, &'static str) {
    ORACLE_PINS
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3, "pins.tsv row {line:?}");
            (fields[0], fields[1], fields[2])
        })
        .find(|(name, _, _)| *name == feature)
        .map(|(_, reference, commit)| (reference, commit))
        .unwrap_or_else(|| panic!("pins.tsv has no pin for {feature}"))
}

/// The version features that have a version baseline.
pub fn version_features() -> impl Iterator<Item = &'static str> {
    VERSION_BASELINES.iter().map(|(feature, _)| *feature)
}

impl VersionBaseline {
    /// The version baseline of the target version of this build.
    pub fn active() -> &'static Self {
        Self::of(pg_sql::TARGET_VERSION.feature())
    }

    /// The version baseline of the version feature `feature`.
    pub fn of(feature: &str) -> &'static Self {
        static BASELINES: OnceLock<BTreeMap<&'static str, VersionBaseline>> = OnceLock::new();
        BASELINES
            .get_or_init(|| {
                VERSION_BASELINES
                    .iter()
                    .map(|(feature, source)| (*feature, Self::load(feature, source)))
                    .collect()
            })
            .get(feature)
            .unwrap_or_else(|| panic!("no version baseline for {feature}"))
    }

    fn load(feature: &str, source: &str) -> Self {
        let document: serde_json::Value = serde_json::from_str(source)
            .unwrap_or_else(|error| panic!("parse the {feature} version baseline: {error}"));
        assert_eq!(document["schema_version"].as_u64(), Some(1), "{feature}");
        assert_eq!(document["target_version"].as_str(), Some(feature));
        assert_eq!(
            document["corpus"]["name"].as_str(),
            Some("postgresql-17.9"),
            "{feature}: the version baselines share the frozen corpus"
        );
        assert_eq!(
            document["corpus"]["gitlink"].as_str(),
            Some(POSTGRES_GITLINK),
            "{feature}"
        );
        let (pin_ref, pin_commit) = oracle_pin(feature);
        let oracle_ref = document["oracle"]["ref"]
            .as_str()
            .expect("oracle ref")
            .to_owned();
        let oracle_commit = document["oracle"]["commit"]
            .as_str()
            .expect("oracle commit")
            .to_owned();
        assert_eq!(
            (oracle_ref.as_str(), oracle_commit.as_str()),
            (pin_ref, pin_commit),
            "{feature}: the version baseline is not from the pinned oracle; regenerate it"
        );

        let frozen = FrozenStatements::pinned();
        let oracle_accepts = document["files"]
            .as_array()
            .expect("version baseline files")
            .iter()
            .map(|file| {
                let name = file["file"].as_str().expect("file name").to_owned();
                let accepts = file["oracle_accepts"]
                    .as_str()
                    .expect("oracle outcomes")
                    .bytes()
                    .map(|outcome| match outcome {
                        b'A' => true,
                        b'R' => false,
                        _ => panic!("{feature}: invalid oracle outcome {outcome:?} in {name}"),
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    accepts.len(),
                    frozen.file(&name).ranges().len(),
                    "{feature}: oracle outcome count for {name}"
                );
                (name, accepts)
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            oracle_accepts
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            frozen.file_names(),
            "{feature}: the version baseline covers every frozen corpus file"
        );

        let mut previous = None;
        let gaps = document["expected_version_gaps"]
            .as_array()
            .expect("expected version gaps")
            .iter()
            .map(|gap| {
                let file = gap["file"].as_str().expect("gap file").to_owned();
                let statement_index = as_usize(&gap["statement_index"], "gap statement index");
                let byte_range = parse_range(gap["byte_range"].as_str().expect("gap byte range"));
                let identity = (file.clone(), statement_index);
                assert!(
                    previous
                        .as_ref()
                        .is_none_or(|previous| previous < &identity),
                    "{feature}: version gaps are not unique and in file/index order at \
                     {file}:{statement_index}"
                );
                previous = Some(identity);
                assert_eq!(
                    frozen.file(&file).ranges().get(statement_index),
                    Some(&byte_range),
                    "{feature}: version gap {file}:{statement_index} byte range"
                );
                let pg_sql = match gap["pg_sql"].as_str().expect("gap pg_sql outcome") {
                    "skip" => GapOutcome::Skip,
                    "fail" => GapOutcome::Fail,
                    other => panic!("{feature}: invalid gap pg_sql outcome {other:?}"),
                };
                let oracle_accepts = match gap["oracle"].as_str().expect("gap oracle outcome") {
                    "accept" => true,
                    "reject" => false,
                    other => panic!("{feature}: invalid gap oracle outcome {other:?}"),
                };
                assert_eq!(
                    oracle_accepts,
                    oracle_accepts_at(&document, &file, statement_index),
                    "{feature}: version gap {file}:{statement_index} oracle outcome"
                );
                assert!(
                    pg_sql != GapOutcome::Skip || oracle_accepts,
                    "{feature}: a skip gap is a statement that the oracle accepts \
                     ({file}:{statement_index})"
                );
                VersionGap {
                    file,
                    statement_index,
                    byte_range,
                    pg_sql,
                    oracle_accepts,
                }
            })
            .collect::<Vec<_>>();

        let baseline = Self {
            feature: feature.to_owned(),
            oracle_ref,
            oracle_commit,
            oracle_accepts,
            gaps,
        };
        let totals = &document["totals"];
        let expected = baseline.expected_totals();
        assert_eq!(
            (
                as_usize(&totals["statements"], "total statements"),
                as_usize(&totals["oracle_accepts"], "total oracle accepts"),
                as_usize(&totals["pass"], "total pass"),
                as_usize(&totals["skip"], "total skip"),
                as_usize(&totals["expected_version_gaps"], "total gaps"),
            ),
            (
                frozen.total_statements(),
                baseline
                    .oracle_accepts
                    .values()
                    .flatten()
                    .filter(|a| **a)
                    .count(),
                expected.pass,
                expected.skip,
                baseline.gaps.len(),
            ),
            "{feature}: version baseline totals"
        );
        baseline
    }

    /// The oracle outcome of every statement of `file`.
    pub fn oracle_accepts(&self, file: &str) -> &[bool] {
        self.oracle_accepts
            .get(file)
            .unwrap_or_else(|| panic!("{file} is absent from the {} baseline", self.feature))
    }

    pub fn gaps(&self) -> &[VersionGap] {
        &self.gaps
    }

    /// The expected version gaps of `file`, by statement index.
    pub fn gaps_in(&self, file: &str) -> BTreeMap<usize, &VersionGap> {
        self.gaps
            .iter()
            .filter(|gap| gap.file == file)
            .map(|gap| (gap.statement_index, gap))
            .collect()
    }

    /// The frozen-identity outcome counts of `file` with this oracle: a
    /// legacy parse error that the oracle accepts is a skip, and every
    /// other statement is a pass.
    pub fn expected_outcomes(&self, file: &str) -> OutcomeCounts {
        let kinds = FrozenStatements::pinned().file(file).legacy_item_kinds();
        let mut counts = OutcomeCounts {
            pass: 0,
            skip: 0,
            fail: 0,
        };
        for (kind, accepts) in kinds.iter().zip(self.oracle_accepts(file)) {
            match kind.expected_outcome(*accepts) {
                BaselineOutcome::Pass => counts.pass += 1,
                BaselineOutcome::Skip => counts.skip += 1,
            }
        }
        counts
    }

    pub fn expected_totals(&self) -> OutcomeCounts {
        self.oracle_accepts.keys().fold(
            OutcomeCounts {
                pass: 0,
                skip: 0,
                fail: 0,
            },
            |totals, file| {
                let counts = self.expected_outcomes(file);
                OutcomeCounts {
                    pass: totals.pass + counts.pass,
                    skip: totals.skip + counts.skip,
                    fail: totals.fail + counts.fail,
                }
            },
        )
    }
}

fn oracle_accepts_at(document: &serde_json::Value, file: &str, index: usize) -> bool {
    let outcomes = document["files"]
        .as_array()
        .expect("version baseline files")
        .iter()
        .find(|entry| entry["file"].as_str() == Some(file))
        .unwrap_or_else(|| panic!("version gap file {file} has no oracle outcomes"))["oracle_accepts"]
        .as_str()
        .expect("oracle outcomes");
    match outcomes.as_bytes().get(index) {
        Some(b'A') => true,
        Some(b'R') => false,
        _ => panic!("version gap {file}:{index} has no oracle outcome"),
    }
}

/// Render a version baseline file. The regeneration test writes it; see
/// `docs/differential-baseline.md`.
pub fn render_version_baseline(
    feature: &str,
    oracle_accepts: &BTreeMap<String, Vec<bool>>,
    gaps: &[VersionGap],
) -> String {
    let (reference, commit) = oracle_pin(feature);
    let frozen = FrozenStatements::pinned();
    let kinds_and_accepts = oracle_accepts.iter().flat_map(|(file, accepts)| {
        frozen
            .file(file)
            .legacy_item_kinds()
            .iter()
            .zip(accepts)
            .map(|(kind, accepts)| kind.expected_outcome(*accepts))
    });
    let (pass, skip) = kinds_and_accepts.fold((0, 0), |(pass, skip), outcome| match outcome {
        BaselineOutcome::Pass => (pass + 1, skip),
        BaselineOutcome::Skip => (pass, skip + 1),
    });
    // One gap and one file on each line, so a change is a small diff.
    let line = |value: serde_json::Value| serde_json::to_string(&value).expect("render JSON");
    let gaps = gaps
        .iter()
        .map(|gap| {
            line(serde_json::json!({
                "file": gap.file,
                "statement_index": gap.statement_index,
                "byte_range": format!("{}:{}", gap.byte_range.start, gap.byte_range.end),
                "pg_sql": gap.pg_sql.name(),
                "oracle": if gap.oracle_accepts { "accept" } else { "reject" },
            }))
        })
        .collect::<Vec<_>>();
    let files = oracle_accepts
        .iter()
        .map(|(file, accepts)| {
            line(serde_json::json!({
                "file": file,
                "oracle_accepts": accepts
                    .iter()
                    .map(|accepts| if *accepts { 'A' } else { 'R' })
                    .collect::<String>(),
            }))
        })
        .collect::<Vec<_>>();
    let list = |items: Vec<String>| {
        if items.is_empty() {
            "[]".to_owned()
        } else {
            format!("[\n    {}\n  ]", items.join(",\n    "))
        }
    };
    format!(
        "{{\n  \"schema_version\": 1,\n  \"target_version\": {},\n  \"oracle\": {},\n  \
         \"corpus\": {},\n  \"totals\": {},\n  \"expected_version_gaps\": {},\n  \
         \"files\": {}\n}}\n",
        line(serde_json::json!(feature)),
        line(serde_json::json!({ "ref": reference, "commit": commit })),
        line(serde_json::json!({ "name": "postgresql-17.9", "gitlink": POSTGRES_GITLINK })),
        line(serde_json::json!({
            "statements": frozen.total_statements(),
            "oracle_accepts": oracle_accepts.values().flatten().filter(|a| **a).count(),
            "pass": pass,
            "skip": skip,
            "expected_version_gaps": gaps.len(),
        })),
        list(gaps),
        list(files),
    )
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
