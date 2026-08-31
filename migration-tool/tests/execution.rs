use std::path::Path;
use std::process::Command;

use pg_sql_migrate::execution::{
    publication_tree_digest, published_source_digest, verify_reviewed_semantic_changes,
};

fn verify(repository: &Path, record: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pg-sql-migrate"))
        .args(["execution", "verify"])
        .arg("--repository-root")
        .arg(repository)
        .arg("--record")
        .arg(record)
        .output()
        .unwrap()
}

fn unused_reviewed_semantic_source(repository: &Path, ledger: &serde_json::Value) -> String {
    let reviewed_sources = ledger["changes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|change| change["source_ids"].as_array().unwrap())
        .map(|source| source.as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    let inventory: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("migration/contract/inventory.json")).unwrap(),
    )
    .unwrap();
    inventory["semantics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["id"].as_str())
        .find(|id| !reviewed_sources.contains(id))
        .unwrap()
        .to_owned()
}

fn rejects_record_change(mutate: impl FnOnce(&mut serde_json::Value), expected_error: &str) {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut record: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("migration/execution.json")).unwrap(),
    )
    .unwrap();
    mutate(&mut record);
    let temporary = tempfile::tempdir().unwrap();
    let record_path = temporary.path().join("execution.json");
    std::fs::write(&record_path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();

    let output = verify(repository, &record_path);

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_error),
        "expected {expected_error:?}, got {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn checked_migration_execution_reproduces_the_historical_publication() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let record = repository.join("migration/execution.json");

    let output = verify(repository, &record);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("migration execution reproduces {}\n", record.display())
    );
}

#[test]
fn checked_reviewed_semantic_changes_resolve_frozen_sources_and_live_destinations() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let ledger = repository.join("migration/reviewed-semantic-changes.json");

    verify_reviewed_semantic_changes(repository, &ledger).unwrap();
}

#[test]
fn reviewed_semantic_changes_reject_unknown_sources_and_destinations() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let canonical: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("migration/reviewed-semantic-changes.json")).unwrap(),
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();

    let mut unknown_source = canonical.clone();
    let existing_destination = canonical["changes"][0]["destinations"][0].clone();
    unknown_source["changes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "issue-9-999-unknown-source-regression",
            "source_ids": ["ast::shared::expr::Expr::Missing"],
            "destinations": [existing_destination],
            "rationale": "Exercise unknown-source validation after the frozen prefix."
        }));
    let path = temporary.path().join("unknown-source.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&unknown_source).unwrap()).unwrap();
    assert!(
        verify_reviewed_semantic_changes(repository, &path)
            .unwrap_err()
            .to_string()
            .contains("unknown frozen semantic source")
    );

    let mut unknown_destination = canonical;
    let valid_source = unused_reviewed_semantic_source(repository, &unknown_destination);
    let mut missing_destination = unknown_destination["changes"][0]["destinations"][0].clone();
    missing_destination["id"] =
        serde_json::Value::String("ast::dml::select::SelectItem::Missing".into());
    missing_destination["member"] = serde_json::Value::String("Missing".into());
    unknown_destination["changes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "issue-9-999-unknown-destination-regression",
            "source_ids": [valid_source],
            "destinations": [missing_destination],
            "rationale": "Exercise unknown-destination validation after the frozen prefix."
        }));
    let path = temporary.path().join("unknown-destination.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&unknown_destination).unwrap(),
    )
    .unwrap();
    assert!(
        verify_reviewed_semantic_changes(repository, &path)
            .unwrap_err()
            .to_string()
            .contains("does not exist in the live AST")
    );
}

#[test]
fn reviewed_semantic_changes_reject_reordering_and_duplicate_sources() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let canonical: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("migration/reviewed-semantic-changes.json")).unwrap(),
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();

    let mut reordered = canonical.clone();
    let valid_source = unused_reviewed_semantic_source(repository, &reordered);
    let existing_destination = reordered["changes"][0]["destinations"][0].clone();
    reordered["changes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "issue-9-000-reordered",
            "source_ids": [valid_source],
            "destinations": [existing_destination],
            "rationale": "Exercise ordering validation after the frozen prefix."
        }));
    let path = temporary.path().join("reordered.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&reordered).unwrap()).unwrap();
    assert!(
        verify_reviewed_semantic_changes(repository, &path)
            .unwrap_err()
            .to_string()
            .contains("not strictly append-ordered")
    );

    let mut duplicate = canonical;
    let first_source = duplicate["changes"][0]["source_ids"][0].clone();
    let existing_destination = duplicate["changes"][0]["destinations"][0].clone();
    duplicate["changes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "issue-9-999-duplicate-source-regression",
            "source_ids": [first_source],
            "destinations": [existing_destination],
            "rationale": "Exercise duplicate-source validation after the frozen prefix."
        }));
    let path = temporary.path().join("duplicate.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&duplicate).unwrap()).unwrap();
    assert!(
        verify_reviewed_semantic_changes(repository, &path)
            .unwrap_err()
            .to_string()
            .contains("reviewed more than once")
    );
}

#[test]
fn reviewed_semantic_changes_reject_rewriting_or_deleting_the_frozen_prefix() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let canonical: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("migration/reviewed-semantic-changes.json")).unwrap(),
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();

    let mut rewritten = canonical.clone();
    rewritten["changes"][0]["rationale"] =
        serde_json::Value::String("retroactively rewritten review".into());
    let path = temporary.path().join("rewritten-prefix.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&rewritten).unwrap()).unwrap();
    assert!(
        verify_reviewed_semantic_changes(repository, &path)
            .unwrap_err()
            .to_string()
            .contains("frozen prefix digest")
    );

    let mut deleted = canonical;
    deleted["changes"].as_array_mut().unwrap().remove(0);
    let path = temporary.path().join("deleted-prefix-entry.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&deleted).unwrap()).unwrap();
    assert!(
        verify_reviewed_semantic_changes(repository, &path)
            .unwrap_err()
            .to_string()
            .contains("frozen prefix")
    );
}

#[test]
fn reviewed_semantic_changes_allow_entries_after_the_frozen_prefix() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut ledger: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository.join("migration/reviewed-semantic-changes.json")).unwrap(),
    )
    .unwrap();
    let unused_source = unused_reviewed_semantic_source(repository, &ledger);
    let existing_destination = ledger["changes"][0]["destinations"][0].clone();
    ledger["changes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "issue-9-999-append-only-regression",
            "source_ids": [unused_source],
            "destinations": [existing_destination],
            "rationale": "A future reviewed entry may append without changing the frozen prefix."
        }));

    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("appended-entry.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&ledger).unwrap()).unwrap();

    verify_reviewed_semantic_changes(repository, &path).unwrap();
}

#[test]
fn historical_execution_and_inventory_remain_byte_frozen() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let digest = |relative: &str| {
        use sha2::{Digest, Sha256};

        let bytes = std::fs::read(repository.join(relative)).unwrap();
        format!("{:x}", Sha256::digest(bytes))
    };

    assert_eq!(
        digest("migration/execution.json"),
        "0c0c76016571f281d4bee0a00449800d31a32530076f045578f739d276464c76"
    );
    assert_eq!(
        digest("migration/contract/inventory.json"),
        "823091d9e06bfd915ddd254874b6a678b4c1a3ef9fa0e5c324f4456365797cd7"
    );
}

#[test]
fn execution_rejects_identity_command_semantic_and_checkpoint_drift() {
    rejects_record_change(
        |record| {
            record["immutable_inputs"]["migration_source_tree"] =
                serde_json::Value::String("0".repeat(40));
        },
        "recorded migration source tree",
    );
    rejects_record_change(
        |record| {
            record["commands"]["grammar_pass"] =
                serde_json::Value::String("unreviewed grammar command".into());
        },
        "execution command grammar_pass",
    );
    rejects_record_change(
        |record| record["review"]["mapped_semantic_inventory_rows"] = 8_039.into(),
        "execution maps 8039/8040 semantic rows",
    );
    rejects_record_change(
        |record| record["review"]["compile_checkpoint"]["diagnostic_count"] = 63.into(),
        "compile checkpoint must record RCA1013 discovery handoff",
    );
}

#[test]
fn execution_replay_rejects_preservation_and_pass_digest_drift() {
    rejects_record_change(
        |record| {
            record["output"]["source_input_tree_sha256"] =
                serde_json::Value::String("0".repeat(64));
        },
        "source input tree digest is",
    );
    rejects_record_change(
        |record| {
            record["output"]["grammar_tree_sha256"] = serde_json::Value::String("0".repeat(64));
        },
        "replayed grammar pass digest is",
    );
}

#[test]
fn publication_manifest_rejects_extra_missing_and_chmodded_owned_files() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path();
    std::fs::create_dir_all(repository.join("src")).unwrap();
    std::fs::write(
        repository.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\n",
    )
    .unwrap();
    std::fs::write(repository.join("build.rs"), "fn main() {}\n").unwrap();
    std::fs::write(repository.join("src/lib.rs"), "pub struct Kept;\n").unwrap();
    let manifest = repository.join("provenance.tsv");
    std::fs::write(
        &manifest,
        "pg-sql\t100644\t0000000000000000000000000000000000000000\tCargo.toml\npg-sql\t100644\t0000000000000000000000000000000000000000\tsrc/lib.rs\n",
    )
    .unwrap();

    assert_eq!(
        publication_tree_digest(repository, &manifest)
            .unwrap()
            .len(),
        64
    );

    std::fs::write(repository.join("src/extra.rs"), "pub struct Extra;\n").unwrap();
    assert!(
        publication_tree_digest(repository, &manifest)
            .unwrap_err()
            .to_string()
            .contains("unexpected publication-owned path")
    );
    std::fs::remove_file(repository.join("src/extra.rs")).unwrap();

    std::fs::remove_file(repository.join("src/lib.rs")).unwrap();
    assert!(
        publication_tree_digest(repository, &manifest)
            .unwrap_err()
            .to_string()
            .contains("missing publication-owned path")
    );
    std::fs::write(repository.join("src/lib.rs"), "pub struct Kept;\n").unwrap();

    let moved_source = repository.join("moved-src");
    std::fs::rename(repository.join("src"), &moved_source).unwrap();
    symlink(&moved_source, repository.join("src")).unwrap();
    assert!(
        publication_tree_digest(repository, &manifest)
            .unwrap_err()
            .to_string()
            .contains("src path is a symlink")
    );
    std::fs::remove_file(repository.join("src")).unwrap();
    std::fs::rename(moved_source, repository.join("src")).unwrap();

    let mut permissions = std::fs::metadata(repository.join("build.rs"))
        .unwrap()
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(repository.join("build.rs"), permissions).unwrap();
    assert!(
        publication_tree_digest(repository, &manifest)
            .unwrap_err()
            .to_string()
            .contains("mode")
    );
}

#[test]
fn publication_digest_paths_reject_manifest_column_drift_uniformly() {
    let temporary = tempfile::tempdir().unwrap();
    let repository = temporary.path();
    std::fs::create_dir(repository.join("src")).unwrap();
    let manifest = repository.join("provenance.tsv");
    std::fs::write(&manifest, "pg-sql\t100644\tmissing-path\n").unwrap();
    let expected = "invalid import manifest row 1: expected 4 tab-separated columns";

    assert_eq!(
        publication_tree_digest(repository, &manifest)
            .unwrap_err()
            .to_string(),
        expected
    );
    assert_eq!(
        published_source_digest(repository, &manifest, &Default::default())
            .unwrap_err()
            .to_string(),
        expected
    );
}
