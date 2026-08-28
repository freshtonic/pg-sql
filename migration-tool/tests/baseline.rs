use pg_sql_migrate::baseline::{
    Baseline, BuildFixture, CaptureBuild, CaptureCommands, CaptureOptions, Corpus, FileOutcome,
    Identity, OutcomeCounts, PostgresIdentity, capture_baseline, discover_corpus,
    install_out_of_tree_build_plumbing, parse_transcript, to_canonical_json, validate_baseline,
    write_baseline,
};
use std::fs;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "pg-sql-baseline-test-{}-{name}",
        std::process::id()
    ));
    if path.exists() {
        fs::remove_dir_all(&path).unwrap();
    }
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn discovery_freezes_the_legacy_macro_membership_and_explains_exclusions() {
    let root = fixture_dir("discovery");
    let sql = root.join("vendor/postgres/src/test/regress/sql");
    fs::create_dir_all(&sql).unwrap();
    for name in ["select.sql", "async.sql", "collate.utf8.sql"] {
        fs::write(sql.join(name), "SELECT 1;").unwrap();
    }
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/differential.rs"),
        "macro_rules! corpus_tests { ($($name:ident),*) => {} }\n\
         corpus_tests! { select, r#async, }\n",
    )
    .unwrap();

    let membership = discover_corpus(&root).unwrap();

    assert_eq!(membership.included, vec!["async.sql", "select.sql"]);
    assert_eq!(membership.excluded.len(), 1);
    assert_eq!(membership.excluded[0].file, "collate.utf8.sql");
    assert_eq!(
        membership.excluded[0].reason,
        "legacy differential suite omitted fixture names that are not Rust identifiers"
    );
}

#[test]
fn discovery_rejects_a_new_undeclared_fixture_that_the_legacy_rule_would_include() {
    let root = fixture_dir("membership-drift");
    let sql = root.join("vendor/postgres/src/test/regress/sql");
    fs::create_dir_all(&sql).unwrap();
    fs::write(sql.join("select.sql"), "SELECT 1;").unwrap();
    fs::write(sql.join("new_fixture.sql"), "SELECT 2;").unwrap();
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/differential.rs"),
        "corpus_tests! { select, }\n",
    )
    .unwrap();

    let error = discover_corpus(&root).unwrap_err();

    assert!(error.to_string().contains("unexpectedly omits"));
    assert!(error.to_string().contains("new_fixture.sql"));
}

#[test]
fn transcript_records_statement_outcomes_independent_of_test_harness_noise() {
    let transcript = "running 2 tests\n\
        [select] pass=7 skip=2 fail=0\n\
        test select ... ok\n\
        warning: unrelated\n\
        [async] pass=3 skip=0 fail=1\n";

    assert_eq!(
        parse_transcript(transcript).unwrap(),
        vec![
            FileOutcome::new("async.sql", 3, 0, 1),
            FileOutcome::new("select.sql", 7, 2, 0),
        ]
    );
}

#[test]
fn validation_rejects_a_capture_that_does_not_cover_every_included_file() {
    let baseline = sample_baseline(vec![FileOutcome::new("select.sql", 7, 2, 0)]);

    let error = validate_baseline(&baseline).unwrap_err();

    assert!(error.to_string().contains("async.sql has no outcome"));
}

#[test]
fn canonical_serialization_is_byte_stable_and_ends_with_one_newline() {
    let baseline = sample_baseline(vec![
        FileOutcome::new("select.sql", 7, 2, 0),
        FileOutcome::new("async.sql", 3, 0, 1),
    ]);

    let first = to_canonical_json(&baseline).unwrap();
    let second = to_canonical_json(&baseline).unwrap();

    assert_eq!(first, second);
    assert!(first.ends_with("\n"));
    assert!(!first.ends_with("\n\n"));
    assert!(first.find("async.sql").unwrap() < first.find("select.sql").unwrap());
    assert!(first.contains("\"statements\": 4"));
    assert!(first.contains("\"pass\": 10"));
    assert!(first.contains("\"skip\": 2"));
    assert!(first.contains("\"fail\": 1"));
}

#[test]
fn disposable_build_plumbing_routes_all_postgresql_outputs_out_of_tree() {
    let root = fixture_dir("out-of-tree-build");
    fs::create_dir_all(root.join("pg-oracle/scripts")).unwrap();
    fs::write(root.join("pg-oracle/build.rs"), "legacy build").unwrap();
    fs::write(
        root.join("pg-oracle/scripts/build-pg.sh"),
        "legacy build script",
    )
    .unwrap();

    install_out_of_tree_build_plumbing(&root).unwrap();

    let build_rs = fs::read_to_string(root.join("pg-oracle/build.rs")).unwrap();
    let build_script = fs::read_to_string(root.join("pg-oracle/scripts/build-pg.sh")).unwrap();
    assert!(build_rs.contains("var_os(\"OUT_DIR\")"));
    assert!(build_rs.contains("verify_pg_built(&pg_source, &pg_build)"));
    assert!(build_script.contains("make -C \"$PG_BUILD\""));
    assert!(!build_script.contains("make -C \"$PG_SRC\""));
    assert!(!build_script.contains("cd \"$PG_SRC\" &&"));
}

#[test]
#[ignore = "manual capture from pinned local repositories"]
fn capture_from_pinned_local_repositories() {
    let legacy = std::env::var_os("PG_SQL_LEGACY_REPOSITORY")
        .expect("set PG_SQL_LEGACY_REPOSITORY to the legacy Recursa clone");
    let postgres = std::env::var_os("PG_SQL_POSTGRES_REPOSITORY")
        .expect("set PG_SQL_POSTGRES_REPOSITORY to a PostgreSQL 17.9 clone");
    let output = std::env::var_os("PG_SQL_BASELINE_OUTPUT")
        .expect("set PG_SQL_BASELINE_OUTPUT to the baseline JSON path");

    let baseline = capture_baseline(&CaptureOptions::new(legacy, postgres)).unwrap();
    write_baseline(std::path::Path::new(&output), &baseline).unwrap();
}

fn sample_baseline(files: Vec<FileOutcome>) -> Baseline {
    Baseline {
        schema_version: 1,
        name: "postgresql-17.9".into(),
        legacy: Identity {
            commit: "1e71421d66baac15c8c5264e8f29b5f80122f50e".into(),
            tree: "f3191ab707c8a957d1bb5fe142e74fc624fe6661".into(),
            pg_sql_tree: "50e1376d16796e5f05db88d99dab42252a9f78a4".into(),
        },
        postgres: PostgresIdentity {
            release: "17.9".into(),
            gitlink: "6d396980fc5aed4f1a525e0bd75cb16b25ed40ca".into(),
        },
        corpus: Corpus {
            root: "vendor/postgres/src/test/regress/sql".into(),
            inclusion_rule:
                "exact file list declared by corpus_tests! in the legacy differential suite".into(),
            skip_rules: vec![
                "psql directives and COPY-from-stdin payloads are non-SQL".into(),
                "statements containing psql variable interpolation are not standalone SQL".into(),
                "a whole-file legacy parse failure yields no statements".into(),
                "PostgreSQL-accepted statements the legacy parser cannot model are skips".into(),
            ],
            available_files: 2,
            included: vec!["async.sql".into(), "select.sql".into()],
            excluded: vec![],
            files,
            total_statements: 13,
            totals: OutcomeCounts::default(),
        },
        capture_build: CaptureBuild {
            strategy: "out of tree".into(),
            fixtures: vec![BuildFixture {
                file: "fixture".into(),
                sha256: "abc".into(),
            }],
        },
        commands: CaptureCommands {
            review: "cargo run -p pg-sql-migrate -- baseline review".into(),
            update: "cargo run -p pg-sql-migrate -- baseline capture".into(),
        },
    }
}
