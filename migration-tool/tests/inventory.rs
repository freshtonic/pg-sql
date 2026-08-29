use std::fs;
use std::path::Path;
use std::process::Command;

use pg_sql_migrate::migration_contract::{MIGRATION_SOURCE_COMMIT, POSTGRES_GITLINK};
use pg_sql_migrate::{Mapping, inventory, to_canonical_inventory_json};

fn git_output(repository: &Path, arguments: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn materialize_frozen_inventory_input(repository: &Path, destination: &Path) {
    let listing = git_output(repository, &["ls-tree", "-r", MIGRATION_SOURCE_COMMIT]);
    for line in String::from_utf8(listing).unwrap().lines() {
        let (metadata, relative) = line.split_once('\t').unwrap();
        let mode = metadata.split_whitespace().next().unwrap();
        if mode == "160000" {
            continue;
        }
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let object = format!("{MIGRATION_SOURCE_COMMIT}:{relative}");
        fs::write(target, git_output(repository, &["show", &object])).unwrap();
    }

    let common_git =
        String::from_utf8(git_output(repository, &["rev-parse", "--git-common-dir"])).unwrap();
    let postgres_git = Path::new(common_git.trim()).join("modules/vendor/postgres");
    assert!(
        postgres_git.is_dir(),
        "PostgreSQL submodule object database is unavailable"
    );
    let corpus_prefix = "src/test/regress/sql";
    let output = Command::new("git")
        .arg(format!("--git-dir={}", postgres_git.display()))
        .args([
            "ls-tree",
            "-r",
            "--name-only",
            POSTGRES_GITLINK,
            "--",
            corpus_prefix,
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    for relative in String::from_utf8(output.stdout).unwrap().lines() {
        let target = destination.join("vendor/postgres").join(relative);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let object = format!("{POSTGRES_GITLINK}:{relative}");
        let output = Command::new("git")
            .arg(format!("--git-dir={}", postgres_git.display()))
            .args(["show", &object])
            .output()
            .unwrap();
        assert!(output.status.success());
        fs::write(target, output.stdout).unwrap();
    }
}

fn write_provenance(root: &std::path::Path) {
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    fs::write(
        root.join("docs/import-provenance.tsv"),
        "# legacy-commit 1111111111111111111111111111111111111111\n# legacy-tree 2222222222222222222222222222222222222222\n# pg-sql-tree 3333333333333333333333333333333333333333\n# pg-oracle-tree 4444444444444444444444444444444444444444\n# postgres-gitlink 5555555555555555555555555555555555555555\n# source-checkpoint 6666666666666666666666666666666666666666\n",
    ).unwrap();
}

#[test]
fn inventory_accounts_for_grammar_semantics_and_tests_without_mutating_source() {
    let root = tempfile::tempdir().unwrap();
    write_provenance(root.path());
    fs::create_dir_all(root.path().join("src/ast")).unwrap();
    fs::create_dir_all(root.path().join("tests/fmt")).unwrap();
    fs::create_dir_all(root.path().join("fixtures/stress")).unwrap();
    fs::write(
        root.path().join("src/ast/query.rs"),
        r#"
#[recursa::parser(rules = SqlRules)]
pub enum Query<'input> {
    #[parse(left_recursive)]
    Select {
        select: keyword::SELECT,
        optional_all: Option<keyword::ALL>,
        minus: Option<punct::Minus>,
        r#as: Option<keyword::AS>,
        or_replace: Option<(keyword::OR, keyword::REPLACE)>,
        star: Surrounded<punct::LParen, punct::Star, punct::RParen>,
        name: Surrounded<punct::LParen, Ident<'input>, punct::RParen>,
    },
}

#[cfg(test)] mod tests {
    #[test] fn parses_select() {}
    #[test] #[ignore = "known gap"] fn preserves_comment() {}
}
"#,
    )
    .unwrap();
    fs::write(root.path().join("tests/fmt/select.input.sql"), "select x").unwrap();
    fs::write(
        root.path().join("tests/fmt/select.golden.sql"),
        "SELECT x\n",
    )
    .unwrap();
    fs::write(root.path().join("fixtures/stress/select.sql"), "select x").unwrap();

    let before = fs::read(root.path().join("src/ast/query.rs")).unwrap();
    let report = inventory(root.path(), &Mapping::default()).unwrap();

    assert_eq!(report.summary.parser_types, 1);
    assert_eq!(report.summary.parse_roles, 2);
    assert_eq!(report.tests.literal_tests.len(), 2);
    assert_eq!(report.tests.ignored_tests.len(), 1);
    assert_eq!(report.tests.formatter_pairs.len(), 1);
    assert_eq!(report.tests.formatter_goldens.len(), 1);
    assert_eq!(report.tests.stress_workloads.len(), 1);
    assert!(
        report
            .semantics
            .iter()
            .any(|row| row.id.ends_with("Query::Select.name")
                && row.rule_id == "semantic.recursa-container-transform"
                && row.ported_shape.as_deref()
                    == Some("#[tok(LPAREN, this, RPAREN)] Ident < 'input >"))
    );
    assert!(report.semantics.iter().any(|row| {
        row.id.ends_with("Query::Select.optional_all")
            && row.rule_id == "semantic.optional-fixed-token.bool"
    }));
    assert!(
        report
            .semantics
            .iter()
            .any(|row| row.id.ends_with("Query::Select.minus")
                && row.rule_id == "semantic.optional-fixed-token.sign-bool")
    );
    assert!(
        report
            .semantics
            .iter()
            .any(|row| row.id.ends_with("Query::Select.r#as")
                && row.rule_id == "syntax.optional-fixed-token"
                && row.ported_shape.is_none())
    );
    assert!(
        report
            .semantics
            .iter()
            .any(|row| row.id.ends_with("Query::Select.select")
                && row.rule_id == "syntax.fixed-token")
    );
    assert!(report.semantics.iter().any(|row| {
        row.id.ends_with("Query::Select.star")
            && row.rule_id == "syntax.fixed-token-container"
            && row.ported_shape.is_none()
    }));
    assert!(report.semantics.iter().any(|row| {
        row.id.ends_with("Query::Select.or_replace")
            && row.rule_id == "semantic.optional-fixed-token.bool"
            && row.ported_shape.as_deref()
                == Some("or_replace: bool; OR, REPLACE presence moves to #[presence(OR, REPLACE)]")
    }));
    assert_eq!(
        fs::read(root.path().join("src/ast/query.rs")).unwrap(),
        before
    );
}

#[test]
fn inventory_rejects_contract_count_drift() {
    let root = tempfile::tempdir().unwrap();
    write_provenance(root.path());
    fs::create_dir_all(root.path().join("src/ast")).unwrap();
    fs::write(
        root.path().join("src/ast/empty.rs"),
        "#[recursa::parser] pub struct NewSyntax { mystery: Option<keyword::WITH> }",
    )
    .unwrap();
    let error = inventory(root.path(), &Mapping::default()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unreviewed optional fixed-token field ast::empty::NewSyntax.mystery")
    );
    fs::write(root.path().join("src/ast/empty.rs"), "").unwrap();
    let mapping = Mapping {
        expected_parser_types: Some(1),
        ..Mapping::default()
    };
    let error = inventory(root.path(), &mapping).unwrap_err();
    assert!(error.to_string().contains("parser type count drift"));
}

#[test]
fn inventory_rejects_non_scalar_provenance_identity() {
    let root = tempfile::tempdir().unwrap();
    write_provenance(root.path());
    fs::create_dir_all(root.path().join("src/ast")).unwrap();
    fs::write(root.path().join("src/ast/empty.rs"), "").unwrap();
    let path = root.path().join("docs/import-provenance.tsv");
    let invalid = fs::read_to_string(&path).unwrap().replace(
        "# legacy-tree 2222222222222222222222222222222222222222",
        "# legacy-tree tree plus commit evidence",
    );
    fs::write(path, invalid).unwrap();
    let error = inventory(root.path(), &Mapping::default()).unwrap_err();
    assert!(error.to_string().contains("invalid legacy-tree identity"));
}

#[test]
fn workspace_member_tests_are_discovered_separately_from_legacy_tests() {
    let root = tempfile::tempdir().unwrap();
    write_provenance(root.path());
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"helper\"]\n",
    )
    .unwrap();
    fs::create_dir_all(root.path().join("src/ast")).unwrap();
    fs::create_dir_all(root.path().join("helper/src")).unwrap();
    fs::write(root.path().join("src/ast/empty.rs"), "").unwrap();
    fs::write(
        root.path().join("helper/src/lib.rs"),
        "#[cfg(test)] mod tests { #[test] fn helper_contract() {} }",
    )
    .unwrap();
    let report = inventory(root.path(), &Mapping::default()).unwrap();
    assert!(report.tests.literal_tests.is_empty());
    assert_eq!(report.tests.workspace_members[0].member, "helper");
    assert_eq!(report.tests.workspace_members[0].tests.len(), 1);
    let mapping = Mapping {
        expected_workspace_members: Some(std::collections::BTreeMap::from([(
            "helper".into(),
            pg_sql_migrate::WorkspaceTestCounts {
                tests: 2,
                ignored: 0,
            },
        )])),
        ..Mapping::default()
    };
    let error = inventory(root.path(), &mapping).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("workspace member test contract drift")
    );
}

#[test]
fn checked_in_legacy_contract_remains_fully_reviewed_after_publication() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    materialize_frozen_inventory_input(root, temporary.path());
    let regenerated = inventory(temporary.path(), &Mapping::migration_contract()).unwrap();
    let regenerated = to_canonical_inventory_json(&regenerated).unwrap();
    let checked_in = fs::read(root.join("migration/contract/inventory.json")).unwrap();
    assert_eq!(regenerated.as_bytes(), checked_in);
    let report: serde_json::Value = serde_json::from_str(&regenerated).unwrap();
    let provenance = report["provenance"].as_object().unwrap();
    assert_eq!(
        provenance["legacy_commit"],
        "1e71421d66baac15c8c5264e8f29b5f80122f50e"
    );
    assert_eq!(
        provenance["legacy_tree"],
        "f3191ab707c8a957d1bb5fe142e74fc624fe6661"
    );
    assert_eq!(
        provenance["pg_sql_tree"],
        "50e1376d16796e5f05db88d99dab42252a9f78a4"
    );
    assert_eq!(
        provenance["pg_oracle_tree"],
        "0780d057e4d54db150d0f388c45a720a825bcbcf"
    );
    assert_eq!(
        provenance["postgres_gitlink"],
        "6d396980fc5aed4f1a525e0bd75cb16b25ed40ca"
    );
    assert_eq!(
        provenance["source_checkpoint"],
        "e97d3c3570c2a04ca9a233334b46d3f443800a5a"
    );

    let summary = report["summary"].as_object().unwrap();
    assert_eq!(summary["semantic_rows"], 8_040);
    assert_eq!(summary["expanded_tests"], 1_539);
    assert_eq!(summary["file_recovery_sites"], 238);
    let tests = report["tests"].as_object().unwrap();
    assert_eq!(tests["corpus_fixtures"].as_array().unwrap().len(), 222);
    assert_eq!(tests["formatter_goldens"].as_array().unwrap().len(), 10);
    assert!(
        tests["corpus_fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fixture| fixture["content_sha256"].as_str().is_some())
    );

    let semantics = report["semantics"].as_array().unwrap();
    assert_eq!(semantics.len(), 8_040);
    assert!(semantics.iter().all(|row| {
        row["id"].as_str().is_some_and(|value| !value.is_empty())
            && row["rule_id"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
            && row["rationale"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
            && matches!(
                row["disposition"].as_str(),
                Some(
                    "ported_equivalent"
                        | "reviewed_change"
                        | "syntax_only_exclusion"
                        | "framework_exclusion"
                        | "recursa_gap"
                )
            )
    }));
    let semantic = |id: &str| {
        semantics
            .iter()
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("missing reviewed semantic row {id}"))
    };
    assert_eq!(
        semantic("ast::file::FileItem")["disposition"],
        "reviewed_change"
    );
    assert_eq!(
        semantic("ast::shared::expr::StringLitSeq0.parts")["rule_id"],
        "semantic.recursa-container-transform"
    );
    assert_eq!(
        semantic("ast::ddl::function::ExtractedFuncBody")["kind"],
        "semantic_view"
    );
    assert_eq!(
        semantics
            .iter()
            .filter(|row| {
                row["rule_id"].as_str().is_some_and(|rule| {
                    rule.starts_with("semantic.optional-fixed-token")
                        || rule.starts_with("syntax.optional-fixed-token")
                })
            })
            .count(),
        132
    );
    assert_eq!(
        semantic("ast::ddl::index::CreateIndexStmt.unique")["rule_id"],
        "semantic.optional-fixed-token.bool"
    );
    assert_eq!(
        semantic("ast::ddl::sequence::SeqRestartOption.with")["rule_id"],
        "semantic.optional-fixed-token.nested-syntax-exclusion"
    );
    assert!(semantics.iter().all(|row| {
        row["rule_id"] != "unsupported.optional-fixed-token"
            && row["ported_shape"].as_str().is_none_or(|shape| {
                !shape.contains("WithWith")
                    && !shape.contains("WithoutWith")
                    && !shape.contains("punct ::")
                    && !shape.contains("keyword ::")
            })
    }));
    assert!(semantics.iter().any(|row| {
        row["rule_id"] == "semantic.recursa-container-transform"
            && row["ported_shape"]
                .as_str()
                .is_some_and(|shape| shape.contains("#[sep(COMMA)]"))
    }));
    let obsolete_file_rows = semantics
        .iter()
        .filter(|row| {
            let id = row["id"].as_str().unwrap();
            [
                "ast::file::PsqlDirective",
                "ast::file::PsqlCommand",
                "ast::file::FileItem",
            ]
            .iter()
            .any(|root| {
                id == *root
                    || id
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with("::"))
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(obsolete_file_rows.len(), 16);
    assert!(obsolete_file_rows.iter().all(|row| {
        row["rule_id"] == "semantic.obsolete-file-recovery-surface"
            && row["disposition"] == "reviewed_change"
            && row["ported_shape"].is_null()
    }));
    assert!(
        report["recursa_gaps"]
            .as_array()
            .unwrap()
            .iter()
            .all(|gap| !gap["examples"].as_array().unwrap().is_empty())
    );
}
