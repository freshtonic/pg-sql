use std::fs;

use pg_sql_migrate::{Mapping, inventory, to_canonical_inventory_json};

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
                    == Some("#[surrounded(LPAREN, this, RPAREN)] Ident < 'input >"))
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
                && row.rule_id == "semantic.optional-fixed-token.sign-enum")
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
                == Some("or_replace: bool; OR REPLACE syntax moves to #[kwd] attributes")
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
fn checked_in_repository_matches_the_reviewed_contract() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let report = inventory(root, &Mapping::migration_contract()).unwrap();
    assert_eq!(
        report.provenance.legacy_commit,
        "1e71421d66baac15c8c5264e8f29b5f80122f50e"
    );
    assert_eq!(
        report.provenance.legacy_tree,
        "f3191ab707c8a957d1bb5fe142e74fc624fe6661"
    );
    assert_eq!(
        report.provenance.pg_sql_tree,
        "50e1376d16796e5f05db88d99dab42252a9f78a4"
    );
    assert_eq!(
        report.provenance.pg_oracle_tree,
        "0780d057e4d54db150d0f388c45a720a825bcbcf"
    );
    assert_eq!(
        report.provenance.postgres_gitlink,
        "6d396980fc5aed4f1a525e0bd75cb16b25ed40ca"
    );
    assert_eq!(
        report.provenance.source_checkpoint,
        "e97d3c3570c2a04ca9a233334b46d3f443800a5a"
    );
    assert_eq!(report.summary.expanded_tests, 1_539);
    assert_eq!(report.summary.file_recovery_sites, 238);
    assert_eq!(report.tests.corpus_fixtures.len(), 222);
    assert_eq!(report.tests.formatter_goldens.len(), 10);
    let oracle = report
        .tests
        .workspace_members
        .iter()
        .find(|member| member.member == "pg-oracle")
        .unwrap();
    assert_eq!(oracle.tests.len(), 3);
    assert!(
        report
            .tests
            .corpus_fixtures
            .iter()
            .all(|fixture| fixture.content_sha256.is_some())
    );
    assert!(report.semantics.len() > 5_000);
    assert!(
        report
            .semantics
            .iter()
            .any(|row| row.id == "ast::file::FileItem")
    );
    assert!(
        report
            .semantics
            .iter()
            .any(|row| row.id == "ast::shared::expr::StringLitSeq0")
    );
    assert!(report.semantics.iter().any(|row| {
        row.id == "ast::shared::expr::StringLitSeq0.parts"
            && row.rule_id == "semantic.recursa-container-transform"
    }));
    assert!(report.semantics.iter().any(|row| {
        row.id == "ast::ddl::function::ExtractedFuncBody" && row.kind == "semantic_view"
    }));
    assert_eq!(
        report
            .semantics
            .iter()
            .filter(|row| {
                row.rule_id.starts_with("semantic.optional-fixed-token")
                    || row.rule_id.starts_with("syntax.optional-fixed-token")
            })
            .count(),
        132
    );
    assert!(report.semantics.iter().any(|row| {
        row.id == "ast::ddl::database::CreateDatabaseStmt.with"
            && row.rule_id == "syntax.optional-fixed-token"
            && row.ported_shape.is_none()
    }));
    assert!(report.semantics.iter().any(|row| {
        row.id == "ast::ddl::index::CreateIndexStmt.unique"
            && row.rule_id == "semantic.optional-fixed-token.bool"
    }));
    assert!(report.semantics.iter().any(|row| {
        row.id == "ast::ddl::sequence::SeqRestartOption.with"
            && row.rule_id == "semantic.optional-fixed-token.nested-syntax-exclusion"
    }));
    assert!(report.semantics.iter().all(|row| {
        row.rule_id != "unsupported.optional-fixed-token"
            && row
                .ported_shape
                .as_deref()
                .is_none_or(|shape| !shape.contains("WithWith") && !shape.contains("WithoutWith"))
    }));
    assert!(report.semantics.iter().all(|row| {
        row.ported_shape
            .as_ref()
            .is_none_or(|shape| !shape.contains("punct ::") && !shape.contains("keyword ::"))
    }));
    assert!(report.semantics.iter().any(|row| {
        row.rule_id == "semantic.recursa-container-transform"
            && row
                .ported_shape
                .as_deref()
                .is_some_and(|shape| shape.contains("#[sep(COMMA)]"))
    }));
    assert!(
        report
            .recursa_gaps
            .iter()
            .all(|gap| !gap.examples.is_empty())
    );
    let checked_in = fs::read_to_string(root.join("migration/contract/inventory.json")).unwrap();
    assert_eq!(to_canonical_inventory_json(&report).unwrap(), checked_in);
}
