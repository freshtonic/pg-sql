//! Reproducible statement boundaries captured from the pinned legacy parser.

use crate::baseline::{Baseline, CaptureOptions, Identity, PostgresIdentity, validate_baseline};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

const CAPTURE_SOURCE: &str = include_str!("../fixtures/baseline/capture-statement-spans.rs");
const ENCODING: &str = "comma-separated-byte-ranges-v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatementSpanBaseline {
    pub schema_version: u32,
    pub name: String,
    pub legacy: Identity,
    pub postgres: PostgresIdentity,
    pub corpus_root: String,
    pub encoding: String,
    pub capture: StatementSpanCapture,
    pub files: Vec<StatementSpanFile>,
    pub total_statements: usize,
    pub commands: StatementSpanCommands,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatementSpanCapture {
    pub method: String,
    pub fixture: String,
    pub fixture_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatementSpanFile {
    pub file: String,
    pub source_git_blob: String,
    pub source_bytes: usize,
    pub statement_count: usize,
    pub byte_ranges: String,
    pub legacy_item_kinds: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StatementSpanCommands {
    pub review: String,
    pub update: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub struct StatementSpanError(String);

impl StatementSpanError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for StatementSpanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StatementSpanError {}

impl From<std::io::Error> for StatementSpanError {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

pub fn parse_byte_ranges(encoded: &str) -> Result<Vec<ByteRange>, StatementSpanError> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    encoded
        .split(',')
        .map(|range| {
            let (start, end) = range.split_once(':').ok_or_else(|| {
                StatementSpanError::new(format!("malformed byte range {range:?}"))
            })?;
            let start = start
                .parse()
                .map_err(|_| StatementSpanError::new(format!("malformed byte offset {start:?}")))?;
            let end = end
                .parse()
                .map_err(|_| StatementSpanError::new(format!("malformed byte offset {end:?}")))?;
            Ok(ByteRange { start, end })
        })
        .collect()
}

pub fn validate_statement_spans(
    spans: &StatementSpanBaseline,
    outcomes: &Baseline,
) -> Result<(), StatementSpanError> {
    validate_baseline(outcomes).map_err(|error| StatementSpanError::new(error.to_string()))?;
    if spans.schema_version != 1 {
        return Err(StatementSpanError::new(
            "unsupported statement-span schema version",
        ));
    }
    if spans.name != outcomes.name
        || spans.legacy != outcomes.legacy
        || spans.postgres != outcomes.postgres
        || spans.corpus_root != outcomes.corpus.root
    {
        return Err(StatementSpanError::new(
            "statement spans do not share the differential baseline provenance",
        ));
    }
    if spans.encoding != ENCODING {
        return Err(StatementSpanError::new(
            "unsupported statement-span encoding",
        ));
    }
    if spans.capture
        != (StatementSpanCapture {
            method: "pinned legacy parse_sql_file_with_spans".into(),
            fixture: "migration-tool/fixtures/baseline/capture-statement-spans.rs".into(),
            fixture_sha256: format!("{:x}", Sha256::digest(CAPTURE_SOURCE.as_bytes())),
        })
    {
        return Err(StatementSpanError::new(
            "statement-span capture fixture provenance differs",
        ));
    }

    let expected = outcomes
        .corpus
        .files
        .iter()
        .map(|file| (file.file.as_str(), file.statements))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut total = 0;
    for file in &spans.files {
        if !seen.insert(file.file.as_str()) {
            return Err(StatementSpanError::new(format!(
                "duplicate statement spans for {}",
                file.file
            )));
        }
        let Some(expected_count) = expected.get(file.file.as_str()) else {
            return Err(StatementSpanError::new(format!(
                "statement spans contain unexpected file {}",
                file.file
            )));
        };
        let ranges = parse_byte_ranges(&file.byte_ranges)?;
        if ranges.len() != file.statement_count || file.statement_count != *expected_count {
            return Err(StatementSpanError::new(format!(
                "{} statement-span count does not match the differential baseline",
                file.file
            )));
        }
        if file.legacy_item_kinds.len() != file.statement_count
            || !file
                .legacy_item_kinds
                .bytes()
                .all(|kind| matches!(kind, b'S' | b'E'))
        {
            return Err(StatementSpanError::new(format!(
                "{} legacy item kind count or encoding is invalid",
                file.file
            )));
        }
        let mut previous_end = 0;
        for range in ranges {
            if range.start >= range.end
                || range.start < previous_end
                || range.end > file.source_bytes
            {
                return Err(StatementSpanError::new(format!(
                    "{} contains an invalid or overlapping byte range {}:{}",
                    file.file, range.start, range.end
                )));
            }
            previous_end = range.end;
        }
        total += file.statement_count;
    }
    if seen != expected.keys().copied().collect() {
        return Err(StatementSpanError::new(
            "statement spans do not cover the frozen corpus membership",
        ));
    }
    if total != spans.total_statements || total != outcomes.corpus.total_statements {
        return Err(StatementSpanError::new(
            "statement-span total does not match the differential baseline",
        ));
    }
    Ok(())
}

pub fn validate_statement_sources(
    spans: &StatementSpanBaseline,
    postgres_repository: &Path,
) -> Result<(), StatementSpanError> {
    let head = git_stdout(postgres_repository, &["rev-parse", "HEAD"])?;
    if head != spans.postgres.gitlink {
        return Err(StatementSpanError::new(format!(
            "PostgreSQL checkout is {head}, expected {}",
            spans.postgres.gitlink
        )));
    }
    for file in &spans.files {
        let relative = Path::new(&spans.corpus_root)
            .strip_prefix("vendor/postgres")
            .expect("canonical corpus root is below vendor/postgres")
            .join(&file.file);
        let source = fs::read(postgres_repository.join(&relative))?;
        if source.len() != file.source_bytes {
            return Err(StatementSpanError::new(format!(
                "{} byte length differs from the frozen source",
                file.file
            )));
        }
        let blob = git_stdout(
            postgres_repository,
            &[
                "hash-object",
                "--no-filters",
                relative.to_string_lossy().as_ref(),
            ],
        )?;
        if blob != file.source_git_blob {
            return Err(StatementSpanError::new(format!(
                "{} content differs from the frozen source",
                file.file
            )));
        }
        for range in parse_byte_ranges(&file.byte_ranges)? {
            std::str::from_utf8(&source[range.start..range.end]).map_err(|_| {
                StatementSpanError::new(format!(
                    "{} range {}:{} is not valid UTF-8",
                    file.file, range.start, range.end
                ))
            })?;
        }
    }
    Ok(())
}

pub fn capture_statement_spans(
    options: &CaptureOptions,
    outcomes: &Baseline,
) -> Result<StatementSpanBaseline, StatementSpanError> {
    validate_baseline(outcomes).map_err(|error| StatementSpanError::new(error.to_string()))?;
    let temporary = tempfile::tempdir()?;
    let legacy = temporary.path().join("legacy");
    let postgres = legacy.join("pg-sql/vendor/postgres");
    clone_at(
        &options.legacy_repository,
        &legacy,
        &options.legacy_commit,
        "legacy repository",
    )?;
    clone_at(
        &options.postgres_repository,
        &postgres,
        &outcomes.postgres.gitlink,
        "PostgreSQL repository",
    )?;

    let commit = git_stdout(&legacy, &["rev-parse", "HEAD"])?;
    let tree = git_stdout(&legacy, &["show", "-s", "--format=%T", "HEAD"])?;
    let pg_sql_tree = git_stdout(&legacy, &["rev-parse", "HEAD:pg-sql"])?;
    if commit != outcomes.legacy.commit
        || tree != outcomes.legacy.tree
        || pg_sql_tree != outcomes.legacy.pg_sql_tree
    {
        return Err(StatementSpanError::new(
            "legacy capture checkout does not match differential baseline provenance",
        ));
    }

    let capture = temporary.path().join("capture");
    fs::create_dir_all(capture.join("src"))?;
    fs::write(capture.join("src/main.rs"), CAPTURE_SOURCE)?;
    fs::write(
        capture.join("Cargo.toml"),
        capture_manifest(&legacy.join("pg-sql"), &legacy),
    )?;
    fs::copy(legacy.join("Cargo.lock"), capture.join("Cargo.lock"))?;
    let membership = temporary.path().join("membership.txt");
    fs::write(
        &membership,
        format!("{}\n", outcomes.corpus.included.join("\n")),
    )?;

    let mut offline_lock = Command::new("cargo");
    offline_lock
        .env("CARGO_TARGET_DIR", temporary.path().join("target"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(capture.join("Cargo.toml"));
    if let Err(offline_error) = run_checked(
        &mut offline_lock,
        "finalize the isolated capture lockfile offline",
    ) {
        let mut online_lock = Command::new("cargo");
        online_lock
            .env("CARGO_TARGET_DIR", temporary.path().join("target"))
            .args(["check", "--quiet", "--manifest-path"])
            .arg(capture.join("Cargo.toml"));
        run_checked(
            &mut online_lock,
            "finalize the isolated capture lockfile online",
        )
        .map_err(|online_error| {
            StatementSpanError::new(format!(
                "{offline_error}; online fallback also failed: {online_error}"
            ))
        })?;
    }

    let output = Command::new("cargo")
        .env("CARGO_TARGET_DIR", temporary.path().join("target"))
        .args(["run", "--quiet", "--locked", "--offline", "--manifest-path"])
        .arg(capture.join("Cargo.toml"))
        .arg("--")
        .arg(postgres.join("src/test/regress/sql"))
        .arg(&membership)
        .output()
        .map_err(|error| StatementSpanError::new(format!("cannot run span capture: {error}")))?;
    if !output.status.success() {
        return Err(StatementSpanError::new(format!(
            "legacy statement-span capture failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let captured = parse_capture(&String::from_utf8(output.stdout).map_err(|error| {
        StatementSpanError::new(format!("capture output is not UTF-8: {error}"))
    })?)?;

    let expected_files = outcomes
        .corpus
        .files
        .iter()
        .map(|file| file.file.as_str())
        .collect::<BTreeSet<_>>();
    if captured.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_files {
        return Err(StatementSpanError::new(
            "capture output does not match the frozen corpus membership",
        ));
    }

    let mut files = Vec::new();
    for expected in &outcomes.corpus.files {
        let statements = captured
            .get(&expected.file)
            .ok_or_else(|| StatementSpanError::new(format!("capture omitted {}", expected.file)))?;
        let relative = Path::new("src/test/regress/sql").join(&expected.file);
        let source = fs::read(postgres.join(&relative))?;
        let source_git_blob = git_stdout(
            &postgres,
            &[
                "hash-object",
                "--no-filters",
                relative.to_string_lossy().as_ref(),
            ],
        )?;
        files.push(StatementSpanFile {
            file: expected.file.clone(),
            source_git_blob,
            source_bytes: source.len(),
            statement_count: statements.len(),
            byte_ranges: encode_byte_ranges(
                &statements
                    .iter()
                    .map(|statement| statement.range)
                    .collect::<Vec<_>>(),
            ),
            legacy_item_kinds: statements
                .iter()
                .map(|statement| statement.legacy_item_kind)
                .collect(),
        });
    }
    files.sort_by(|left, right| left.file.cmp(&right.file));
    let spans = StatementSpanBaseline {
        schema_version: 1,
        name: outcomes.name.clone(),
        legacy: outcomes.legacy.clone(),
        postgres: outcomes.postgres.clone(),
        corpus_root: outcomes.corpus.root.clone(),
        encoding: ENCODING.into(),
        capture: StatementSpanCapture {
            method: "pinned legacy parse_sql_file_with_spans".into(),
            fixture: "migration-tool/fixtures/baseline/capture-statement-spans.rs".into(),
            fixture_sha256: format!("{:x}", Sha256::digest(CAPTURE_SOURCE.as_bytes())),
        },
        total_statements: files.iter().map(|file| file.statement_count).sum(),
        files,
        commands: StatementSpanCommands {
            review: "cargo run --locked -p pg-sql-migrate -- baseline review-statements --legacy-repository ../recursa-old --postgres-repository vendor/postgres --baseline baselines/postgresql-17.9.json --spans baselines/postgresql-17.9-statements.json".into(),
            update: "cargo run --locked -p pg-sql-migrate -- baseline capture-statements --legacy-repository ../recursa-old --postgres-repository vendor/postgres --baseline baselines/postgresql-17.9.json --output baselines/postgresql-17.9-statements.json".into(),
        },
    };
    validate_statement_spans(&spans, outcomes)?;
    validate_statement_sources(&spans, &postgres)?;
    Ok(spans)
}

pub fn to_canonical_statement_spans(
    spans: &StatementSpanBaseline,
    outcomes: &Baseline,
) -> Result<String, StatementSpanError> {
    let mut canonical = spans.clone();
    canonical
        .files
        .sort_by(|left, right| left.file.cmp(&right.file));
    validate_statement_spans(&canonical, outcomes)?;
    let mut json = serde_json::to_string_pretty(&canonical)
        .map_err(|error| StatementSpanError::new(error.to_string()))?;
    json.push('\n');
    Ok(json)
}

pub fn write_statement_spans(
    path: &Path,
    spans: &StatementSpanBaseline,
    outcomes: &Baseline,
) -> Result<(), StatementSpanError> {
    let content = to_canonical_statement_spans(spans, outcomes)?;
    let parent = path
        .parent()
        .ok_or_else(|| StatementSpanError::new("statement-span path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .expect("span artifact file name")
            .to_string_lossy()
    ));
    fs::write(&temporary, content)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn capture_manifest(pg_sql: &Path, recursa: &Path) -> String {
    format!(
        "[package]\nname = \"pg-sql-span-capture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\
         [workspace]\n\
         [dependencies]\npg-sql = {{ path = {} }}\nrecursa = {{ path = {} }}\n",
        serde_json::to_string(&pg_sql.to_string_lossy()).expect("serialize pg-sql path"),
        serde_json::to_string(&recursa.to_string_lossy()).expect("serialize Recursa path"),
    )
}

fn clone_at(
    source: &Path,
    destination: &Path,
    revision: &str,
    label: &str,
) -> Result<(), StatementSpanError> {
    run_checked(
        Command::new("git")
            .args(["clone", "--quiet", "--no-hardlinks", "--no-checkout"])
            .arg(source)
            .arg(destination),
        &format!("clone {label}"),
    )?;
    run_checked(
        Command::new("git")
            .arg("-C")
            .arg(destination)
            .args(["checkout", "--quiet", "--detach", revision]),
        &format!("check out {label}"),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapturedStatement {
    range: ByteRange,
    legacy_item_kind: char,
}

fn parse_capture(
    output: &str,
) -> Result<BTreeMap<String, Vec<CapturedStatement>>, StatementSpanError> {
    let mut files = BTreeMap::<String, Vec<CapturedStatement>>::new();
    for line in output.lines() {
        let mut fields = line.split('\t');
        let file = fields
            .next()
            .ok_or_else(|| StatementSpanError::new("capture row has no file"))?;
        let start = fields
            .next()
            .ok_or_else(|| StatementSpanError::new("capture row has no start"))?
            .parse()
            .map_err(|_| StatementSpanError::new("capture start is not an integer"))?;
        let end = fields
            .next()
            .ok_or_else(|| StatementSpanError::new("capture row has no end"))?
            .parse()
            .map_err(|_| StatementSpanError::new("capture end is not an integer"))?;
        let legacy_item_kind = match fields
            .next()
            .ok_or_else(|| StatementSpanError::new("capture row has no legacy item kind"))?
        {
            "S" => 'S',
            "E" => 'E',
            _ => {
                return Err(StatementSpanError::new(
                    "capture row has an invalid legacy item kind",
                ));
            }
        };
        if fields.next().is_some() {
            return Err(StatementSpanError::new("capture row has extra fields"));
        }
        files
            .entry(file.to_owned())
            .or_default()
            .push(CapturedStatement {
                range: ByteRange { start, end },
                legacy_item_kind,
            });
    }
    Ok(files)
}

fn encode_byte_ranges(ranges: &[ByteRange]) -> String {
    ranges
        .iter()
        .map(|range| format!("{}:{}", range.start, range.end))
        .collect::<Vec<_>>()
        .join(",")
}

fn git_stdout(repository: &Path, arguments: &[&str]) -> Result<String, StatementSpanError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| StatementSpanError::new(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(StatementSpanError::new(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run_checked(command: &mut Command, operation: &str) -> Result<(), StatementSpanError> {
    let output = command
        .output()
        .map_err(|error| StatementSpanError::new(format!("cannot {operation}: {error}")))?;
    if !output.status.success() {
        return Err(StatementSpanError::new(format!(
            "cannot {operation}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}
