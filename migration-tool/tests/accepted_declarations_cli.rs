use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod support;

use support::assert_single_token_attachment;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/rewrite/grammar")
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), files);
            } else {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_owned(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn cli_publishes_every_audited_optional_token_disposition_from_the_real_tree() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("legacy");
    let new_repository = temporary.path().join("new-repository");
    let destination = new_repository.join("migrated");
    copy_tree(&repository.join("src"), &source.join("src"));
    fs::create_dir_all(&new_repository).unwrap();
    let source_before = snapshot_tree(&source);

    let output = Command::new(env!("CARGO_BIN_EXE_pg-sql-migrate"))
        .args(["rewrite", "grammar"])
        .arg(&source)
        .arg(&destination)
        .arg("--new-repository-root")
        .arg(&new_repository)
        .arg("--manifest")
        .arg(fixture_root().join("manifest.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(snapshot_tree(&source), source_before);

    let migrated = snapshot_tree(&destination.join("src"));
    let authored = migrated
        .values()
        .filter_map(|bytes| std::str::from_utf8(bytes).ok())
        .collect::<Vec<_>>()
        .join("\n");
    for (path, bytes) in &migrated {
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            assert_single_token_attachment(path, std::str::from_utf8(bytes).unwrap());
        }
    }
    assert_eq!(authored.matches("#[presence(").count(), 83);
    // 48 dedicated filler rows, the reviewed nested RESTART filler, and
    // WITH RECURSIVE's optional member inside its fixed-token container.
    assert_eq!(authored.matches("optional(").count(), 50);

    let materialized_view =
        fs::read_to_string(destination.join("src/ast/ddl/materialized_view.rs")).unwrap();
    assert!(materialized_view.contains(
        "#[tok(CREATE, this, MATERIALIZED, VIEW)]\n    #[presence(UNLOGGED)]\n    pub unlogged: bool,"
    ));
    let expression = fs::read_to_string(destination.join("src/ast/shared/expr.rs")).unwrap();
    assert!(expression.contains("#[tok(this, JSON)]\n    #[presence(NOT)]\n    pub not: bool,"));
    let signed = fs::read_to_string(destination.join("src/ast/session/set_reset.rs")).unwrap();
    assert_eq!(signed.matches("#[presence(MINUS)]").count(), 2);
    assert_eq!(signed.matches("pub negative: bool,").count(), 2);
    assert!(!signed.contains("pub minus: Option<punct::Minus>"));
    let sequence = fs::read_to_string(destination.join("src/ast/ddl/sequence.rs")).unwrap();
    assert!(sequence.contains(
        "#[tok(RESTART, optional(WITH), this)]\n    pub with: Option<NumericOnly<'input>>,"
    ));

    assert!(!authored.contains("physical_line"));
    assert!(!authored.contains("pub struct RestOfLine"));
    assert!(!authored.contains("RestOfLine<'input>"));
    assert!(!authored.contains("RawLines("));
}

#[test]
fn cli_rewrites_real_required_tokens_hooks_and_raw_line_omissions() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("legacy");
    let new_repository = temporary.path().join("new-repository");
    let destination = new_repository.join("migrated");
    for relative in [
        "src/lib.rs",
        "src/tokens.rs",
        "src/ast/mod.rs",
        "src/ast/cursor/declare.rs",
        "src/ast/file.rs",
        "src/formatter.rs",
    ] {
        let target = source.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(repository.join(relative), target).unwrap();
    }
    fs::create_dir_all(&new_repository).unwrap();
    let tokens_before = fs::read(source.join("src/tokens.rs")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pg-sql-migrate"))
        .args(["rewrite", "grammar"])
        .arg(&source)
        .arg(&destination)
        .arg("--new-repository-root")
        .arg(&new_repository)
        .arg("--manifest")
        .arg(fixture_root().join("manifest.json"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(source.join("src/tokens.rs")).unwrap(),
        tokens_before
    );
    assert!(!destination.join("src/generated/first_set.rs").exists());
    let crate_root = fs::read_to_string(destination.join("src/lib.rs")).unwrap();
    assert_eq!(crate_root.matches("recursa::grammar!").count(), 1);
    assert!(crate_root.contains(
        "recursa::grammar! {\n    module = crate,\n    keyword_matching = ascii_insensitive,\n    max_lookahead = 5,\n}"
    ));
    assert!(!crate_root.contains("__firstset"));
    let file_surface = fs::read_to_string(destination.join("src/ast/file.rs")).unwrap();
    assert!(file_surface.contains("pub enum PsqlTerminator"));
    assert!(file_surface.contains("pub enum StatementTerminator"));
    assert!(file_surface.contains("pub struct TerminatedStatement"));
    assert!(!file_surface.contains("pub struct PsqlDirective"));
    assert!(!file_surface.contains("pub enum PsqlCommand"));
    assert!(!file_surface.contains("pub enum FileItem"));
    assert!(!file_surface.contains("pub fn parse_sql_file"));
    let formatter = fs::read_to_string(destination.join("src/formatter.rs")).unwrap();
    assert!(formatter.contains("pub fn format_tokens_sql"));
    assert!(!formatter.contains("pub fn format_file"));
    assert!(crate_root.contains("pub mod formatter;"));
    let ast_root = fs::read_to_string(destination.join("src/ast/mod.rs")).unwrap();
    assert!(ast_root.contains("pub mod file;"));
    assert!(ast_root.contains(
        "pub use self::file::{PsqlTerminator, StatementTerminator, TerminatedStatement};"
    ));
    assert!(!ast_root.contains("PsqlDirective"));
    assert!(!ast_root.contains("RawLines"));
    let migrated_tokens = fs::read_to_string(destination.join("src/tokens.rs")).unwrap();
    assert!(!migrated_tokens.contains("pub struct RestOfLine"));
    assert!(!migrated_tokens.contains("RestOfLine<'input>"));
    assert!(!migrated_tokens.contains("physical_line"));
    assert!(migrated_tokens.contains("DollarStringLit => same_delimiter"));
    assert!(migrated_tokens.contains("BlockComment => nested"));
    let cursor = fs::read_to_string(destination.join("src/ast/cursor/declare.rs")).unwrap();
    assert!(cursor.contains("#[tok(NO, SCROLL)] NoScroll"));
    assert!(!cursor.contains("NoScroll((NO, SCROLL))"));
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
