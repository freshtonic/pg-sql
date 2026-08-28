use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/rewrite/grammar")
}

#[test]
fn cli_publishes_reviewed_configuration_and_admissions_without_touching_source() {
    let fixture = fixture_root();
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("legacy");
    let new_repository = temporary.path().join("new-repository");
    let destination = new_repository.join("migrated");
    fs::create_dir_all(source.join("grammar")).unwrap();
    fs::create_dir_all(&new_repository).unwrap();
    let input = fs::read(fixture.join("tokens.input.rs")).unwrap();
    fs::write(source.join("grammar/tokens.rs"), &input).unwrap();
    let hooks_input = fs::read(fixture.join("hooks.input.rs")).unwrap();
    fs::write(source.join("grammar/hooks.rs"), &hooks_input).unwrap();
    fs::write(source.join("untouched.bin"), [0, 159, 146, 150]).unwrap();
    fs::write(source.join(".recursa-revision"), "reviewed-revision\n").unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_pg-sql-migrate"))
        .args(["rewrite", "grammar"])
        .arg(&source)
        .arg(&destination)
        .arg("--new-repository-root")
        .arg(&new_repository)
        .arg("--manifest")
        .arg(fixture.join("manifest.json"))
        .status()
        .unwrap();

    assert!(status.success());
    assert_eq!(
        fs::read(destination.join("grammar/tokens.rs")).unwrap(),
        fs::read(fixture.join("tokens.expected.rs")).unwrap()
    );
    let rewritten_tokens = fs::read_to_string(destination.join("grammar/tokens.rs")).unwrap();
    assert!(!rewritten_tokens.contains("PsqlDirectiveLine"));
    assert!(!rewritten_tokens.contains("physical_line"));
    assert!(!rewritten_tokens.contains("RestOfLine"));
    assert!(rewritten_tokens.contains(
        r##"#[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(ColId))]"##
    ));
    assert!(rewritten_tokens.contains(
        r##"#[lex(pattern = r#"(?:[A-Za-z_][A-Za-z0-9_]*|'[^']*'|"[^"]*")"#, admits(PsqlVariableName))]"##
    ));
    assert!(!rewritten_tokens.contains("AcceptedLexicalSites"));
    assert_eq!(fs::read(source.join("grammar/tokens.rs")).unwrap(), input);
    assert_eq!(
        fs::read(destination.join("grammar/hooks.rs")).unwrap(),
        fs::read(fixture.join("hooks.expected.rs")).unwrap()
    );
    assert_eq!(
        fs::read(source.join("grammar/hooks.rs")).unwrap(),
        hooks_input
    );
    assert_eq!(
        fs::read(destination.join("untouched.bin")).unwrap(),
        [0, 159, 146, 150]
    );
    assert_eq!(
        fs::read(destination.join(".recursa-revision")).unwrap(),
        b"reviewed-revision\n"
    );
    assert_eq!(
        fs::read(source.join(".recursa-revision")).unwrap(),
        b"reviewed-revision\n"
    );

    let second_destination = new_repository.join("migrated-again");
    let second_status = Command::new(env!("CARGO_BIN_EXE_pg-sql-migrate"))
        .args(["rewrite", "grammar"])
        .arg(&source)
        .arg(&second_destination)
        .arg("--new-repository-root")
        .arg(&new_repository)
        .arg("--manifest")
        .arg(fixture.join("manifest.json"))
        .status()
        .unwrap();
    assert!(second_status.success());
    assert_eq!(
        fs::read(second_destination.join("grammar/tokens.rs")).unwrap(),
        fs::read(destination.join("grammar/tokens.rs")).unwrap()
    );
    assert_eq!(
        fs::read(second_destination.join("grammar/hooks.rs")).unwrap(),
        fs::read(destination.join("grammar/hooks.rs")).unwrap()
    );
}

#[test]
fn cli_rejects_an_unreviewed_hook_before_publishing_any_destination() {
    let fixture = fixture_root();
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("legacy");
    let new_repository = temporary.path().join("new-repository");
    let destination = new_repository.join("migrated");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&new_repository).unwrap();
    let input = fs::read(fixture.join("unsupported-inline-callback.input.rs")).unwrap();
    fs::write(source.join("grammar.rs"), &input).unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_pg-sql-migrate"))
        .args(["rewrite", "grammar"])
        .arg(&source)
        .arg(&destination)
        .arg("--new-repository-root")
        .arg(&new_repository)
        .arg("--manifest")
        .arg(fixture.join("manifest.json"))
        .status()
        .unwrap();

    assert!(!status.success());
    assert!(!destination.exists());
    assert_eq!(fs::read(source.join("grammar.rs")).unwrap(), input);
}
