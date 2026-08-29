//! Verification for the checked deterministic-migration execution record.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output as ProcessOutput};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::migration_contract::{
    LEGACY_COMMIT, LEGACY_TREE, MIGRATION_SOURCE_COMMIT, MIGRATION_SOURCE_TREE, OMITTED_PATHS,
    PG_ORACLE_TREE, PG_SQL_TREE, POSTGRES_GITLINK, PUBLICATION_ADDITIONS, RECURSA_REVISION,
    SOURCE_CHECKPOINT, is_publication_owned_import,
};
use crate::rewrite::{RewriteTreeRequest, rewrite_tree};
use crate::test_call_rewrite::TestCallRewritePass;
use crate::{Mapping, inventory, to_canonical_inventory_json};
use crate::{grammar_rewrite::GeneratedWhitespaceCleanupPass, grammar_rewrite::GrammarRewritePass};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionRecord {
    schema_version: u32,
    immutable_inputs: ImmutableInputs,
    recursa_revision: String,
    commands: Commands,
    output: Output,
    review: Review,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImmutableInputs {
    migration_source_commit: String,
    migration_source_tree: String,
    legacy_commit: String,
    legacy_tree: String,
    pg_sql_tree: String,
    pg_oracle_tree: String,
    postgres_gitlink: String,
    source_checkpoint: String,
    import_manifest_sha256: String,
    inventory_sha256: String,
    grammar_manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Commands {
    verify_imports: String,
    verify_recursa: String,
    check_inventory: String,
    materialize_input: String,
    grammar_pass: String,
    test_call_pass: String,
    combined_pass: String,
    repeated_combined_pass: String,
    compile_checkpoint: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    source_input_tree_sha256: String,
    grammar_tree_sha256: String,
    test_call_tree_sha256: String,
    combined_tree_sha256: String,
    repeated_combined_tree_sha256: String,
    published_payload_tree_sha256: String,
    publication_tree_sha256: String,
    omitted_paths: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    semantic_inventory_rows: usize,
    mapped_semantic_inventory_rows: usize,
    compile_checkpoint: CompileCheckpoint,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompileCheckpoint {
    outcome: String,
    diagnostic_code: String,
    diagnostic_count: usize,
    next_issue: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImportSection {
    PgSql,
    PgOracle,
    Bootstrap,
}

#[derive(Clone, Copy, Debug)]
struct ImportManifestRow<'a> {
    section: ImportSection,
    mode: &'a str,
    _object_id: &'a str,
    path: &'a str,
}

#[derive(Debug)]
pub struct ExecutionError(String);

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ExecutionError {}

fn parse_import_manifest(manifest: &str) -> Result<Vec<ImportManifestRow<'_>>, ExecutionError> {
    let mut rows = Vec::new();
    for (line_number, line) in manifest.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let (Some(section), Some(mode), Some(object_id), Some(path), None) = (
            columns.next(),
            columns.next(),
            columns.next(),
            columns.next(),
            columns.next(),
        ) else {
            return Err(fail(format!(
                "invalid import manifest row {}: expected 4 tab-separated columns",
                line_number + 1
            )));
        };
        let section = match section {
            "pg-sql" => ImportSection::PgSql,
            "pg-oracle" => ImportSection::PgOracle,
            "bootstrap" => ImportSection::Bootstrap,
            other => {
                return Err(fail(format!(
                    "invalid import manifest row {}: unknown section {other:?}",
                    line_number + 1
                )));
            }
        };
        rows.push(ImportManifestRow {
            section,
            mode,
            _object_id: object_id,
            path,
        });
    }
    Ok(rows)
}

/// Verify that the reviewed execution record describes the currently
/// published migration output and the immutable inputs recorded by the repo.
pub fn verify_execution(repository: &Path, record_path: &Path) -> Result<(), ExecutionError> {
    let record_bytes = read(record_path)?;
    let record: ExecutionRecord = serde_json::from_slice(&record_bytes)
        .map_err(|error| fail(format!("parse {}: {error}", record_path.display())))?;

    if record.schema_version != 2 {
        return Err(fail(format!(
            "unsupported migration execution schema {}",
            record.schema_version
        )));
    }

    let manifest_path = repository.join("docs/import-provenance.tsv");
    let inventory_path = repository.join("migration/contract/inventory.json");
    let grammar_manifest_path =
        repository.join("migration-tool/fixtures/rewrite/grammar/manifest.json");
    expect_digest(
        "import manifest",
        &record.immutable_inputs.import_manifest_sha256,
        &read(&manifest_path)?,
    )?;
    let inventory_bytes = read(&inventory_path)?;
    expect_digest(
        "migration inventory",
        &record.immutable_inputs.inventory_sha256,
        &inventory_bytes,
    )?;
    expect_digest(
        "grammar manifest",
        &record.immutable_inputs.grammar_manifest_sha256,
        &read(&grammar_manifest_path)?,
    )?;
    verify_inventory(&inventory_bytes, &record)?;
    verify_provenance(repository, &record)?;

    let recorded_revision = read_text(&repository.join(".recursa-revision"))?;
    if record.recursa_revision != RECURSA_REVISION {
        return Err(fail(format!(
            "execution records unapproved Recursa revision {}",
            record.recursa_revision
        )));
    }
    if recorded_revision.trim() != record.recursa_revision {
        return Err(fail(format!(
            ".recursa-revision is {}, execution records {}",
            recorded_revision.trim(),
            record.recursa_revision
        )));
    }
    verify_recursa_checkout(repository)?;

    verify_review(&record)?;
    verify_commands(&record.commands)?;
    for (name, digest) in [
        ("source input tree", &record.output.source_input_tree_sha256),
        ("grammar tree", &record.output.grammar_tree_sha256),
        ("test-call tree", &record.output.test_call_tree_sha256),
        ("combined tree", &record.output.combined_tree_sha256),
        (
            "repeated combined tree",
            &record.output.repeated_combined_tree_sha256,
        ),
        (
            "published payload tree",
            &record.output.published_payload_tree_sha256,
        ),
        ("publication tree", &record.output.publication_tree_sha256),
    ] {
        verify_sha256(name, digest)?;
    }
    if record.output.combined_tree_sha256 != record.output.repeated_combined_tree_sha256 {
        return Err(fail(
            "combined migration runs did not record byte-identical trees".into(),
        ));
    }
    if record.output.combined_tree_sha256 != record.output.published_payload_tree_sha256 {
        return Err(fail(
            "published source tree differs from the reviewed combined migration".into(),
        ));
    }
    let expected_omissions = OMITTED_PATHS
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    if record.output.omitted_paths != expected_omissions {
        return Err(fail(format!(
            "execution records unexpected omitted paths: {:?}",
            record.output.omitted_paths
        )));
    }

    let published =
        published_source_digest(repository, &manifest_path, &record.output.omitted_paths)?;
    if published != record.output.published_payload_tree_sha256 {
        return Err(fail(format!(
            "published payload tree digest is {published}, expected {}",
            record.output.published_payload_tree_sha256
        )));
    }
    let publication = publication_tree_digest(repository, &manifest_path)?;
    if publication != record.output.publication_tree_sha256 {
        return Err(fail(format!(
            "publication tree digest is {publication}, expected {}",
            record.output.publication_tree_sha256
        )));
    }

    verify_obsolete_artifacts(repository)?;
    reproduce_execution(repository, &record, &manifest_path, &grammar_manifest_path)?;
    verify_compile_checkpoint(repository, &record.review.compile_checkpoint)?;
    Ok(())
}

fn verify_provenance(repository: &Path, record: &ExecutionRecord) -> Result<(), ExecutionError> {
    let expected = [
        (
            "migration source commit",
            &record.immutable_inputs.migration_source_commit,
            MIGRATION_SOURCE_COMMIT,
        ),
        (
            "migration source tree",
            &record.immutable_inputs.migration_source_tree,
            MIGRATION_SOURCE_TREE,
        ),
        (
            "legacy commit",
            &record.immutable_inputs.legacy_commit,
            LEGACY_COMMIT,
        ),
        (
            "legacy tree",
            &record.immutable_inputs.legacy_tree,
            LEGACY_TREE,
        ),
        (
            "pg-sql tree",
            &record.immutable_inputs.pg_sql_tree,
            PG_SQL_TREE,
        ),
        (
            "pg-oracle tree",
            &record.immutable_inputs.pg_oracle_tree,
            PG_ORACLE_TREE,
        ),
        (
            "PostgreSQL gitlink",
            &record.immutable_inputs.postgres_gitlink,
            POSTGRES_GITLINK,
        ),
        (
            "source checkpoint",
            &record.immutable_inputs.source_checkpoint,
            SOURCE_CHECKPOINT,
        ),
    ];
    for (name, actual, expected) in expected {
        if actual != expected {
            return Err(fail(format!(
                "recorded {name} is {actual}, expected {expected}"
            )));
        }
    }
    for (name, revision, expected) in [
        (
            "migration source tree",
            &format!("{MIGRATION_SOURCE_COMMIT}^{{tree}}"),
            MIGRATION_SOURCE_TREE,
        ),
        (
            "legacy tree",
            &format!("{LEGACY_COMMIT}^{{tree}}"),
            LEGACY_TREE,
        ),
        (
            "legacy pg-sql tree",
            &format!("{LEGACY_COMMIT}:pg-sql"),
            PG_SQL_TREE,
        ),
        (
            "legacy pg-oracle tree",
            &format!("{LEGACY_COMMIT}:pg-oracle"),
            PG_ORACLE_TREE,
        ),
        (
            "source checkpoint",
            &SOURCE_CHECKPOINT.to_owned(),
            SOURCE_CHECKPOINT,
        ),
        (
            "migration source PostgreSQL gitlink",
            &format!("{MIGRATION_SOURCE_COMMIT}:vendor/postgres"),
            POSTGRES_GITLINK,
        ),
        (
            "source checkpoint PostgreSQL gitlink",
            &format!("{SOURCE_CHECKPOINT}:vendor/postgres"),
            POSTGRES_GITLINK,
        ),
    ] {
        let actual = git_text(repository, &["rev-parse", revision])?;
        if actual.trim() != expected {
            return Err(fail(format!(
                "actual {name} is {}, expected {expected}",
                actual.trim()
            )));
        }
    }
    expect_git_success(
        repository,
        &[
            "merge-base",
            "--is-ancestor",
            SOURCE_CHECKPOINT,
            MIGRATION_SOURCE_COMMIT,
        ],
        "source checkpoint is not an ancestor of the migration source commit",
    )?;
    expect_git_success(
        repository,
        &["diff", "--quiet", "HEAD", "--", "vendor/postgres"],
        "PostgreSQL gitlink differs from HEAD",
    )?;
    expect_git_success(
        repository,
        &[
            "diff",
            "--quiet",
            "HEAD",
            "--",
            ".gitattributes",
            "CLAUDE.md",
            "README.md",
            "benches",
            "fixtures",
            "pg-oracle",
            "tests",
            "vendor",
        ],
        "immutable non-publication inputs differ from HEAD",
    )?;
    let untracked = git_text(
        repository,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "--",
            "benches",
            "fixtures",
            "pg-oracle",
            "tests",
            "vendor",
        ],
    )?;
    if !untracked.trim().is_empty() {
        return Err(fail(format!(
            "untracked immutable input survives: {}",
            untracked.lines().next().unwrap_or_default()
        )));
    }
    let import_check = Command::new(repository.join("scripts/verify-import-provenance"))
        .current_dir(repository)
        .output()
        .map_err(|error| fail(format!("run import provenance verifier: {error}")))?;
    if !import_check.status.success() {
        return Err(fail(format!(
            "import provenance verifier failed: {}",
            String::from_utf8_lossy(&import_check.stderr)
        )));
    }
    Ok(())
}

fn git_output(repository: &Path, arguments: &[&str]) -> Result<ProcessOutput, ExecutionError> {
    Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| fail(format!("run git {arguments:?}: {error}")))
}

fn git_text(repository: &Path, arguments: &[&str]) -> Result<String, ExecutionError> {
    let output = git_output(repository, arguments)?;
    if !output.status.success() {
        return Err(fail(format!(
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| {
        fail(format!(
            "git {arguments:?} returned non-UTF-8 output: {error}"
        ))
    })
}

fn expect_git_success(
    repository: &Path,
    arguments: &[&str],
    message: &str,
) -> Result<(), ExecutionError> {
    let output = git_output(repository, arguments)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(fail(format!(
            "{message}: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}

fn verify_recursa_checkout(repository: &Path) -> Result<(), ExecutionError> {
    let repository = fs::canonicalize(repository).map_err(|error| {
        fail(format!(
            "resolve repository root {}: {error}",
            repository.display()
        ))
    })?;
    let sibling = repository
        .parent()
        .ok_or_else(|| fail("repository has no parent for sibling Recursa checkout".into()))?
        .join("recursa");
    let revision = git_text(&sibling, &["rev-parse", "HEAD"])?;
    if revision.trim() != RECURSA_REVISION {
        return Err(fail(format!(
            "sibling Recursa checkout is {}, expected {RECURSA_REVISION}",
            revision.trim()
        )));
    }
    let symbolic_ref = git_output(&sibling, &["symbolic-ref", "--quiet", "HEAD"])?;
    if symbolic_ref.status.success() {
        return Err(fail(format!(
            "sibling Recursa checkout is attached to {}",
            String::from_utf8_lossy(&symbolic_ref.stdout).trim()
        )));
    }
    let status = git_text(&sibling, &["status", "--porcelain"])?;
    if !status.trim().is_empty() {
        return Err(fail("sibling Recursa checkout is dirty".into()));
    }
    Ok(())
}

fn verify_inventory(inventory: &[u8], record: &ExecutionRecord) -> Result<(), ExecutionError> {
    let value: serde_json::Value = serde_json::from_slice(inventory)
        .map_err(|error| fail(format!("parse migration inventory: {error}")))?;
    let semantic_rows = value
        .get("summary")
        .and_then(|summary| summary.get("semantic_rows"))
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| fail("migration inventory has no semantic row count".into()))?
        as usize;
    if semantic_rows != record.review.semantic_inventory_rows
        || semantic_rows != record.review.mapped_semantic_inventory_rows
    {
        return Err(fail(format!(
            "execution maps {}/{} semantic rows, inventory contains {semantic_rows}",
            record.review.mapped_semantic_inventory_rows, record.review.semantic_inventory_rows
        )));
    }
    Ok(())
}

fn verify_review(record: &ExecutionRecord) -> Result<(), ExecutionError> {
    if record.review.semantic_inventory_rows != 8_040
        || record.review.mapped_semantic_inventory_rows != 8_040
    {
        return Err(fail(
            "compile evidence must account for all 8040 semantic rows".into(),
        ));
    }
    if record.review.compile_checkpoint.outcome != "expected_failure"
        || record.review.compile_checkpoint.diagnostic_code != "RCA1013"
        || record.review.compile_checkpoint.diagnostic_count != 64
        || record.review.compile_checkpoint.next_issue != 9
    {
        return Err(fail(
            "compile checkpoint must record RCA1013 discovery handoff to issue 9".into(),
        ));
    }
    Ok(())
}

fn verify_commands(commands: &Commands) -> Result<(), ExecutionError> {
    let expected = Commands {
        verify_imports: "scripts/verify-import-provenance".into(),
        verify_recursa: "scripts/verify-recursa-revision ../recursa".into(),
        check_inventory: "cargo test -p pg-sql-migrate --test inventory checked_in_legacy_contract_remains_fully_reviewed_after_publication -- --exact".into(),
        materialize_input: format!("git archive {MIGRATION_SOURCE_COMMIT} .gitattributes CLAUDE.md Cargo.toml README.md benches fixtures pg-oracle src tests vendor | tar -x -C $RUN_ROOT/input"),
        grammar_pass: "cargo run -p pg-sql-migrate -- rewrite grammar $RUN_ROOT/input $RUN_ROOT/outputs/grammar --new-repository-root $RUN_ROOT/outputs --manifest migration-tool/fixtures/rewrite/grammar/manifest.json".into(),
        test_call_pass: "cargo run -p pg-sql-migrate -- rewrite test-calls $RUN_ROOT/input $RUN_ROOT/outputs/test-calls --new-repository-root $RUN_ROOT/outputs".into(),
        combined_pass: "cargo run -p pg-sql-migrate -- rewrite test-calls $RUN_ROOT/outputs/grammar $RUN_ROOT/outputs/combined --new-repository-root $RUN_ROOT/outputs".into(),
        repeated_combined_pass: "cargo run -p pg-sql-migrate -- rewrite grammar $RUN_ROOT/input $RUN_ROOT/outputs/grammar-repeat --new-repository-root $RUN_ROOT/outputs --manifest migration-tool/fixtures/rewrite/grammar/manifest.json && cargo run -p pg-sql-migrate -- rewrite test-calls $RUN_ROOT/outputs/grammar-repeat $RUN_ROOT/outputs/combined-repeat --new-repository-root $RUN_ROOT/outputs".into(),
        compile_checkpoint: "cargo check -p pg-sql --lib".into(),
    };
    for (name, actual, expected) in [
        (
            "verify_imports",
            &commands.verify_imports,
            &expected.verify_imports,
        ),
        (
            "verify_recursa",
            &commands.verify_recursa,
            &expected.verify_recursa,
        ),
        (
            "check_inventory",
            &commands.check_inventory,
            &expected.check_inventory,
        ),
        (
            "materialize_input",
            &commands.materialize_input,
            &expected.materialize_input,
        ),
        (
            "grammar_pass",
            &commands.grammar_pass,
            &expected.grammar_pass,
        ),
        (
            "test_call_pass",
            &commands.test_call_pass,
            &expected.test_call_pass,
        ),
        (
            "combined_pass",
            &commands.combined_pass,
            &expected.combined_pass,
        ),
        (
            "repeated_combined_pass",
            &commands.repeated_combined_pass,
            &expected.repeated_combined_pass,
        ),
        (
            "compile_checkpoint",
            &commands.compile_checkpoint,
            &expected.compile_checkpoint,
        ),
    ] {
        if actual != expected {
            return Err(fail(format!(
                "execution command {name} is {actual:?}, expected {expected:?}"
            )));
        }
    }
    Ok(())
}

fn reproduce_execution(
    repository: &Path,
    record: &ExecutionRecord,
    manifest_path: &Path,
    grammar_manifest_path: &Path,
) -> Result<(), ExecutionError> {
    let temporary = tempfile::tempdir()
        .map_err(|error| fail(format!("create execution verification directory: {error}")))?;
    let frozen = temporary.path().join("frozen");
    materialize_commit(repository, MIGRATION_SOURCE_COMMIT, &frozen)?;
    materialize_postgres_corpus(repository, &frozen)?;

    let regenerated = inventory(&frozen, &Mapping::migration_contract())
        .map_err(|error| fail(format!("regenerate frozen inventory: {error}")))?;
    let regenerated = to_canonical_inventory_json(&regenerated)
        .map_err(|error| fail(format!("serialize frozen inventory: {error}")))?;
    let checked_inventory = read(&repository.join("migration/contract/inventory.json"))?;
    if regenerated.as_bytes() != checked_inventory {
        return Err(fail(
            "frozen migration inventory does not match canonical checked bytes".into(),
        ));
    }

    let input = temporary.path().join("input");
    materialize_selected_input(&frozen, &input, manifest_path)?;
    let empty = BTreeSet::new();
    let source_before = published_source_digest(&input, manifest_path, &empty)?;
    if source_before != record.output.source_input_tree_sha256 {
        return Err(fail(format!(
            "source input tree digest is {source_before}, expected {}",
            record.output.source_input_tree_sha256
        )));
    }

    let outputs = temporary.path().join("outputs");
    fs::create_dir(&outputs)
        .map_err(|error| fail(format!("create {}: {error}", outputs.display())))?;
    let grammar_json = read_text(grammar_manifest_path)?;
    let grammar = GrammarRewritePass::from_manifest_json(&grammar_json)
        .map_err(|error| fail(format!("load grammar manifest: {error}")))?;
    let cleanup = GeneratedWhitespaceCleanupPass;
    let test_calls = TestCallRewritePass;

    let grammar_output = outputs.join("grammar");
    rewrite_tree(RewriteTreeRequest {
        source_root: &input,
        destination_root: &grammar_output,
        new_repository_root: &outputs,
        passes: &[&grammar, &cleanup],
    })
    .map_err(|error| fail(format!("replay grammar pass: {error}")))?;

    let test_call_output = outputs.join("test-calls");
    rewrite_tree(RewriteTreeRequest {
        source_root: &input,
        destination_root: &test_call_output,
        new_repository_root: &outputs,
        passes: &[&test_calls],
    })
    .map_err(|error| fail(format!("replay test-call pass: {error}")))?;

    let combined_output = outputs.join("combined");
    rewrite_tree(RewriteTreeRequest {
        source_root: &grammar_output,
        destination_root: &combined_output,
        new_repository_root: &outputs,
        passes: &[&test_calls],
    })
    .map_err(|error| fail(format!("replay combined pass: {error}")))?;

    let grammar_repeat = outputs.join("grammar-repeat");
    rewrite_tree(RewriteTreeRequest {
        source_root: &input,
        destination_root: &grammar_repeat,
        new_repository_root: &outputs,
        passes: &[&grammar, &cleanup],
    })
    .map_err(|error| fail(format!("replay repeated grammar pass: {error}")))?;
    let combined_repeat = outputs.join("combined-repeat");
    rewrite_tree(RewriteTreeRequest {
        source_root: &grammar_repeat,
        destination_root: &combined_repeat,
        new_repository_root: &outputs,
        passes: &[&test_calls],
    })
    .map_err(|error| fail(format!("replay repeated combined pass: {error}")))?;

    let source_after = published_source_digest(&input, manifest_path, &empty)?;
    if source_after != source_before {
        return Err(fail(format!(
            "immutable migration input changed from {source_before} to {source_after}"
        )));
    }
    for (name, root, omitted, expected) in [
        (
            "grammar pass",
            &grammar_output,
            &record.output.omitted_paths,
            &record.output.grammar_tree_sha256,
        ),
        (
            "test-call pass",
            &test_call_output,
            &empty,
            &record.output.test_call_tree_sha256,
        ),
        (
            "combined pass",
            &combined_output,
            &record.output.omitted_paths,
            &record.output.combined_tree_sha256,
        ),
        (
            "repeated combined pass",
            &combined_repeat,
            &record.output.omitted_paths,
            &record.output.repeated_combined_tree_sha256,
        ),
    ] {
        let actual = published_source_digest(root, manifest_path, omitted)?;
        if &actual != expected {
            return Err(fail(format!(
                "replayed {name} digest is {actual}, expected {expected}"
            )));
        }
    }
    Ok(())
}

fn materialize_commit(
    repository: &Path,
    commit: &str,
    destination: &Path,
) -> Result<(), ExecutionError> {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir(destination)
        .map_err(|error| fail(format!("create {}: {error}", destination.display())))?;
    let listing = git_text(repository, &["ls-tree", "-r", commit])?;
    for line in listing.lines() {
        let (metadata, relative) = line
            .split_once('\t')
            .ok_or_else(|| fail(format!("invalid git tree row: {line}")))?;
        let mode = metadata
            .split_whitespace()
            .next()
            .ok_or_else(|| fail(format!("invalid git tree metadata: {metadata}")))?;
        if mode == "160000" {
            continue;
        }
        let target = destination.join(relative);
        fs::create_dir_all(
            target
                .parent()
                .expect("materialized repository path has a parent"),
        )
        .map_err(|error| fail(format!("create parent of {}: {error}", target.display())))?;
        let object = format!("{commit}:{relative}");
        let output = git_output(repository, &["show", &object])?;
        if !output.status.success() {
            return Err(fail(format!(
                "materialize {object}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        fs::write(&target, output.stdout)
            .map_err(|error| fail(format!("write {}: {error}", target.display())))?;
        let permissions = if mode == "100755" { 0o755 } else { 0o644 };
        fs::set_permissions(&target, fs::Permissions::from_mode(permissions))
            .map_err(|error| fail(format!("set mode on {}: {error}", target.display())))?;
    }
    Ok(())
}

fn materialize_selected_input(
    frozen: &Path,
    destination: &Path,
    manifest: &Path,
) -> Result<(), ExecutionError> {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir(destination)
        .map_err(|error| fail(format!("create {}: {error}", destination.display())))?;
    let manifest = read_text(manifest)?;
    for row in parse_import_manifest(&manifest)? {
        if !matches!(row.section, ImportSection::PgSql | ImportSection::PgOracle)
            || row.path == "vendor/postgres"
        {
            continue;
        }
        let source = frozen.join(row.path);
        let target = destination.join(row.path);
        fs::create_dir_all(target.parent().expect("selected path has a parent"))
            .map_err(|error| fail(format!("create parent of {}: {error}", target.display())))?;
        fs::copy(&source, &target).map_err(|error| {
            fail(format!(
                "copy {} to {}: {error}",
                source.display(),
                target.display()
            ))
        })?;
        let permissions = u32::from_str_radix(
            row.mode
                .strip_prefix("100")
                .ok_or_else(|| fail(format!("unsupported source mode {}", row.mode)))?,
            8,
        )
        .map_err(|error| fail(format!("invalid source mode {}: {error}", row.mode)))?;
        fs::set_permissions(&target, fs::Permissions::from_mode(permissions))
            .map_err(|error| fail(format!("set mode on {}: {error}", target.display())))?;
    }
    Ok(())
}

fn materialize_postgres_corpus(
    repository: &Path,
    destination: &Path,
) -> Result<(), ExecutionError> {
    let common_git =
        PathBuf::from(git_text(repository, &["rev-parse", "--git-common-dir"])?.trim());
    let common_git = if common_git.is_absolute() {
        common_git
    } else {
        repository.join(common_git)
    };
    let postgres_git = common_git.join("modules/vendor/postgres");
    if !postgres_git.is_dir() {
        return Err(fail(format!(
            "PostgreSQL submodule object database is unavailable at {}",
            postgres_git.display()
        )));
    }
    let git_dir = format!("--git-dir={}", postgres_git.display());
    let listing = Command::new("git")
        .arg(&git_dir)
        .args([
            "ls-tree",
            "-r",
            "--name-only",
            POSTGRES_GITLINK,
            "--",
            "src/test/regress/sql",
        ])
        .output()
        .map_err(|error| fail(format!("list PostgreSQL corpus: {error}")))?;
    if !listing.status.success() {
        return Err(fail(format!(
            "list PostgreSQL corpus: {}",
            String::from_utf8_lossy(&listing.stderr)
        )));
    }
    let listing = String::from_utf8(listing.stdout)
        .map_err(|error| fail(format!("PostgreSQL corpus paths are not UTF-8: {error}")))?;
    for relative in listing.lines() {
        let target = destination.join("vendor/postgres").join(relative);
        fs::create_dir_all(target.parent().expect("corpus path has a parent"))
            .map_err(|error| fail(format!("create parent of {}: {error}", target.display())))?;
        let object = format!("{POSTGRES_GITLINK}:{relative}");
        let output = Command::new("git")
            .arg(&git_dir)
            .args(["show", &object])
            .output()
            .map_err(|error| fail(format!("materialize {object}: {error}")))?;
        if !output.status.success() {
            return Err(fail(format!(
                "materialize {object}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        fs::write(&target, output.stdout)
            .map_err(|error| fail(format!("write {}: {error}", target.display())))?;
    }
    Ok(())
}

fn verify_compile_checkpoint(
    repository: &Path,
    expected: &CompileCheckpoint,
) -> Result<(), ExecutionError> {
    let output = Command::new("cargo")
        .args(["check", "-p", "pg-sql", "--lib"])
        .current_dir(repository)
        .output()
        .map_err(|error| fail(format!("run compile checkpoint: {error}")))?;
    if output.status.success() {
        return Err(fail(
            "compile checkpoint unexpectedly succeeded before issue 9".into(),
        ));
    }
    let diagnostics = String::from_utf8_lossy(&output.stderr);
    let mut codes = BTreeMap::new();
    for remainder in diagnostics.split("error[").skip(1) {
        let Some((code, _)) = remainder.split_once(']') else {
            continue;
        };
        *codes.entry(code.to_owned()).or_insert(0usize) += 1;
    }
    let expected_codes =
        BTreeMap::from([(expected.diagnostic_code.clone(), expected.diagnostic_count)]);
    if codes != expected_codes {
        return Err(fail(format!(
            "compile checkpoint diagnostics are {codes:?}, expected {expected_codes:?}"
        )));
    }
    Ok(())
}

/// Validate and digest the complete grammar-publication ownership boundary.
///
/// Imported `Cargo.toml` and `src/**` membership comes from the immutable
/// provenance manifest. Reviewed omissions are required to be absent and
/// repository-integration additions (currently `build.rs`) are explicit.
pub fn publication_tree_digest(
    repository: &Path,
    manifest: &Path,
) -> Result<String, ExecutionError> {
    use std::os::unix::fs::PermissionsExt;

    let manifest = read_text(manifest)?;
    let mut expected = BTreeMap::new();
    for row in parse_import_manifest(&manifest)? {
        if row.section != ImportSection::PgSql
            || !is_publication_owned_import(row.path)
            || OMITTED_PATHS.contains(&row.path)
        {
            continue;
        }
        if expected
            .insert(row.path.to_owned(), row.mode.to_owned())
            .is_some()
        {
            return Err(fail(format!(
                "duplicate publication-owned manifest path: {}",
                row.path
            )));
        }
    }
    for &(relative, permissions) in PUBLICATION_ADDITIONS {
        expected.insert(relative.to_owned(), format!("100{permissions:03o}"));
    }

    for relative in OMITTED_PATHS {
        if fs::symlink_metadata(repository.join(relative)).is_ok() {
            return Err(fail(format!(
                "reviewed omitted path still exists: {relative}"
            )));
        }
    }

    let actual = publication_owned_files(repository)?;
    let expected_paths = expected.keys().cloned().collect::<BTreeSet<_>>();
    if let Some(path) = actual.difference(&expected_paths).next() {
        return Err(fail(format!("unexpected publication-owned path: {path}")));
    }
    if let Some(path) = expected_paths.difference(&actual).next() {
        return Err(fail(format!("missing publication-owned path: {path}")));
    }

    let mut digest = Sha256::new();
    for (relative, expected_mode) in expected {
        let path = repository.join(&relative);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| fail(format!("inspect {}: {error}", path.display())))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(fail(format!(
                "publication-owned path is not a regular file: {relative}"
            )));
        }
        let permissions = expected_mode
            .strip_prefix("100")
            .ok_or_else(|| fail(format!("unsupported publication mode {expected_mode}")))?;
        let expected_permissions = u32::from_str_radix(permissions, 8)
            .map_err(|error| fail(format!("invalid mode {expected_mode}: {error}")))?;
        let actual_permissions = metadata.permissions().mode() & 0o777;
        if actual_permissions != expected_permissions {
            return Err(fail(format!(
                "publication-owned path {relative} has mode {actual_permissions:03o}, expected {expected_permissions:03o}"
            )));
        }
        let bytes = read(&path)?;
        digest.update(expected_mode.as_bytes());
        digest.update([0]);
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(&bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn publication_owned_files(repository: &Path) -> Result<BTreeSet<String>, ExecutionError> {
    fn visit(
        repository: &Path,
        directory: &Path,
        files: &mut BTreeSet<String>,
    ) -> Result<(), ExecutionError> {
        for entry in fs::read_dir(directory)
            .map_err(|error| fail(format!("read {}: {error}", directory.display())))?
        {
            let entry = entry.map_err(|error| {
                fail(format!(
                    "read directory entry in {}: {error}",
                    directory.display()
                ))
            })?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| fail(format!("inspect {}: {error}", path.display())))?;
            if metadata.file_type().is_symlink() {
                return Err(fail(format!(
                    "publication-owned path is a symlink: {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                visit(repository, &path, files)?;
            } else if metadata.is_file() {
                files.insert(
                    path.strip_prefix(repository)
                        .expect("walk remains beneath repository")
                        .to_string_lossy()
                        .into_owned(),
                );
            } else {
                return Err(fail(format!(
                    "publication-owned path is not a regular file: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    let mut files = BTreeSet::new();
    for relative in ["Cargo.toml", "build.rs"] {
        if fs::symlink_metadata(repository.join(relative)).is_ok() {
            files.insert(relative.to_owned());
        }
    }
    let source = repository.join("src");
    match fs::symlink_metadata(&source) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(fail("publication-owned src path is a symlink".into()));
        }
        Ok(metadata) if metadata.is_dir() => visit(repository, &source, &mut files)?,
        Ok(_) => return Err(fail("publication-owned src path is not a directory".into())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(fail(format!("inspect {}: {error}", source.display()))),
    }
    Ok(files)
}

/// Digest every published pg-sql path in the immutable import manifest.
///
/// The digest commits to each reviewed mode, relative path, byte length, and
/// file content in path order. The PostgreSQL gitlink has its own immutable
/// identity and is therefore excluded from this regular-file tree digest.
pub fn published_source_digest(
    repository: &Path,
    manifest: &Path,
    omitted: &BTreeSet<String>,
) -> Result<String, ExecutionError> {
    let manifest = read_text(manifest)?;
    let mut entries = Vec::new();
    for row in parse_import_manifest(&manifest)? {
        if row.section != ImportSection::PgSql || row.path == "vendor/postgres" {
            continue;
        }
        entries.push((row.mode, row.path));
    }
    entries.sort_by_key(|(_, path)| *path);

    let mut digest = Sha256::new();
    for (mode, relative) in entries {
        if omitted.contains(relative) {
            if repository.join(relative).exists() {
                return Err(fail(format!(
                    "reviewed omitted path still exists: {relative}"
                )));
            }
            continue;
        }
        let path = repository.join(relative);
        let bytes = read(&path)?;
        digest.update(mode.as_bytes());
        digest.update([0]);
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(&bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_sha256(name: &str, value: &str) -> Result<(), ExecutionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(fail(format!("{name} is not a lowercase SHA-256 digest")));
    }
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(fail(format!("{name} is not a lowercase SHA-256 digest")));
    }
    Ok(())
}

fn verify_obsolete_artifacts(repository: &Path) -> Result<(), ExecutionError> {
    let source_root = repository.join("src");
    let forbidden_text = [
        "__firstset",
        "pub struct RestOfLine",
        "RawLines(",
        "ParseError(",
    ];
    let forbidden_attributes = [
        "#[postcondition",
        "#[parse(postcondition",
        "#[recursa::parser(postcondition",
        "#[tokens(post_lex",
        "#[lex(callback",
    ];
    for path in rust_files(&source_root)? {
        let source = read_text(&path)?;
        for needle in forbidden_text {
            if source.contains(needle) {
                return Err(fail(format!(
                    "obsolete artifact {needle:?} survives in {}",
                    path.display()
                )));
            }
        }
        for line in source.lines() {
            let line = line.trim_start();
            for prefix in forbidden_attributes {
                if line.starts_with(prefix) {
                    return Err(fail(format!(
                        "obsolete attribute {prefix:?} survives in {}",
                        path.display()
                    )));
                }
            }
        }
    }
    if source_root.join("generated/first_set.rs").exists() {
        return Err(fail("legacy generated FIRST-set source survives".into()));
    }
    let manifest = read_text(&repository.join("Cargo.toml"))?;
    let mut explicit_bin = false;
    for line in manifest.lines() {
        if line.starts_with("[[") {
            explicit_bin = line == "[[bin]]";
        } else if explicit_bin
            && let Some(relative) = line
                .strip_prefix("path = \"")
                .and_then(|path| path.strip_suffix('"'))
            && !repository.join(relative).is_file()
        {
            return Err(fail(format!(
                "explicit binary path does not exist after migration: {relative}"
            )));
        }
    }
    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>, ExecutionError> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| fail(format!("read {}: {error}", directory.display())))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                fail(format!(
                    "read directory entry in {}: {error}",
                    directory.display()
                ))
            })?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .map_err(|error| fail(format!("inspect {}: {error}", path.display())))?;
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path.extension().and_then(|value| value.to_str()) == Some("rs")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn expect_digest(name: &str, expected: &str, bytes: &[u8]) -> Result<(), ExecutionError> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        return Err(fail(format!(
            "{name} digest is {actual}, expected {expected}"
        )));
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>, ExecutionError> {
    fs::read(path).map_err(|error| fail(format!("read {}: {error}", path.display())))
}

fn read_text(path: &Path) -> Result<String, ExecutionError> {
    fs::read_to_string(path).map_err(|error| fail(format!("read {}: {error}", path.display())))
}

fn fail(message: String) -> ExecutionError {
    ExecutionError(message)
}
