//! Deterministic grammar-only migration over reviewed source shapes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr;

use proc_macro2::{Span, TokenStream, TokenTree};
use serde::Deserialize;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::Mapping;
use crate::rewrite::{
    FileDisposition, RewriteError, SourceRewritePass, SpanEdit, apply_span_edits,
};

pub const SUPPORTED_SHAPES: &[&str] = &[
    "attributed_enum",
    "attributed_struct",
    "attributed_type",
    "derive_stack",
    "fixed_syntax",
    "handwritten_bare_alias_name",
    "handwritten_custom_op",
    "handwritten_rest_of_line",
    "handwritten_string_lit_seq",
    "handwritten_unquoted_ident",
    "lexer_callback",
    "nested_container",
    "obsolete_firstset_ref",
    "obsolete_generated_artifact",
    "obsolete_sql_rules",
    "optional_syntax_bool",
    "optional_syntax_exclusion",
    "optional_trailing",
    "parser_attr",
    "pratt_enum",
    "pratt_infix",
    "pratt_postfix",
    "pratt_prefix",
    "seq0",
    "seq1",
    "surrounded",
    "token_callbacks",
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
}

impl GrammarRewritePass {
    pub fn from_manifest_json(json: &str) -> Result<Self, GrammarRewriteError> {
        let manifest: GrammarRewriteManifest = serde_json::from_str(json).map_err(|error| {
            GrammarRewriteError::new("manifest.invalid-json", None, error.to_string())
        })?;
        validate_manifest(&manifest)?;
        Ok(Self { manifest })
    }

    pub fn manifest(&self) -> &GrammarRewriteManifest {
        &self.manifest
    }

    pub fn plan_edits(
        &self,
        _path: &Path,
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
        let mut containers = ContainerPlanner::new(source, &structure);
        containers.visit_file(&parsed);
        edits.extend(containers.finish()?);
        for &(needle, replacement) in REWRITES {
            for start in structural_matches(source, needle, &structure) {
                edits.push(SpanEdit {
                    start,
                    end: start + needle.len(),
                    replacement: replacement.into(),
                });
            }
        }
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
        let rewritten = apply_span_edits(source, &edits).map_err(|error| {
            GrammarRewriteError::new("rewrite.invalid-span", None, error.to_string())
        })?;
        syn::parse_file(&rewritten).map_err(|error| {
            GrammarRewriteError::new("rewrite.invalid-rust", None, error.to_string())
        })?;
        let rewritten_structure = StructuralSpans::parse(&rewritten)?;
        for legacy in FORBIDDEN_AFTER_REWRITE {
            if let Some(offset) = structural_matches(&rewritten, legacy, &rewritten_structure)
                .into_iter()
                .next()
            {
                return Err(GrammarRewriteError::new(
                    "rewrite.unhandled-legacy-shape",
                    Some(offset),
                    format!("no reviewed rewrite for remaining {legacy:?}"),
                ));
            }
        }
        Ok(edits)
    }
}

struct ContainerPlanner<'a> {
    source: &'a str,
    structure: &'a StructuralSpans,
    line_starts: Vec<usize>,
    edits: Vec<SpanEdit>,
    error: Option<GrammarRewriteError>,
}

impl<'a> ContainerPlanner<'a> {
    fn new(source: &'a str, structure: &'a StructuralSpans) -> Self {
        Self {
            source,
            structure,
            line_starts: line_starts(source),
            edits: Vec::new(),
            error: None,
        }
    }

    fn finish(self) -> Result<Vec<SpanEdit>, GrammarRewriteError> {
        self.error.map_or(Ok(self.edits), Err)
    }

    fn plan_field(&mut self, field: &syn::Field) -> Result<(), GrammarRewriteError> {
        let mut attributes = Vec::new();
        self.plan_type(&field.ty, &mut attributes)?;
        if !attributes.is_empty() {
            let field_start = self.range(field.span())?.start;
            let indentation = self.source[..field_start]
                .rsplit_once('\n')
                .map_or("", |(_, indentation)| indentation);
            self.edits.push(SpanEdit {
                start: field_start,
                end: field_start,
                replacement: format!("{}{}", attributes.join(indentation), indentation),
            });
        }
        Ok(())
    }

    fn plan_type(
        &mut self,
        ty: &syn::Type,
        attributes: &mut Vec<String>,
    ) -> Result<(), GrammarRewriteError> {
        let syn::Type::Path(type_path) = ty else {
            return Ok(());
        };
        let Some(segment) = type_path.path.segments.last() else {
            return Ok(());
        };
        let name = segment.ident.to_string();
        let is_container = matches!(name.as_str(), "Option" | "Surrounded" | "Seq0" | "Seq1");
        if is_container
            && (type_path.qself.is_some()
                || type_path.path.leading_colon.is_some()
                || type_path.path.segments.len() != 1)
        {
            return Err(GrammarRewriteError::new(
                "unsupported.qualified-container",
                Some(self.range(type_path.span())?.start),
                format!("reviewed container {name} must be an unqualified TypePath"),
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
                attributes.push(format!("#[surrounded({left}, this, {right})]\n"));

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
                let Some(separator) = grammar_token_name(arguments[1], separator_offset)? else {
                    return Ok(());
                };
                let trailing = if let Some(disposition) = arguments.get(2) {
                    if !is_exact_bare_type(disposition, "OptionalTrailing") {
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
                attributes.push(format!("#[sep({separator}{trailing})]\n"));

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
}

impl<'ast> Visit<'ast> for ContainerPlanner<'_> {
    fn visit_field(&mut self, field: &'ast syn::Field) {
        if self.error.is_none()
            && let Err(error) = self.plan_field(field)
        {
            self.error = Some(error);
        }
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

fn grammar_token_name(
    ty: &syn::Type,
    offset: usize,
) -> Result<Option<&'static str>, GrammarRewriteError> {
    let syn::Type::Path(path) = ty else {
        return Ok(None);
    };
    let reviewed_name = path.path.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "Comma" | "LParen" | "RParen"
        )
    });
    let exact_path = path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 2
        && path
            .path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, syn::PathArguments::None));
    if reviewed_name && !exact_path {
        return Err(GrammarRewriteError::new(
            "unsupported.token-path",
            Some(offset),
            "reviewed grammar token must be an exact unparameterized punct path",
        ));
    }
    let names: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match names.as_slice() {
        [module, token] if module == "punct" && token == "Comma" => Ok(Some("COMMA")),
        [module, token] if module == "punct" && token == "LParen" => Ok(Some("LPAREN")),
        [module, token] if module == "punct" && token == "RParen" => Ok(Some("RPAREN")),
        _ if reviewed_name => Err(GrammarRewriteError::new(
            "unsupported.token-path",
            Some(offset),
            "reviewed grammar token has the wrong module qualification",
        )),
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
        "#[recursa::parser(custom",
        "unsupported.custom-parser-option",
    ),
    (
        "OptionalTrailing<",
        "unsupported.malformed-optional-trailing",
    ),
];

const FORBIDDEN_AFTER_REWRITE: &[&str] = &[
    "#[recursa::parser(",
    "impl Parse for ",
    "Seq0<",
    "Seq1<",
    "Surrounded<",
    "OptionalTrailing",
    "rules = SqlRules",
    "::__firstset",
];

const REWRITES: &[(&str, &str)] = &[
    ("use recursa::seq::{Seq0, Seq1, OptionalTrailing};\n", ""),
    ("use recursa::surrounded::Surrounded;\n", ""),
    ("use recursa::{FormatTokens, Transform, Visit};\n", ""),
    ("use crate::__firstset::*;\n", ""),
    ("pub struct SqlRules;\n", ""),
    (
        "pub mod __firstset { include!(\"generated/first_set.rs\"); }\n",
        "",
    ),
    ("#[recursa::ast]\npub type", "#[parse(skip)]\npub type"),
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
    ("#[recursa::parser(rules = SqlRules)]\n", ""),
    ("#[recursa::parser(rules = SqlRules, pratt)]", "#[pratt]"),
    ("pub select: SELECT,\n    ", "#[kwd(SELECT)]\n    "),
    (
        "    pub r#as: Option<AS>,\n    pub alias: Name<'input>,",
        "    pub alias: Alias<'input>,",
    ),
    (
        "pub unique: Option<UNIQUE>,",
        "#[kwd(UNIQUE)]\n    pub unique: bool,",
    ),
    ("Only(ONLY),", "#[kwd(ONLY)]\n    Only,"),
    (
        "Named((AS, Name<'input>)),",
        "#[kwd(AS)]\n    Named(Name<'input>),",
    ),
    ("Not(NOT, Box<Self>),", "#[kwd(NOT)]\n    Not(Box<Self>),"),
    (
        "#[parse(infix, bp = 10)]\n    Add((Box<Self>, punct::Plus, Box<Self>)),",
        "#[parse(infix, lbp = 10, rbp = 11)]\n    Add(Box<Self>, #[tok(PLUS)] Box<Self>),",
    ),
    (
        "Factorial((Box<Self>, punct::Bang)),",
        "Factorial(#[tok(BANG)] Box<Self>),",
    ),
    ("impl Parse for StringLitSeq0 { fn parse() {} }\n", ""),
    ("impl Parse for CustomOp { fn parse() {} }\n", ""),
    ("impl Parse for UnquotedIdent { fn parse() {} }\n", ""),
    ("impl Parse for BareAliasName { fn parse() {} }\n", ""),
    ("impl Parse for RestOfLine { fn parse() {} }\n", ""),
];
