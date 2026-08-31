//! Auditable tooling for the reproducible PostgreSQL grammar migration.

pub mod baseline;
pub mod execution;
pub mod grammar_rewrite;
pub mod migration_contract;
pub mod rewrite;
pub mod statement_spans;
pub mod test_call_rewrite;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::{LineColumn, Span, TokenStream, TokenTree};
use quote::ToTokens;
use serde::Serialize;
use sha2::{Digest, Sha256};
use syn::spanned::Spanned;
use syn::{Attribute, Fields, Item, ItemEnum, ItemStruct, ItemType};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct Mapping {
    pub expected_parser_types: Option<usize>,
    pub expected_ast_types: Option<usize>,
    pub expected_parse_roles: Option<usize>,
    pub expected_pratt_enums: Option<usize>,
    pub expected_handwritten_parsers: Option<usize>,
    pub expected_literal_tests: Option<usize>,
    pub expected_ignored_tests: Option<usize>,
    pub expected_corpus_tests: Option<usize>,
    pub expected_file_recovery_sites: Option<usize>,
    pub expected_file_recovery_fixtures: Option<usize>,
    pub expected_formatter_pairs: Option<usize>,
    pub expected_stress_workloads: Option<usize>,
    pub expected_token_counts: Option<BTreeMap<String, usize>>,
    pub expected_semantic_types: Option<usize>,
    pub expected_workspace_members: Option<BTreeMap<String, WorkspaceTestCounts>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceTestCounts {
    /// All `#[test]` functions, including ignored tests.
    pub tests: usize,
    /// The subset of `tests` carrying `#[ignore]`.
    pub ignored: usize,
}

impl Mapping {
    /// The reviewed legacy contract. Count changes require changing this method.
    pub fn migration_contract() -> Self {
        let expected_token_counts = [
            ("token_categories", 4),
            ("token_flags", 1),
            ("token_keywords", 87),
            ("token_soft_keywords", 397),
            ("token_punctuation", 99),
            ("token_literals", 12),
            ("token_lexer_tokens", 3),
            ("token_classes", 1),
            ("token_targets", 5),
            ("token_callbacks", 5),
        ]
        .into_iter()
        .map(|(kind, count)| (kind.into(), count))
        .collect();
        Self {
            expected_parser_types: Some(1_267),
            expected_ast_types: Some(4),
            expected_parse_roles: Some(178),
            expected_pratt_enums: Some(1),
            expected_handwritten_parsers: Some(5),
            expected_literal_tests: Some(1_318),
            expected_ignored_tests: Some(2),
            expected_corpus_tests: Some(222),
            expected_file_recovery_sites: Some(238),
            expected_file_recovery_fixtures: Some(222),
            expected_formatter_pairs: Some(10),
            expected_stress_workloads: Some(15),
            expected_token_counts: Some(expected_token_counts),
            expected_semantic_types: Some(1_273),
            expected_workspace_members: Some(BTreeMap::from([
                (
                    "migration-tool".into(),
                    WorkspaceTestCounts {
                        tests: 46,
                        ignored: 1,
                    },
                ),
                (
                    "pg-oracle".into(),
                    WorkspaceTestCounts {
                        tests: 3,
                        ignored: 0,
                    },
                ),
            ])),
        }
    }
}

#[derive(Debug)]
pub struct InventoryError(String);

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InventoryError {}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    pub legacy_commit: String,
    pub legacy_tree: String,
    pub pg_sql_tree: String,
    pub pg_oracle_tree: String,
    pub postgres_gitlink: String,
    pub source_checkpoint: String,
    pub generated_excluded: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GrammarRow {
    pub id: String,
    pub kind: String,
    pub location: SourceLocation,
    pub detail: String,
    pub supported_by_current_recursa: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    PortedEquivalent,
    ReviewedChange,
    SyntaxOnlyExclusion,
    FrameworkExclusion,
    RecursaGap,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticRow {
    pub id: String,
    pub kind: String,
    pub location: SourceLocation,
    pub legacy_shape: String,
    pub disposition: Disposition,
    pub rule_id: String,
    pub ported_shape: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetRow {
    pub id: String,
    pub location: SourceLocation,
    pub contract: String,
    pub content_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TestInventory {
    pub literal_tests: Vec<AssetRow>,
    pub ignored_tests: Vec<AssetRow>,
    pub corpus_tests: Vec<AssetRow>,
    pub corpus_fixtures: Vec<AssetRow>,
    pub corpus_exclusions: Vec<AssetRow>,
    pub file_recovery_sites: Vec<AssetRow>,
    pub file_recovery_fixtures: Vec<AssetRow>,
    pub formatter_pairs: Vec<AssetRow>,
    pub formatter_goldens: Vec<AssetRow>,
    pub stress_workloads: Vec<AssetRow>,
    pub benchmark_sources: Vec<AssetRow>,
    pub workspace_members: Vec<WorkspaceMemberTests>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceMemberTests {
    pub member: String,
    pub tests: Vec<AssetRow>,
    pub ignored_tests: Vec<AssetRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Exclusion {
    pub id: String,
    pub legacy_location: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gap {
    pub id: String,
    pub design_session: String,
    pub capability: String,
    pub design_notes: Option<AssetRow>,
    pub examples: Vec<AssetRow>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Summary {
    pub parser_types: usize,
    pub ast_types: usize,
    pub parse_roles: usize,
    pub pratt_enums: usize,
    pub handwritten_parsers: usize,
    pub grammar_rows: usize,
    pub semantic_rows: usize,
    pub semantic_types: usize,
    pub literal_tests: usize,
    pub expanded_tests: usize,
    pub ignored_tests: usize,
    pub file_recovery_sites: usize,
    pub fixtures: usize,
    pub benchmark_workloads: usize,
    pub unsupported_cases: usize,
    pub workspace_member_tests: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryReport {
    pub provenance: Provenance,
    pub summary: Summary,
    pub grammar: Vec<GrammarRow>,
    pub semantics: Vec<SemanticRow>,
    pub tests: TestInventory,
    pub obsolete_artifacts: Vec<Exclusion>,
    pub recursa_gaps: Vec<Gap>,
}

/// Build a complete report using reads only. It never creates, updates, or removes files.
pub fn inventory(root: &Path, mapping: &Mapping) -> Result<InventoryReport, InventoryError> {
    let root = root
        .canonicalize()
        .map_err(|e| err(format!("canonicalize {}: {e}", root.display())))?;
    let mut scan = Scan::new(&root);
    for path in rust_files(&root) {
        if relative(&root, &path) == "src/generated/first_set.rs" {
            continue;
        }
        scan.scan_rust(&path)?;
    }
    scan.tests.workspace_members = scan_workspace_member_tests(&root)?;
    scan.scan_formatter_pairs()?;
    scan.scan_sql_assets()?;
    scan.add_contract_exclusions();
    scan.scan_gaps()?;
    scan.finish(mapping)
}

/// Serialize a report deterministically for checked-in review and byte comparison.
pub fn to_canonical_inventory_json(report: &InventoryReport) -> Result<String, InventoryError> {
    let mut json = serde_json::to_string_pretty(report)
        .map_err(|error| err(format!("serialize inventory: {error}")))?;
    json.push('\n');
    Ok(json)
}

struct Scan<'a> {
    root: &'a Path,
    grammar: Vec<GrammarRow>,
    semantics: Vec<SemanticRow>,
    tests: TestInventory,
    obsolete: Vec<Exclusion>,
    gaps: Vec<Gap>,
    parser_types: usize,
    ast_types: usize,
    parse_roles: usize,
    pratt_enums: usize,
    handwritten: usize,
}

impl<'a> Scan<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            grammar: vec![],
            semantics: vec![],
            tests: TestInventory::default(),
            obsolete: vec![],
            gaps: vec![],
            parser_types: 0,
            ast_types: 0,
            parse_roles: 0,
            pratt_enums: 0,
            handwritten: 0,
        }
    }

    fn scan_rust(&mut self, path: &Path) -> Result<(), InventoryError> {
        let source =
            fs::read_to_string(path).map_err(|e| err(format!("read {}: {e}", path.display())))?;
        let parsed =
            syn::parse_file(&source).map_err(|e| err(format!("parse {}: {e}", path.display())))?;
        let rel = relative(self.root, path);
        let module = source_module(&rel);
        self.scan_items(&rel, &source, &module, &parsed.items);
        if rel == "src/tokens.rs" {
            self.scan_token_dsl(&rel, &source, &parsed.items);
        }
        Ok(())
    }

    fn scan_items(&mut self, rel: &str, source: &str, module: &str, items: &[Item]) {
        for item in items {
            match item {
                Item::Mod(m) if m.content.is_some() => {
                    let next = qualify(module, &m.ident.to_string());
                    self.scan_items(rel, source, &next, &m.content.as_ref().unwrap().1);
                }
                Item::Struct(s) => self.scan_struct(rel, source, module, s),
                Item::Enum(e) => self.scan_enum(rel, source, module, e),
                Item::Type(t) => self.scan_type(rel, source, module, t),
                Item::Fn(function) => {
                    if has_attr(&function.attrs, "test") {
                        let id = qualify(module, &function.sig.ident.to_string());
                        let row = asset(
                            id,
                            location(rel, source, function.sig.ident.span()),
                            "literal Rust test",
                        );
                        self.tests.literal_tests.push(row.clone());
                        if rel == "src/ast/file.rs" {
                            self.tests.file_recovery_sites.push(AssetRow {
                                contract: "SQL file item parsing/recovery test".into(),
                                ..row.clone()
                            });
                        }
                        if has_attr(&function.attrs, "ignore") {
                            self.tests.ignored_tests.push(row);
                        }
                    }
                    let name = function.sig.ident.to_string();
                    if [
                        "scan_dollar_string",
                        "skip_block_comment",
                        "reject_trailing_word",
                    ]
                    .contains(&name.as_str())
                    {
                        self.grammar.push(grammar(
                            name,
                            "lexer_callback",
                            location(rel, source, function.sig.ident.span()),
                            "logos callback",
                            false,
                        ));
                    }
                    self.scan_expr_assets(rel, source, &function.block);
                }
                Item::Impl(implementation) => {
                    let trait_name = implementation
                        .trait_
                        .as_ref()
                        .map(|(_, p, _)| p.to_token_stream().to_string())
                        .unwrap_or_default();
                    if trait_name.ends_with("Parse < 'input >")
                        || trait_name.ends_with("Parse < '_ >")
                    {
                        let ty = qualify(
                            module,
                            &implementation.self_ty.to_token_stream().to_string(),
                        );
                        self.handwritten += 1;
                        self.grammar.push(grammar(
                            ty,
                            "handwritten_parse",
                            location(rel, source, implementation.impl_token.span()),
                            "handwritten Parse implementation",
                            false,
                        ));
                    }
                }
                Item::Macro(mac)
                    if mac
                        .mac
                        .path
                        .segments
                        .last()
                        .is_some_and(|s| s.ident == "corpus_tests") =>
                {
                    for (name, span) in top_level_idents(&mac.mac.tokens) {
                        self.tests.corpus_tests.push(asset(
                            qualify(module, &name),
                            location(rel, source, span),
                            "PostgreSQL 17.9 differential corpus test",
                        ));
                    }
                }
                Item::Macro(mac)
                    if mac
                        .ident
                        .as_ref()
                        .is_some_and(|name| name == "corpus_tests")
                        && mac.mac.tokens.to_string().contains("# [test]") =>
                {
                    let span = mac.ident.as_ref().unwrap().span();
                    self.tests.literal_tests.push(asset(
                        "corpus_tests::$template",
                        location(rel, source, span),
                        "literal test template expanded by corpus_tests!",
                    ));
                }
                _ => {}
            }
        }
    }

    fn scan_struct(&mut self, rel: &str, source: &str, module: &str, item: &ItemStruct) {
        let Some(kind) = grammar_attr_kind(&item.attrs) else {
            if rel.starts_with("src/ast/") && matches!(item.vis, syn::Visibility::Public(_)) {
                let id = qualify(module, &item.ident.to_string());
                self.semantics.push(semantic(
                    id.clone(),
                    if item.ident == "ExtractedFuncBody" {
                        "semantic_view"
                    } else {
                        "type"
                    },
                    location(rel, source, item.ident.span()),
                    item.to_token_stream().to_string(),
                    false,
                ));
                self.scan_fields(rel, source, &id, &item.fields);
            }
            return;
        };
        self.count_type(kind);
        let id = qualify(module, &item.ident.to_string());
        let loc = location(rel, source, item.ident.span());
        self.grammar.push(grammar(
            id.clone(),
            kind,
            loc.clone(),
            "grammar type declaration",
            true,
        ));
        self.semantics.push(semantic(
            id.clone(),
            "type",
            loc,
            item.to_token_stream().to_string(),
            false,
        ));
        self.scan_fields(rel, source, &id, &item.fields);
    }

    fn scan_enum(&mut self, rel: &str, source: &str, module: &str, item: &ItemEnum) {
        let Some(kind) = grammar_attr_kind(&item.attrs) else {
            if rel.starts_with("src/ast/") && matches!(item.vis, syn::Visibility::Public(_)) {
                let id = qualify(module, &item.ident.to_string());
                self.semantics.push(semantic(
                    id.clone(),
                    "type",
                    location(rel, source, item.ident.span()),
                    item.to_token_stream().to_string(),
                    false,
                ));
                for variant in &item.variants {
                    let variant_id = format!("{id}::{}", variant.ident);
                    self.semantics.push(semantic(
                        variant_id.clone(),
                        "variant",
                        location(rel, source, variant.ident.span()),
                        variant.to_token_stream().to_string(),
                        false,
                    ));
                    self.scan_fields(rel, source, &variant_id, &variant.fields);
                }
            }
            return;
        };
        self.count_type(kind);
        let id = qualify(module, &item.ident.to_string());
        let loc = location(rel, source, item.ident.span());
        let pratt = attr_contains(&item.attrs, "parser", "pratt")
            || item
                .variants
                .iter()
                .any(|v| attr_contains(&v.attrs, "parse", "left_recursive"));
        if pratt {
            self.pratt_enums += 1;
            self.parse_roles += 1;
            let attr = item
                .attrs
                .iter()
                .find(|attr| path_ends(attr, "parser"))
                .expect("grammar enum has a parser attribute");
            self.grammar.push(grammar(
                id.clone(),
                "parse_role",
                location(rel, source, attr.span()),
                "Pratt expression parse role",
                false,
            ));
        }
        self.grammar.push(grammar(
            id.clone(),
            if pratt { "pratt_enum" } else { kind },
            loc.clone(),
            "grammar enum declaration",
            !pratt,
        ));
        self.semantics.push(semantic(
            id.clone(),
            "type",
            loc,
            item.to_token_stream().to_string(),
            false,
        ));
        for variant in &item.variants {
            let variant_id = format!("{id}::{}", variant.ident);
            let vloc = location(rel, source, variant.ident.span());
            self.semantics.push(semantic(
                variant_id.clone(),
                "variant",
                vloc.clone(),
                variant.to_token_stream().to_string(),
                false,
            ));
            for attr in &variant.attrs {
                if attr.path().is_ident("parse") {
                    self.parse_roles += 1;
                    self.grammar.push(grammar(
                        variant_id.clone(),
                        "parse_role",
                        location(rel, source, attr.span()),
                        attr.meta.to_token_stream().to_string(),
                        true,
                    ));
                }
            }
            self.scan_fields(rel, source, &variant_id, &variant.fields);
        }
    }

    fn scan_type(&mut self, rel: &str, source: &str, module: &str, item: &ItemType) {
        let Some(kind) = grammar_attr_kind(&item.attrs) else {
            return;
        };
        self.count_type(kind);
        let id = qualify(module, &item.ident.to_string());
        let loc = location(rel, source, item.ident.span());
        self.grammar.push(grammar(
            id.clone(),
            kind,
            loc.clone(),
            "grammar type alias",
            true,
        ));
        let legacy_shape = item.ty.to_token_stream().to_string();
        let ported = port_type(&item.ty);
        self.semantics.push(SemanticRow {
            ported_shape: Some(if ported.changed { ported.shape } else { format!("type {id} = {legacy_shape}") }),
            id,
            kind: "type".into(),
            location: loc,
            legacy_shape,
            disposition: Disposition::PortedEquivalent,
            rule_id: if ported.changed { "semantic.recursa-container-transform" } else { "semantic.type-alias" }.into(),
            rationale: "the alias preserves its semantic target; any legacy grammar containers are replaced by reviewed Recursa API shapes".into(),
        });
    }

    fn count_type(&mut self, kind: &str) {
        if kind == "parser_type" {
            self.parser_types += 1
        } else {
            self.ast_types += 1
        }
    }

    fn scan_fields(&mut self, rel: &str, source: &str, parent: &str, fields: &Fields) {
        for (index, field) in fields.iter().enumerate() {
            let name = field
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| index.to_string());
            self.semantics.push(semantic_field(
                format!("{parent}.{name}"),
                location(rel, source, field.span()),
                &field.ty,
            ));
        }
    }

    fn scan_expr_assets(&mut self, rel: &str, source: &str, block: &syn::Block) {
        use syn::visit::Visit;
        struct Calls<'a> {
            rel: &'a str,
            source: &'a str,
            rows: Vec<AssetRow>,
        }
        impl<'ast> Visit<'ast> for Calls<'_> {
            fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
                let name = call.func.to_token_stream().to_string();
                if (name == "parse_fixture" || name == "parse_fixture_with_spans")
                    && !call.args.is_empty()
                    && let syn::Expr::Lit(lit) = &call.args[0]
                    && let syn::Lit::Str(s) = &lit.lit
                {
                    self.rows.push(asset(
                        s.value(),
                        location(self.rel, self.source, s.span()),
                        "file parser/recovery fixture site",
                    ));
                }
                syn::visit::visit_expr_call(self, call);
            }
        }
        let mut calls = Calls {
            rel,
            source,
            rows: vec![],
        };
        calls.visit_block(block);
        self.tests.file_recovery_fixtures.extend(calls.rows);
    }

    fn scan_token_dsl(&mut self, rel: &str, source: &str, items: &[Item]) {
        for item in items {
            let Item::Macro(mac) = item else { continue };
            if !mac
                .mac
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "tokens")
            {
                continue;
            }
            let sections = named_groups(&mac.mac.tokens);
            for (section, group) in sections {
                if ![
                    "categories",
                    "flags",
                    "keywords",
                    "soft_keywords",
                    "punctuation",
                    "literals",
                    "lexer_tokens",
                    "classes",
                    "targets",
                ]
                .contains(&section.as_str())
                {
                    continue;
                }
                for (name, span) in token_entries(&section, &group) {
                    self.grammar.push(grammar(
                        format!("tokens::{section}::{name}"),
                        &format!("token_{section}"),
                        location(rel, source, span),
                        "tokens! DSL entry",
                        section != "lexer_tokens",
                    ));
                }
                if ["literals", "lexer_tokens"].contains(&section.as_str()) {
                    for (name, span) in callback_entries(&group) {
                        self.grammar.push(grammar(
                            format!(
                                "tokens::callbacks::{name}@{}:{}",
                                span.start().line,
                                span.start().column + 1
                            ),
                            "token_callbacks",
                            location(rel, source, span),
                            "tokens! DSL callback use",
                            false,
                        ));
                    }
                }
            }
        }
    }

    fn scan_formatter_pairs(&mut self) -> Result<(), InventoryError> {
        let dir = self.root.join("tests/fmt");
        if !dir.exists() {
            return Ok(());
        }
        let mut input = BTreeMap::new();
        let mut golden = BTreeMap::new();
        for entry in fs::read_dir(&dir).map_err(|e| err(format!("read {}: {e}", dir.display())))? {
            let path = entry.map_err(|e| err(e.to_string()))?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(stem) = name.strip_suffix(".input.sql") {
                input.insert(stem.to_owned(), path.clone());
            }
            if let Some(stem) = name.strip_suffix(".golden.sql") {
                golden.insert(stem.to_owned(), path);
            }
        }
        if input.keys().collect::<Vec<_>>() != golden.keys().collect::<Vec<_>>() {
            return Err(err("formatter input/golden membership differs"));
        }
        for (name, path) in input {
            self.tests.formatter_pairs.push(file_asset(
                self.root,
                name.clone(),
                &path,
                "formatter input + golden + idempotence + parse-golden",
            )?);
            let golden_path = golden.get(&name).expect("membership checked above");
            self.tests.formatter_goldens.push(file_asset(
                self.root,
                name,
                golden_path,
                "formatter golden + parse-golden + idempotence fixture",
            )?);
        }
        Ok(())
    }

    fn scan_sql_assets(&mut self) -> Result<(), InventoryError> {
        let stress = self.root.join("fixtures/stress");
        if stress.exists() {
            for path in files_with_extension(&stress, "sql") {
                let id = path.file_stem().unwrap().to_string_lossy().into_owned();
                self.tests.stress_workloads.push(file_asset(
                    self.root,
                    id,
                    &path,
                    "stress fixture and benchmark workload",
                )?);
            }
        }
        let bench = self.root.join("benches");
        if bench.exists() {
            for path in files_with_extension(&bench, "rs") {
                let id = path.file_stem().unwrap().to_string_lossy().into_owned();
                self.tests.benchmark_sources.push(file_asset(
                    self.root,
                    id,
                    &path,
                    "legacy benchmark source; preserve corpus and parser comparisons",
                )?);
            }
        }
        let corpus = self.root.join("vendor/postgres/src/test/regress/sql");
        if corpus.exists() {
            let included: BTreeSet<_> = self
                .tests
                .corpus_tests
                .iter()
                .filter_map(|r| r.id.rsplit("::").next())
                .map(|id| id.trim_start_matches("r#").to_owned())
                .collect();
            for path in files_with_extension(&corpus, "sql") {
                let name = path.file_stem().unwrap().to_string_lossy().into_owned();
                if name.contains('.') {
                    self.tests.corpus_exclusions.push(file_asset(
                        self.root,
                        name,
                        &path,
                        "explicitly excluded: dotted filename cannot be a Rust test identifier",
                    )?);
                } else if !included.contains(&name) {
                    // The source corpus includes support files not selected by the legacy suite.
                    self.tests.corpus_exclusions.push(file_asset(
                        self.root,
                        name,
                        &path,
                        "explicitly outside frozen legacy differential membership",
                    )?);
                } else {
                    self.tests.corpus_fixtures.push(file_asset(
                        self.root,
                        name,
                        &path,
                        "PostgreSQL 17.9 differential, file-recovery, and corpus benchmark fixture",
                    )?);
                }
            }
        }
        Ok(())
    }

    fn add_contract_exclusions(&mut self) {
        self.obsolete = vec![
            exclusion(
                "fixed-token-fields",
                "src/ast/**/*.rs",
                "fixed keyword and punctuation fields are syntax, not semantic values",
            ),
            exclusion(
                "obsolete-wrappers",
                "src/ast/**/*.rs",
                "Seq0, Seq1, OptionalTrailing, and Surrounded map to current Recursa container shapes; legacy wrapper APIs are not retained",
            ),
            exclusion(
                "sql-rules",
                "src/ast/mod.rs:SqlRules",
                "legacy rules marker is replaced by current generated rules plumbing",
            ),
            exclusion(
                "derive-stacks",
                "src/ast/**/*.rs:derive",
                "legacy framework derive spelling is regenerated; Transform maps to VisitMut",
            ),
            exclusion(
                "generated-first-set",
                "src/generated/first_set.rs",
                "checked-in generation output is never migration input",
            ),
            exclusion(
                "first-set-references",
                "src/**/*.rs:__firstset",
                "legacy generated helper references are replaced by current Recursa Generation",
            ),
            exclusion(
                "generation-drift",
                "legacy workspace codegen checks",
                "current Recursa owns matching-version generation in OUT_DIR",
            ),
            exclusion(
                "obsolete-file-cli",
                "src/main.rs",
                "the legacy CLI constructs the retired FileItem recovery AST and is replaced only after strict document APIs land",
            ),
            exclusion(
                "depth-and-flame-tools",
                "src/bin/depth_probe.rs; src/flame*.rs",
                "explicitly deferred unless needed for regression diagnosis",
            ),
        ];
    }

    fn scan_gaps(&mut self) -> Result<(), InventoryError> {
        let specifications = [
            (
                "lexical-grammar",
                "PostgreSQL lexical and grammar expressiveness",
                "admissions, custom operators/identifiers, callbacks, postconditions, and source capture",
            ),
            (
                "trivia-formatting",
                "Trivia and formatting",
                "comment ownership, adjacent strings, and formatter round trips",
            ),
            (
                "file-recovery",
                "File parsing and recovery",
                "statement segmentation, raw/COPY regions, spans, errors, and continuation",
            ),
        ];
        for (id, session, capability) in specifications {
            let dir = self.root.join("migration/gaps").join(id);
            let examples = if dir.exists() {
                files_with_extension(&dir, "sql")
                    .into_iter()
                    .map(|p| {
                        let name = p.file_stem().unwrap().to_string_lossy().into_owned();
                        file_asset(self.root, name, &p, "minimized pg-sql capability example")
                    })
                    .collect::<Result<Vec<_>, InventoryError>>()?
            } else {
                vec![]
            };
            let notes = dir.join("README.md");
            self.gaps.push(Gap {
                id: id.into(),
                design_session: session.into(),
                capability: capability.into(),
                design_notes: notes
                    .exists()
                    .then(|| {
                        file_asset(
                            self.root,
                            "README",
                            &notes,
                            "expected behavior and legacy/current limitation for gap examples",
                        )
                    })
                    .transpose()?,
                examples,
            });
        }
        Ok(())
    }

    fn finish(mut self, mapping: &Mapping) -> Result<InventoryReport, InventoryError> {
        self.grammar.sort_by(|a, b| {
            (&a.location.path, a.location.byte_start, &a.id).cmp(&(
                &b.location.path,
                b.location.byte_start,
                &b.id,
            ))
        });
        self.semantics.sort_by(|a, b| {
            (&a.location.path, a.location.byte_start, &a.id).cmp(&(
                &b.location.path,
                b.location.byte_start,
                &b.id,
            ))
        });
        sort_assets(&mut self.tests.literal_tests);
        sort_assets(&mut self.tests.ignored_tests);
        sort_assets(&mut self.tests.corpus_tests);
        sort_assets(&mut self.tests.corpus_fixtures);
        sort_assets(&mut self.tests.corpus_exclusions);
        sort_assets(&mut self.tests.file_recovery_sites);
        sort_assets(&mut self.tests.file_recovery_fixtures);
        sort_assets(&mut self.tests.formatter_pairs);
        sort_assets(&mut self.tests.formatter_goldens);
        sort_assets(&mut self.tests.stress_workloads);
        sort_assets(&mut self.tests.benchmark_sources);
        self.tests
            .workspace_members
            .sort_by(|a, b| a.member.cmp(&b.member));
        for member in &mut self.tests.workspace_members {
            sort_assets(&mut member.tests);
            sort_assets(&mut member.ignored_tests);
        }
        if let Some(expected) = &mapping.expected_workspace_members {
            let found: BTreeMap<_, _> = self
                .tests
                .workspace_members
                .iter()
                .map(|member| {
                    (
                        member.member.clone(),
                        WorkspaceTestCounts {
                            tests: member.tests.len(),
                            ignored: member.ignored_tests.len(),
                        },
                    )
                })
                .collect();
            if &found != expected {
                return Err(err(format!(
                    "workspace member test contract drift: expected {expected:?}, found {found:?}"
                )));
            }
        }
        if let Some(row) = self
            .semantics
            .iter()
            .find(|row| row.rule_id == "unsupported.optional-fixed-token")
        {
            return Err(err(format!(
                "unreviewed optional fixed-token field {}; add a qualified-ID mapping",
                row.id
            )));
        }
        validate_count(
            "parser type",
            self.parser_types,
            mapping.expected_parser_types,
        )?;
        validate_count("ast type", self.ast_types, mapping.expected_ast_types)?;
        validate_count("parse role", self.parse_roles, mapping.expected_parse_roles)?;
        validate_count("Pratt enum", self.pratt_enums, mapping.expected_pratt_enums)?;
        validate_count(
            "handwritten parser",
            self.handwritten,
            mapping.expected_handwritten_parsers,
        )?;
        validate_count(
            "literal test",
            self.tests.literal_tests.len(),
            mapping.expected_literal_tests,
        )?;
        validate_count(
            "ignored test",
            self.tests.ignored_tests.len(),
            mapping.expected_ignored_tests,
        )?;
        validate_count(
            "corpus test",
            self.tests.corpus_tests.len(),
            mapping.expected_corpus_tests,
        )?;
        validate_count(
            "hashed corpus fixture",
            self.tests.corpus_fixtures.len(),
            mapping.expected_corpus_tests,
        )?;
        validate_count(
            "file recovery test site",
            self.tests.file_recovery_sites.len(),
            mapping.expected_file_recovery_sites,
        )?;
        validate_count(
            "named file recovery fixture",
            self.tests
                .file_recovery_fixtures
                .iter()
                .map(|row| &row.id)
                .collect::<BTreeSet<_>>()
                .len(),
            mapping.expected_file_recovery_fixtures,
        )?;
        validate_count(
            "formatter pair",
            self.tests.formatter_pairs.len(),
            mapping.expected_formatter_pairs,
        )?;
        validate_count(
            "formatter golden",
            self.tests.formatter_goldens.len(),
            mapping.expected_formatter_pairs,
        )?;
        validate_count(
            "stress workload",
            self.tests.stress_workloads.len(),
            mapping.expected_stress_workloads,
        )?;
        if let Some(expected) = &mapping.expected_token_counts {
            for (kind, expected) in expected {
                let found = self.grammar.iter().filter(|row| &row.kind == kind).count();
                validate_count(kind, found, Some(*expected))?;
            }
        }
        let semantic_types = self
            .semantics
            .iter()
            .filter(|row| row.kind == "type")
            .count();
        validate_count(
            "semantic type",
            semantic_types,
            mapping.expected_semantic_types,
        )?;
        if mapping.expected_parser_types.is_some()
            && self
                .gaps
                .iter()
                .any(|g| g.examples.is_empty() || g.design_notes.is_none())
        {
            return Err(err(
                "each Recursa gap group requires design notes and at least one minimized .sql example",
            ));
        }
        let fixtures = self
            .tests
            .file_recovery_fixtures
            .iter()
            .map(|r| &r.id)
            .collect::<BTreeSet<_>>()
            .len()
            + self.tests.formatter_pairs.len()
            + self.tests.stress_workloads.len();
        let benchmark_workloads =
            self.tests.stress_workloads.len() + usize::from(!self.tests.corpus_tests.is_empty());
        let summary = Summary {
            parser_types: self.parser_types,
            ast_types: self.ast_types,
            parse_roles: self.parse_roles,
            pratt_enums: self.pratt_enums,
            handwritten_parsers: self.handwritten,
            grammar_rows: self.grammar.len(),
            semantic_rows: self.semantics.len(),
            semantic_types,
            literal_tests: self.tests.literal_tests.len(),
            expanded_tests: self.tests.literal_tests.len() + self.tests.corpus_tests.len()
                - usize::from(!self.tests.corpus_tests.is_empty()),
            ignored_tests: self.tests.ignored_tests.len(),
            file_recovery_sites: self.tests.file_recovery_sites.len(),
            fixtures,
            benchmark_workloads,
            unsupported_cases: self
                .grammar
                .iter()
                .filter(|r| !r.supported_by_current_recursa)
                .count()
                + self.gaps.len(),
            workspace_member_tests: self
                .tests
                .workspace_members
                .iter()
                .map(|member| member.tests.len())
                .sum(),
        };
        Ok(InventoryReport {
            provenance: provenance(self.root)?,
            summary,
            grammar: self.grammar,
            semantics: self.semantics,
            tests: self.tests,
            obsolete_artifacts: self.obsolete,
            recursa_gaps: self.gaps,
        })
    }
}

fn grammar(
    id: impl Into<String>,
    kind: &str,
    location: SourceLocation,
    detail: impl Into<String>,
    supported: bool,
) -> GrammarRow {
    GrammarRow {
        id: id.into(),
        kind: kind.into(),
        location,
        detail: detail.into(),
        supported_by_current_recursa: supported,
    }
}
fn semantic(
    id: String,
    kind: &str,
    location: SourceLocation,
    shape: String,
    fixed: bool,
) -> SemanticRow {
    if is_obsolete_file_surface_semantic(&id) {
        return obsolete_file_surface_semantic(id, kind, location, shape);
    }
    if fixed {
        SemanticRow {
            id,
            kind: kind.into(),
            location,
            legacy_shape: shape,
            disposition: Disposition::SyntaxOnlyExclusion,
            rule_id: "syntax.fixed-token".into(),
            ported_shape: None,
            rationale: "fixed keyword/punctuation carries no semantic information".into(),
        }
    } else {
        SemanticRow {
            ported_shape: Some(format!(
                "ported {kind} `{id}` with the same semantic identity; child field/variant rows define payload transformations and Recursa Node attributes define syntax"
            )),
            id,
            kind: kind.into(),
            location,
            legacy_shape: shape,
            disposition: Disposition::PortedEquivalent,
            rule_id: "semantic.qualified-declaration".into(),
            rationale: "qualified semantic identity is preserved independently of legacy wrapper, derive, and token-field spelling".into(),
        }
    }
}
fn semantic_field(id: String, location: SourceLocation, ty: &syn::Type) -> SemanticRow {
    let shape = ty.to_token_stream().to_string();
    if is_obsolete_file_surface_semantic(&id) {
        return obsolete_file_surface_semantic(id, "field", location, shape);
    }
    if is_bare_fixed_token(ty) {
        return SemanticRow {
            id,
            kind: "field".into(),
            location,
            legacy_shape: shape,
            disposition: Disposition::SyntaxOnlyExclusion,
            rule_id: "syntax.fixed-token".into(),
            ported_shape: None,
            rationale: "bare fixed keyword/punctuation carries no semantic information".into(),
        };
    }
    if outer_type(ty).as_deref() == Some("Option")
        && first_type_argument(ty).is_some_and(is_bare_fixed_token)
    {
        let field = id
            .rsplit('.')
            .next()
            .unwrap_or("token")
            .trim_start_matches("r#")
            .to_owned();
        let syntax = fixed_syntax_ids(first_type_argument(ty).unwrap()).join(", ");
        let (disposition, rule, ported, rationale) = match optional_fixed_decision(&id) {
            Some(OptionalFixedDecision::SyntaxOnly) => (
                Disposition::SyntaxOnlyExclusion,
                "syntax.optional-fixed-token",
                None,
                "this optional token is PostgreSQL grammar filler and carries no semantic distinction",
            ),
            Some(OptionalFixedDecision::Bool) => (
                Disposition::ReviewedChange,
                "semantic.optional-fixed-token.bool",
                Some(format!(
                    "{field}: bool; {syntax} presence moves to #[presence({syntax})]"
                )),
                "token presence changes PostgreSQL behavior and is retained as an explicitly reviewed boolean",
            ),
            Some(OptionalFixedDecision::Sign) => (
                Disposition::ReviewedChange,
                "semantic.optional-fixed-token.sign-bool",
                Some("negative: bool; MINUS syntax moves to #[presence(MINUS)]".to_owned()),
                "minus presence becomes the explicit `negative` semantic boolean accepted by Recursa's fixed-token presence declaration",
            ),
            Some(OptionalFixedDecision::NestedSyntaxOnly) => (
                Disposition::ReviewedChange,
                "semantic.optional-fixed-token.nested-syntax-exclusion",
                Some("semantic payload retained; nested filler token removed".to_owned()),
                "the nested optional token is syntax-only while the enclosing field retains semantic payload",
            ),
            None => (
                Disposition::RecursaGap,
                "unsupported.optional-fixed-token",
                None,
                "optional fixed-token fields require an explicit reviewed decision keyed by full qualified ID",
            ),
        };
        return SemanticRow {
            id,
            kind: "field".into(),
            location,
            legacy_shape: shape,
            disposition,
            rule_id: rule.into(),
            ported_shape: ported,
            rationale: rationale.into(),
        };
    }
    if outer_type(ty).as_deref() == Some("Option")
        && first_type_argument(ty).is_some_and(|inner| !has_semantic_payload(inner))
    {
        let field = id
            .rsplit('.')
            .next()
            .unwrap_or("syntax")
            .trim_start_matches("r#")
            .to_owned();
        let syntax = fixed_syntax_ids(first_type_argument(ty).unwrap()).join(", ");
        return match optional_fixed_decision(&id) {
            Some(OptionalFixedDecision::SyntaxOnly) => SemanticRow {
                id,
                kind: "field".into(),
                location,
                legacy_shape: shape,
                disposition: Disposition::SyntaxOnlyExclusion,
                rule_id: "syntax.optional-fixed-token-container".into(),
                ported_shape: None,
                rationale: "the optional compound is grammar filler with no PostgreSQL semantic distinction".into(),
            },
            Some(OptionalFixedDecision::Bool) => SemanticRow {
                id,
                kind: "field".into(),
                location,
                legacy_shape: shape,
                disposition: Disposition::ReviewedChange,
                rule_id: "semantic.optional-fixed-token.bool".into(),
                ported_shape: Some(format!(
                    "{field}: bool; {syntax} presence moves to #[presence({syntax})]"
                )),
                rationale: "presence of this reviewed pure-syntax group changes PostgreSQL behavior and is retained as a named semantic boolean".into(),
            },
            _ => unsupported_optional_fixed(id, location, shape),
        };
    }
    if !has_semantic_payload(ty) {
        return SemanticRow {
            id,
            kind: "field".into(),
            location,
            legacy_shape: shape,
            disposition: Disposition::SyntaxOnlyExclusion,
            rule_id: "syntax.fixed-token-container".into(),
            ported_shape: None,
            rationale: "the compound field contains only fixed syntax; the enclosing semantic variant preserves the choice".into(),
        };
    }
    if contains_optional_fixed_token(ty) {
        let field = id
            .rsplit('.')
            .next()
            .unwrap_or("token")
            .trim_start_matches("r#")
            .to_owned();
        return match optional_fixed_decision(&id) {
            Some(OptionalFixedDecision::NestedSyntaxOnly) => SemanticRow {
                id,
                kind: "field".into(),
                location,
                legacy_shape: shape,
                disposition: Disposition::ReviewedChange,
                rule_id: "semantic.optional-fixed-token.nested-syntax-exclusion".into(),
                ported_shape: Some(format!("{field}: Option<NumericOnly<'input>>")),
                rationale: "the optional WITH token is grammar filler; the optional numeric semantic payload is retained".into(),
            },
            _ => unsupported_optional_fixed(id, location, shape),
        };
    }
    let ported = port_type(ty);
    SemanticRow {
        ported_shape: Some(if ported.changed { ported.shape } else { format!("preserve semantic Rust type `{shape}` in the ported module; grammar syntax is expressed only by Node attributes") }),
        id, kind: "field".into(), location, legacy_shape: shape,
        disposition: Disposition::PortedEquivalent,
        rule_id: if ported.changed { "semantic.recursa-container-transform" } else { "semantic.same-shape" }.into(),
        rationale: if ported.changed { "Recursa's reviewed API deletes grammar wrappers: Vec/Vec1 retain cardinality, #[sep(..., trailing)] retains separators, and #[tok(open, this, close)] retains delimiters" } else { "the field carries semantic data whose qualified role and Rust value shape are preserved independently of legacy parser syntax" }.into(),
    }
}

fn is_obsolete_file_surface_semantic(id: &str) -> bool {
    const ROOTS: &[&str] = &[
        "ast::file::PsqlDirective",
        "ast::file::PsqlCommand",
        "ast::file::FileItem",
    ];
    ROOTS.iter().any(|root| {
        id == *root
            || id
                .strip_prefix(root)
                .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with("::"))
    })
}

fn obsolete_file_surface_semantic(
    id: String,
    kind: &str,
    location: SourceLocation,
    legacy_shape: String,
) -> SemanticRow {
    SemanticRow {
        id,
        kind: kind.into(),
        location,
        legacy_shape,
        disposition: Disposition::ReviewedChange,
        rule_id: "semantic.obsolete-file-recovery-surface".into(),
        ported_shape: None,
        rationale: "the legacy undifferentiated psql/recovery surface is removed; ADR 0005 requires separate strict SQL and psql documents plus a grammar-erased recovery projection".into(),
    }
}

fn unsupported_optional_fixed(
    id: String,
    location: SourceLocation,
    legacy_shape: String,
) -> SemanticRow {
    SemanticRow {
        id,
        kind: "field".into(),
        location,
        legacy_shape,
        disposition: Disposition::RecursaGap,
        rule_id: "unsupported.optional-fixed-token".into(),
        ported_shape: None,
        rationale: "optional fixed-token fields require an explicit reviewed decision keyed by full qualified ID".into(),
    }
}

struct PortedType {
    shape: String,
    changed: bool,
}

fn port_type(ty: &syn::Type) -> PortedType {
    if is_bare_fixed_token(ty) {
        let (attribute, id) = fixed_token_attribute(ty);
        return PortedType {
            shape: format!("#[{attribute}({id})] ()"),
            changed: true,
        };
    }
    if let syn::Type::Tuple(tuple) = ty {
        let parts: Vec<_> = tuple
            .elems
            .iter()
            .filter(|part| !is_bare_fixed_token(part))
            .map(port_type)
            .collect();
        let changed = parts.len() != tuple.elems.len() || parts.iter().any(|part| part.changed);
        let shape = match parts.as_slice() {
            [] => "()".into(),
            [part] => part.shape.clone(),
            parts => format!(
                "({})",
                parts
                    .iter()
                    .map(|part| part.shape.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        return PortedType { shape, changed };
    }
    let name = outer_type(ty);
    let args = type_arguments(ty);
    match name.as_deref() {
        Some("Seq0" | "Seq1") if !args.is_empty() => {
            let value = port_type(args[0]);
            let container = if name.as_deref() == Some("Seq0") {
                "Vec"
            } else {
                "recursa::Vec1"
            };
            let separator = args.get(1).filter(|separator| !is_unit_type(separator));
            let trailing = args
                .get(2)
                .is_some_and(|arg| outer_type(arg).as_deref() == Some("OptionalTrailing"));
            let attribute = separator
                .map(|separator| {
                    let (_, separator) = fixed_token_attribute(separator);
                    format!(
                        "#[sep({}{})] ",
                        separator,
                        if trailing { ", trailing" } else { "" }
                    )
                })
                .unwrap_or_default();
            PortedType {
                shape: format!("{attribute}{container}<{}>", value.shape),
                changed: true,
            }
        }
        Some("Surrounded") if args.len() == 3 => {
            let inner = port_type(args[1]);
            let (_, open) = fixed_token_attribute(args[0]);
            let (_, close) = fixed_token_attribute(args[2]);
            PortedType {
                shape: format!("#[tok({}, this, {})] {}", open, close, inner.shape),
                changed: true,
            }
        }
        Some("Option" | "Box") if args.len() == 1 => {
            if name.as_deref() == Some("Option") && is_bare_fixed_token(args[0]) {
                return PortedType {
                    shape: "bool".into(),
                    changed: true,
                };
            }
            let inner = port_type(args[0]);
            let changed = inner.changed;
            PortedType {
                shape: format!("{}<{}>", name.unwrap(), inner.shape),
                changed,
            }
        }
        _ => PortedType {
            shape: ty.to_token_stream().to_string(),
            changed: false,
        },
    }
}

fn type_arguments(ty: &syn::Type) -> Vec<&syn::Type> {
    let syn::Type::Path(path) = ty else {
        return vec![];
    };
    let Some(segment) = path.path.segments.last() else {
        return vec![];
    };
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return vec![];
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect()
}

fn is_unit_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Tuple(tuple) if tuple.elems.is_empty())
}
#[derive(Clone, Copy)]
enum OptionalFixedDecision {
    SyntaxOnly,
    Bool,
    Sign,
    NestedSyntaxOnly,
}

// Every optional fixed-token field is reviewed by qualified semantic identity.
// Unknown IDs fail inventory so a newly added field cannot inherit a name-based policy.
const OPTIONAL_FIXED_DECISIONS: &[(&str, OptionalFixedDecision)] = &[
    (
        "ast::ddl::aggregate::CreateAggregateStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::conversion::CreateConversionStmt.default",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::database::CreateDbOption.eq",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::database::CreateDatabaseStmt.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::database::DropDatabaseOptions.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::domain::CreateDomainStmt.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::extension::CreateExtensionStmt.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::extension::AlterExtensionLanguageMember.procedural",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::foreign::AlterForeignTableBody.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::foreign::AlterForeignTableBody.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::foreign::ForeignTablePartitionBody.default",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::function::CreateFunctionStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::function::DropFunctionStmt.if_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::function::DropRoutineStmt.if_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::function::AlterFuncOptions.restrict",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::index::CreateIndexStmt.unique",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::index::CreateIndexStmt.concurrently",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::index::CreateIndexStmt.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::index::DropIndexStmt.concurrently",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::index::AlterColumnSetStatistics.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::index::AlterColumnSetReloptions.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::index::AllInTablespaceBody.nowait",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::language::CreateLanguageStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::language::CreateLanguageStmt.trusted",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::language::CreateLanguageStmt.procedural",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::language::DropLanguageStmt.procedural",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::language::AlterLanguageStmt.procedural",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::materialized_view::CreateMaterializedViewStmt.unlogged",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::materialized_view::AlterColumnSetCompression.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::operator::OpclassItemOperator.recheck",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::operator::CreateOperatorClassStmt.default",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::procedure::CreateProcedureStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::procedure::DropProcedureStmt.if_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::publication::PublicationObjTable.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::publication::PublicationObjTable.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::publication::PublicationObjOnly.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::publication::PublicationObjBare.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::role::CreateGroupStmt.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::role::CreateRoleStmt.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::role::CreateUserStmt.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::rule::CreateRuleStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::sequence::SeqIncrementOption.by",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::sequence::SeqStartOption.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::sequence::SeqRestartOption.with",
        OptionalFixedDecision::NestedSyntaxOnly,
    ),
    (
        "ast::ddl::table::NullsDistinctQualifier.not",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::ReferencesConstraint.not_valid",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::CheckConstraint.no_inherit",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::CheckConstraint.not_valid",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::SeqOptStartWith.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::table::SeqOptIncrementBy.by",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::table::PartitionColumnOptionDef.with_options",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::PartitionOfBody.default",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::CreateTableStmt.unlogged",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::CreateTableStmt.if_not_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::DropTableStmt.if_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::AlterTableSingle.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::AlterTableSingle.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::table::AlterColumnCmd.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::table::RestartWith.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::table::DropColumnIfExistsCmd.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::table::DropColumnCmd.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::tablespace::DropTablespaceStmt.if_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::transform::CreateTransformStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::trigger::TriggerForSpec.each",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::trigger::TriggerTransition.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::trigger::CreateTriggerStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::trigger::DependsOnExtension.no",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::trigger::CreateConstraintTriggerStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::view::CreateViewStmt.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::view::CreateViewStmt.recursive",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::view::DropViewStmt.if_exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::ddl::view::AlterColumnSetDefault.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::view::AlterColumnDropDefault.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::ddl::view::RenameColumnClause.column",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::dml::delete::DeleteStmt.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::PlainTable.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::ColumnDefList.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::dml::select::FuncTableRef.ordinality",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::SpecialFuncTableRef.ordinality",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::JsonTableTypedColumn.exists",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::JsonTableNestedColumn.path",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::dml::select::RowsFromRef.ordinality",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::JoinSuffix.natural",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::JoinSuffix.outer",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::dml::select::SelectIntoClause.unlogged",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::dml::select::SelectIntoClause.table",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::dml::update::UpdateTableAlias.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::dml::update::UpdateStmt.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::session::set_reset::SetRoleStmt.to",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::session::set_reset::SetSessionAuthStmt.local",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::session::set_reset::SignedNumeric.minus",
        OptionalFixedDecision::Sign,
    ),
    (
        "ast::session::set_reset::SignedInteger.minus",
        OptionalFixedDecision::Sign,
    ),
    (
        "ast::shared::expr::FuncCall.star_arg",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::FuncCall.distinct",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::QuotedFuncCall.star_arg",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::QuotedFuncCall.distinct",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::CastType.varying",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::IsDocumentTail.not",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::JsonUniqueKeys.keys",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::shared::expr::JsonObjectEntry.key_kw",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::shared::expr::JsonWrapper.array",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::shared::expr::IsJsonTail.not",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::tcl::prepared::DeallocateStmt.prepare",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::tcl::savepoint::ReleaseStmt.savepoint",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::tcl::transaction::RollbackToClause.savepoint",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::analyze::AnalyzeStmt.verbose",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::cluster::ClusterStmt.verbose",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::comment::CommentConstraintObject.domain",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::copy::CopyTableBody.program",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::copy::CopyTableBody.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::copy::CopyQueryBody.program",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::copy::CopyQueryBody.with",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::copy::CopyUsingDelimiters.using",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::copy::CopyDelimiterOpt.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::copy::CopyNullOpt.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::copy::CopyQuoteOpt.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::copy::CopyEscapeOpt.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::lock::LockRelation.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::lock::LockRelation.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::lock::LockStmt.table",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::lock::LockStmt.nowait",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::refresh::RefreshStmt.concurrently",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::reindex::ReindexRelation.concurrently",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::reindex::ReindexSchemaTarget.concurrently",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::reindex::ReindexAllTarget.concurrently",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::truncate::TruncateRelation.only",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::truncate::TruncateRelation.star",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::truncate::TruncateStmt.table",
        OptionalFixedDecision::SyntaxOnly,
    ),
    (
        "ast::utility::vacuum::VacuumStmt.full",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::vacuum::VacuumStmt.freeze",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::vacuum::VacuumStmt.verbose",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::utility::vacuum::VacuumStmt.analyze",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::query::Query::Select.optional_all",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::query::Query::Select.or_replace",
        OptionalFixedDecision::Bool,
    ),
    (
        "ast::query::Query::Select.minus",
        OptionalFixedDecision::Sign,
    ),
    (
        "ast::query::Query::Select.r#as",
        OptionalFixedDecision::SyntaxOnly,
    ),
];

fn optional_fixed_decision(id: &str) -> Option<OptionalFixedDecision> {
    OPTIONAL_FIXED_DECISIONS
        .iter()
        .find_map(|(candidate, decision)| (*candidate == id).then_some(*decision))
}

fn contains_optional_fixed_token(ty: &syn::Type) -> bool {
    if outer_type(ty).as_deref() == Some("Option")
        && first_type_argument(ty).is_some_and(is_bare_fixed_token)
    {
        return true;
    }
    match ty {
        syn::Type::Tuple(tuple) => tuple.elems.iter().any(contains_optional_fixed_token),
        syn::Type::Path(_) => type_arguments(ty)
            .into_iter()
            .any(contains_optional_fixed_token),
        _ => false,
    }
}
fn has_semantic_payload(ty: &syn::Type) -> bool {
    if is_unit_type(ty) || is_bare_fixed_token(ty) {
        return false;
    }
    if let syn::Type::Tuple(tuple) = ty {
        return tuple.elems.iter().any(has_semantic_payload);
    }
    let args = type_arguments(ty);
    match outer_type(ty).as_deref() {
        Some("Surrounded") if args.len() == 3 => has_semantic_payload(args[1]),
        Some("Seq0" | "Seq1" | "Option" | "Box" | "OptionalTrailing") if !args.is_empty() => {
            has_semantic_payload(args[0])
        }
        _ => true,
    }
}
fn fixed_syntax_ids(ty: &syn::Type) -> Vec<String> {
    if is_bare_fixed_token(ty) {
        return vec![fixed_token_attribute(ty).1];
    }
    match ty {
        syn::Type::Tuple(tuple) => tuple.elems.iter().flat_map(fixed_syntax_ids).collect(),
        syn::Type::Path(_) => type_arguments(ty)
            .into_iter()
            .flat_map(fixed_syntax_ids)
            .collect(),
        _ => Vec::new(),
    }
}
fn fixed_token_attribute(ty: &syn::Type) -> (&'static str, String) {
    let syn::Type::Path(path) = ty else {
        return ("tok", ty.to_token_stream().to_string());
    };
    let name = path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .unwrap_or_default();
    let keyword = path
        .path
        .segments
        .iter()
        .any(|segment| segment.ident == "keyword")
        || name
            .chars()
            .all(|character| character.is_ascii_uppercase() || character == '_');
    let mut token = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_ascii_uppercase()
            && index > 0
            && name.as_bytes()[index - 1].is_ascii_lowercase()
        {
            token.push('_');
        }
        token.push(character.to_ascii_uppercase());
    }
    (if keyword { "kwd" } else { "tok" }, token)
}

fn is_bare_fixed_token(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some()
        || path
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, syn::PathArguments::None))
    {
        return false;
    }
    path.path
        .segments
        .iter()
        .any(|segment| matches!(segment.ident.to_string().as_str(), "keyword" | "punct"))
        || path.path.segments.last().is_some_and(|segment| {
            let name = segment.ident.to_string();
            name.len() > 1
                && name
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character == '_')
        })
}
fn outer_type(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}
fn first_type_argument(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let syn::PathArguments::AngleBracketed(args) = &path.path.segments.last()?.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        syn::GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}
fn asset(id: impl Into<String>, location: SourceLocation, contract: impl Into<String>) -> AssetRow {
    AssetRow {
        id: id.into(),
        location,
        contract: contract.into(),
        content_sha256: None,
    }
}
fn exclusion(id: &str, location: &str, rationale: &str) -> Exclusion {
    Exclusion {
        id: id.into(),
        legacy_location: location.into(),
        rationale: rationale.into(),
    }
}
fn grammar_attr_kind(attrs: &[Attribute]) -> Option<&'static str> {
    if attrs.iter().any(|a| path_ends(a, "parser")) {
        Some("parser_type")
    } else if attrs.iter().any(|a| path_ends(a, "ast")) {
        Some("ast_type")
    } else {
        None
    }
}
fn path_ends(attr: &Attribute, name: &str) -> bool {
    attr.path().segments.last().is_some_and(|s| s.ident == name)
}
fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}
fn attr_contains(attrs: &[Attribute], name: &str, needle: &str) -> bool {
    attrs
        .iter()
        .any(|a| path_ends(a, name) && a.meta.to_token_stream().to_string().contains(needle))
}
fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.into()
    } else {
        format!("{module}::{name}")
    }
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for base in [root.join("src"), root.join("tests"), root.join("benches")] {
        if !base.exists() {
            continue;
        }
        out.extend(files_with_extension(&base, "rs"));
    }
    out.sort();
    out
}
fn scan_workspace_member_tests(root: &Path) -> Result<Vec<WorkspaceMemberTests>, InventoryError> {
    let manifest_path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| err(format!("read {}: {error}", manifest_path.display())))?;
    let workspace = manifest
        .split("[workspace]")
        .nth(1)
        .and_then(|rest| rest.split("\n[").next())
        .ok_or_else(|| err("Cargo.toml has no [workspace] section"))?;
    let members_value = workspace
        .split("members")
        .nth(1)
        .and_then(|rest| rest.split_once('[').map(|(_, value)| value))
        .and_then(|value| value.split_once(']').map(|(value, _)| value))
        .ok_or_else(|| err("workspace has no members array"))?;
    let members: Vec<_> = members_value
        .split(',')
        .filter_map(|value| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .collect();
    let mut inventories = Vec::new();
    for member in members {
        let member_root = root.join(member);
        let mut inventory = WorkspaceMemberTests {
            member: member.into(),
            tests: vec![],
            ignored_tests: vec![],
        };
        for base in [
            member_root.join("src"),
            member_root.join("tests"),
            member_root.join("benches"),
        ] {
            if !base.exists() {
                continue;
            }
            for path in files_with_extension(&base, "rs") {
                let source = fs::read_to_string(&path)
                    .map_err(|error| err(format!("read {}: {error}", path.display())))?;
                let parsed = syn::parse_file(&source)
                    .map_err(|error| err(format!("parse {}: {error}", path.display())))?;
                let rel = relative(root, &path);
                collect_member_tests(&rel, &source, "", &parsed.items, &mut inventory);
            }
        }
        inventories.push(inventory);
    }
    Ok(inventories)
}
fn collect_member_tests(
    rel: &str,
    source: &str,
    module: &str,
    items: &[Item],
    inventory: &mut WorkspaceMemberTests,
) {
    for item in items {
        match item {
            Item::Mod(item) if item.content.is_some() => {
                collect_member_tests(
                    rel,
                    source,
                    &qualify(module, &item.ident.to_string()),
                    &item.content.as_ref().unwrap().1,
                    inventory,
                );
            }
            Item::Fn(function) if has_attr(&function.attrs, "test") => {
                let row = asset(
                    qualify(module, &function.sig.ident.to_string()),
                    location(rel, source, function.sig.ident.span()),
                    "post-bootstrap workspace-member test",
                );
                inventory.tests.push(row.clone());
                if has_attr(&function.attrs, "ignore") {
                    inventory.ignored_tests.push(row);
                }
            }
            _ => {}
        }
    }
}
fn files_with_extension(root: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().and_then(|x| x.to_str()) == Some(ext)
        })
        .map(|e| e.into_path())
        .collect();
    out.sort();
    out
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn source_module(rel: &str) -> String {
    let without_extension = rel.strip_suffix(".rs").unwrap_or(rel);
    let without_mod = without_extension
        .strip_suffix("/mod")
        .unwrap_or(without_extension);
    without_mod
        .split('/')
        .filter(|part| *part != "src")
        .collect::<Vec<_>>()
        .join("::")
}
fn location(rel: &str, source: &str, span: Span) -> SourceLocation {
    let start = span.start();
    let end = span.end();
    SourceLocation {
        path: rel.into(),
        line: start.line,
        column: start.column + 1,
        byte_start: byte_offset(source, start),
        byte_end: byte_offset(source, end),
    }
}
fn byte_offset(source: &str, point: LineColumn) -> usize {
    let line_start = if point.line <= 1 {
        0
    } else {
        source
            .match_indices('\n')
            .nth(point.line - 2)
            .map_or(source.len(), |(i, _)| i + 1)
    };
    line_start.saturating_add(point.column).min(source.len())
}
fn file_asset(
    root: &Path,
    id: impl Into<String>,
    path: &Path,
    contract: &str,
) -> Result<AssetRow, InventoryError> {
    let bytes = fs::read(path).map_err(|error| err(format!("read {}: {error}", path.display())))?;
    let mut row = asset(
        id,
        SourceLocation {
            path: relative(root, path),
            line: 1,
            column: 1,
            byte_start: 0,
            byte_end: bytes.len(),
        },
        contract,
    );
    row.content_sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    Ok(row)
}
fn provenance(root: &Path) -> Result<Provenance, InventoryError> {
    let path = root.join("docs/import-provenance.tsv");
    let manifest = fs::read_to_string(&path)
        .map_err(|error| err(format!("read {}: {error}", path.display())))?;
    let mut identities = BTreeMap::new();
    for line in manifest.lines().filter_map(|line| line.strip_prefix("# ")) {
        if let Some((name, value)) = line.split_once(' ')
            && [
                "legacy-commit",
                "legacy-tree",
                "pg-sql-tree",
                "pg-oracle-tree",
                "postgres-gitlink",
                "source-checkpoint",
            ]
            .contains(&name)
        {
            if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(err(format!(
                    "invalid {name} identity in {}",
                    path.display()
                )));
            }
            identities.insert(name, value.to_owned());
        }
    }
    let mut take = |name| {
        identities
            .remove(name)
            .ok_or_else(|| err(format!("missing {name} identity in {}", path.display())))
    };
    Ok(Provenance {
        legacy_commit: take("legacy-commit")?,
        legacy_tree: take("legacy-tree")?,
        pg_sql_tree: take("pg-sql-tree")?,
        pg_oracle_tree: take("pg-oracle-tree")?,
        postgres_gitlink: take("postgres-gitlink")?,
        source_checkpoint: take("source-checkpoint")?,
        generated_excluded: vec![
            "src/generated/first_set.rs".into(),
            "__firstset references".into(),
            "legacy generation drift checks".into(),
        ],
    })
}
fn validate_count(name: &str, found: usize, expected: Option<usize>) -> Result<(), InventoryError> {
    if let Some(expected) = expected
        && found != expected
    {
        return Err(err(format!(
            "{name} count drift: expected {expected}, found {found}"
        )));
    }
    Ok(())
}
fn err(message: impl Into<String>) -> InventoryError {
    InventoryError(message.into())
}
fn sort_assets(rows: &mut [AssetRow]) {
    rows.sort_by(|a, b| {
        (&a.location.path, a.location.byte_start, &a.id).cmp(&(
            &b.location.path,
            b.location.byte_start,
            &b.id,
        ))
    });
}
fn top_level_idents(tokens: &TokenStream) -> Vec<(String, Span)> {
    tokens
        .clone()
        .into_iter()
        .filter_map(|tree| match tree {
            TokenTree::Ident(i) => Some((i.to_string(), i.span())),
            _ => None,
        })
        .collect()
}
fn named_groups(tokens: &TokenStream) -> Vec<(String, TokenStream)> {
    let trees: Vec<_> = tokens.clone().into_iter().collect();
    let mut out = vec![];
    for pair in trees.windows(2) {
        if let (TokenTree::Ident(name), TokenTree::Group(group)) = (&pair[0], &pair[1]) {
            out.push((name.to_string(), group.stream()));
        }
    }
    out
}
fn token_entries(section: &str, tokens: &TokenStream) -> Vec<(String, Span)> {
    let trees: Vec<_> = tokens.clone().into_iter().collect();
    let mut out = vec![];
    for (index, tree) in trees.iter().enumerate() {
        let TokenTree::Ident(ident) = tree else {
            continue;
        };
        let marker = trees.get(index + 1).and_then(|next| match next {
            TokenTree::Punct(p) => Some(p.as_char()),
            _ => None,
        });
        let is_entry = match section {
            "categories" | "flags" => true,
            "classes" => {
                marker == Some('=')
                    && !matches!(trees.get(index + 2), Some(TokenTree::Punct(p)) if p.as_char() == '>')
            }
            "targets" => {
                marker == Some(':')
                    && !matches!(trees.get(index + 2), Some(TokenTree::Punct(p)) if p.as_char() == ':')
            }
            _ => {
                marker == Some('=')
                    && matches!(trees.get(index + 2), Some(TokenTree::Punct(p)) if p.as_char() == '>')
            }
        };
        if is_entry {
            out.push((ident.to_string(), ident.span()));
        }
    }
    out
}
fn callback_entries(tokens: &TokenStream) -> Vec<(String, Span)> {
    let trees: Vec<_> = tokens.clone().into_iter().collect();
    let mut out = vec![];
    for (index, tree) in trees.iter().enumerate() {
        if matches!(tree, TokenTree::Ident(ident) if ident == "with") {
            let callback = trees[index + 1..]
                .iter()
                .take_while(|tree| !matches!(tree, TokenTree::Punct(p) if p.as_char() == ','))
                .filter_map(|tree| match tree {
                    TokenTree::Ident(ident) => Some(ident),
                    _ => None,
                })
                .last();
            if let Some(callback) = callback {
                out.push((callback.to_string(), callback.span()));
            }
        }
    }
    out
}
