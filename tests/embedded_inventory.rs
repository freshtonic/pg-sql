//! Exact accounting for test items relocated outside Recursa discovery.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

type InventoryRow = (String, String, bool);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LegacyTest {
    id: String,
    source_path: String,
    name: String,
    ignored: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CurrentTestId {
    path: String,
    name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Disposition {
    FileRecoveryScope,
    Introduced,
    MigrationToolingScope,
    Renamed,
    RetainedIntegration,
    Superseded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Rationale {
    Adr0005FileRecovery,
    ClassifiedTokenPublicCoverage,
    ClosedWindowRefAdmission,
    CorrectedAdr0004,
    FrozenStatementSpanAdapter,
    Issue8OmittedTooling,
    Issue9GeneratedExpression,
    Issue9GeneratedStatement,
    RetainedOutsideEmbedded,
    ReviewedRename,
    StrictEntryPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DispositionRow {
    legacy_id: Option<String>,
    disposition: Disposition,
    current: Option<CurrentTestId>,
    rationale: Rationale,
}

fn immutable_legacy_inventory() -> BTreeMap<String, LegacyTest> {
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("../migration/contract/inventory.json"))
            .expect("parse immutable migration inventory");
    let ignored = contract["tests"]["ignored_tests"]
        .as_array()
        .expect("legacy ignored-test inventory")
        .iter()
        .map(|row| row["id"].as_str().expect("legacy ignored-test id"))
        .collect::<BTreeSet<_>>();

    let mut inventory = BTreeMap::new();
    let mut source_identities = BTreeSet::new();
    for row in contract["tests"]["literal_tests"]
        .as_array()
        .expect("legacy literal-test inventory")
    {
        let id = row["id"].as_str().expect("legacy test id");
        let test = LegacyTest {
            id: id.to_owned(),
            source_path: row["location"]["path"]
                .as_str()
                .expect("legacy test source path")
                .to_owned(),
            name: id
                .rsplit("::")
                .next()
                .expect("legacy test function name")
                .to_owned(),
            ignored: ignored.contains(id),
        };
        assert!(
            source_identities.insert((test.source_path.clone(), test.name.clone())),
            "duplicate immutable source test identity for {id}"
        );
        assert!(
            inventory.insert(id.to_owned(), test).is_none(),
            "duplicate immutable test id {id}"
        );
    }
    assert_eq!(
        ignored,
        inventory
            .values()
            .filter(|test| test.ignored)
            .map(|test| test.id.as_str())
            .collect(),
        "every immutable ignored test must also be a literal test"
    );
    inventory
}

fn relocated_source_path(path: &str) -> String {
    let source_path = path
        .strip_prefix("embedded-tests/")
        .expect("relocated test path below embedded-tests");
    if source_path == "src/tokens.ident_enum_tests.rs" {
        return "src/tokens.rs".to_owned();
    }
    source_path
        .strip_suffix(".tests.rs")
        .map(|prefix| format!("{prefix}.rs"))
        .expect("relocated test module has .tests.rs suffix")
}

fn current_inventory(
    rows: impl IntoIterator<Item = InventoryRow>,
) -> Result<BTreeMap<CurrentTestId, bool>, String> {
    let mut inventory = BTreeMap::new();
    for (path, name, ignored) in rows {
        let id = CurrentTestId { path, name };
        if inventory.insert(id.clone(), ignored).is_some() {
            return Err(format!("duplicate current test identity {id:?}"));
        }
    }
    Ok(inventory)
}

fn disposition_ledger(source: &str) -> Result<Vec<DispositionRow>, String> {
    let mut lines = source.lines();
    let header = lines
        .next()
        .ok_or_else(|| "empty embedded-test disposition ledger".to_owned())?;
    if header != "legacy_id\tdisposition\tcurrent_path\tcurrent_name\trationale" {
        return Err(format!("unexpected disposition-ledger header {header:?}"));
    }

    lines
        .enumerate()
        .map(|(index, line)| {
            let line_number = index + 2;
            let fields = line.split('\t').collect::<Vec<_>>();
            if fields.len() != 5 {
                return Err(format!(
                    "disposition-ledger line {line_number} has {} fields, expected 5",
                    fields.len()
                ));
            }
            let legacy_id = (fields[0] != "-").then(|| fields[0].to_owned());
            let disposition = match fields[1] {
                "file-recovery-scope" => Disposition::FileRecoveryScope,
                "introduced" => Disposition::Introduced,
                "migration-tooling-scope" => Disposition::MigrationToolingScope,
                "renamed" => Disposition::Renamed,
                "retained-integration" => Disposition::RetainedIntegration,
                "superseded" => Disposition::Superseded,
                other => {
                    return Err(format!(
                        "unknown disposition {other:?} at ledger line {line_number}"
                    ));
                }
            };
            let current = match (fields[2], fields[3]) {
                ("-", "-") => None,
                ("-", _) | (_, "-") => {
                    return Err(format!(
                        "partial current test identity at ledger line {line_number}"
                    ));
                }
                (path, name) => Some(CurrentTestId {
                    path: path.to_owned(),
                    name: name.to_owned(),
                }),
            };
            let rationale = match fields[4] {
                "adr-0005-file-recovery" => Rationale::Adr0005FileRecovery,
                "classified-token-public-coverage" => Rationale::ClassifiedTokenPublicCoverage,
                "closed-window-ref-admission" => Rationale::ClosedWindowRefAdmission,
                "corrected-adr-0004" => Rationale::CorrectedAdr0004,
                "frozen-statement-span-adapter" => Rationale::FrozenStatementSpanAdapter,
                "issue-8-omitted-tooling" => Rationale::Issue8OmittedTooling,
                "issue-9-generated-expression" => Rationale::Issue9GeneratedExpression,
                "issue-9-generated-statement" => Rationale::Issue9GeneratedStatement,
                "retained-outside-embedded" => Rationale::RetainedOutsideEmbedded,
                "reviewed-rename" => Rationale::ReviewedRename,
                "strict-entry-point" => Rationale::StrictEntryPoint,
                other => {
                    return Err(format!(
                        "unknown rationale {other:?} at ledger line {line_number}"
                    ));
                }
            };
            Ok(DispositionRow {
                legacy_id,
                disposition,
                current,
                rationale,
            })
        })
        .collect()
}

fn expected_inventory() -> BTreeSet<InventoryRow> {
    let mut inventory = BTreeSet::new();
    for (index, line) in include_str!("../embedded-tests/inventory.tsv")
        .lines()
        .enumerate()
    {
        let mut fields = line.split('\t');
        let path = fields.next().expect("inventory path").to_owned();
        let name = fields.next().expect("inventory test name").to_owned();
        let ignored = fields
            .next()
            .expect("inventory ignored status")
            .parse()
            .expect("boolean ignored status");
        assert!(fields.next().is_none(), "three inventory fields");
        assert!(
            inventory.insert((path, name, ignored)),
            "duplicate embedded-test inventory row at line {}",
            index + 1
        );
    }
    inventory
}

fn rust_files_below(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }
    files
}

fn discovered_test_modules(root: &Path) -> BTreeSet<String> {
    rust_files_below(&root.join("embedded-tests"))
        .into_iter()
        .filter(|path| !test_rows(root, path).is_empty())
        .map(|path| {
            path.strip_prefix(root)
                .expect("embedded test below repository root")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

fn included_test_modules(root: &Path, discovered: &BTreeSet<String>) -> BTreeMap<String, usize> {
    let mut included = BTreeMap::new();
    for path in rust_files_below(&root.join("tests/embedded")) {
        let source = fs::read_to_string(&path).expect("read Rust source");
        let parsed = syn::parse_file(&source).expect("parse Rust source");
        let mut pending = vec![parsed.items.as_slice()];
        while let Some(items) = pending.pop() {
            for item in items {
                if let syn::Item::Mod(module) = item
                    && let Some((_, items)) = &module.content
                {
                    pending.push(items);
                }
                let syn::Item::Macro(item) = item else {
                    continue;
                };
                if !item.mac.path.is_ident("include") {
                    continue;
                }
                let arguments = item.mac.tokens.to_string();
                for test_path in discovered {
                    if arguments.contains(test_path) {
                        *included.entry(test_path.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    included
}

fn test_rows(root: &Path, path: &Path) -> BTreeSet<InventoryRow> {
    let inventory_path = path
        .strip_prefix(root)
        .expect("embedded test below repository root")
        .to_string_lossy()
        .into_owned();
    let source = fs::read_to_string(path).expect("read relocated test module");
    let parsed = syn::parse_file(&source).expect("parse relocated test module");
    let mut rows = BTreeSet::new();
    let mut pending = vec![parsed.items.as_slice()];
    while let Some(items) = pending.pop() {
        for item in items {
            if let syn::Item::Mod(module) = item
                && let Some((_, items)) = &module.content
            {
                pending.push(items);
            }
            if let syn::Item::Fn(function) = item {
                if !function
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("test"))
                {
                    continue;
                }
                let ignored = function
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("ignore"));
                rows.insert((
                    inventory_path.clone(),
                    function.sig.ident.to_string(),
                    ignored,
                ));
            }
        }
    }
    rows
}

fn actual_inventory(root: &Path, paths: &BTreeSet<String>) -> BTreeSet<InventoryRow> {
    let mut actual = BTreeSet::new();
    for path in paths {
        actual.extend(test_rows(root, &root.join(path)));
    }
    actual
}

fn source_contains_test_template(source: &str) -> bool {
    let parsed = syn::parse_file(source).expect("parse integration test source");
    let mut pending = vec![parsed.items.as_slice()];
    while let Some(items) = pending.pop() {
        for item in items {
            if let syn::Item::Mod(module) = item
                && let Some((_, items)) = &module.content
            {
                pending.push(items);
            }
            if let syn::Item::Macro(item) = item {
                let tokens = item.mac.tokens.to_string().replace(' ', "");
                if tokens.contains("#[test]fn$name") {
                    return true;
                }
            }
        }
    }
    false
}

fn retained_integration_tests(
    root: &Path,
    legacy: &BTreeMap<String, LegacyTest>,
) -> BTreeSet<String> {
    legacy
        .values()
        .filter(|test| test.source_path.starts_with("tests/"))
        .filter(|test| {
            let path = root.join(&test.source_path);
            if !path.is_file() {
                return false;
            }
            if test.name == "$template" {
                return source_contains_test_template(
                    &fs::read_to_string(path).expect("read integration test source"),
                );
            }
            test_rows(root, &path)
                .iter()
                .any(|(_, name, ignored)| name == &test.name && *ignored == test.ignored)
        })
        .map(|test| test.id.clone())
        .collect()
}

fn omitted_migration_paths() -> BTreeSet<String> {
    let execution: serde_json::Value =
        serde_json::from_str(include_str!("../migration/execution.json"))
            .expect("parse immutable migration execution record");
    execution["output"]["omitted_paths"]
        .as_array()
        .expect("migration omitted paths")
        .iter()
        .map(|path| {
            path.as_str()
                .expect("migration omitted path string")
                .to_owned()
        })
        .collect()
}

fn validate_reconciliation(
    legacy: &BTreeMap<String, LegacyTest>,
    current: &BTreeMap<CurrentTestId, bool>,
    ledger: &[DispositionRow],
    omitted_paths: &BTreeSet<String>,
    retained_integrations: &BTreeSet<String>,
) -> Result<(), String> {
    let legacy_by_source = legacy
        .values()
        .map(|test| {
            (
                (test.source_path.clone(), test.name.clone()),
                test.id.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if legacy_by_source.len() != legacy.len() {
        return Err("duplicate immutable source test identity".to_owned());
    }

    let mut current_by_source = BTreeMap::new();
    for (id, ignored) in current {
        let key = (relocated_source_path(&id.path), id.name.clone());
        if current_by_source.insert(key, (id, ignored)).is_some() {
            return Err(format!("duplicate relocated source test identity {id:?}"));
        }
    }

    let mut directly_preserved_legacy = BTreeSet::new();
    let mut directly_preserved_current = BTreeSet::new();
    let mut expected_ignored_current = BTreeSet::new();
    for (source_identity, legacy_id) in &legacy_by_source {
        let Some((current_id, current_ignored)) = current_by_source.get(source_identity) else {
            continue;
        };
        let legacy_test = &legacy[legacy_id];
        if legacy_test.ignored != **current_ignored {
            return Err(format!(
                "ignored status drift for directly preserved test {legacy_id}"
            ));
        }
        directly_preserved_legacy.insert(legacy_id.clone());
        directly_preserved_current.insert((*current_id).clone());
        if legacy_test.ignored {
            expected_ignored_current.insert((*current_id).clone());
        }
    }

    let legacy_only = legacy
        .keys()
        .filter(|id| !directly_preserved_legacy.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let current_only = current
        .keys()
        .filter(|id| !directly_preserved_current.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut accounted_legacy = BTreeSet::new();
    let mut accounted_current = BTreeSet::new();
    for row in ledger {
        if let Some(legacy_id) = &row.legacy_id {
            if !legacy_only.contains(legacy_id) {
                return Err(format!(
                    "unknown or directly preserved legacy ledger source {legacy_id}"
                ));
            }
            if !accounted_legacy.insert(legacy_id.clone()) {
                return Err(format!("duplicate legacy ledger source {legacy_id}"));
            }
        }
        if let Some(current_id) = &row.current {
            if !current_only.contains(current_id) {
                return Err(format!(
                    "dangling or directly preserved current target {current_id:?}"
                ));
            }
            if !accounted_current.insert(current_id.clone()) {
                return Err(format!("duplicate current ledger target {current_id:?}"));
            }
        }

        validate_disposition_rationale(row)?;

        match row.disposition {
            Disposition::Introduced => {
                if row.legacy_id.is_some() || row.current.is_none() {
                    return Err("introduced rows require only a current target".to_owned());
                }
            }
            Disposition::Renamed => {
                let (Some(legacy_id), Some(current_id)) = (&row.legacy_id, &row.current) else {
                    return Err("renamed rows require legacy and current identities".to_owned());
                };
                if legacy[legacy_id].ignored != current[current_id] {
                    return Err(format!("ignored status drift across rename {legacy_id}"));
                }
                if legacy[legacy_id].ignored {
                    expected_ignored_current.insert(current_id.clone());
                }
            }
            Disposition::Superseded => {
                if row.legacy_id.is_none() {
                    return Err("superseded rows require a legacy identity".to_owned());
                }
                if legacy[row.legacy_id.as_ref().expect("checked above")].ignored {
                    return Err("an ignored legacy test may not be superseded".to_owned());
                }
            }
            Disposition::FileRecoveryScope => {
                let Some(legacy_id) = &row.legacy_id else {
                    return Err("file-recovery-scope rows require a legacy identity".to_owned());
                };
                if row.current.is_some() || legacy[legacy_id].source_path != "src/ast/file.rs" {
                    return Err(format!(
                        "file-recovery-scope row is not a target-free src/ast/file.rs test: {legacy_id}"
                    ));
                }
            }
            Disposition::MigrationToolingScope => {
                let Some(legacy_id) = &row.legacy_id else {
                    return Err("migration-tooling-scope rows require a legacy identity".to_owned());
                };
                if row.current.is_some() || !omitted_paths.contains(&legacy[legacy_id].source_path)
                {
                    return Err(format!(
                        "migration-tooling-scope source was not omitted by issue 8: {legacy_id}"
                    ));
                }
            }
            Disposition::RetainedIntegration => {
                let Some(legacy_id) = &row.legacy_id else {
                    return Err("retained-integration rows require a legacy identity".to_owned());
                };
                if row.current.is_some() || !retained_integrations.contains(legacy_id) {
                    return Err(format!(
                        "retained integration test is absent or changed: {legacy_id}"
                    ));
                }
                if legacy[legacy_id].ignored {
                    return Err("ignored tests must remain in the relocated inventory".to_owned());
                }
            }
        }
    }

    if accounted_legacy != legacy_only {
        let missing = legacy_only
            .difference(&accounted_legacy)
            .take(20)
            .collect::<Vec<_>>();
        return Err(format!("legacy dispositions missing: {missing:?}"));
    }
    if accounted_current != current_only {
        let missing = current_only
            .difference(&accounted_current)
            .take(20)
            .collect::<Vec<_>>();
        return Err(format!("current dispositions missing: {missing:?}"));
    }

    let actual_ignored_current = current
        .iter()
        .filter(|(_, ignored)| **ignored)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    if actual_ignored_current != expected_ignored_current {
        return Err(format!(
            "relocated ignored-test inventory drift: expected={expected_ignored_current:?}, actual={actual_ignored_current:?}"
        ));
    }
    Ok(())
}

fn validate_disposition_rationale(row: &DispositionRow) -> Result<(), String> {
    let expected = match row.disposition {
        Disposition::FileRecoveryScope => Rationale::Adr0005FileRecovery,
        Disposition::MigrationToolingScope => Rationale::Issue8OmittedTooling,
        Disposition::RetainedIntegration => Rationale::RetainedOutsideEmbedded,
        Disposition::Renamed => Rationale::ReviewedRename,
        Disposition::Introduced => {
            if row.current.as_ref().is_some_and(|current| {
                current.path == "embedded-tests/src/ast/shared/expr.tests.rs"
            }) {
                Rationale::Issue9GeneratedExpression
            } else {
                Rationale::Issue9GeneratedStatement
            }
        }
        Disposition::Superseded => match row.legacy_id.as_deref() {
            Some("ast::tests::parse_psql_command_directive")
            | Some("ast::tests::parse_psql_command_statement") => Rationale::StrictEntryPoint,
            Some("ast::shared::expr::tests::reject_string_continuation_across_comment") => {
                Rationale::CorrectedAdr0004
            }
            Some("tokens::tests::token_kind_is_soft_classifies_correctly") => {
                Rationale::ClassifiedTokenPublicCoverage
            }
            Some("tokens::tests::window_ref_name_rejects_frame_units") => {
                Rationale::ClosedWindowRefAdmission
            }
            Some("tests::support::tests::extracts_sql_skips_directives")
            | Some("tests::support::tests::skips_psql_interpolation") => {
                Rationale::FrozenStatementSpanAdapter
            }
            _ => {
                return Err(format!(
                    "superseded row has no reviewed rationale mapping: {:?}",
                    row.legacy_id
                ));
            }
        },
    };
    if row.rationale != expected {
        return Err(format!(
            "rationale {:?} does not match reviewed {:?} disposition {:?}",
            row.rationale, expected, row.disposition
        ));
    }
    Ok(())
}

#[test]
fn all_imported_embedded_tests_and_ignored_statuses_are_accounted_for() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected = expected_inventory();
    let expected_paths = expected
        .iter()
        .map(|(path, _, _)| path.clone())
        .collect::<BTreeSet<_>>();
    let discovered_paths = discovered_test_modules(root);
    let included_paths = included_test_modules(root, &discovered_paths);
    let actual = actual_inventory(root, &discovered_paths);

    assert_eq!(expected.len(), 1_098);
    assert_eq!(discovered_paths, expected_paths);
    assert_eq!(
        included_paths,
        discovered_paths
            .iter()
            .map(|path| (path.clone(), 1))
            .collect::<BTreeMap<_, _>>(),
        "every relocated test module must be included exactly once from the embedded target"
    );
    let missing = expected.difference(&actual).take(20).collect::<Vec<_>>();
    let unexpected = actual.difference(&expected).take(20).collect::<Vec<_>>();
    assert!(
        actual == expected,
        "embedded inventory mismatch: missing={missing:?}, unexpected={unexpected:?}"
    );
    assert_eq!(
        actual
            .iter()
            .filter(|(_, _, ignored)| *ignored)
            .map(|(_, name, _)| name.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "parse_select_func_table_bare_alias_col_def",
            "report_ast_sizes",
        ])
    );
}

#[test]
fn every_frozen_legacy_test_and_new_relocated_test_has_a_disposition() {
    let legacy = immutable_legacy_inventory();
    let current = current_inventory(expected_inventory()).expect("unique current test inventory");
    let ledger = disposition_ledger(include_str!("../embedded-tests/reconciliation.tsv"))
        .expect("valid embedded-test disposition ledger");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    assert_eq!(legacy.len(), 1_318);
    assert_eq!(current.len(), 1_098);
    validate_reconciliation(
        &legacy,
        &current,
        &ledger,
        &omitted_migration_paths(),
        &retained_integration_tests(root, &legacy),
    )
    .unwrap_or_else(|error| panic!("embedded-test reconciliation failed: {error}"));
}

fn one_legacy_test(id: &str, source_path: &str, name: &str, ignored: bool) -> LegacyTest {
    LegacyTest {
        id: id.to_owned(),
        source_path: source_path.to_owned(),
        name: name.to_owned(),
        ignored,
    }
}

fn introduced_row(path: &str, name: &str) -> DispositionRow {
    DispositionRow {
        legacy_id: None,
        disposition: Disposition::Introduced,
        current: Some(CurrentTestId {
            path: path.to_owned(),
            name: name.to_owned(),
        }),
        rationale: if path == "embedded-tests/src/ast/shared/expr.tests.rs" {
            Rationale::Issue9GeneratedExpression
        } else {
            Rationale::Issue9GeneratedStatement
        },
    }
}

#[test]
fn reconciliation_rejects_omitted_legacy_and_current_identities() {
    let legacy_test = one_legacy_test("ast::x::tests::old", "src/ast/x.rs", "old", false);
    let legacy = BTreeMap::from([(legacy_test.id.clone(), legacy_test)]);
    let current = current_inventory([(
        "embedded-tests/src/ast/x.tests.rs".to_owned(),
        "new".to_owned(),
        false,
    )])
    .unwrap();

    let error = validate_reconciliation(&legacy, &current, &[], &BTreeSet::new(), &BTreeSet::new())
        .unwrap_err();
    assert!(error.contains("legacy dispositions missing"), "{error}");

    let error = validate_reconciliation(
        &BTreeMap::new(),
        &current,
        &[],
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.contains("current dispositions missing"), "{error}");
}

#[test]
fn reconciliation_rejects_duplicate_sources_and_dangling_targets() {
    let legacy_test = one_legacy_test(
        "ast::tests::parse_psql_command_directive",
        "src/ast/mod.rs",
        "parse_psql_command_directive",
        false,
    );
    let legacy = BTreeMap::from([(legacy_test.id.clone(), legacy_test)]);
    let superseded = DispositionRow {
        legacy_id: Some("ast::tests::parse_psql_command_directive".to_owned()),
        disposition: Disposition::Superseded,
        current: None,
        rationale: Rationale::StrictEntryPoint,
    };
    let error = validate_reconciliation(
        &legacy,
        &BTreeMap::new(),
        &[superseded.clone(), superseded],
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.contains("duplicate legacy ledger source"), "{error}");

    let dangling = introduced_row("embedded-tests/src/ast/x.tests.rs", "absent");
    let error = validate_reconciliation(
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[dangling],
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.contains("dangling"), "{error}");

    let current = current_inventory([(
        "embedded-tests/src/ast/x.tests.rs".to_owned(),
        "new".to_owned(),
        false,
    )])
    .unwrap();
    let introduced = introduced_row("embedded-tests/src/ast/x.tests.rs", "new");
    let error = validate_reconciliation(
        &BTreeMap::new(),
        &current,
        &[introduced.clone(), introduced],
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.contains("duplicate current ledger target"), "{error}");
}

#[test]
fn reconciliation_rejects_unknown_and_mismatched_rationales() {
    let error = disposition_ledger(
        "legacy_id\tdisposition\tcurrent_path\tcurrent_name\trationale\n-\tintroduced\tembedded-tests/src/ast/x.tests.rs\tnew\tarbitrary-text\n",
    )
    .unwrap_err();
    assert!(error.contains("unknown rationale"), "{error}");

    let current = current_inventory([(
        "embedded-tests/src/ast/shared/expr.tests.rs".to_owned(),
        "new".to_owned(),
        false,
    )])
    .unwrap();
    let mut introduced = introduced_row("embedded-tests/src/ast/shared/expr.tests.rs", "new");
    introduced.rationale = Rationale::Issue9GeneratedStatement;
    let error = validate_reconciliation(
        &BTreeMap::new(),
        &current,
        &[introduced],
        &BTreeSet::new(),
        &BTreeSet::new(),
    )
    .unwrap_err();
    assert!(error.contains("does not match reviewed"), "{error}");
}

#[test]
fn reconciliation_preserves_ignored_status_by_immutable_identity() {
    let legacy_test = one_legacy_test("ast::x::tests::same", "src/ast/x.rs", "same", true);
    let legacy = BTreeMap::from([(legacy_test.id.clone(), legacy_test)]);
    let current = current_inventory([(
        "embedded-tests/src/ast/x.tests.rs".to_owned(),
        "same".to_owned(),
        false,
    )])
    .unwrap();
    let error = validate_reconciliation(&legacy, &current, &[], &BTreeSet::new(), &BTreeSet::new())
        .unwrap_err();
    assert!(error.contains("ignored status drift"), "{error}");
}
