use std::path::Path;
use std::process::Command;

use pg_sql_migrate::execution::{publication_tree_digest, published_source_digest};

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
fn checked_migration_execution_matches_the_published_tree() {
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
        format!("migration execution matches {}\n", record.display())
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
