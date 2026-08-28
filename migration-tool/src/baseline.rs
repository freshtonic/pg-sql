//! Reproducible capture of the legacy PostgreSQL differential baseline.
//!
//! Capture deliberately operates on local Git repositories. It clones both
//! immutable inputs into a private temporary directory before invoking Cargo;
//! neither the legacy source repository nor the PostgreSQL source repository
//! is built or generated in place.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LEGACY_COMMIT: &str = "1e71421d66baac15c8c5264e8f29b5f80122f50e";
pub const BASELINE_NAME: &str = "postgresql-17.9";
pub const POSTGRES_RELEASE: &str = "17.9";
pub const CORPUS_ROOT: &str = "vendor/postgres/src/test/regress/sql";

const INCLUSION_RULE: &str =
    "exact file list declared by corpus_tests! in the legacy differential suite";
const NON_IDENTIFIER_REASON: &str =
    "legacy differential suite omitted fixture names that are not Rust identifiers";
const SKIP_RULES: &[&str] = &[
    "psql directives and COPY-from-stdin payloads are non-SQL",
    "statements containing psql variable interpolation are not standalone SQL",
    "a whole-file legacy parse failure yields no statements",
    "PostgreSQL-accepted statements the legacy parser cannot model are skips",
];
const OUT_OF_TREE_BUILD_RS: &str = include_str!("../fixtures/baseline/pg-oracle-build.rs");
const OUT_OF_TREE_BUILD_SCRIPT: &str = include_str!("../fixtures/baseline/build-pg.sh");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Baseline {
    pub schema_version: u32,
    pub name: String,
    pub legacy: Identity,
    pub postgres: PostgresIdentity,
    pub corpus: Corpus,
    pub capture_build: CaptureBuild,
    pub commands: CaptureCommands,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Identity {
    pub commit: String,
    pub tree: String,
    pub pg_sql_tree: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PostgresIdentity {
    pub release: String,
    pub gitlink: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Corpus {
    pub root: String,
    pub inclusion_rule: String,
    pub skip_rules: Vec<String>,
    pub available_files: usize,
    pub included: Vec<String>,
    pub excluded: Vec<ExcludedFile>,
    pub files: Vec<FileOutcome>,
    pub total_statements: usize,
    pub totals: OutcomeCounts,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExcludedFile {
    pub file: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileOutcome {
    pub file: String,
    pub statements: usize,
    pub outcomes: OutcomeCounts,
}

impl FileOutcome {
    pub fn new(file: impl Into<String>, pass: usize, skip: usize, fail: usize) -> Self {
        Self {
            file: file.into(),
            statements: pass + skip + fail,
            outcomes: OutcomeCounts { pass, skip, fail },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutcomeCounts {
    pub pass: usize,
    pub skip: usize,
    pub fail: usize,
}

impl OutcomeCounts {
    fn add(self, other: Self) -> Self {
        Self {
            pass: self.pass + other.pass,
            skip: self.skip + other.skip,
            fail: self.fail + other.fail,
        }
    }

    fn statements(self) -> usize {
        self.pass + self.skip + self.fail
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureCommands {
    pub review: String,
    pub update: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureBuild {
    pub strategy: String,
    pub fixtures: Vec<BuildFixture>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildFixture {
    pub file: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusMembership {
    pub included: Vec<String>,
    pub excluded: Vec<ExcludedFile>,
}

#[derive(Clone, Debug)]
pub struct CaptureOptions {
    pub legacy_repository: PathBuf,
    pub postgres_repository: PathBuf,
    pub legacy_commit: String,
}

impl CaptureOptions {
    pub fn new(
        legacy_repository: impl Into<PathBuf>,
        postgres_repository: impl Into<PathBuf>,
    ) -> Self {
        Self {
            legacy_repository: legacy_repository.into(),
            postgres_repository: postgres_repository.into(),
            legacy_commit: LEGACY_COMMIT.into(),
        }
    }
}

#[derive(Debug)]
pub struct BaselineError(String);

impl BaselineError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for BaselineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BaselineError {}

impl From<std::io::Error> for BaselineError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

/// Discover the exact file membership declared by the legacy test driver.
pub fn discover_corpus(pg_sql_checkout: &Path) -> Result<CorpusMembership, BaselineError> {
    let driver_path = pg_sql_checkout.join("tests/differential.rs");
    let driver = fs::read_to_string(&driver_path).map_err(|error| {
        BaselineError::new(format!("cannot read {}: {error}", driver_path.display()))
    })?;
    let declared = declared_corpus_files(&driver)?;
    let corpus_path = pg_sql_checkout.join(CORPUS_ROOT);
    let mut available = Vec::new();
    for entry in fs::read_dir(&corpus_path).map_err(|error| {
        BaselineError::new(format!("cannot read {}: {error}", corpus_path.display()))
    })? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("sql")
        {
            available.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    available.sort();

    let available_set: BTreeSet<_> = available.iter().cloned().collect();
    for file in &declared {
        if !available_set.contains(file) {
            return Err(BaselineError::new(format!(
                "legacy differential suite declares missing corpus file {file}"
            )));
        }
    }

    let declared_set: BTreeSet<_> = declared.iter().cloned().collect();
    let mut excluded = Vec::new();
    for file in available
        .into_iter()
        .filter(|file| !declared_set.contains(file))
    {
        if is_rust_identifier_file(&file) {
            return Err(BaselineError::new(format!(
                "legacy differential suite unexpectedly omits Rust-identifier corpus file {file}"
            )));
        }
        excluded.push(ExcludedFile {
            file,
            reason: NON_IDENTIFIER_REASON.into(),
        });
    }

    Ok(CorpusMembership {
        included: declared,
        excluded,
    })
}

fn is_rust_identifier_file(file: &str) -> bool {
    let Some(stem) = file.strip_suffix(".sql") else {
        return false;
    };
    let mut bytes = stem.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn declared_corpus_files(driver: &str) -> Result<Vec<String>, BaselineError> {
    let marker = "corpus_tests!";
    let start = driver
        .rfind(marker)
        .ok_or_else(|| BaselineError::new("legacy differential suite has no corpus_tests! call"))?;
    let body_start = driver[start + marker.len()..]
        .find('{')
        .map(|offset| start + marker.len() + offset + 1)
        .ok_or_else(|| BaselineError::new("corpus_tests! call has no opening brace"))?;
    let body_end = driver[body_start..]
        .find('}')
        .map(|offset| body_start + offset)
        .ok_or_else(|| BaselineError::new("corpus_tests! call has no closing brace"))?;

    let mut files = Vec::new();
    for token in driver[body_start..body_end].split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token.split_whitespace().count() != 1 {
            return Err(BaselineError::new(format!(
                "unexpected token in corpus_tests! call: {token:?}"
            )));
        }
        let name = token.strip_prefix("r#").unwrap_or(token);
        files.push(format!("{name}.sql"));
    }
    files.sort();
    if files.is_empty() {
        return Err(BaselineError::new("corpus_tests! declares no files"));
    }
    if files.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(BaselineError::new(
            "corpus_tests! declares a file more than once",
        ));
    }
    Ok(files)
}

/// Parse the stable per-file summary emitted by the legacy differential test.
pub fn parse_transcript(transcript: &str) -> Result<Vec<FileOutcome>, BaselineError> {
    let mut outcomes = BTreeMap::new();
    for line in transcript.lines().map(str::trim) {
        if !line.starts_with('[') || !line.contains("] pass=") {
            continue;
        }
        let close = line
            .find(']')
            .ok_or_else(|| BaselineError::new(format!("malformed outcome line: {line}")))?;
        let name = &line[1..close];
        let fields: Vec<_> = line[close + 1..].split_whitespace().collect();
        if fields.len() != 3 {
            return Err(BaselineError::new(format!(
                "malformed outcome line: {line}"
            )));
        }
        let pass = parse_count(fields[0], "pass=", line)?;
        let skip = parse_count(fields[1], "skip=", line)?;
        let fail = parse_count(fields[2], "fail=", line)?;
        let file = format!("{name}.sql");
        if outcomes
            .insert(
                file.clone(),
                FileOutcome::new(file.clone(), pass, skip, fail),
            )
            .is_some()
        {
            return Err(BaselineError::new(format!(
                "transcript contains duplicate outcome for {file}"
            )));
        }
    }
    if outcomes.is_empty() {
        return Err(BaselineError::new(
            "test transcript contains no differential outcomes",
        ));
    }
    Ok(outcomes.into_values().collect())
}

fn parse_count(field: &str, prefix: &str, line: &str) -> Result<usize, BaselineError> {
    field
        .strip_prefix(prefix)
        .ok_or_else(|| BaselineError::new(format!("malformed outcome line: {line}")))?
        .parse()
        .map_err(|_| BaselineError::new(format!("malformed outcome line: {line}")))
}

/// Verify membership coverage and all derived statement/outcome counts.
pub fn validate_baseline(baseline: &Baseline) -> Result<(), BaselineError> {
    if baseline.schema_version != 1 {
        return Err(BaselineError::new("unsupported baseline schema version"));
    }
    let included: BTreeSet<_> = baseline.corpus.included.iter().collect();
    if included.len() != baseline.corpus.included.len() {
        return Err(BaselineError::new("included corpus contains duplicates"));
    }
    let excluded: BTreeSet<_> = baseline
        .corpus
        .excluded
        .iter()
        .map(|entry| &entry.file)
        .collect();
    if excluded.len() != baseline.corpus.excluded.len() {
        return Err(BaselineError::new("excluded corpus contains duplicates"));
    }
    if let Some(file) = included.intersection(&excluded).next() {
        return Err(BaselineError::new(format!(
            "{file} is both included and excluded"
        )));
    }
    if baseline.corpus.available_files != included.len() + excluded.len() {
        return Err(BaselineError::new(
            "available file count does not equal included plus excluded files",
        ));
    }

    let mut seen = BTreeSet::new();
    let mut totals = OutcomeCounts::default();
    for file in &baseline.corpus.files {
        if !included.contains(&file.file) {
            return Err(BaselineError::new(format!(
                "{} has outcomes but is not included",
                file.file
            )));
        }
        if !seen.insert(&file.file) {
            return Err(BaselineError::new(format!(
                "{} has more than one outcome",
                file.file
            )));
        }
        if file.statements != file.outcomes.statements() {
            return Err(BaselineError::new(format!(
                "{} statement count does not equal its outcomes",
                file.file
            )));
        }
        totals = totals.add(file.outcomes);
    }
    if let Some(file) = included.iter().find(|file| !seen.contains(*file)) {
        return Err(BaselineError::new(format!("{file} has no outcome")));
    }
    if baseline.corpus.totals != totals {
        return Err(BaselineError::new(
            "corpus totals do not equal the per-file outcomes",
        ));
    }
    if baseline.corpus.total_statements != totals.statements() {
        return Err(BaselineError::new(
            "total statement count does not equal the outcome totals",
        ));
    }
    Ok(())
}

/// Return deterministic, pretty JSON with exactly one trailing newline.
pub fn to_canonical_json(baseline: &Baseline) -> Result<String, BaselineError> {
    let mut canonical = baseline.clone();
    canonical.corpus.included.sort();
    canonical
        .corpus
        .excluded
        .sort_by(|left, right| left.file.cmp(&right.file));
    canonical
        .corpus
        .files
        .sort_by(|left, right| left.file.cmp(&right.file));
    canonical.corpus.totals = canonical
        .corpus
        .files
        .iter()
        .fold(OutcomeCounts::default(), |total, file| {
            total.add(file.outcomes)
        });
    canonical.corpus.available_files =
        canonical.corpus.included.len() + canonical.corpus.excluded.len();
    canonical.corpus.total_statements = canonical.corpus.totals.statements();
    validate_baseline(&canonical)?;
    let mut json = serde_json::to_string_pretty(&canonical)
        .map_err(|error| BaselineError::new(format!("cannot serialize baseline: {error}")))?;
    json.push('\n');
    Ok(json)
}

/// Atomically replace a baseline after validating and serializing it.
pub fn write_baseline(path: &Path, baseline: &Baseline) -> Result<(), BaselineError> {
    let bytes = to_canonical_json(baseline)?;
    let parent = path
        .parent()
        .ok_or_else(|| BaselineError::new("baseline path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap().to_string_lossy()
    ));
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

/// Install deterministic, capture-only build plumbing in a disposable legacy clone.
///
/// The legacy parser, formatter, oracle C sources, tests, and PostgreSQL source
/// remain byte-for-byte unchanged. Only the two legacy build entry points are
/// replaced so generated PostgreSQL files and libraries live under Cargo's
/// `OUT_DIR`.
pub fn install_out_of_tree_build_plumbing(legacy_checkout: &Path) -> Result<(), BaselineError> {
    let build_rs = legacy_checkout.join("pg-oracle/build.rs");
    let build_script = legacy_checkout.join("pg-oracle/scripts/build-pg.sh");
    if !build_rs.is_file() || !build_script.is_file() {
        return Err(BaselineError::new(
            "disposable legacy checkout is missing pg-oracle build plumbing",
        ));
    }
    fs::write(build_rs, OUT_OF_TREE_BUILD_RS)?;
    fs::write(build_script, OUT_OF_TREE_BUILD_SCRIPT)?;
    Ok(())
}

fn capture_build_metadata() -> CaptureBuild {
    let mut fixtures = vec![
        BuildFixture {
            file: "migration-tool/fixtures/baseline/build-pg.sh".into(),
            sha256: format!("{:x}", Sha256::digest(OUT_OF_TREE_BUILD_SCRIPT.as_bytes())),
        },
        BuildFixture {
            file: "migration-tool/fixtures/baseline/pg-oracle-build.rs".into(),
            sha256: format!("{:x}", Sha256::digest(OUT_OF_TREE_BUILD_RS.as_bytes())),
        },
    ];
    fixtures.sort_by(|left, right| left.file.cmp(&right.file));
    CaptureBuild {
        strategy: "capture-only pg-oracle build entry points direct generated PostgreSQL artifacts to Cargo OUT_DIR".into(),
        fixtures,
    }
}

/// Capture from disposable clones of the pinned local legacy and PostgreSQL repositories.
pub fn capture_baseline(options: &CaptureOptions) -> Result<Baseline, BaselineError> {
    let legacy_before = repository_state(&options.legacy_repository)?;
    let postgres_before = repository_state(&options.postgres_repository)?;
    let identity = legacy_identity(&options.legacy_repository, &options.legacy_commit)?;
    let postgres_gitlink = git_stdout(
        &options.legacy_repository,
        &[
            "rev-parse",
            &format!("{}:pg-sql/vendor/postgres", options.legacy_commit),
        ],
    )?;

    let temporary = DisposableDirectory::new()?;
    let checkout = temporary.path.join("legacy");
    run_checked(
        Command::new("git")
            .args(["clone", "--no-hardlinks", "--no-checkout"])
            .arg(&options.legacy_repository)
            .arg(&checkout),
        "clone legacy repository",
    )?;
    run_checked(
        Command::new("git").arg("-C").arg(&checkout).args([
            "checkout",
            "--detach",
            &options.legacy_commit,
        ]),
        "check out pinned legacy commit",
    )?;

    let postgres_checkout = checkout.join("pg-sql/vendor/postgres");
    run_checked(
        Command::new("git")
            .args(["clone", "--no-hardlinks", "--no-checkout"])
            .arg(&options.postgres_repository)
            .arg(&postgres_checkout),
        "clone PostgreSQL repository",
    )?;
    run_checked(
        Command::new("git").arg("-C").arg(&postgres_checkout).args([
            "checkout",
            "--detach",
            &postgres_gitlink,
        ]),
        "check out pinned PostgreSQL gitlink",
    )?;
    let disposable_postgres_before = repository_state(&postgres_checkout)?;
    install_out_of_tree_build_plumbing(&checkout)?;

    let pg_sql_checkout = checkout.join("pg-sql");
    let membership = discover_corpus(&pg_sql_checkout)?;
    let output = Command::new("cargo")
        .current_dir(&checkout)
        .env("CARGO_TARGET_DIR", temporary.path.join("target"))
        .env_remove("PROFILE")
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .args([
            "test",
            "--locked",
            "-p",
            "pg-sql",
            "--test",
            "differential",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .map_err(|error| {
            BaselineError::new(format!("cannot run legacy differential test: {error}"))
        })?;
    let transcript = combined_output(&output);
    let files = parse_transcript(&transcript).map_err(|error| {
        BaselineError::new(format!(
            "legacy differential test produced no complete baseline: {error}\n{}",
            tail(&transcript, 80)
        ))
    })?;

    let totals = files.iter().fold(OutcomeCounts::default(), |total, file| {
        total.add(file.outcomes)
    });
    let baseline = Baseline {
        schema_version: 1,
        name: BASELINE_NAME.into(),
        legacy: identity,
        postgres: PostgresIdentity {
            release: POSTGRES_RELEASE.into(),
            gitlink: postgres_gitlink,
        },
        corpus: Corpus {
            root: CORPUS_ROOT.into(),
            inclusion_rule: INCLUSION_RULE.into(),
            skip_rules: SKIP_RULES.iter().map(|rule| (*rule).into()).collect(),
            available_files: membership.included.len() + membership.excluded.len(),
            included: membership.included,
            excluded: membership.excluded,
            files,
            total_statements: totals.statements(),
            totals,
        },
        capture_build: capture_build_metadata(),
        commands: CaptureCommands {
            review: "cargo run --locked -p pg-sql-migrate -- baseline review --legacy-repository ../recursa-old --postgres-repository vendor/postgres --baseline baselines/postgresql-17.9.json".into(),
            update: "cargo run --locked -p pg-sql-migrate -- baseline capture --legacy-repository ../recursa-old --postgres-repository vendor/postgres --output baselines/postgresql-17.9.json".into(),
        },
    };
    validate_baseline(&baseline)?;
    if !output.status.success() && baseline.corpus.totals.fail == 0 {
        return Err(BaselineError::new(format!(
            "legacy differential command failed despite reporting no statement failures:\n{}",
            tail(&transcript, 80)
        )));
    }
    let disposable_postgres_after = repository_state(&postgres_checkout)?;
    if disposable_postgres_before != disposable_postgres_after {
        return Err(BaselineError::new(
            "out-of-tree oracle build modified the disposable PostgreSQL source checkout",
        ));
    }
    let legacy_after = repository_state(&options.legacy_repository)?;
    if legacy_before != legacy_after {
        return Err(BaselineError::new(
            "legacy repository changed during disposable capture",
        ));
    }
    let postgres_after = repository_state(&options.postgres_repository)?;
    if postgres_before != postgres_after {
        return Err(BaselineError::new(
            "PostgreSQL repository changed during disposable capture",
        ));
    }
    Ok(baseline)
}

fn legacy_identity(repository: &Path, commit: &str) -> Result<Identity, BaselineError> {
    Ok(Identity {
        commit: git_stdout(repository, &["rev-parse", &format!("{commit}^{{commit}}")])?,
        tree: git_stdout(repository, &["show", "-s", "--format=%T", commit])?,
        pg_sql_tree: git_stdout(repository, &["rev-parse", &format!("{commit}:pg-sql")])?,
    })
}

fn repository_state(repository: &Path) -> Result<(String, String), BaselineError> {
    Ok((
        git_stdout(repository, &["rev-parse", "HEAD"])?,
        git_stdout(repository, &["status", "--porcelain=v1"])?,
    ))
}

fn git_stdout(repository: &Path, args: &[&str]) -> Result<String, BaselineError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .map_err(|error| BaselineError::new(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(BaselineError::new(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run_checked(command: &mut Command, operation: &str) -> Result<(), BaselineError> {
    let output = command
        .output()
        .map_err(|error| BaselineError::new(format!("cannot {operation}: {error}")))?;
    if !output.status.success() {
        return Err(BaselineError::new(format!(
            "cannot {operation}: {}",
            combined_output(&output).trim()
        )));
    }
    Ok(())
}

fn combined_output(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<_> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

struct DisposableDirectory {
    path: PathBuf,
}

impl DisposableDirectory {
    fn new() -> Result<Self, BaselineError> {
        let base = std::env::temp_dir();
        for attempt in 0..100u32 {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| BaselineError::new(error.to_string()))?
                .as_nanos();
            let path = base.join(format!(
                "pg-sql-baseline-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(BaselineError::new(
            "could not allocate disposable capture directory",
        ))
    }
}

impl Drop for DisposableDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
