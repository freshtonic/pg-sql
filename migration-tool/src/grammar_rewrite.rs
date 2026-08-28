//! Deterministic grammar-only migration over reviewed source shapes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use syn::Token;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::Mapping;
use crate::rewrite::{
    FileDisposition, RewriteError, SourceRewritePass, SpanEdit, apply_span_edits,
};

pub const SUPPORTED_SHAPES: &[&str] = &[
    "admission_all_word_kinds",
    "admission_bare_alias_name",
    "admission_bare_col_label",
    "admission_col_label",
    "admission_col_id",
    "admission_non_reserved_word",
    "admission_psql_variable_name",
    "admission_type_function_name",
    "admission_unquoted_ident",
    "admission_window_ref_name",
    "attributed_enum",
    "attributed_struct",
    "attributed_type",
    "binding_bare_alias_name_variant",
    "binding_bare_col_label_delete_alias",
    "binding_col_id_savepoint",
    "binding_col_label_def_elem",
    "binding_custom_op_operator_expr",
    "binding_dollar_num_positional_param",
    "binding_dollar_string_do_stmt",
    "binding_integer_value",
    "binding_non_reserved_word_role_spec",
    "binding_numeric_value",
    "binding_psql_variable_name_structural",
    "binding_type_function_name_param",
    "binding_unquoted_ident_variant",
    "binding_window_ref_name_wrapper",
    "derive_stack",
    "fixed_syntax",
    "grammar_keyword_matching_ascii_insensitive",
    "grammar_max_lookahead_five",
    "handwritten_bare_alias_name",
    "handwritten_custom_op",
    "handwritten_rest_of_line",
    "handwritten_string_lit_seq",
    "handwritten_unquoted_ident",
    "ignore_nested_block_comment",
    "lexer_callback",
    "matcher_custom_op",
    "matcher_dollar_num",
    "matcher_dollar_string",
    "matcher_integer",
    "matcher_numeric",
    "nested_container",
    "obsolete_firstset_ref",
    "obsolete_generated_artifact",
    "obsolete_callback_functions",
    "obsolete_parser_postcondition",
    "obsolete_raw_line_parser",
    "obsolete_sql_rules",
    "optional_syntax_bool",
    "optional_syntax_exclusion",
    "optional_trailing",
    "parser_attr",
    "pratt_enum",
    "pratt_infix",
    "pratt_postfix",
    "pratt_prefix",
    "remove_frame_unit_predicate",
    "remove_frame_unit_wrapper",
    "remove_operator_comment_repair",
    "remove_pg_lex",
    "remove_psql_variable_repair",
    "remove_reject_trailing_word",
    "remove_scan_dollar_string",
    "remove_skip_block_comment",
    "seq0",
    "seq1",
    "surrounded",
    "token_categories",
    "token_classes",
    "token_flags",
    "token_keywords",
    "token_lexer_tokens",
    "token_literals",
    "token_punctuation",
    "token_soft_keywords",
    "token_targets",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GrammarRewriteManifest {
    pub schema_version: u32,
    pub inventory_contract: GrammarInventoryContract,
    pub cases: Vec<GrammarFixtureCase>,
    pub omissions: Vec<GrammarOmissionCase>,
    pub unsupported: Vec<UnsupportedFixtureCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GrammarInventoryContract {
    pub parser_types: usize,
    pub ast_types: usize,
    pub parse_roles: usize,
    pub pratt_enums: usize,
    pub handwritten_parsers: usize,
    pub token_counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GrammarFixtureCase {
    pub id: String,
    pub input: String,
    pub expected: String,
    pub shapes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct UnsupportedFixtureCase {
    pub id: String,
    pub input: String,
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct GrammarOmissionCase {
    pub id: String,
    pub input: String,
    pub shapes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct GrammarRewritePass {
    manifest: GrammarRewriteManifest,
    semantic_rows: BTreeMap<String, Vec<CanonicalSemanticRow>>,
}

#[derive(Clone, Debug, Deserialize)]
struct CanonicalInventory {
    semantics: Vec<CanonicalSemanticRow>,
}

#[derive(Clone, Debug, Deserialize)]
struct CanonicalSemanticRow {
    id: String,
    location: CanonicalLocation,
    legacy_shape: String,
    rule_id: String,
}

#[derive(Clone, Debug, Deserialize)]
struct CanonicalLocation {
    path: String,
    byte_start: usize,
    byte_end: usize,
}

impl GrammarRewritePass {
    pub fn from_manifest_json(json: &str) -> Result<Self, GrammarRewriteError> {
        let manifest: GrammarRewriteManifest = serde_json::from_str(json).map_err(|error| {
            GrammarRewriteError::new("manifest.invalid-json", None, error.to_string())
        })?;
        validate_manifest(&manifest)?;
        let inventory: CanonicalInventory =
            serde_json::from_str(include_str!("../../migration/contract/inventory.json")).map_err(
                |error| GrammarRewriteError::new("inventory.invalid-json", None, error.to_string()),
            )?;
        let mut semantic_rows: BTreeMap<String, Vec<CanonicalSemanticRow>> = BTreeMap::new();
        for row in inventory.semantics {
            if is_tracked_semantic_rule(&row.rule_id) {
                semantic_rows
                    .entry(row.location.path.clone())
                    .or_default()
                    .push(row);
            }
        }
        Ok(Self {
            manifest,
            semantic_rows,
        })
    }

    pub fn manifest(&self) -> &GrammarRewriteManifest {
        &self.manifest
    }

    pub fn plan_edits(
        &self,
        path: &Path,
        source: &str,
    ) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
        let parsed = syn::parse_file(source).map_err(|error| {
            GrammarRewriteError::new("source.invalid-rust", None, error.to_string())
        })?;
        let structure = StructuralSpans::parse(source)?;
        for &(needle, code) in UNSUPPORTED {
            if let Some(offset) = structural_matches(source, needle, &structure)
                .into_iter()
                .next()
            {
                return Err(GrammarRewriteError::new(
                    code,
                    Some(offset),
                    format!("unsupported grammar construct {needle:?}"),
                ));
            }
        }

        let mut edits = Vec::new();
        let mut accounted_semantic_rows = BTreeSet::new();
        if path == Path::new("src/lib.rs") {
            edits.extend(plan_crate_grammar_declaration(source)?);
        }
        if let Some(path) = path.to_str()
            && let Some(rows) = self.semantic_rows.get(path)
        {
            let mut fixed_syntax = FixedSyntaxPlanner::new(source, rows);
            fixed_syntax.visit_file(&parsed);
            let plan = fixed_syntax.finish()?;
            edits.extend(plan.edits);
            accounted_semantic_rows = plan.accounted_rows;
        }
        edits.extend(plan_reviewed_lexical_bindings(path, source, &structure)?);
        let mut containers = ContainerPlanner::new(source, &structure);
        containers.visit_file(&parsed);
        for edit in containers.finish()? {
            let covered = edits.iter().any(|existing| {
                existing.start < existing.end
                    && existing.start <= edit.start
                    && edit.end <= existing.end
            });
            if !covered {
                edits.push(edit);
            }
        }
        let mut legacy_attributes = LegacyAttributePlanner::new(source);
        legacy_attributes.visit_file(&parsed);
        edits.extend(legacy_attributes.finish()?);
        let mut obsolete_items = ObsoleteItemPlanner::new(source);
        obsolete_items.visit_file(&parsed);
        edits.extend(obsolete_items.finish()?);
        let mut tokens = TokenMacroPlanner::new(source);
        tokens.visit_file(&parsed);
        edits.extend(tokens.finish()?);
        for &(needle, replacement) in
            REWRITES
                .iter()
                .chain(FIXTURE_OPTIONAL_REWRITES.iter().filter(|_| {
                    path.file_name()
                        .is_some_and(|name| name == "nodes.input.rs")
                }))
        {
            for start in structural_matches(source, needle, &structure) {
                if replacement
                    .strip_suffix(needle)
                    .is_some_and(|prefix| !prefix.is_empty() && source[..start].ends_with(prefix))
                {
                    continue;
                }
                let end = start + needle.len();
                if edits
                    .iter()
                    .any(|edit| start < edit.end && edit.start < end)
                {
                    continue;
                }
                edits.push(SpanEdit {
                    start,
                    end,
                    replacement: replacement.into(),
                });
            }
        }
        let obsolete_surface_edits = plan_obsolete_file_surface(path, source, &parsed)?;
        for obsolete in obsolete_surface_edits {
            let mut partial_overlap = None;
            edits.retain(|edit| {
                let overlaps = edit.start < obsolete.end && obsolete.start < edit.end
                    || edit.start == edit.end
                        && obsolete.start <= edit.start
                        && edit.start < obsolete.end;
                if !overlaps {
                    return true;
                }
                let contained = obsolete.start <= edit.start && edit.end <= obsolete.end;
                if !contained {
                    partial_overlap = Some(edit.start);
                }
                false
            });
            if let Some(offset) = partial_overlap {
                return Err(GrammarRewriteError::new(
                    "rewrite.overlapping-obsolete-surface",
                    Some(offset),
                    "a reviewed obsolete file item partially overlaps another grammar rewrite",
                ));
            }
            edits.push(obsolete);
        }
        merge_insertions(&mut edits);
        edits.sort_by_key(|edit| (edit.start, edit.end));
        edits.dedup();
        for pair in edits.windows(2) {
            if pair[1].start < pair[0].end {
                return Err(GrammarRewriteError::new(
                    "rewrite.overlapping-rules",
                    Some(pair[1].start),
                    "reviewed grammar rules selected overlapping spans",
                ));
            }
        }
        self.validate_semantic_accounting(path, &edits, &accounted_semantic_rows)?;
        let rewritten = apply_span_edits(source, &edits).map_err(|error| {
            GrammarRewriteError::new("rewrite.invalid-span", None, error.to_string())
        })?;
        let rewritten_parsed = syn::parse_file(&rewritten).map_err(|error| {
            GrammarRewriteError::new("rewrite.invalid-rust", None, error.to_string())
        })?;
        let mut obsolete_syntax = ObsoleteSyntaxDetector::new(&rewritten);
        obsolete_syntax.visit_file(&rewritten_parsed);
        obsolete_syntax.finish()?;
        Ok(edits)
    }

    fn validate_semantic_accounting(
        &self,
        path: &Path,
        edits: &[SpanEdit],
        accounted_rows: &BTreeSet<String>,
    ) -> Result<(), GrammarRewriteError> {
        let Some(path) = path.to_str() else {
            return Err(GrammarRewriteError::new(
                "inventory.non-utf8-path",
                None,
                "grammar inventory paths must be UTF-8",
            ));
        };
        let Some(rows) = self.semantic_rows.get(path) else {
            return Ok(());
        };
        if let Some(row) = rows.iter().find(|row| {
            is_fixed_syntax_inventory_rule(&row.rule_id) && !accounted_rows.contains(&row.id)
        }) {
            return Err(GrammarRewriteError::new(
                "rewrite.unaccounted-fixed-syntax-row",
                Some(row.location.byte_start),
                format!(
                    "canonical semantic row `{}` ({}) has no exact reviewed field disposition",
                    row.id, row.rule_id
                ),
            ));
        }
        if let Some(row) = rows.iter().find(|row| {
            is_edit_accounted_semantic_rule(&row.rule_id)
                && !edits.iter().any(|edit| {
                    edit.start < row.location.byte_end && row.location.byte_start < edit.end
                })
        }) {
            return Err(GrammarRewriteError::new(
                "rewrite.unaccounted-semantic-row",
                Some(row.location.byte_start),
                format!(
                    "canonical semantic row `{}` ({}) has no reviewed edit or defer disposition",
                    row.id, row.rule_id
                ),
            ));
        }
        Ok(())
    }
}

fn merge_insertions(edits: &mut Vec<SpanEdit>) {
    let mut insertions: BTreeMap<usize, String> = BTreeMap::new();
    edits.retain(|edit| {
        if edit.start == edit.end {
            insertions
                .entry(edit.start)
                .or_default()
                .push_str(&edit.replacement);
            false
        } else {
            true
        }
    });
    for (start, replacement) in insertions {
        let replacement = compose_generated_token_attachments(&replacement);
        if let Some(edit) = edits.iter_mut().find(|edit| edit.start == start) {
            edit.replacement.insert_str(0, &replacement);
        } else if let Some(edit) = edits
            .iter_mut()
            .find(|edit| edit.start < edit.end && edit.end == start)
        {
            edit.replacement.push_str(&replacement);
            edit.replacement = compose_generated_token_attachments(&edit.replacement);
        } else {
            edits.push(SpanEdit {
                start,
                end: start,
                replacement,
            });
        }
    }
}

fn compose_generated_token_attachments(replacement: &str) -> String {
    const MARKER: &str = "#[tok(";
    let mut attributes = Vec::new();
    let mut search = 0;
    while let Some(relative) = replacement[search..].find(MARKER) {
        let start = search + relative;
        let content_start = start + MARKER.len();
        let mut depth = 1usize;
        let mut close = None;
        for (relative, character) in replacement[content_start..].char_indices() {
            match character {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(content_start + relative);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            return replacement.into();
        };
        if replacement.as_bytes().get(close + 1) != Some(&b']') {
            return replacement.into();
        }
        attributes.push((start, close + 2, &replacement[content_start..close]));
        search = close + 2;
    }
    if attributes.len() < 2 {
        return replacement.into();
    }

    let mut sequence = split_generated_token_sequence(attributes[0].2);
    for (_, _, attachment) in attributes.iter().skip(1) {
        let attachment = split_generated_token_sequence(attachment);
        match (
            sequence.iter().position(|entry| entry == "this"),
            attachment.iter().position(|entry| entry == "this"),
        ) {
            (Some(this), Some(_)) => {
                sequence.splice(this..=this, attachment);
            }
            (Some(this), None) => {
                sequence.splice(this..this, attachment);
            }
            (None, _) => sequence.extend(attachment),
        }
    }
    let combined = format!("#[tok({})]", sequence.join(", "));
    let mut output = replacement.to_owned();
    let only_whitespace_between = attributes.windows(2).all(|pair| {
        replacement[pair[0].1..pair[1].0]
            .chars()
            .all(char::is_whitespace)
    });
    if only_whitespace_between {
        output.replace_range(attributes[0].0..attributes.last().unwrap().1, &combined);
    } else {
        for (index, (start, end, _)) in attributes.iter().enumerate().rev() {
            output.replace_range(*start..*end, if index == 0 { &combined } else { "" });
        }
    }
    output
}

fn split_generated_token_sequence(sequence: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (offset, character) in sequence.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                entries.push(sequence[start..offset].trim().to_owned());
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    entries.push(sequence[start..].trim().to_owned());
    entries
}

struct ReviewedPathRewrite {
    path: &'static str,
    needle: &'static str,
    replacement: &'static str,
}

fn plan_reviewed_lexical_bindings(
    path: &Path,
    source: &str,
    structure: &StructuralSpans,
) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
    let Some(path) = path.to_str() else {
        return Ok(Vec::new());
    };
    let mut edits = Vec::new();
    for rewrite in REVIEWED_LEXICAL_BINDINGS
        .iter()
        .filter(|rewrite| rewrite.path == path)
    {
        let matches = structural_matches(source, rewrite.needle, structure);
        let expected_matches = match (rewrite.path, rewrite.needle) {
            ("src/ast/tcl/savepoint.rs", "    pub name: crate::tokens::ColId<'input>,")
            | (
                "src/ast/ddl/function.rs",
                "    pub name: crate::tokens::type_function_name<'input>,",
            ) => 2,
            _ => 1,
        };
        if matches.len() != expected_matches {
            return Err(GrammarRewriteError::new(
                "unsupported.lexical-binding-site",
                matches.first().copied(),
                format!(
                    "reviewed lexical binding in {} matched {} source spans",
                    rewrite.path,
                    matches.len()
                ),
            ));
        }
        edits.extend(matches.into_iter().map(|start| {
            if let Some(attribute) = rewrite.replacement.strip_suffix(rewrite.needle) {
                let indentation_len = rewrite
                    .needle
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .map(char::len_utf8)
                    .sum::<usize>();
                let indentation = &rewrite.needle[..indentation_len];
                let attribute = attribute.strip_prefix(indentation).unwrap_or(attribute);
                SpanEdit {
                    start: start + indentation_len,
                    end: start + indentation_len,
                    replacement: format!("{attribute}{indentation}"),
                }
            } else {
                SpanEdit {
                    start,
                    end: start + rewrite.needle.len(),
                    replacement: rewrite.replacement.into(),
                }
            }
        }));
    }
    if path == "src/tokens.rs" {
        const ANCHOR: &str = "    /// Catch-all for Postgres user-defined operator names.";
        let anchors: Vec<_> = source.match_indices(ANCHOR).collect();
        if anchors.len() != 1 {
            return Err(GrammarRewriteError::new(
                "unsupported.psql-variable-binding-site",
                anchors.first().map(|(offset, _)| *offset),
                "reviewed PsqlVariable declaration anchor changed",
            ));
        }
        edits.push(SpanEdit {
            start: anchors[0].0,
            end: anchors[0].0,
            replacement: "    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]\n    pub struct PsqlVariable<'input> {\n        #[tok(COLON)]\n        #[lex(pattern = r#\"(?:[A-Za-z_][A-Za-z0-9_]*|'[^']*'|\"[^\"]*\")\"#, admits(PsqlVariableName))]\n        pub name: PsqlVariableName<'input>,\n    }\n\n"
                .into(),
        });
    }
    Ok(edits)
}

const REVIEWED_LEXICAL_BINDINGS: &[ReviewedPathRewrite] = &[
    ReviewedPathRewrite {
        path: "src/ast/tcl/savepoint.rs",
        needle: "    pub name: crate::tokens::ColId<'input>,",
        replacement: "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(ColId))]\n    pub name: crate::tokens::ColId<'input>,",
    },
    ReviewedPathRewrite {
        path: "src/ast/ddl/function.rs",
        needle: "    pub name: crate::tokens::type_function_name<'input>,",
        replacement: "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(type_function_name))]\n    pub name: crate::tokens::type_function_name<'input>,",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/names.rs",
        needle: "    pub name: crate::tokens::NonReservedWord<'input>,",
        replacement: "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(NonReservedWord))]\n    pub name: crate::tokens::NonReservedWord<'input>,",
    },
    ReviewedPathRewrite {
        path: "src/ast/dml/delete.rs",
        needle: "    Bare(crate::tokens::BareColLabel<'input>),",
        replacement: "    Bare(#[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(BareColLabel))] crate::tokens::BareColLabel<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/utility/do.rs",
        needle: "    pub body: literal::DollarStringLit<'input>,",
        replacement: "    #[lex(matcher)]\n    pub body: literal::DollarStringLit<'input>,",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/numbers.rs",
        needle: "    Numeric(literal::NumericLit<'input>),",
        replacement: "    Numeric(#[lex(matcher)] literal::NumericLit<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/numbers.rs",
        needle: "    Integer(literal::IntegerLit<'input>),",
        replacement: "    Integer(#[lex(matcher)] literal::IntegerLit<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/expr.rs",
        needle: "    PositionalParam(literal::DollarNum<'input>),",
        replacement: "    PositionalParam(#[lex(matcher)] literal::DollarNum<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/dml/select.rs",
        needle: "    Custom(literal::CustomOp<'input>),",
        replacement: "    Custom(#[lex(matcher)] literal::CustomOp<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/expr.rs",
        needle: "    pub lit: literal::UnicodeStringLit<'input>,",
        replacement: "    #[lex(pattern = r\"(?i:U)&'(?:[^']|'')*'\")]\n    pub lit: literal::UnicodeStringLit<'input>,",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/expr.rs",
        needle: "    EscapeStringLit(literal::EscapeStringLit<'input>),",
        replacement: "    EscapeStringLit(#[lex(pattern = r\"(?i:E)'(?:[^'\\\\]|\\\\.|'')*'\")] literal::EscapeStringLit<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/expr.rs",
        needle: "    BitStringLit(literal::BitStringLit<'input>),",
        replacement: "    BitStringLit(#[lex(pattern = r\"(?i:B)'[^']*'\")] literal::BitStringLit<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/shared/expr.rs",
        needle: "    HexStringLit(literal::HexStringLit<'input>),",
        replacement: "    HexStringLit(#[lex(pattern = r\"(?i:X)'[^']*'\")] literal::HexStringLit<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/ast/utility/analyze.rs",
        needle: "    String(literal::StringLit<'input>),",
        replacement: "    String(#[lex(pattern = r\"'[^']*(?:''[^']*)*'\")] literal::StringLit<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/tokens.rs",
        needle: "    pub enum Ident<'input> {\n        #[railroad(label = \"<Unicode Quoted>\")]\n        UnicodeQuoted(UnicodeQuotedIdent<'input>),",
        replacement: "    pub enum Ident<'input> {\n        #[railroad(label = \"<Unicode Quoted>\")]\n        UnicodeQuoted(#[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"\"#)] UnicodeQuotedIdent<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/tokens.rs",
        needle: "        Quoted(literal::QuotedIdent<'input>),",
        replacement: "        Quoted(#[lex(pattern = r#\"\"[^\"]*(?:\"\"[^\"]*)*\"\"#)] literal::QuotedIdent<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/tokens.rs",
        needle: "        Unquoted(UnquotedIdent<'input>),",
        replacement: "        Unquoted(#[lex(pattern = r\"[A-Za-z_][A-Za-z0-9_]*\", admits(UnquotedIdent))] UnquotedIdent<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/tokens.rs",
        needle: "        Bare(BareAliasName<'input>),",
        replacement: "        Bare(#[lex(pattern = r\"[A-Za-z_][A-Za-z0-9_]*\", admits(BareAliasName))] BareAliasName<'input>),",
    },
    ReviewedPathRewrite {
        path: "src/tokens.rs",
        needle: "        Ident(Ident<'input>),",
        replacement: "        Ident(#[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(WindowRefName))] WindowRefNameText<'input>),",
    },
];

fn is_tracked_semantic_rule(rule_id: &str) -> bool {
    matches!(
        rule_id,
        "semantic.obsolete-file-recovery-surface"
            | "semantic.recursa-container-transform"
            | "syntax.fixed-token"
            | "syntax.fixed-token-container"
            | "syntax.optional-fixed-token"
            | "semantic.optional-fixed-token.bool"
            | "semantic.optional-fixed-token.sign-bool"
            | "semantic.optional-fixed-token.nested-syntax-exclusion"
    )
}

fn is_edit_accounted_semantic_rule(rule_id: &str) -> bool {
    matches!(
        rule_id,
        "semantic.obsolete-file-recovery-surface"
            | "semantic.recursa-container-transform"
            | "syntax.fixed-token"
            | "syntax.fixed-token-container"
            | "syntax.optional-fixed-token"
            | "semantic.optional-fixed-token.bool"
            | "semantic.optional-fixed-token.sign-bool"
            | "semantic.optional-fixed-token.nested-syntax-exclusion"
    )
}

fn is_fixed_syntax_inventory_rule(rule_id: &str) -> bool {
    matches!(
        rule_id,
        "syntax.fixed-token"
            | "syntax.fixed-token-container"
            | "syntax.optional-fixed-token"
            | "semantic.optional-fixed-token.bool"
            | "semantic.optional-fixed-token.sign-bool"
            | "semantic.optional-fixed-token.nested-syntax-exclusion"
    )
}

struct LegacyAttributePlanner<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
    edits: Vec<SpanEdit>,
    error: Option<GrammarRewriteError>,
}

impl<'a> LegacyAttributePlanner<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            line_starts: line_starts(source),
            edits: Vec::new(),
            error: None,
        }
    }

    fn finish(self) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
        self.error.map_or(Ok(self.edits), Err)
    }

    fn range(&self, span: Span) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
        span_range(span, &self.line_starts, self.source.len())
    }

    fn replace_attribute(
        &mut self,
        attribute: &syn::Attribute,
        replacement: &str,
    ) -> Result<(), GrammarRewriteError> {
        let mut range = self.range(attribute.span())?;
        if replacement.is_empty() {
            if self.source[range.end..].starts_with("\r\n") {
                range.end += 2;
            } else if self.source[range.end..].starts_with('\n') {
                range.end += 1;
            }
        }
        self.edits.push(SpanEdit {
            start: range.start,
            end: range.end,
            replacement: replacement.into(),
        });
        Ok(())
    }

    fn plan_parser_attribute(
        &mut self,
        attribute: &syn::Attribute,
    ) -> Result<(), GrammarRewriteError> {
        let syn::Meta::List(list) = &attribute.meta else {
            return self.replace_attribute(attribute, "");
        };
        let options = Punctuated::<syn::Meta, Token![,]>::parse_terminated
            .parse2(list.tokens.clone())
            .map_err(|error| {
                GrammarRewriteError::new(
                    "unsupported.obsolete-parser-attribute",
                    self.range(attribute.span()).ok().map(|range| range.start),
                    error.to_string(),
                )
            })?;
        let mut rules = false;
        let mut pratt = false;
        let mut metadata = false;
        let mut postcondition = false;
        for option in options {
            match option {
                syn::Meta::NameValue(option)
                    if option.path.is_ident("rules")
                        && expr_path_is(&option.value, &["SqlRules"]) =>
                {
                    rules = true;
                }
                syn::Meta::NameValue(option)
                    if option.path.is_ident("meta_tags") && reviewed_meta_tags(&option.value) =>
                {
                    metadata = true;
                }
                syn::Meta::Path(path) if path.is_ident("pratt") => pratt = true,
                syn::Meta::NameValue(option)
                    if option.path.is_ident("postcondition")
                        && expr_path_is(
                            &option.value,
                            &["crate", "tokens", "literal", "not_frame_unit_wrapper"],
                        ) =>
                {
                    postcondition = true;
                }
                option => {
                    return Err(GrammarRewriteError::new(
                        parser_option_error_code(&option),
                        self.range(option.span()).ok().map(|range| range.start),
                        "parser attribute is not a reviewed legacy declaration",
                    ));
                }
            }
        }
        let reviewed = postcondition && !rules && !pratt && !metadata
            || rules && !postcondition && (!pratt || !metadata);
        if !reviewed {
            return Err(GrammarRewriteError::new(
                "unsupported.obsolete-parser-attribute",
                self.range(attribute.span()).ok().map(|range| range.start),
                "parser attribute combines legacy options in an unreviewed shape",
            ));
        }
        self.replace_attribute(attribute, if pratt { "#[pratt]" } else { "" })
    }

    fn plan_ast_attribute(
        &mut self,
        attributes: &[syn::Attribute],
        replacement: &str,
    ) -> Result<(), GrammarRewriteError> {
        for attribute in attributes
            .iter()
            .filter(|attribute| is_recursa_attribute(attribute.path(), "ast"))
        {
            let reviewed = match &attribute.meta {
                syn::Meta::Path(_) => true,
                syn::Meta::List(list) => Punctuated::<syn::Meta, Token![,]>::parse_terminated
                    .parse2(list.tokens.clone())
                    .is_ok_and(|options| {
                        options.len() == 1
                            && matches!(
                                &options[0],
                                syn::Meta::NameValue(option)
                                    if option.path.is_ident("meta_tags")
                                        && reviewed_meta_tags(&option.value)
                            )
                    }),
                syn::Meta::NameValue(_) => false,
            };
            if !reviewed {
                return Err(GrammarRewriteError::new(
                    "unsupported.obsolete-ast-attribute",
                    self.range(attribute.span()).ok().map(|range| range.start),
                    "ast attribute is not a reviewed legacy declaration",
                ));
            }
            self.replace_attribute(attribute, replacement)?;
        }
        Ok(())
    }

    fn record(&mut self, result: Result<(), GrammarRewriteError>) {
        if self.error.is_none()
            && let Err(error) = result
        {
            self.error = Some(error);
        }
    }
}

impl<'ast> Visit<'ast> for LegacyAttributePlanner<'_> {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if self.error.is_none() && is_recursa_parser(attribute.path()) {
            let result = self.plan_parser_attribute(attribute);
            self.record(result);
        }
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if self.error.is_none() {
            let result = self.plan_ast_attribute(&item.attrs, "#[derive(recursa::Node)]");
            self.record(result);
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if self.error.is_none() {
            let result = self.plan_ast_attribute(&item.attrs, "#[derive(recursa::Node)]");
            self.record(result);
        }
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if self.error.is_none() {
            let result = self.plan_ast_attribute(&item.attrs, "#[parse(skip)]");
            self.record(result);
        }
        syn::visit::visit_item_type(self, item);
    }
}

fn is_recursa_attribute(path: &syn::Path, name: &str) -> bool {
    path.segments.len() == 2
        && path.segments[0].ident == "recursa"
        && path.segments[1].ident == name
}

fn expr_path_is(expression: &syn::Expr, segments: &[&str]) -> bool {
    let syn::Expr::Path(path) = expression else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == segments.len()
        && path
            .path
            .segments
            .iter()
            .zip(segments)
            .all(|(actual, expected)| actual.ident == *expected)
}

fn reviewed_meta_tags(expression: &syn::Expr) -> bool {
    const REVIEWED: &[&str] = &[
        "dcl",
        "ddl",
        "dml",
        "dql",
        "procedural",
        "session",
        "tcl",
        "utility",
    ];
    let syn::Expr::Array(array) = expression else {
        return false;
    };
    array.elems.len() == 1
        && matches!(
            &array.elems[0],
            syn::Expr::Lit(literal)
                if matches!(&literal.lit, syn::Lit::Str(value) if REVIEWED.contains(&value.value().as_str()))
        )
}

fn parser_option_error_code(option: &syn::Meta) -> &'static str {
    if option.path().is_ident("postcondition") {
        "unsupported.parser-postcondition"
    } else if option.path().is_ident("custom") {
        "unsupported.custom-parser-option"
    } else if option.path().is_ident("rules") {
        "rewrite.unhandled-legacy-shape"
    } else {
        "unsupported.obsolete-parser-attribute"
    }
}

struct ObsoleteSyntaxDetector {
    source_len: usize,
    line_starts: Vec<usize>,
    error: Option<GrammarRewriteError>,
}

impl ObsoleteSyntaxDetector {
    fn new(source: &str) -> Self {
        Self {
            source_len: source.len(),
            line_starts: line_starts(source),
            error: None,
        }
    }

    fn finish(self) -> Result<(), GrammarRewriteError> {
        self.error.map_or(Ok(()), Err)
    }

    fn reject(&mut self, code: &'static str, span: Span, message: &'static str) {
        if self.error.is_none() {
            self.error = Some(GrammarRewriteError::new(
                code,
                span_range(span, &self.line_starts, self.source_len)
                    .ok()
                    .map(|range| range.start),
                message,
            ));
        }
    }
}

impl<'ast> Visit<'ast> for ObsoleteSyntaxDetector {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if is_recursa_parser(attribute.path()) {
            if let syn::Meta::List(list) = &attribute.meta {
                if let Some(span) = token_ident_span(&list.tokens, "postcondition") {
                    self.reject(
                        "unsupported.parser-postcondition",
                        span,
                        "authored parser postconditions are obsolete",
                    );
                    return;
                }
                if let Some(span) = token_ident_span(&list.tokens, "custom") {
                    self.reject(
                        "unsupported.custom-parser-option",
                        span,
                        "authored custom parser options are obsolete",
                    );
                    return;
                }
                if let Some(span) = token_ident_span(&list.tokens, "rules") {
                    self.reject(
                        "rewrite.unhandled-legacy-shape",
                        span,
                        "legacy parser rule bindings are obsolete",
                    );
                    return;
                }
            }
            self.reject(
                "unsupported.obsolete-parser-attribute",
                attribute.path().span(),
                "remaining authored recursa::parser attributes are obsolete",
            );
            return;
        }

        if attribute.path().is_ident("lex")
            && let syn::Meta::List(list) = &attribute.meta
            && let Some(span) = token_ident_span(&list.tokens, "callback")
        {
            self.reject(
                "unsupported.inline-callback",
                span,
                "authored lexical callbacks are obsolete",
            );
        }
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if item
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "tokens")
        {
            for &(name, code, message) in OBSOLETE_TOKEN_SYNTAX {
                if let Some(span) = token_ident_span(&item.tokens, name) {
                    self.reject(code, span, message);
                    break;
                }
            }
        }
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.ident == "__firstset" {
            self.reject(
                "rewrite.unhandled-legacy-shape",
                item.ident.span(),
                "legacy generated first-set modules are obsolete",
            );
        }
        syn::visit::visit_item_mod(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if let Some(span) = use_tree_ident_span(&item.tree, "__firstset") {
            self.reject(
                "rewrite.unhandled-legacy-shape",
                span,
                "legacy generated first-set imports are obsolete",
            );
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if item.ident == "SqlRules" {
            self.reject(
                "rewrite.unhandled-legacy-shape",
                item.ident.span(),
                "legacy SqlRules marker types are obsolete",
            );
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_type_path(&mut self, type_path: &'ast syn::TypePath) {
        if let Some(segment) = type_path.path.segments.last()
            && matches!(
                segment.ident.to_string().as_str(),
                "Seq0" | "Seq1" | "Surrounded" | "OptionalTrailing"
            )
        {
            self.reject(
                "rewrite.unhandled-legacy-shape",
                segment.ident.span(),
                "legacy grammar containers are obsolete",
            );
        }
        syn::visit::visit_type_path(self, type_path);
    }
}

fn is_recursa_parser(path: &syn::Path) -> bool {
    path.segments.len() == 2
        && path.segments[0].ident == "recursa"
        && path.segments[1].ident == "parser"
}

const OBSOLETE_TOKEN_SYNTAX: &[(&str, &str, &str)] = &[
    (
        "callbacks",
        "unsupported.callback-declarations",
        "authored callback declarations are obsolete",
    ),
    (
        "post_lex",
        "unsupported.post-lex-hook",
        "authored post-lex hooks are obsolete",
    ),
    (
        "with",
        "unsupported.central-callback",
        "authored central lexical callbacks are obsolete",
    ),
    (
        "lexer_tokens",
        "unsupported.lexer-tokens",
        "legacy lexer token declarations are obsolete",
    ),
    (
        "soft_keywords",
        "unsupported.soft-keywords",
        "legacy soft keyword declarations are obsolete",
    ),
    (
        "targets",
        "unsupported.token-targets",
        "legacy token targets are obsolete",
    ),
    (
        "classes",
        "unsupported.token-classes",
        "legacy token classes are obsolete",
    ),
    (
        "literals",
        "unsupported.token-literals",
        "legacy literal declarations are obsolete",
    ),
];

fn token_ident_span(tokens: &TokenStream, expected: &str) -> Option<Span> {
    for token in tokens.clone() {
        match token {
            TokenTree::Ident(ident) if ident == expected => return Some(ident.span()),
            TokenTree::Group(group) => {
                if let Some(span) = token_ident_span(&group.stream(), expected) {
                    return Some(span);
                }
            }
            _ => {}
        }
    }
    None
}

fn use_tree_ident_span(tree: &syn::UseTree, expected: &str) -> Option<Span> {
    match tree {
        syn::UseTree::Path(path) if path.ident == expected => Some(path.ident.span()),
        syn::UseTree::Path(path) => use_tree_ident_span(&path.tree, expected),
        syn::UseTree::Name(name) if name.ident == expected => Some(name.ident.span()),
        syn::UseTree::Rename(rename) if rename.ident == expected => Some(rename.ident.span()),
        syn::UseTree::Group(group) => group
            .items
            .iter()
            .find_map(|item| use_tree_ident_span(item, expected)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
struct FixedTokenReference {
    name: String,
}

struct FixedSyntaxPlanner<'a> {
    source: &'a str,
    rows: &'a [CanonicalSemanticRow],
    line_starts: Vec<usize>,
    edits: Vec<SpanEdit>,
    accounted_rows: BTreeSet<String>,
    error: Option<GrammarRewriteError>,
}

struct FixedSyntaxPlan {
    edits: Vec<SpanEdit>,
    accounted_rows: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct AttachedSyntax {
    tokens: Vec<FixedTokenReference>,
    optional: bool,
}

#[derive(Default)]
struct FieldAttachment {
    prefix: Vec<AttachedSyntax>,
    suffix: Vec<AttachedSyntax>,
}

enum FieldDisposition {
    Semantic,
    Presence {
        row: CanonicalSemanticRow,
        tokens: Vec<FixedTokenReference>,
        rename_to_negative: bool,
    },
    NestedOptionalSyntax {
        row: CanonicalSemanticRow,
        tokens: Vec<FixedTokenReference>,
        semantic_range: std::ops::Range<usize>,
    },
    Syntax {
        row: CanonicalSemanticRow,
        syntax: Vec<AttachedSyntax>,
    },
}

impl FieldDisposition {
    fn is_semantic(&self) -> bool {
        !matches!(self, Self::Syntax { .. })
    }

    fn is_presence(&self) -> bool {
        matches!(self, Self::Presence { .. })
    }
}

impl<'a> FixedSyntaxPlanner<'a> {
    fn new(source: &'a str, rows: &'a [CanonicalSemanticRow]) -> Self {
        Self {
            source,
            rows,
            line_starts: line_starts(source),
            edits: Vec::new(),
            accounted_rows: BTreeSet::new(),
            error: None,
        }
    }

    fn finish(self) -> Result<FixedSyntaxPlan, GrammarRewriteError> {
        self.error.map_or(
            Ok(FixedSyntaxPlan {
                edits: self.edits,
                accounted_rows: self.accounted_rows,
            }),
            Err,
        )
    }

    fn range(&self, span: Span) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
        span_range(span, &self.line_starts, self.source.len())
    }

    fn row_for_field(
        &self,
        field: &syn::Field,
        accepted_rules: &[&str],
    ) -> Result<Option<CanonicalSemanticRow>, GrammarRewriteError> {
        let range = self.range(field.span())?;
        let candidates = self
            .rows
            .iter()
            .filter(|row| {
                accepted_rules.contains(&row.rule_id.as_str())
                    && range.start <= row.location.byte_start
                    && row.location.byte_end <= range.end
            })
            .cloned()
            .collect::<Vec<_>>();
        if candidates.len() > 1 {
            return Err(GrammarRewriteError::new(
                "inventory.ambiguous-field-disposition",
                Some(range.start),
                "multiple canonical semantic rows claim the same fixed-syntax field",
            ));
        }
        Ok(candidates.into_iter().next())
    }

    fn claim(&mut self, row: &CanonicalSemanticRow) -> Result<(), GrammarRewriteError> {
        if !self.accounted_rows.insert(row.id.clone()) {
            return Err(GrammarRewriteError::new(
                "inventory.duplicate-field-disposition",
                Some(row.location.byte_start),
                format!(
                    "canonical semantic row `{}` was applied more than once",
                    row.id
                ),
            ));
        }
        Ok(())
    }

    fn classify_field(&self, field: &syn::Field) -> Result<FieldDisposition, GrammarRewriteError> {
        let Some(row) = self.row_for_field(
            field,
            &[
                "syntax.fixed-token",
                "syntax.fixed-token-container",
                "syntax.optional-fixed-token",
                "semantic.optional-fixed-token.bool",
                "semantic.optional-fixed-token.sign-bool",
                "semantic.optional-fixed-token.nested-syntax-exclusion",
            ],
        )?
        else {
            return Ok(FieldDisposition::Semantic);
        };
        let offset = self.range(field.span())?.start;
        let actual_shape = field.ty.to_token_stream().to_string();
        if actual_shape != row.legacy_shape {
            return Err(GrammarRewriteError::new(
                "inventory.field-shape-drift",
                Some(offset),
                format!(
                    "canonical semantic row `{}` expected `{}` but found `{actual_shape}`",
                    row.id, row.legacy_shape
                ),
            ));
        }
        match row.rule_id.as_str() {
            "syntax.fixed-token" | "syntax.fixed-token-container" => {
                let syntax = fixed_syntax_sequence(&field.ty).ok_or_else(|| {
                    GrammarRewriteError::new(
                        "unsupported.fixed-token-shape",
                        Some(offset),
                        "inventory-classified fixed syntax is not an exact reviewed token sequence",
                    )
                })?;
                if row.rule_id == "syntax.fixed-token" && syntax.iter().any(|entry| entry.optional)
                {
                    return Err(GrammarRewriteError::new(
                        "inventory.fixed-token-rule-mismatch",
                        Some(offset),
                        "a fixed-token row unexpectedly contains optional syntax",
                    ));
                }
                Ok(FieldDisposition::Syntax { row, syntax })
            }
            "syntax.optional-fixed-token" => {
                let tokens = optional_fixed_token_sequence(&field.ty).ok_or_else(|| {
                    GrammarRewriteError::new(
                        "unsupported.optional-fixed-token-shape",
                        Some(offset),
                        "inventory-classified erased syntax is not one exact optional token sequence",
                    )
                })?;
                Ok(FieldDisposition::Syntax {
                    row,
                    syntax: vec![AttachedSyntax {
                        tokens,
                        optional: true,
                    }],
                })
            }
            "semantic.optional-fixed-token.bool" | "semantic.optional-fixed-token.sign-bool" => {
                let tokens = optional_fixed_token_sequence(&field.ty).ok_or_else(|| {
                    GrammarRewriteError::new(
                        "unsupported.presence-token-shape",
                        Some(offset),
                        "inventory-classified presence is not one exact optional token sequence",
                    )
                })?;
                let rename_to_negative = row.rule_id == "semantic.optional-fixed-token.sign-bool";
                if rename_to_negative
                    && field
                        .ident
                        .as_ref()
                        .is_none_or(|identifier| identifier.unraw() != "minus")
                {
                    return Err(GrammarRewriteError::new(
                        "unsupported.sign-presence-field",
                        Some(offset),
                        "reviewed sign presence must originate at a field named `minus`",
                    ));
                }
                Ok(FieldDisposition::Presence {
                    row,
                    tokens,
                    rename_to_negative,
                })
            }
            "semantic.optional-fixed-token.nested-syntax-exclusion" => {
                let (tokens, semantic) = nested_optional_syntax_projection(&field.ty).ok_or_else(|| {
                    GrammarRewriteError::new(
                        "unsupported.nested-optional-fixed-token-shape",
                        Some(offset),
                        "reviewed nested optional syntax no longer has its exact semantic projection",
                    )
                })?;
                Ok(FieldDisposition::NestedOptionalSyntax {
                    row,
                    tokens,
                    semantic_range: self.range(semantic.span())?,
                })
            }
            _ => unreachable!("the field query admits only closed fixed-syntax rules"),
        }
    }

    fn plan_fields(
        &mut self,
        fields: &syn::Fields,
        unit_attribute_insertion: Option<usize>,
    ) -> Result<(), GrammarRewriteError> {
        let field_list: Vec<_> = fields.iter().collect();
        if field_list.is_empty() {
            return Ok(());
        }
        let dispositions = field_list
            .iter()
            .map(|field| self.classify_field(field))
            .collect::<Result<Vec<_>, _>>()?;

        if dispositions
            .iter()
            .all(|disposition| matches!(disposition, FieldDisposition::Syntax { syntax, .. } if syntax.iter().all(|entry| !entry.optional)))
        {
            let tokens = dispositions
                .iter()
                .flat_map(|disposition| match disposition {
                    FieldDisposition::Syntax { syntax, .. } => syntax
                        .iter()
                        .flat_map(|entry| entry.tokens.clone())
                        .collect(),
                    _ => Vec::new(),
                })
                .collect::<Vec<_>>();
            let fields_range = self.range(fields.span())?;
            self.edits.push(SpanEdit {
                start: fields_range.start,
                end: fields_range.end,
                replacement: String::new(),
            });
            let Some(insertion) = unit_attribute_insertion else {
                return Err(GrammarRewriteError::new(
                    "unsupported.fixed-token-only-struct",
                    Some(fields_range.start),
                    "a fixed-token-only struct requires an explicit reviewed remodel",
                ));
            };
            self.edits.push(SpanEdit {
                start: insertion,
                end: insertion,
                replacement: fixed_token_attribute(&tokens),
            });
            for disposition in &dispositions {
                if let FieldDisposition::Syntax { row, .. } = disposition {
                    self.claim(row)?;
                }
            }
            return Ok(());
        }

        if dispositions
            .iter()
            .all(|disposition| !disposition.is_semantic())
        {
            let row = dispositions
                .iter()
                .find_map(|disposition| match disposition {
                    FieldDisposition::Syntax { row, .. } => Some(row),
                    _ => None,
                });
            return Err(GrammarRewriteError::new(
                "unsupported.optional-fixed-token-only-node",
                row.map(|row| row.location.byte_start),
                "a Node cannot consist only of erased optional fixed syntax",
            ));
        }

        let mut attachments = (0..field_list.len())
            .map(|_| FieldAttachment::default())
            .collect::<Vec<_>>();
        let mut index = 0;
        while index < dispositions.len() {
            if dispositions[index].is_semantic() {
                index += 1;
                continue;
            }
            let start = index;
            let mut syntax = Vec::new();
            while index < dispositions.len() && !dispositions[index].is_semantic() {
                if let FieldDisposition::Syntax { syntax: entry, .. } = &dispositions[index] {
                    syntax.extend(entry.iter().cloned());
                }
                index += 1;
            }
            let previous = (0..start)
                .rev()
                .find(|candidate| dispositions[*candidate].is_semantic());
            let next = (index..dispositions.len())
                .find(|candidate| dispositions[*candidate].is_semantic());
            if let Some(previous) = previous
                && (next.is_none() || dispositions[previous].is_presence())
            {
                attachments[previous].suffix.extend(syntax);
            } else if let Some(next) = next {
                attachments[next].prefix.extend(syntax);
            }
        }

        for (index, disposition) in dispositions.iter().enumerate() {
            match disposition {
                FieldDisposition::Semantic => {}
                FieldDisposition::Presence {
                    row,
                    tokens: _,
                    rename_to_negative,
                } => {
                    let type_range = self.range(field_list[index].ty.span())?;
                    self.edits.push(SpanEdit {
                        start: type_range.start,
                        end: type_range.end,
                        replacement: "bool".into(),
                    });
                    if *rename_to_negative {
                        let identifier = field_list[index]
                            .ident
                            .as_ref()
                            .expect("reviewed sign fields are named");
                        let identifier_range = self.range(identifier.span())?;
                        self.edits.push(SpanEdit {
                            start: identifier_range.start,
                            end: identifier_range.end,
                            replacement: "negative".into(),
                        });
                    }
                    self.claim(row)?;
                }
                FieldDisposition::NestedOptionalSyntax {
                    row,
                    tokens,
                    semantic_range,
                } => {
                    let type_range = self.range(field_list[index].ty.span())?;
                    self.edits.push(SpanEdit {
                        start: type_range.start,
                        end: type_range.end,
                        replacement: format!("Option<{}>", &self.source[semantic_range.clone()]),
                    });
                    attachments[index].prefix.push(AttachedSyntax {
                        tokens: tokens.clone(),
                        optional: true,
                    });
                    self.claim(row)?;
                }
                FieldDisposition::Syntax { row, .. } => {
                    let has_later_semantic = (index + 1..dispositions.len())
                        .any(|candidate| dispositions[candidate].is_semantic());
                    if has_later_semantic {
                        self.delete_field_with_following_comma(field_list[index])?;
                    } else {
                        let previous = index
                            .checked_sub(1)
                            .map(|candidate| field_list[candidate])
                            .ok_or_else(|| {
                                GrammarRewriteError::new(
                                    "unsupported.fixed-token-field-boundary",
                                    self.range(field_list[index].span())
                                        .ok()
                                        .map(|range| range.start),
                                    "trailing fixed syntax has no preceding field boundary",
                                )
                            })?;
                        self.delete_field_with_preceding_comma(field_list[index], previous)?;
                    }
                    self.claim(row)?;
                }
            }
        }

        for (index, attachment) in attachments.iter().enumerate() {
            let mut attributes = Vec::new();
            if !attachment.prefix.is_empty() || !attachment.suffix.is_empty() {
                attributes.push(attachment_attribute(&attachment.prefix, &attachment.suffix));
            }
            if let FieldDisposition::Presence { tokens, .. } = &dispositions[index] {
                attributes.push(presence_attribute(tokens));
            }
            if attributes.is_empty() {
                continue;
            }
            self.insert_field_attributes(field_list[index], &attributes)?;
        }
        Ok(())
    }

    fn fixed_only_tokens(
        &self,
        fields: &syn::Fields,
    ) -> Result<Option<Vec<FixedTokenReference>>, GrammarRewriteError> {
        let mut tokens = Vec::new();
        let mut saw_field = false;
        for field in fields {
            saw_field = true;
            if self
                .row_for_field(
                    field,
                    &["syntax.fixed-token", "syntax.fixed-token-container"],
                )?
                .is_none()
            {
                return Ok(None);
            }
            let Some(field_tokens) = fixed_token_sequence(&field.ty) else {
                return Ok(None);
            };
            tokens.extend(field_tokens);
        }
        Ok((saw_field && !tokens.is_empty()).then_some(tokens))
    }

    fn plan_fixed_only_struct(
        &mut self,
        item: &syn::ItemStruct,
        tokens: &[FixedTokenReference],
    ) -> Result<(), GrammarRewriteError> {
        let keyword = self.range(item.struct_token.span)?;
        self.edits.push(SpanEdit {
            start: keyword.start,
            end: keyword.end,
            replacement: "enum".into(),
        });
        let fields = self.range(item.fields.span())?;
        self.edits.push(SpanEdit {
            start: fields.start,
            end: fields.end,
            replacement: format!("{{ {}Value, }}", fixed_token_attribute(tokens)),
        });
        for field in &item.fields {
            let row = self
                .row_for_field(
                    field,
                    &["syntax.fixed-token", "syntax.fixed-token-container"],
                )?
                .ok_or_else(|| {
                    GrammarRewriteError::new(
                        "inventory.missing-fixed-only-row",
                        self.range(field.span()).ok().map(|range| range.start),
                        "fixed-token-only field lost its canonical inventory row",
                    )
                })?;
            self.claim(&row)?;
        }
        Ok(())
    }

    fn insert_field_attributes(
        &mut self,
        field: &syn::Field,
        attributes: &[String],
    ) -> Result<(), GrammarRewriteError> {
        let start = self.range(field.span())?.start;
        let line_prefix = self.source[..start]
            .rsplit_once('\n')
            .map_or(self.source[..start].as_ref(), |(_, prefix)| prefix);
        let at_line_start = line_prefix.chars().all(char::is_whitespace);
        let indentation = if at_line_start { line_prefix } else { "" };
        let replacement = if at_line_start {
            format!("{}{}", attributes.join(indentation), indentation)
        } else {
            format!(
                "{} ",
                attributes
                    .iter()
                    .map(|attribute| attribute.trim_end())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        self.edits.push(SpanEdit {
            start,
            end: start,
            replacement,
        });
        Ok(())
    }

    fn field_core_range(
        &self,
        field: &syn::Field,
    ) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
        let start = match (&field.vis, &field.ident) {
            (syn::Visibility::Inherited, Some(identifier)) => self.range(identifier.span())?.start,
            (syn::Visibility::Inherited, None) => self.range(field.ty.span())?.start,
            (visibility, _) => self.range(visibility.span())?.start,
        };
        Ok(start..self.range(field.span())?.end)
    }

    fn delete_field_with_following_comma(
        &mut self,
        field: &syn::Field,
    ) -> Result<(), GrammarRewriteError> {
        let range = self.field_core_range(field)?;
        let tail = &self.source[range.end..];
        let comma = tail.find(',').ok_or_else(|| {
            GrammarRewriteError::new(
                "unsupported.fixed-token-field-boundary",
                Some(range.end),
                "reviewed fixed syntax field has no following comma",
            )
        })?;
        if !tail[..comma].chars().all(char::is_whitespace) {
            return Err(GrammarRewriteError::new(
                "unsupported.fixed-token-field-boundary",
                Some(range.end),
                "unexpected source content before fixed syntax field comma",
            ));
        }
        self.edits.push(SpanEdit {
            start: range.start,
            end: range.end + comma + 1,
            replacement: String::new(),
        });
        Ok(())
    }

    fn delete_field_with_preceding_comma(
        &mut self,
        field: &syn::Field,
        previous: &syn::Field,
    ) -> Result<(), GrammarRewriteError> {
        let range = self.field_core_range(field)?;
        let previous_end = self.range(previous.span())?.end;
        let prefix = &self.source[previous_end..range.start];
        let comma = prefix.rfind(',').ok_or_else(|| {
            GrammarRewriteError::new(
                "unsupported.fixed-token-field-boundary",
                Some(range.start),
                "reviewed trailing fixed syntax field has no preceding comma",
            )
        })?;
        if !prefix[comma + 1..].chars().all(char::is_whitespace) {
            return Err(GrammarRewriteError::new(
                "unsupported.fixed-token-field-boundary",
                Some(range.start),
                "unexpected source content after fixed syntax field comma",
            ));
        }
        self.edits.push(SpanEdit {
            start: previous_end + comma,
            end: range.end,
            replacement: String::new(),
        });
        Ok(())
    }
}

impl<'ast> Visit<'ast> for FixedSyntaxPlanner<'_> {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if self.error.is_some() {
            return;
        }
        let result = match self.fixed_only_tokens(&item.fields) {
            Ok(Some(tokens)) => self.plan_fixed_only_struct(item, &tokens),
            Ok(None) => self.plan_fields(&item.fields, None),
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if self.error.is_some() {
            return;
        }
        for variant in &item.variants {
            let insertion = if let Some(attribute) = variant.attrs.first() {
                self.range(attribute.span()).ok().map(|range| range.start)
            } else {
                self.range(variant.ident.span())
                    .ok()
                    .map(|range| range.start)
            };
            if let Err(error) = self.plan_fields(&variant.fields, insertion) {
                self.error = Some(error);
                return;
            }
        }
    }
}

fn fixed_token_sequence(ty: &syn::Type) -> Option<Vec<FixedTokenReference>> {
    match ty {
        syn::Type::Tuple(tuple) => {
            let mut tokens = Vec::new();
            for element in &tuple.elems {
                tokens.extend(fixed_token_sequence(element)?);
            }
            (!tokens.is_empty()).then_some(tokens)
        }
        syn::Type::Path(path)
            if path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "Surrounded")
                && is_reviewed_container_path(path, "Surrounded") =>
        {
            let segment = path.path.segments.last()?;
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            let types: Vec<_> = arguments
                .args
                .iter()
                .map(|argument| match argument {
                    syn::GenericArgument::Type(ty) => Some(ty),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            if types.len() != 3 {
                return None;
            }
            let mut tokens = fixed_token_sequence(types[0])?;
            if !is_unit_type(types[1]) {
                tokens.extend(fixed_token_sequence(types[1])?);
            }
            tokens.extend(fixed_token_sequence(types[2])?);
            Some(tokens)
        }
        syn::Type::Path(path)
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path
                    .path
                    .segments
                    .iter()
                    .all(|segment| matches!(segment.arguments, syn::PathArguments::None)) =>
        {
            let names: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            match names.as_slice() {
                [name]
                    if name.chars().all(|character| {
                        !character.is_ascii_alphabetic() || character.is_ascii_uppercase()
                    }) =>
                {
                    Some(vec![FixedTokenReference { name: name.clone() }])
                }
                [module, name] if module == "punct" => Some(vec![FixedTokenReference {
                    name: name.to_ascii_uppercase(),
                }]),
                [module, name] if matches!(module.as_str(), "keyword" | "soft_keyword") => {
                    Some(vec![FixedTokenReference { name: name.clone() }])
                }
                [root, tokens, module, name]
                    if root == "crate" && tokens == "tokens" && module == "punct" =>
                {
                    Some(vec![FixedTokenReference {
                        name: name.to_ascii_uppercase(),
                    }])
                }
                [root, tokens, module, name]
                    if root == "crate"
                        && tokens == "tokens"
                        && matches!(module.as_str(), "keyword" | "soft_keyword") =>
                {
                    Some(vec![FixedTokenReference { name: name.clone() }])
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn optional_fixed_token_sequence(ty: &syn::Type) -> Option<Vec<FixedTokenReference>> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if !is_reviewed_container_path(path, "Option") {
        return None;
    }
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(inner) = &arguments.args[0] else {
        return None;
    };
    fixed_token_sequence(inner)
}

fn fixed_syntax_sequence(ty: &syn::Type) -> Option<Vec<AttachedSyntax>> {
    if let Some(tokens) = fixed_token_sequence(ty) {
        return Some(vec![AttachedSyntax {
            tokens,
            optional: false,
        }]);
    }
    if let Some(tokens) = optional_fixed_token_sequence(ty) {
        return Some(vec![AttachedSyntax {
            tokens,
            optional: true,
        }]);
    }
    let syn::Type::Tuple(tuple) = ty else {
        return None;
    };
    let mut syntax = Vec::new();
    for element in &tuple.elems {
        syntax.extend(fixed_syntax_sequence(element)?);
    }
    (!syntax.is_empty()).then_some(syntax)
}

fn nested_optional_syntax_projection(
    ty: &syn::Type,
) -> Option<(Vec<FixedTokenReference>, &syn::Type)> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if !is_reviewed_container_path(path, "Option") {
        return None;
    }
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(syn::Type::Tuple(tuple)) = arguments.args.first()? else {
        return None;
    };
    if tuple.elems.len() != 2 {
        return None;
    }
    let tokens = optional_fixed_token_sequence(&tuple.elems[0])?;
    let semantic = &tuple.elems[1];
    fixed_token_sequence(semantic)
        .is_none()
        .then_some((tokens, semantic))
}

fn render_attached_syntax(syntax: &AttachedSyntax) -> String {
    let tokens = syntax
        .tokens
        .iter()
        .map(|token| token.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if syntax.optional {
        format!("optional({tokens})")
    } else {
        tokens
    }
}

fn attachment_attribute(prefix: &[AttachedSyntax], suffix: &[AttachedSyntax]) -> String {
    let mut sequence = prefix
        .iter()
        .map(render_attached_syntax)
        .collect::<Vec<_>>();
    sequence.push("this".into());
    sequence.extend(suffix.iter().map(render_attached_syntax));
    format!("#[tok({})]\n", sequence.join(", "))
}

fn presence_attribute(tokens: &[FixedTokenReference]) -> String {
    let names = tokens
        .iter()
        .map(|token| token.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("#[presence({names})]\n")
}

fn fixed_token_attribute(tokens: &[FixedTokenReference]) -> String {
    let names = tokens
        .iter()
        .map(|token| token.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("#[tok({names})] ")
}

fn token_around_attribute(
    prefix: &[FixedTokenReference],
    suffix: &[FixedTokenReference],
) -> String {
    let prefix = prefix
        .iter()
        .map(|token| token.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let suffix = suffix
        .iter()
        .map(|token| token.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let sequence = match (prefix.is_empty(), suffix.is_empty()) {
        (false, false) => format!("{prefix}, this, {suffix}"),
        (false, true) => format!("{prefix}, this"),
        (true, false) => format!("this, {suffix}"),
        (true, true) => "this".into(),
    };
    format!("#[tok({sequence})]\n")
}

fn semantic_tuple_projection(
    tuple: &syn::TypeTuple,
) -> Option<(
    &syn::Type,
    Vec<FixedTokenReference>,
    Vec<FixedTokenReference>,
)> {
    let semantic_indices: Vec<_> = tuple
        .elems
        .iter()
        .enumerate()
        .filter_map(|(index, element)| fixed_token_sequence(element).is_none().then_some(index))
        .collect();
    if semantic_indices.len() != 1 {
        return None;
    }
    let semantic_index = semantic_indices[0];
    let mut prefix = Vec::new();
    for element in tuple.elems.iter().take(semantic_index) {
        prefix.extend(fixed_token_sequence(element)?);
    }
    let mut suffix = Vec::new();
    for element in tuple.elems.iter().skip(semantic_index + 1) {
        suffix.extend(fixed_token_sequence(element)?);
    }
    if prefix.is_empty() && suffix.is_empty() {
        return None;
    }
    Some((&tuple.elems[semantic_index], prefix, suffix))
}

struct LegacyPunctuationEntry {
    name: syn::Ident,
    pattern: syn::LitStr,
    spelling: syn::LitStr,
}

impl Parse for LegacyPunctuationEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = syn::Ident::parse_any(input)?;
        input.parse::<Token![=>]>()?;
        let pattern = input.parse()?;
        input.parse::<Token![,]>()?;
        let spelling = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        } else if !input.is_empty() {
            return Err(input.error("expected `,` after legacy punctuation declaration"));
        }
        Ok(Self {
            name,
            pattern,
            spelling,
        })
    }
}

struct LegacyPunctuationSection(Vec<LegacyPunctuationEntry>);

impl Parse for LegacyPunctuationSection {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            entries.push(input.parse()?);
        }
        Ok(Self(entries))
    }
}

#[derive(Debug)]
struct LegacyContentEntry {
    name: String,
    pattern: String,
    callback: Option<String>,
}

struct LegacyContentSection(Vec<LegacyContentEntry>);

impl Parse for LegacyContentSection {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            let _attributes = input.call(syn::Attribute::parse_outer)?;
            let name = syn::Ident::parse_any(input)?.to_string();
            input.parse::<Token![=>]>()?;
            let pattern: syn::LitStr = input.parse()?;
            let callback = if input.peek(syn::Ident) {
                let marker = syn::Ident::parse_any(input)?;
                if marker != "with" {
                    return Err(syn::Error::new(marker.span(), "expected `with path`"));
                }
                let path: syn::Path = input.parse()?;
                Some(
                    path.to_token_stream()
                        .to_string()
                        .chars()
                        .filter(|character| !character.is_whitespace())
                        .collect(),
                )
            } else {
                None
            };
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            } else if !input.is_empty() {
                return Err(input.error("expected `,` after legacy content declaration"));
            }
            entries.push(LegacyContentEntry {
                name,
                pattern: pattern.value(),
                callback,
            });
        }
        Ok(Self(entries))
    }
}

#[derive(Clone)]
struct TokenMacroSection {
    name_span: Span,
    group: proc_macro2::Group,
    trailing_comma: Option<Span>,
}

struct TokenMacroPlanner<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
    edits: Vec<SpanEdit>,
    error: Option<GrammarRewriteError>,
}

impl<'a> TokenMacroPlanner<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            line_starts: line_starts(source),
            edits: Vec::new(),
            error: None,
        }
    }

    fn finish(self) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
        self.error.map_or(Ok(self.edits), Err)
    }

    fn range(&self, span: Span) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
        span_range(span, &self.line_starts, self.source.len())
    }

    fn plan_macro(&mut self, item: &syn::Macro) -> Result<(), GrammarRewriteError> {
        let trees: Vec<_> = item.tokens.clone().into_iter().collect();
        let mut sections = BTreeMap::new();
        let mut index = 0;
        while index + 1 < trees.len() {
            let TokenTree::Ident(name) = &trees[index] else {
                index += 1;
                continue;
            };
            let TokenTree::Group(group) = &trees[index + 1] else {
                index += 1;
                continue;
            };
            let trailing_comma = trees.get(index + 2).and_then(|token| match token {
                TokenTree::Punct(punctuation) if punctuation.as_char() == ',' => {
                    Some(punctuation.span())
                }
                _ => None,
            });
            sections.insert(
                name.to_string(),
                TokenMacroSection {
                    name_span: name.span(),
                    group: group.clone(),
                    trailing_comma,
                },
            );
            index += 2;
        }
        let required = [
            "keywords",
            "soft_keywords",
            "punctuation",
            "literals",
            "lexer_tokens",
            "classes",
            "targets",
        ];
        if required.iter().any(|name| !sections.contains_key(*name)) {
            return Ok(());
        }
        let soft = sections.get("soft_keywords").unwrap();
        if count_fat_arrows(&soft.group.stream()) < 300 {
            return Ok(());
        }
        self.validate_content_sections(&sections)?;
        self.plan_soft_keyword_merge(&sections)?;
        self.plan_punctuation(&sections)?;
        let literals = sections.get("literals").unwrap();
        let targets = sections.get("targets").unwrap();
        let replacement_start = self.range(literals.name_span)?.start;
        let replacement_end = self.range(targets.group.span_close())?.end;
        self.edits.push(SpanEdit {
            start: replacement_start,
            end: replacement_end,
            replacement: REAL_CLOSED_TOKEN_SECTIONS.into(),
        });
        Ok(())
    }

    fn validate_content_sections(
        &self,
        sections: &BTreeMap<String, TokenMacroSection>,
    ) -> Result<(), GrammarRewriteError> {
        let literals =
            syn::parse2::<LegacyContentSection>(sections.get("literals").unwrap().group.stream())
                .map_err(|error| {
                GrammarRewriteError::new(
                    "unsupported.real-literal-declarations",
                    self.range(sections.get("literals").unwrap().name_span)
                        .ok()
                        .map(|range| range.start),
                    error.to_string(),
                )
            })?;
        let expected_literals = [
            (
                "DollarStringLit",
                r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$",
                Some("crate::tokens::scan_dollar_string"),
            ),
            (
                "UnicodeQuotedIdent",
                "(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"",
                None,
            ),
            ("QuotedIdent", "\"[^\"]*(?:\"\"[^\"]*)*\"", None),
            ("UnicodeStringLit", "(?i:U)&'(?:[^']|'')*'", None),
            ("EscapeStringLit", "(?i:E)'(?:[^'\\\\]|\\\\.|'')*'", None),
            ("BitStringLit", "(?i:B)'[^']*'", None),
            ("HexStringLit", "(?i:X)'[^']*'", None),
            ("StringLit", "'[^']*(?:''[^']*)*'", None),
            (
                "NumericLit",
                r"(?:(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\.)",
                Some("crate::tokens::reject_trailing_word"),
            ),
            (
                "IntegerLit",
                r"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)",
                Some("crate::tokens::reject_trailing_word"),
            ),
            (
                "DollarNum",
                r"\$[0-9]+",
                Some("crate::tokens::reject_trailing_word"),
            ),
            (
                "PsqlVar",
                ":(?:[A-Za-z_][A-Za-z0-9_]*|'[^']*'|\"[^\"]*\")",
                None,
            ),
        ];
        let actual_literals: Vec<_> = literals
            .0
            .iter()
            .map(|entry| {
                (
                    entry.name.as_str(),
                    entry.pattern.as_str(),
                    entry.callback.as_deref(),
                )
            })
            .collect();
        if actual_literals != expected_literals {
            return Err(GrammarRewriteError::new(
                "unsupported.real-literal-declarations",
                self.range(sections.get("literals").unwrap().name_span)?
                    .start
                    .into(),
                "the real literal declaration ledger changed",
            ));
        }
        let lexer = syn::parse2::<LegacyContentSection>(
            sections.get("lexer_tokens").unwrap().group.stream(),
        )
        .map_err(|error| {
            GrammarRewriteError::new(
                "unsupported.real-lexer-declarations",
                self.range(sections.get("lexer_tokens").unwrap().name_span)
                    .ok()
                    .map(|range| range.start),
                error.to_string(),
            )
        })?;
        let expected_lexer = [
            (
                "BlockComment",
                "/\\*",
                Some("crate::tokens::skip_block_comment"),
            ),
            ("UnquotedIdent", "[A-Za-z_][A-Za-z0-9_]*", None),
            (
                "CustomOp",
                "([-+*/<>=~!@#%^&|?]*[~!@#%^&|?][-+*/<>=~!@#%^&|?]*|[-+*/<>=]+[*/<>=])",
                None,
            ),
        ];
        let actual_lexer: Vec<_> = lexer
            .0
            .iter()
            .map(|entry| {
                (
                    entry.name.as_str(),
                    entry.pattern.as_str(),
                    entry.callback.as_deref(),
                )
            })
            .collect();
        if actual_lexer != expected_lexer {
            return Err(GrammarRewriteError::new(
                "unsupported.real-lexer-declarations",
                Some(
                    self.range(sections.get("lexer_tokens").unwrap().name_span)?
                        .start,
                ),
                "the real lexer-only declaration ledger changed",
            ));
        }
        let classes = compact_tokens(&sections.get("classes").unwrap().group.stream());
        if classes != "bare_label_keywords=keywordswherebare_label," {
            return Err(GrammarRewriteError::new(
                "unsupported.real-token-classes",
                Some(
                    self.range(sections.get("classes").unwrap().name_span)?
                        .start,
                ),
                "the real token class ledger changed",
            ));
        }
        let targets = compact_tokens(&sections.get("targets").unwrap().group.stream());
        let expected_targets = "ColId:literal::IdentadmitsUNRESERVED,COL_NAME,type_function_name:literal::IdentadmitsUNRESERVED,TYPE_FUNC_NAME,NonReservedWord:literal::IdentadmitsUNRESERVED,COL_NAME,TYPE_FUNC_NAME,ColLabel:literal::IdentadmitsUNRESERVED,COL_NAME,TYPE_FUNC_NAME,RESERVED,BareColLabel:literal::Identadmitsbare_label_keywords,";
        if targets != expected_targets {
            return Err(GrammarRewriteError::new(
                "unsupported.real-token-targets",
                Some(
                    self.range(sections.get("targets").unwrap().name_span)?
                        .start,
                ),
                "the real token target ledger changed",
            ));
        }
        Ok(())
    }

    fn plan_soft_keyword_merge(
        &mut self,
        sections: &BTreeMap<String, TokenMacroSection>,
    ) -> Result<(), GrammarRewriteError> {
        let keywords = sections.get("keywords").unwrap();
        let soft = sections.get("soft_keywords").unwrap();
        let close_start = self.range(keywords.group.span_close())?.start;
        let close_end = keywords.trailing_comma.map_or_else(
            || {
                self.range(keywords.group.span_close())
                    .map(|range| range.end)
            },
            |span| self.range(span).map(|range| range.end),
        )?;
        self.edits.push(SpanEdit {
            start: close_start,
            end: close_end,
            replacement: String::new(),
        });
        self.edits.push(SpanEdit {
            start: self.range(soft.name_span)?.start,
            end: self.range(soft.group.span_open())?.end,
            replacement: String::new(),
        });
        Ok(())
    }

    fn plan_punctuation(
        &mut self,
        sections: &BTreeMap<String, TokenMacroSection>,
    ) -> Result<(), GrammarRewriteError> {
        let section = sections.get("punctuation").unwrap();
        let entries =
            syn::parse2::<LegacyPunctuationSection>(section.group.stream()).map_err(|error| {
                GrammarRewriteError::new(
                    "unsupported.real-punctuation-declarations",
                    self.range(section.name_span).ok().map(|range| range.start),
                    error.to_string(),
                )
            })?;
        if entries.0.len() < 80 {
            return Err(GrammarRewriteError::new(
                "unsupported.real-punctuation-declarations",
                Some(self.range(section.name_span)?.start),
                "the real punctuation declaration ledger is incomplete",
            ));
        }
        for entry in entries.0 {
            let name_range = self.range(entry.name.span())?;
            self.edits.push(SpanEdit {
                start: name_range.start,
                end: name_range.end,
                replacement: entry.name.to_string().to_ascii_uppercase(),
            });
            let pattern_range = self.range(entry.pattern.span())?;
            let spelling_range = self.range(entry.spelling.span())?;
            self.edits.push(SpanEdit {
                start: pattern_range.start,
                end: spelling_range.end,
                replacement: self.source[spelling_range].into(),
            });
        }
        Ok(())
    }
}

impl<'ast> Visit<'ast> for TokenMacroPlanner<'_> {
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if self.error.is_some() || !expr_path_segments_are(&item.path, &["recursa", "tokens"]) {
            return;
        }
        if let Err(error) = self.plan_macro(item) {
            self.error = Some(error);
        }
    }
}

fn count_fat_arrows(stream: &TokenStream) -> usize {
    let trees: Vec<_> = stream.clone().into_iter().collect();
    trees
        .windows(2)
        .filter(|pair| {
            matches!(&pair[0], TokenTree::Punct(punctuation) if punctuation.as_char() == '=')
                && matches!(&pair[1], TokenTree::Punct(punctuation) if punctuation.as_char() == '>')
        })
        .count()
}

fn compact_tokens(stream: &TokenStream) -> String {
    stream
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

const REAL_CLOSED_TOKEN_SECTIONS: &str = r#"matchers {
        DollarStringLit => same_delimiter(opener = r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$"),
        NumericLit => next_exclusion(pattern = r"(?:(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\.)", excluded = r"[A-Za-z0-9_]"),
        IntegerLit => next_exclusion(pattern = r"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)", excluded = r"[A-Za-z0-9_]"),
        DollarNum => next_exclusion(pattern = r"\$[0-9]+", excluded = r"[A-Za-z0-9_]"),
        CustomOp => operator_run(
            characters = "-+*/<>=~!@#%^&|?",
            fences = ["/*", "--"],
            trailing = "+-",
            qualifying = "~!@#%^&|?"
        ),
    }
    ignore {
        // Nested comments remain non-emitting until classified trivia lands in #93.
        BlockComment => nested(opener = "/*", closer = "*/"),
    }
    admissions {
        AllWordKinds = keywords,
        ColId = UNRESERVED + COL_NAME,
        type_function_name = UNRESERVED + TYPE_FUNC_NAME,
        NonReservedWord = UNRESERVED + COL_NAME + TYPE_FUNC_NAME,
        ColLabel = UNRESERVED + COL_NAME + TYPE_FUNC_NAME + RESERVED,
        BareColLabel = bare_label,
        WindowRefName = ColId - { ROWS, RANGE, GROUPS },
        PsqlVariableName = AllWordKinds - { NULL, TRUE, FALSE },
        UnquotedIdent = NonReservedWord,
        BareAliasName = AllWordKinds,
    }"#;

struct ContainerPlanner<'a> {
    source: &'a str,
    structure: &'a StructuralSpans,
    line_starts: Vec<usize>,
    edits: Vec<SpanEdit>,
    values_row_fields: BTreeSet<usize>,
    error: Option<GrammarRewriteError>,
}

impl<'a> ContainerPlanner<'a> {
    fn new(source: &'a str, structure: &'a StructuralSpans) -> Self {
        Self {
            source,
            structure,
            line_starts: line_starts(source),
            edits: Vec::new(),
            values_row_fields: BTreeSet::new(),
            error: None,
        }
    }

    fn finish(self) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
        self.error.map_or(Ok(self.edits), Err)
    }

    fn plan_field(&mut self, field: &syn::Field) -> Result<(), GrammarRewriteError> {
        let field_start = self.range(field.span())?.start;
        if self.values_row_fields.contains(&field_start) {
            let type_range = self.range(field.ty.span())?;
            self.edits.push(SpanEdit {
                start: field_start,
                end: field_start,
                replacement: "#[sep(COMMA)]\n    ".into(),
            });
            self.edits.push(SpanEdit {
                start: type_range.start,
                end: type_range.end,
                replacement: "Vec<ValuesRow<'input>>".into(),
            });
            return Ok(());
        }
        let mut attributes = Vec::new();
        self.plan_type(&field.ty, &mut attributes)?;
        if !attributes.is_empty() {
            let line_prefix = self.source[..field_start]
                .rsplit_once('\n')
                .map_or(self.source[..field_start].as_ref(), |(_, prefix)| prefix);
            let at_line_start = line_prefix.chars().all(char::is_whitespace);
            let indentation = if at_line_start { line_prefix } else { "" };
            let attribute_text = if at_line_start {
                format!("{}{}", attributes.join(indentation), indentation)
            } else {
                format!(
                    "{} ",
                    attributes
                        .iter()
                        .map(|attribute| attribute.trim_end())
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };
            if let Some(edit) = self
                .edits
                .iter_mut()
                .find(|edit| edit.start == field_start && edit.end > field_start)
            {
                edit.replacement.insert_str(0, &attribute_text);
            } else {
                self.edits.push(SpanEdit {
                    start: field_start,
                    end: field_start,
                    replacement: attribute_text,
                });
            }
        }
        Ok(())
    }

    fn plan_type(
        &mut self,
        ty: &syn::Type,
        attributes: &mut Vec<String>,
    ) -> Result<(), GrammarRewriteError> {
        if let syn::Type::Tuple(tuple) = ty
            && let Some((semantic, prefix, suffix)) = semantic_tuple_projection(tuple)
        {
            attributes.push(token_around_attribute(&prefix, &suffix));
            let tuple_range = self.range(tuple.span())?;
            let semantic_range = self.range(semantic.span())?;
            self.edits.push(SpanEdit {
                start: tuple_range.start,
                end: tuple_range.end,
                replacement: self.source[semantic_range].into(),
            });
            self.plan_type(semantic, attributes)?;
            return Ok(());
        }
        let syn::Type::Path(type_path) = ty else {
            return Ok(());
        };
        let Some(segment) = type_path.path.segments.last() else {
            return Ok(());
        };
        let name = segment.ident.to_string();
        let is_container = matches!(name.as_str(), "Option" | "Surrounded" | "Seq0" | "Seq1");
        if is_container && !is_reviewed_container_path(type_path, &name) {
            return Err(GrammarRewriteError::new(
                "unsupported.qualified-container",
                Some(self.range(type_path.span())?.start),
                format!("container {name} does not use a reviewed legacy path"),
            ));
        }
        let arguments = if is_container {
            type_arguments(segment, self.range(segment.span())?.start)?
        } else {
            Vec::new()
        };
        match name.as_str() {
            "Option" => {
                if arguments.len() != 1 {
                    return Err(GrammarRewriteError::new(
                        "unsupported.option-arity",
                        Some(self.range(segment.span())?.start),
                        "Option must have exactly one type argument",
                    ));
                }
                self.plan_type(arguments[0], attributes)?;
            }
            "Surrounded" => {
                if arguments.len() != 3 {
                    return Err(GrammarRewriteError::new(
                        "unsupported.surrounded-arity",
                        Some(self.range(segment.span())?.start),
                        "Surrounded must have left, inner, and right type arguments",
                    ));
                }
                let left_offset = self.range(arguments[0].span())?.start;
                let left = grammar_token_name(arguments[0], left_offset)?.ok_or_else(|| {
                    GrammarRewriteError::new(
                        "unsupported.surrounded-left-token",
                        Some(left_offset),
                        "Surrounded left delimiter is not a reviewed token path",
                    )
                })?;
                let right_offset = self.range(arguments[2].span())?.start;
                let right = grammar_token_name(arguments[2], right_offset)?.ok_or_else(|| {
                    GrammarRewriteError::new(
                        "unsupported.surrounded-right-token",
                        Some(right_offset),
                        "Surrounded right delimiter is not a reviewed token path",
                    )
                })?;
                attributes.push(format!("#[tok({left}, this, {right})]\n"));

                let segment_range = self.range(segment.span())?;
                let inner_range = self.range(arguments[1].span())?;
                self.delete_syntax_tokens(segment_range.start..inner_range.start)?;
                self.delete_syntax_tokens(inner_range.end..segment_range.end)?;
                self.plan_type(arguments[1], attributes)?;
            }
            "Seq0" | "Seq1" => {
                if arguments.len() < 2 || arguments.len() > 3 {
                    return Err(GrammarRewriteError::new(
                        "unsupported.sequence-arity",
                        Some(self.range(segment.span())?.start),
                        format!("{name} must have item, separator, and optional disposition"),
                    ));
                }
                let separator_offset = self.range(arguments[1].span())?.start;
                let separator = if is_unit_type(arguments[1]) {
                    None
                } else {
                    Some(
                        grammar_token_name(arguments[1], separator_offset)?.ok_or_else(|| {
                            GrammarRewriteError::new(
                                "unsupported.token-path",
                                Some(separator_offset),
                                "sequence separator is not a reviewed token path or unit",
                            )
                        })?,
                    )
                };
                if name == "Seq1" && separator.as_deref() == Some("SEMI") {
                    return Err(GrammarRewriteError::new(
                        "rewrite.unhandled-legacy-shape",
                        Some(separator_offset),
                        "a semicolon-delimited non-empty sequence is not in the reviewed migration inventory",
                    ));
                }
                let trailing = if let Some(disposition) = arguments.get(2) {
                    if !is_reviewed_optional_trailing(disposition) {
                        return Err(GrammarRewriteError::new(
                            "unsupported.sequence-disposition",
                            Some(self.range(disposition.span())?.start),
                            "only OptionalTrailing is reviewed",
                        ));
                    }
                    ", trailing"
                } else {
                    ""
                };
                if let Some(separator) = separator {
                    attributes.push(format!("#[sep({separator}{trailing})]\n"));
                }

                let ident_range = self.range(segment.ident.span())?;
                self.edits.push(SpanEdit {
                    start: ident_range.start,
                    end: ident_range.end,
                    replacement: if name == "Seq1" {
                        "recursa::Vec1".into()
                    } else {
                        "Vec".into()
                    },
                });
                let item_end = self.range(arguments[0].span())?.end;
                let last_end = self.range(arguments.last().unwrap().span())?.end;
                self.delete_syntax_tokens(item_end..last_end)?;
            }
            "Box" if is_exact_bare_type_path(type_path, "Box") => {
                let arguments = type_arguments(segment, self.range(segment.span())?.start)?;
                if arguments.len() != 1 {
                    return Err(GrammarRewriteError::new(
                        "unsupported.box-arity",
                        Some(self.range(segment.span())?.start),
                        "Box must have exactly one type argument",
                    ));
                }
                self.plan_type(arguments[0], attributes)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn delete_syntax_tokens(
        &mut self,
        range: std::ops::Range<usize>,
    ) -> Result<(), GrammarRewriteError> {
        let selected: Vec<_> = self
            .structure
            .syntax
            .iter()
            .filter(|token| range.start <= token.start && token.end <= range.end)
            .cloned()
            .collect();
        if selected.is_empty() {
            return Err(GrammarRewriteError::new(
                "rewrite.missing-structural-token",
                Some(range.start),
                "reviewed container span contains no syntax token",
            ));
        }
        self.edits
            .extend(selected.into_iter().map(|token| SpanEdit {
                start: token.start,
                end: token.end,
                replacement: String::new(),
            }));
        Ok(())
    }

    fn range(&self, span: Span) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
        span_range(span, &self.line_starts, self.source.len())
    }

    fn plan_values_row_wrapper(
        &mut self,
        item: &syn::ItemStruct,
    ) -> Result<(), GrammarRewriteError> {
        let syn::Fields::Named(fields) = &item.fields else {
            return Err(GrammarRewriteError::new(
                "unsupported.values-row-shape",
                Some(self.range(item.ident.span())?.start),
                "ValuesBody must retain its reviewed named-field shape",
            ));
        };
        let item_offset = self.range(item.ident.span())?.start;
        let rows = fields
            .named
            .iter()
            .find(|field| field.ident.as_ref().is_some_and(|ident| ident == "rows"))
            .ok_or_else(|| {
                GrammarRewriteError::new(
                    "unsupported.values-row-shape",
                    Some(item_offset),
                    "ValuesBody must retain its reviewed rows field",
                )
            })?;
        let type_range = self.range(rows.ty.span())?;
        const REVIEWED_ROWS_TYPE: &str = "Seq0<\n        Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,\n        punct::Comma,\n    >";
        if self.source[type_range.clone()] != *REVIEWED_ROWS_TYPE {
            return Err(GrammarRewriteError::new(
                "unsupported.values-row-shape",
                Some(type_range.start),
                "nested VALUES rows changed since their normalized wrapper was reviewed",
            ));
        }
        let field_start = self.range(rows.span())?.start;
        self.values_row_fields.insert(field_start);
        let insertion = if let Some(attribute) = item.attrs.first() {
            self.range(attribute.span())?.start
        } else {
            self.range(item.span())?.start
        };
        self.edits.push(SpanEdit {
            start: insertion,
            end: insertion,
            replacement: "#[derive(recursa::Node, Debug, Clone)]\npub struct ValuesRow<'input> {\n    #[tok(LPAREN, this, RPAREN)]\n    #[sep(COMMA)]\n    pub values: Vec<Expr<'input>>,\n}\n\n".into(),
        });
        Ok(())
    }
}

impl<'ast> Visit<'ast> for ContainerPlanner<'_> {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if self.error.is_none()
            && item.ident == "ValuesBody"
            && let Err(error) = self.plan_values_row_wrapper(item)
        {
            self.error = Some(error);
            return;
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if self.error.is_none()
            && let Err(error) = self.plan_field(field)
        {
            self.error = Some(error);
        }
    }
}

struct ObsoleteItemPlanner<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
    edits: Vec<SpanEdit>,
    error: Option<GrammarRewriteError>,
}

impl<'a> ObsoleteItemPlanner<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            line_starts: line_starts(source),
            edits: Vec::new(),
            error: None,
        }
    }

    fn finish(self) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
        self.error.map_or(Ok(self.edits), Err)
    }

    fn range(&self, span: Span) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
        span_range(span, &self.line_starts, self.source.len())
    }

    fn remove_item(&mut self, span: Span, remove_trailing_line_break: bool) {
        let Ok(mut range) = self.range(span) else {
            self.error = self.range(span).err();
            return;
        };
        if remove_trailing_line_break {
            if self.source[range.end..].starts_with("\r\n") {
                range.end += 2;
            } else if self.source[range.end..].starts_with('\n') {
                range.end += 1;
            }
        }
        self.edits.push(SpanEdit {
            start: range.start,
            end: range.end,
            replacement: String::new(),
        });
    }
}

impl<'ast> Visit<'ast> for ObsoleteItemPlanner<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if self.error.is_some() {
            return;
        }
        if item.ident != "__firstset" {
            syn::visit::visit_item_mod(self, item);
            return;
        }
        let reviewed = item.content.as_ref().is_some_and(|(_, items)| {
            let include_count = items
                .iter()
                .filter(|item| {
                    let syn::Item::Macro(item) = item else {
                        return false;
                    };
                    item.mac.path.is_ident("include")
                        && item.mac.tokens.to_string() == "\"generated/first_set.rs\""
                })
                .count();
            include_count == 1
                && (items.len() == 1 || items.len() > 70)
                && items.iter().all(|item| {
                    matches!(item, syn::Item::Use(_))
                        || matches!(item, syn::Item::Macro(item) if item.mac.path.is_ident("include"))
                })
        });
        if reviewed {
            self.remove_item(item.span(), true);
        } else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.obsolete-first-set-module",
                self.range(item.span()).ok().map(|range| range.start),
                "the legacy generated first-set module changed from its reviewed shape",
            ));
        }
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        if self.error.is_some() || use_tree_ident_span(&item.tree, "__firstset").is_none() {
            return;
        }
        if is_reviewed_first_set_import(item) {
            self.remove_item(item.span(), true);
        } else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.obsolete-first-set-import",
                self.range(item.span()).ok().map(|range| range.start),
                "the legacy generated first-set import changed from its reviewed shape",
            ));
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if self.error.is_some() {
            return;
        }
        let name = item.sig.ident.to_string();
        if let Some((_, reviewed_source)) = REVIEWED_OBSOLETE_FUNCTIONS
            .iter()
            .find(|(reviewed_name, _)| *reviewed_name == name)
        {
            let reviewed_fixture = self
                .range(item.span())
                .is_ok_and(|range| self.source[range] == **reviewed_source);
            if reviewed_fixture || is_reviewed_real_obsolete_function(item, &name) {
                self.remove_item(item.span(), false);
            } else {
                self.error = Some(GrammarRewriteError::new(
                    "unsupported.obsolete-function-shape",
                    self.range(item.span()).ok().map(|range| range.start),
                    "obsolete item has changed since its declarative replacement was reviewed",
                ));
            }
        }
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if self.error.is_some() || item.ident != "WINDOW_FRAME_UNIT_KEYWORDS" {
            return;
        }
        let compact: String = item
            .to_token_stream()
            .to_string()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        if compact.contains("WINDOW_FRAME_UNIT_KEYWORDS:&[&str]=&[\"ROWS\",\"RANGE\",\"GROUPS\"]") {
            self.remove_item(item.span(), true);
        } else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.frame-unit-constant-shape",
                self.range(item.span()).ok().map(|range| range.start),
                "the obsolete window-frame exclusion constant changed from its reviewed shape",
            ));
        }
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if self.error.is_some() || item.ident != "RestOfLine" {
            return;
        }
        let reviewed = matches!(&item.fields, syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1)
            && compact_tokens(&item.to_token_stream())
                .contains("pubstructRestOfLine<'input>(pub::std::borrow::Cow<'input,str>);");
        if reviewed {
            self.remove_item(item.span(), true);
        } else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.raw-line-declaration-shape",
                self.range(item.span()).ok().map(|range| range.start),
                "the obsolete RestOfLine declaration changed from its reviewed shape",
            ));
        }
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if self.error.is_some() {
            return;
        }
        let trait_name = item
            .trait_
            .as_ref()
            .and_then(|(_, path, _)| path.segments.last())
            .map(|segment| segment.ident.to_string());
        let self_name = match item.self_ty.as_ref() {
            syn::Type::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None,
        };
        let Some(self_name) = self_name else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.handwritten-parser",
                self.range(item.span()).ok().map(|range| range.start),
                "handwritten Parse implementation has an unsupported self type",
            ));
            return;
        };
        if self_name == "RestOfLine"
            && matches!(trait_name.as_deref(), Some("FormatTokens" | "Arbitrary"))
        {
            if is_reviewed_rest_of_line_support_impl(item, trait_name.as_deref().unwrap_or("")) {
                self.remove_item(item.span(), true);
            } else {
                self.error = Some(GrammarRewriteError::new(
                    "unsupported.raw-line-support-shape",
                    self.range(item.span()).ok().map(|range| range.start),
                    "obsolete RestOfLine support changed from its reviewed shape",
                ));
            }
            return;
        }
        if trait_name.as_deref() != Some("Parse") {
            return;
        }
        let Some((_, reviewed_source)) = REVIEWED_HANDWRITTEN_PARSERS
            .iter()
            .find(|(reviewed_name, _)| *reviewed_name == self_name)
        else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.handwritten-parser",
                self.range(item.span()).ok().map(|range| range.start),
                format!("no reviewed declarative replacement for Parse implementation {self_name}"),
            ));
            return;
        };
        let reviewed_fixture = self
            .range(item.span())
            .is_ok_and(|range| self.source[range] == **reviewed_source);
        if reviewed_fixture || is_reviewed_real_handwritten_parser(item, &self_name) {
            self.remove_item(item.span(), true);
        } else {
            self.error = Some(GrammarRewriteError::new(
                "unsupported.handwritten-parser-shape",
                self.range(item.span()).ok().map(|range| range.start),
                "obsolete item has changed since its declarative replacement was reviewed",
            ));
        }
    }
}

fn is_reviewed_first_set_import(item: &syn::ItemUse) -> bool {
    fn is_path_to_glob(tree: &syn::UseTree, segments: &[&str]) -> bool {
        match (segments, tree) {
            ([], syn::UseTree::Glob(_)) => true,
            ([expected, rest @ ..], syn::UseTree::Path(path)) if path.ident == *expected => {
                is_path_to_glob(&path.tree, rest)
            }
            _ => false,
        }
    }

    match &item.vis {
        syn::Visibility::Inherited => is_path_to_glob(&item.tree, &["crate", "__firstset"]),
        syn::Visibility::Restricted(visibility)
            if visibility.path.is_ident("crate") && visibility.in_token.is_none() =>
        {
            is_path_to_glob(&item.tree, &["__firstset"])
        }
        _ => false,
    }
}

fn is_reviewed_real_obsolete_function(item: &syn::ItemFn, name: &str) -> bool {
    let compact: String = item
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let required: &[&str] = match name {
        "scan_dollar_string" => &[
            "letclose=lex.slice();",
            "letremainder=lex.remainder();",
            "remainder.find(close)",
            "lex.bump(pos+close.len())",
            "FilterResult::Error(())",
        ],
        "skip_block_comment" => &[
            "letmutdepth:u32=1;",
            "bytes[j]==b'/'&&bytes[j+1]==b'*'",
            "bytes[j]==b'*'&&bytes[j+1]==b'/'",
            "lex.bump(ifdepth==0{j}else{bytes.len()})",
        ],
        "reject_trailing_word" => &[
            "lex.remainder().bytes().next()",
            "b.is_ascii_alphanumeric()||b==b'_'",
            "FilterResult::Error(())",
        ],
        "pg_lex" => &[
            "letmutlexed=lex(src);",
            "split_psql_var_keyword_tokens(src,&mutlexed.tokens);",
            "split_bang_eq_minus_before_dash_comment(src,&mutlexed.tokens);",
        ],
        "split_bang_eq_minus_before_dash_comment" => &[
            "TokenKind::BangEqMinusasu16",
            "TokenKind::CustomOpasu16",
            "text.starts_with(b\"!=--\")",
            "tokens.remove(j);",
        ],
        "split_psql_var_keyword_tokens" => &[
            "TokenKind::PsqlVarasu16",
            "TokenKind::Colonasu16",
            "psql_var_body_keyword_kind(body)",
            "tokens.insert(i+1,body_rec);",
        ],
        "psql_var_body_keyword_kind" => &[
            "body.eq_ignore_ascii_case(\"NULL\")",
            "body.eq_ignore_ascii_case(\"TRUE\")",
            "body.eq_ignore_ascii_case(\"FALSE\")",
        ],
        "is_frame_unit" => &[
            "WINDOW_FRAME_UNIT_KEYWORDS.iter()",
            "kw.eq_ignore_ascii_case(s)",
        ],
        "not_frame_unit" => &[
            "ifletIdent::Unquoted(u)=ident&&is_frame_unit(&u.0)",
            "identifier(notROWS/RANGE/GROUPS)",
        ],
        "not_frame_unit_wrapper" => &[
            "letWindowRefNameIdent::Ident(id)=wrapper;",
            "not_frame_unit(id)",
        ],
        "current_text" => &[
            "letrec=input.current_record()?;",
            "input.source()[rec.startasusize..rec.endasusize]",
        ],
        "unquoted_ident_kind_ok" => &[
            "kind==super::TokenKind::UnquotedIdentasu16",
            "token_kind_is_soft(kind)",
        ],
        "starts_word" => &["b.is_ascii_alphabetic()||*b==b'_'"],
        _ => return false,
    };
    required.iter().all(|needle| compact.contains(needle))
}

fn plan_crate_grammar_declaration(source: &str) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
    const ANCHOR: &str = "pub mod ast;";
    if source.contains("recursa::grammar!") {
        return Err(GrammarRewriteError::new(
            "unsupported.existing-grammar-declaration",
            source.find("recursa::grammar!"),
            "the immutable legacy crate must not already declare a Recursa grammar",
        ));
    }
    let Some(anchor) = source.find(ANCHOR) else {
        return Err(GrammarRewriteError::new(
            "unsupported.grammar-declaration-anchor",
            None,
            "the reviewed crate-root grammar insertion anchor is missing",
        ));
    };
    if source.match_indices(ANCHOR).count() != 1 || !source[..anchor].trim().is_empty() {
        return Err(GrammarRewriteError::new(
            "unsupported.grammar-declaration-anchor",
            Some(anchor),
            "the reviewed crate-root grammar insertion anchor changed",
        ));
    }
    Ok(vec![SpanEdit {
        start: anchor,
        end: anchor,
        replacement: "recursa::grammar! {\n    module = crate,\n    keyword_matching = ascii_insensitive,\n    max_lookahead = 5,\n}\n\n"
            .into(),
    }])
}

fn is_reviewed_real_handwritten_parser(item: &syn::ItemImpl, self_name: &str) -> bool {
    let item_names: Vec<_> = item
        .items
        .iter()
        .filter_map(|item| match item {
            syn::ImplItem::Type(item) => Some(item.ident.to_string()),
            syn::ImplItem::Fn(item) => Some(item.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    if item_names != ["Prefix", "meta", "peek", "parse"] {
        return false;
    }
    let compact: String = item
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let required: &[&str] = match self_name {
        "StringLitSeq0" => &[
            "literal::StringLit::parse(input)?",
            "input.source()[prev_end..next_start]",
            "gap.contains(\"/*\")||gap.contains(\"--\")",
            "Seq1::from_pairs(pairs)",
        ],
        "CustomOp" => &[
            "peek_kind(0)==Some(super::TokenKind::CustomOpasu16)",
            "current_text(input).expect(\"current_recordpresent\")",
            "CustomOp(::std::borrow::Cow::Borrowed(text))",
        ],
        "UnquotedIdent" => &[
            "peek_kind(0).is_some_and(unquoted_ident_kind_ok)",
            "UnquotedIdent(::std::borrow::Cow::Borrowed(text))",
        ],
        "BareAliasName" => &[
            "current_text(input).is_some_and(starts_word)",
            "Some(text)ifstarts_word(text)",
            "BareAliasName(::std::borrow::Cow::Borrowed(text))",
        ],
        "RestOfLine" => &[
            "source[start..].find('\\n').unwrap_or(source.len()-start)",
            "current_record().is_some_and(|r|(r.startasusize)<line_end)",
            "RestOfLine(::std::borrow::Cow::Borrowed(text))",
        ],
        _ => return false,
    };
    required.iter().all(|needle| compact.contains(needle))
}

fn is_reviewed_rest_of_line_support_impl(item: &syn::ItemImpl, trait_name: &str) -> bool {
    let compact = compact_tokens(&item.to_token_stream());
    match trait_name {
        "FormatTokens" => {
            compact.contains("impl<'input>recursa::FormatTokensforRestOfLine<'input>")
                && compact.contains("Token::String(self.0.as_ref().to_string())")
        }
        "Arbitrary" => {
            compact.contains("impl<'a>Arbitrary<'a>forRestOfLine<'_>")
                && compact.contains("letbody=arb_safe_body(u,20)?;")
                && compact.contains("Ok(Self(Cow::Owned(body)))")
        }
        _ => false,
    }
}

fn type_arguments(
    segment: &syn::PathSegment,
    offset: usize,
) -> Result<Vec<&syn::Type>, GrammarRewriteError> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(GrammarRewriteError::new(
            "unsupported.container-arguments",
            Some(offset),
            "reviewed container requires angle-bracketed type arguments",
        ));
    };
    if arguments.colon2_token.is_some() {
        return Err(GrammarRewriteError::new(
            "unsupported.container-arguments",
            Some(offset),
            "reviewed container does not use turbofish arguments",
        ));
    }
    arguments
        .args
        .iter()
        .map(|argument| match argument {
            syn::GenericArgument::Type(ty) => Ok(ty),
            _ => Err(GrammarRewriteError::new(
                "unsupported.non-type-generic-argument",
                Some(offset),
                "reviewed container arguments must all be types",
            )),
        })
        .collect()
}

fn is_exact_bare_type(ty: &syn::Type, expected: &str) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == expected
        && matches!(path.path.segments[0].arguments, syn::PathArguments::None)
}

fn is_exact_bare_type_path(path: &syn::TypePath, expected: &str) -> bool {
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == expected
}

fn is_reviewed_container_path(type_path: &syn::TypePath, name: &str) -> bool {
    if type_path.qself.is_some() || type_path.path.leading_colon.is_some() {
        return false;
    }
    let names: Vec<_> = type_path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match names.as_slice() {
        [actual] if actual == name => true,
        [recursa, module, actual]
            if recursa == "recursa"
                && actual == name
                && ((module == "seq" && matches!(name, "Seq0" | "Seq1"))
                    || (module == "surrounded" && name == "Surrounded")) =>
        {
            true
        }
        _ => false,
    }
}

fn is_unit_type(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn is_reviewed_optional_trailing(ty: &syn::Type) -> bool {
    if is_exact_bare_type(ty, "OptionalTrailing") {
        return true;
    }
    let syn::Type::Path(path) = ty else {
        return false;
    };
    expr_path_segments_are(&path.path, &["recursa", "seq", "OptionalTrailing"])
}

fn expr_path_segments_are(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(segment.arguments, syn::PathArguments::None)
            })
}

fn grammar_token_name(
    ty: &syn::Type,
    offset: usize,
) -> Result<Option<String>, GrammarRewriteError> {
    let syn::Type::Path(path) = ty else {
        return Ok(None);
    };
    let exact_path = path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path
            .path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, syn::PathArguments::None));
    if !exact_path {
        return Err(GrammarRewriteError::new(
            "unsupported.token-path",
            Some(offset),
            "reviewed grammar token must be an exact unparameterized path",
        ));
    }
    let names: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match names.as_slice() {
        [token]
            if token.chars().all(|character| {
                !character.is_ascii_alphabetic() || character.is_ascii_uppercase()
            }) =>
        {
            Ok(Some(token.clone()))
        }
        [module, token] if module == "punct" => Ok(Some(token.to_ascii_uppercase())),
        [root, tokens, module, token]
            if root == "crate" && tokens == "tokens" && module == "punct" =>
        {
            Ok(Some(token.to_ascii_uppercase()))
        }
        _ => Ok(None),
    }
}

#[derive(Debug)]
struct StructuralSpans {
    syntax: Vec<std::ops::Range<usize>>,
    literals: Vec<std::ops::Range<usize>>,
}

impl StructuralSpans {
    fn parse(source: &str) -> Result<Self, GrammarRewriteError> {
        let stream = TokenStream::from_str(source).map_err(|error| {
            GrammarRewriteError::new("source.invalid-token-stream", None, error.to_string())
        })?;
        let line_starts = line_starts(source);
        let mut spans = Self {
            syntax: Vec::new(),
            literals: Vec::new(),
        };
        spans.collect(stream, &line_starts, source.len())?;
        spans.syntax.sort_by_key(|range| (range.start, range.end));
        spans.literals.sort_by_key(|range| (range.start, range.end));
        Ok(spans)
    }

    fn collect(
        &mut self,
        stream: TokenStream,
        line_starts: &[usize],
        source_len: usize,
    ) -> Result<(), GrammarRewriteError> {
        for token in stream {
            match token {
                TokenTree::Group(group) => {
                    self.push_syntax(group.span_open(), line_starts, source_len)?;
                    self.collect(group.stream(), line_starts, source_len)?;
                    self.push_syntax(group.span_close(), line_starts, source_len)?;
                }
                TokenTree::Ident(ident) => {
                    self.push_syntax(ident.span(), line_starts, source_len)?;
                }
                TokenTree::Punct(punct) => {
                    self.push_syntax(punct.span(), line_starts, source_len)?;
                }
                TokenTree::Literal(literal) => {
                    self.literals
                        .push(span_range(literal.span(), line_starts, source_len)?);
                }
            }
        }
        Ok(())
    }

    fn push_syntax(
        &mut self,
        span: Span,
        line_starts: &[usize],
        source_len: usize,
    ) -> Result<(), GrammarRewriteError> {
        self.syntax.push(span_range(span, line_starts, source_len)?);
        Ok(())
    }
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
    );
    starts
}

fn span_range(
    span: Span,
    line_starts: &[usize],
    source_len: usize,
) -> Result<std::ops::Range<usize>, GrammarRewriteError> {
    let start = span.start();
    let end = span.end();
    let byte_start = line_starts
        .get(start.line.saturating_sub(1))
        .and_then(|line| line.checked_add(start.column));
    let byte_end = line_starts
        .get(end.line.saturating_sub(1))
        .and_then(|line| line.checked_add(end.column));
    match (byte_start, byte_end) {
        (Some(start), Some(end)) if start <= end && end <= source_len => Ok(start..end),
        _ => Err(GrammarRewriteError::new(
            "source.invalid-token-span",
            None,
            "parsed Rust token span is outside source bytes",
        )),
    }
}

fn structural_matches(source: &str, needle: &str, structure: &StructuralSpans) -> Vec<usize> {
    let leading_whitespace = needle.len() - needle.trim_start_matches(char::is_whitespace).len();
    structure
        .syntax
        .iter()
        .filter_map(|token| token.start.checked_sub(leading_whitespace))
        .filter(|start| {
            let Some(end) = start.checked_add(needle.len()) else {
                return false;
            };
            source.get(*start..end) == Some(needle)
                && structurally_selected(source, *start, end, structure)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn structurally_selected(
    source: &str,
    start: usize,
    end: usize,
    structure: &StructuralSpans,
) -> bool {
    let mut has_syntax = false;
    for (relative, character) in source[start..end].char_indices() {
        if character.is_whitespace() {
            continue;
        }
        let offset = start + relative;
        if structure
            .syntax
            .iter()
            .any(|range| range.start <= offset && offset < range.end)
        {
            has_syntax = true;
        } else if !structure
            .literals
            .iter()
            .any(|range| range.start <= offset && offset < range.end)
        {
            return false;
        }
    }
    has_syntax
}

fn plan_obsolete_file_surface(
    path: &Path,
    source: &str,
    parsed: &syn::File,
) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
    const FILE_ITEMS: &[(&str, &str)] = &[
        (
            "enum FileItem",
            "7bd68445333edf5ce8cc692594f1cb15110848f316e0af790853c5f0feeb7544",
        ),
        (
            "enum PsqlCommand",
            "90766a2eaeacf73af2ff85ec3f0c229783a6b8c3f3fe194c376ccea3bd93fca1",
        ),
        (
            "fn copy_from_stdin",
            "e8b9480bc97e093cbbdac9997e3a7909e5251226ce332b2890ecb5cc95164200",
        ),
        (
            "fn directive_head",
            "50679988f20c6e03dbfb2f69b6a460f7a8243ca9edaa7f16fb1a57a156b70b2e",
        ),
        (
            "fn is_copy_data_terminator",
            "9fce52d07b45fc542098d6aa7d1c4c13869a0e8bd1dddb673d238fde981772ec",
        ),
        (
            "fn is_psql_conditional_close",
            "c96d27da15bc48149f52b772396dc5fc31381a306dc3c57b0b8313661dac2d26",
        ),
        (
            "fn is_psql_conditional_midbranch",
            "cadc3ddb832b2e9eb8253f4dadb8d9361e08b53aac53fab5560abe91d26ea2ef",
        ),
        (
            "fn is_psql_conditional_open",
            "3b59cde858ed4a056f5cf13c14b9af518c86cf6e50b0b480d47ca5040ecedafd",
        ),
        (
            "fn is_psql_quit",
            "596a38cff43ddbbd4dec7e5f6aba0aeeb1dfa9024ccb327b15b44e2d6e31047b",
        ),
        (
            "fn parse_sql_file",
            "7401254817eb4bde0093c66cdeba1c34cd6c02bdbd6f713258679dad1ab4a1be",
        ),
        (
            "fn parse_sql_file_with_spans",
            "ec72b5da91606a8bb114992ea9d6284668f384641c6f24f6fed968c8ba07f2aa",
        ),
        (
            "fn skip_failed_statement",
            "c302f98a69c0ed4e9273ac44dae07faa0e24e9cb453c0c0798cdb152440ba94c",
        ),
        (
            "fn take_copy_data",
            "3f76ea81c88c8685cd3949329128a153542fe91a56903190f59c85c2c79d13ba",
        ),
        (
            "fn take_line",
            "95543e613cb62f8989d17950417a0d5577e2748b3dc93d0d950240ca6a395957",
        ),
        (
            "mod tests",
            "1d112a9a8c69ff2358902287be1f9e3c7acc00e29a7ad91a344b72939c46bdb4",
        ),
        (
            "struct PsqlDirective",
            "9285400c667ed8b788eb87c17178ae6f3369b38f1685b9bd68ba217c8383fb92",
        ),
    ];
    let expected = match path.to_str() {
        Some("src/ast/file.rs") => FILE_ITEMS,
        Some("src/formatter.rs") => &[(
            "fn format_file",
            "8bad7f34d453575c7d5ced3fbd88905b92c177f0d08481dc6a00634654ac661c",
        )],
        _ => return Ok(Vec::new()),
    };
    let expected = expected
        .iter()
        .map(|(item, digest)| ((*item).to_owned(), *digest))
        .collect::<BTreeMap<_, _>>();
    let line_starts = line_starts(source);
    let mut found = BTreeSet::new();
    let mut edits = Vec::new();
    for item in &parsed.items {
        let (kind, name, attributes) = match item {
            syn::Item::Enum(item) => ("enum", item.ident.to_string(), item.attrs.as_slice()),
            syn::Item::Fn(item) => ("fn", item.sig.ident.to_string(), item.attrs.as_slice()),
            syn::Item::Mod(item) => ("mod", item.ident.to_string(), item.attrs.as_slice()),
            syn::Item::Struct(item) => ("struct", item.ident.to_string(), item.attrs.as_slice()),
            _ => continue,
        };
        let key = format!("{kind} {name}");
        let Some(expected_digest) = expected.get(&key) else {
            continue;
        };
        let actual_digest = format!(
            "{:x}",
            Sha256::digest(compact_tokens(&item.to_token_stream()).as_bytes())
        );
        if actual_digest != *expected_digest {
            return Err(GrammarRewriteError::new(
                "unsupported.obsolete-file-surface-shape",
                span_range(item.span(), &line_starts, source.len())
                    .ok()
                    .map(|range| range.start),
                format!("reviewed obsolete file item `{key}` changed shape"),
            ));
        }
        if !found.insert(key.clone()) {
            return Err(GrammarRewriteError::new(
                "unsupported.obsolete-file-surface",
                span_range(item.span(), &line_starts, source.len())
                    .ok()
                    .map(|range| range.start),
                format!("reviewed obsolete file item `{key}` occurs more than once"),
            ));
        }
        let mut range = span_range(item.span(), &line_starts, source.len())?;
        if let Some(attribute) = attributes.first() {
            range.start = span_range(attribute.span(), &line_starts, source.len())?.start;
        }
        if source[range.end..].starts_with("\r\n") {
            range.end += 2;
        } else if source[range.end..].starts_with('\n') {
            range.end += 1;
        }
        edits.push(SpanEdit {
            start: range.start,
            end: range.end,
            replacement: String::new(),
        });
    }
    let expected_items = expected.keys().cloned().collect::<BTreeSet<_>>();
    if found != expected_items {
        let missing = expected_items
            .difference(&found)
            .cloned()
            .collect::<Vec<_>>();
        return Err(GrammarRewriteError::new(
            "unsupported.obsolete-file-surface",
            None,
            format!(
                "reviewed obsolete file item inventory changed; missing {}",
                missing.join(", ")
            ),
        ));
    }
    Ok(edits)
}

impl SourceRewritePass for GrammarRewritePass {
    fn file_disposition(&self, path: &Path) -> Result<FileDisposition, RewriteError> {
        Ok(if is_generated_first_set(path) {
            FileDisposition::Omit
        } else {
            FileDisposition::Keep
        })
    }

    fn edits(&self, path: &Path, source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        self.plan_edits(path, source)
            .map_err(|error| RewriteError::Pass {
                path: path.to_owned(),
                message: error.to_string(),
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrammarRewriteError {
    pub code: String,
    pub offset: Option<usize>,
    pub message: String,
}

impl GrammarRewriteError {
    fn new(code: impl Into<String>, offset: Option<usize>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            offset,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GrammarRewriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.code)?;
        if let Some(offset) = self.offset {
            write!(formatter, " at byte {offset}")?;
        }
        write!(formatter, ": {}", self.message)
    }
}

impl std::error::Error for GrammarRewriteError {}

fn validate_manifest(manifest: &GrammarRewriteManifest) -> Result<(), GrammarRewriteError> {
    if manifest.schema_version != 1 {
        return Err(GrammarRewriteError::new(
            "manifest.unsupported-version",
            None,
            format!("expected schema version 1, got {}", manifest.schema_version),
        ));
    }
    let mapping = Mapping::migration_contract();
    let mapped = GrammarInventoryContract {
        parser_types: mapping.expected_parser_types.unwrap_or_default(),
        ast_types: mapping.expected_ast_types.unwrap_or_default(),
        parse_roles: mapping.expected_parse_roles.unwrap_or_default(),
        pratt_enums: mapping.expected_pratt_enums.unwrap_or_default(),
        handwritten_parsers: mapping.expected_handwritten_parsers.unwrap_or_default(),
        token_counts: mapping.expected_token_counts.unwrap_or_default(),
    };
    if manifest.inventory_contract != mapped {
        return Err(GrammarRewriteError::new(
            "manifest.inventory-contract-mismatch",
            None,
            format!(
                "fixture inventory {:?} does not match Mapping::migration_contract {:?}",
                manifest.inventory_contract, mapped
            ),
        ));
    }
    let mut actual = BTreeSet::new();
    for case in &manifest.cases {
        for shape in &case.shapes {
            if !actual.insert(shape.as_str()) {
                return Err(GrammarRewriteError::new(
                    "manifest.duplicate-shape",
                    None,
                    format!("rewrite shape {shape:?} is covered more than once"),
                ));
            }
        }
    }
    for case in &manifest.omissions {
        for shape in &case.shapes {
            if !actual.insert(shape.as_str()) {
                return Err(GrammarRewriteError::new(
                    "manifest.duplicate-shape",
                    None,
                    format!("rewrite shape {shape:?} is covered more than once"),
                ));
            }
        }
    }
    let expected: BTreeSet<_> = SUPPORTED_SHAPES.iter().copied().collect();
    if actual != expected {
        let missing: Vec<_> = expected.difference(&actual).copied().collect();
        let extra: Vec<_> = actual.difference(&expected).copied().collect();
        return Err(GrammarRewriteError::new(
            "manifest.shape-set-mismatch",
            None,
            format!("missing={missing:?}, extra={extra:?}"),
        ));
    }
    Ok(())
}

fn is_generated_first_set(path: &Path) -> bool {
    path.ends_with(Path::new("src/generated/first_set.rs"))
}

const UNSUPPORTED: &[(&str, &str)] = &[
    ("UnknownGrammar<", "unsupported.unknown-container"),
    (
        "OptionalTrailing<",
        "unsupported.malformed-optional-trailing",
    ),
    ("parse_raw_source_remainder", "unsupported.raw-line-parser"),
];

const REVIEWED_OBSOLETE_FUNCTIONS: &[(&str, &str)] = &[
    (
        "scan_dollar_string",
        r#"pub fn scan_dollar_string(lexer: &mut Lexer<'_>) -> Action {
    lexer.scan_same_delimiter()
}"#,
    ),
    (
        "reject_trailing_word",
        r#"pub fn reject_trailing_word(lexer: &mut Lexer<'_>) -> Action {
    lexer.reject_word_character()
}"#,
    ),
    (
        "skip_block_comment",
        r#"pub fn skip_block_comment(lexer: &mut Lexer<'_>) -> Action {
    lexer.skip_nested_comment()
}"#,
    ),
    (
        "pg_lex",
        r#"pub fn pg_lex(source: &str) -> LexResult {
    let mut result = lex(source);
    split_psql_var_keyword_tokens(source, &mut result.tokens);
    split_bang_eq_minus_before_dash_comment(source, &mut result.tokens);
    result
}"#,
    ),
    (
        "split_psql_var_keyword_tokens",
        r#"fn split_psql_var_keyword_tokens(source: &str, tokens: &mut Vec<TokenRecord>) {
    repair_psql_variables(source, tokens);
}"#,
    ),
    (
        "split_bang_eq_minus_before_dash_comment",
        r#"fn split_bang_eq_minus_before_dash_comment(source: &str, tokens: &mut Vec<TokenRecord>) {
    repair_operator_comment_fence(source, tokens);
}"#,
    ),
    (
        "not_frame_unit",
        r#"fn not_frame_unit(value: &Ident<'_>) -> Result<(), ParseError> {
    reject_frame_unit(value)
}"#,
    ),
    (
        "not_frame_unit_wrapper",
        r#"pub fn not_frame_unit_wrapper(value: &WindowRefNameIdent<'_>) -> Result<(), ParseError> {
    let WindowRefNameIdent::Ident(value) = value;
    not_frame_unit(value)
}"#,
    ),
    ("psql_var_body_keyword_kind", ""),
    ("is_frame_unit", ""),
    ("current_text", ""),
    ("unquoted_ident_kind_ok", ""),
    ("starts_word", ""),
];

const REVIEWED_HANDWRITTEN_PARSERS: &[(&str, &str)] = &[
    (
        "StringLitSeq0",
        "impl Parse for StringLitSeq0 { fn parse() {} }",
    ),
    (
        "CustomOp",
        "impl<'input> Parse<'input> for CustomOp<'input> { fn parse() {} }",
    ),
    (
        "UnquotedIdent",
        "impl<'input> Parse<'input> for UnquotedIdent<'input> { fn parse() {} }",
    ),
    (
        "BareAliasName",
        "impl<'input> Parse<'input> for BareAliasName<'input> { fn parse() {} }",
    ),
    (
        "RestOfLine",
        "impl<'input> Parse<'input> for RestOfLine<'input> { fn parse() {} }",
    ),
];

const REWRITES: &[(&str, &str)] = &[
    ("crate::tokens::pg_lex(", "crate::tokens::lex("),
    ("Box::new(pg_lex(src))", "Box::new(lex(src))"),
    ("literal::PsqlVar<'input>", "literal::PsqlVariable<'input>"),
    (
        "NumericLit, PsqlVar, QuotedIdent",
        "NumericLit, QuotedIdent",
    ),
    (
        "pub use self::file::{\n    FileItem, PsqlCommand, PsqlDirective, PsqlTerminator, StatementTerminator, TerminatedStatement,\n    is_copy_data_terminator, is_psql_conditional_close, is_psql_conditional_midbranch,\n    is_psql_conditional_open, is_psql_quit, parse_sql_file, parse_sql_file_with_spans,\n};\n",
        "pub use self::file::{PsqlTerminator, StatementTerminator, TerminatedStatement};\n",
    ),
    (
        "    // Hand-written `Parse` impl — a genuine recursa gap. `RestOfLine` matches\n    // raw source up to the next newline, content that is not lexable SQL\n    // (psql `\\directive` argument text). In the logos token model there is no\n    // \"rest of line\" token kind: this impl recovers the raw slice from the\n    // current token's byte offset to the next `\\n` in `source`, then advances\n    // the token cursor past every token whose span lies within that line.\n    // Filed as a recursa limitation: raw-source-spanning tokens have no\n    // first-class model in the token-array design.\n",
        "",
    ),
    (
        "    module = crate::grammar,\n}",
        "    module = crate::grammar,\n    keyword_matching = ascii_insensitive,\n    max_lookahead = 5,\n}",
    ),
    (
        "    classes { bare_label_keywords = keywords where bare_label }\n    targets {\n        ColId: literal::Ident admits UNRESERVED, COL_NAME,\n        type_function_name: literal::Ident admits UNRESERVED, TYPE_FUNC_NAME,\n        NonReservedWord: literal::Ident admits UNRESERVED, COL_NAME, TYPE_FUNC_NAME,\n        ColLabel: literal::Ident admits UNRESERVED, COL_NAME, TYPE_FUNC_NAME, RESERVED,\n        BareColLabel: literal::Ident admits bare_label_keywords,\n    }",
        "    admissions {\n        AllWordKinds = keywords,\n        ColId = UNRESERVED + COL_NAME,\n        type_function_name = UNRESERVED + TYPE_FUNC_NAME,\n        NonReservedWord = UNRESERVED + COL_NAME + TYPE_FUNC_NAME,\n        ColLabel = UNRESERVED + COL_NAME + TYPE_FUNC_NAME + RESERVED,\n        BareColLabel = bare_label,\n        WindowRefName = ColId - { ROWS, RANGE, GROUPS },\n        PsqlVariableName = AllWordKinds - { NULL, TRUE, FALSE },\n        UnquotedIdent = NonReservedWord,\n        BareAliasName = AllWordKinds,\n    }",
    ),
    (
        "        GROUPS => r\"GROUPS\" in UNRESERVED + bare_label,\n    }",
        "        GROUPS => r\"GROUPS\" in UNRESERVED + bare_label,",
    ),
    (
        "    soft_keywords { FORMAT => r\"FORMAT\" in UNRESERVED + bare_label, }\n",
        "        FORMAT => r\"FORMAT\" in UNRESERVED + bare_label,\n    }\n",
    ),
    (
        "    literals {\n        DollarStringLit<'input>(source) => r\"\\$(?:[A-Za-z_][A-Za-z0-9_]*)?\\$\" with scan_dollar_string,\n        NumericLit<'input>(source) => r\"(?:(?:[0-9](?:_?[0-9])*\\.[0-9](?:_?[0-9])*|\\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\\.)\" with reject_trailing_word,\n        IntegerLit<'input>(source) => r\"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)\" with reject_trailing_word,\n        DollarNum<'input>(source) => r\"\\$[0-9]+\" with reject_trailing_word,\n    }",
        "    matchers {\n        DollarStringLit => same_delimiter(opener = r\"\\$(?:[A-Za-z_][A-Za-z0-9_]*)?\\$\"),\n        NumericLit => next_exclusion(pattern = r\"(?:(?:[0-9](?:_?[0-9])*\\.[0-9](?:_?[0-9])*|\\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\\.)\", excluded = r\"[A-Za-z0-9_]\"),\n        IntegerLit => next_exclusion(pattern = r\"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)\", excluded = r\"[A-Za-z0-9_]\"),\n        DollarNum => next_exclusion(pattern = r\"\\$[0-9]+\", excluded = r\"[A-Za-z0-9_]\"),\n        CustomOp => operator_run(\n            characters = \"-+*/<>=~!@#%^&|?\",\n            fences = [\"/*\", \"--\"],\n            trailing = \"+-\",\n            qualifying = \"~!@#%^&|?\"\n        ),\n    }",
    ),
    ("    lexer_tokens {", "    ignore {"),
    (
        "        BlockComment => r\"/\\*\" with skip_block_comment,",
        "        BlockComment => nested(opener = \"/*\", closer = \"*/\"),",
    ),
    (
        "        CustomOp => r\"([-+*/<>=~!@#%^&|?]*[~!@#%^&|?][-+*/<>=~!@#%^&|?]*|[-+*/<>=]+[*/<>=])\",\n",
        "",
    ),
    (
        "    pub name: ColId<'input>,",
        "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(ColId))]\n    pub name: ColId<'input>,",
    ),
    (
        "    pub name: type_function_name<'input>,",
        "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(type_function_name))]\n    pub name: type_function_name<'input>,",
    ),
    (
        "    pub name: NonReservedWord<'input>,",
        "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(NonReservedWord))]\n    pub name: NonReservedWord<'input>,",
    ),
    (
        "    pub name: ColLabel<'input>,",
        "    #[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(ColLabel))]\n    pub name: ColLabel<'input>,",
    ),
    (
        "    Bare(BareColLabel<'input>),",
        "    Bare(#[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(BareColLabel))] BareColLabel<'input>),",
    ),
    (
        "    Unquoted(UnquotedIdent<'input>),",
        "    Unquoted(#[lex(pattern = r\"[A-Za-z_][A-Za-z0-9_]*\", admits(UnquotedIdent))] UnquotedIdent<'input>),",
    ),
    (
        "    Bare(BareAliasName<'input>),",
        "    Bare(#[lex(pattern = r\"[A-Za-z_][A-Za-z0-9_]*\", admits(BareAliasName))] BareAliasName<'input>),",
    ),
    (
        "    pub name: PsqlVariableName<'input>,",
        "    #[lex(pattern = r#\"(?:[A-Za-z_][A-Za-z0-9_]*|'[^']*'|\"[^\"]*\")\"#, admits(PsqlVariableName))]\n    pub name: PsqlVariableName<'input>,",
    ),
    (
        "    pub body: DollarStringLit<'input>,",
        "    #[lex(matcher)]\n    pub body: DollarStringLit<'input>,",
    ),
    (
        "    pub value: NumericLit<'input>,",
        "    #[lex(matcher)]\n    pub value: NumericLit<'input>,",
    ),
    (
        "    pub value: IntegerLit<'input>,",
        "    #[lex(matcher)]\n    pub value: IntegerLit<'input>,",
    ),
    (
        "    pub value: DollarNum<'input>,",
        "    #[lex(matcher)]\n    pub value: DollarNum<'input>,",
    ),
    (
        "    pub operator: CustomOp<'input>,",
        "    #[lex(matcher)]\n    pub operator: CustomOp<'input>,",
    ),
    (
        "pub enum WindowRefNameIdent<'input> {\n    Ident(Ident<'input>),\n}",
        "pub enum WindowRefNameIdent<'input> {\n    Ident(#[lex(pattern = r#\"(?i:U)&\"[^\"]*(?:\"\"[^\"]*)*\"|\"[^\"]*(?:\"\"[^\"]*)*\"|[A-Za-z_][A-Za-z0-9_]*\"#, admits(WindowRefName))] WindowRefNameText<'input>),\n}",
    ),
    (
        "    ) -> Option<\n        &Surrounded<punct::LParen, Seq0<ColumnOrConstraint<'input>, punct::Comma>, punct::RParen>,\n    > {",
        "    ) -> Option<&Vec<ColumnOrConstraint<'input>>> {",
    ),
    ("use recursa::seq::{Seq0, Seq1, OptionalTrailing};\n", ""),
    ("use recursa::surrounded::Surrounded;\n", ""),
    ("use recursa::{FormatTokens, Transform, Visit};\n", ""),
    ("use crate::__firstset::*;\n", ""),
    ("pub struct SqlRules;\n", ""),
    (
        "pub mod __firstset { include!(\"generated/first_set.rs\"); }\n",
        "",
    ),
    ("#[railroad]\n", ""),
    (
        "#[cfg_attr(feature = \"arbitrary\", derive(arbitrary::Arbitrary))]\n",
        "",
    ),
    (
        "#[derive(FormatTokens, Visit, Transform, Debug, Clone)]",
        "#[derive(recursa::Node, Debug, Clone)]",
    ),
    (
        "#[derive(Debug, Clone, FormatTokens, Visit, Transform)]",
        "#[derive(recursa::Node, Debug, Clone)]",
    ),
    ("pub select: SELECT,\n    ", "#[tok(SELECT)]\n    "),
    ("Only(ONLY),", "#[tok(ONLY)]\n    Only,"),
    (
        "Named((AS, Name<'input>)),",
        "Named(#[tok(AS, this)] Name<'input>),",
    ),
    ("Not(NOT, Box<Self>),", "#[tok(NOT)]\n    Not(Box<Self>),"),
    (
        "#[parse(infix, bp = 10)]\n    Add((Box<Self>, punct::Plus, Box<Self>)),",
        "#[parse(infix, lbp = 10, rbp = 11)]\n    Add(Box<Self>, #[tok(PLUS)] Box<Self>),",
    ),
    (
        "Factorial((Box<Self>, punct::Bang)),",
        "Factorial(#[tok(this, BANG)] Box<Self>),",
    ),
];

const FIXTURE_OPTIONAL_REWRITES: &[(&str, &str)] = &[
    (
        "    pub r#as: Option<AS>,\n    pub alias: Name<'input>,",
        "    #[tok(optional(AS), this)]\n    pub alias: Name<'input>,",
    ),
    (
        "pub unique: Option<UNIQUE>,",
        "#[presence(UNIQUE)]\n    pub unique: bool,",
    ),
];
