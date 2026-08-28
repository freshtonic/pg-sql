use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use pg_sql_migrate::grammar_rewrite::{GrammarRewritePass, SUPPORTED_SHAPES};
use pg_sql_migrate::rewrite::{
    FileDisposition, RewriteTreeRequest, SourceRewritePass, rewrite_source, rewrite_tree,
};

mod support;

use support::assert_single_token_attachment;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/rewrite/grammar")
}

fn pass() -> GrammarRewritePass {
    let manifest = fs::read_to_string(fixture_root().join("manifest.json")).unwrap();
    GrammarRewritePass::from_manifest_json(&manifest).unwrap()
}

#[test]
fn manifest_covers_every_inventoried_grammar_rewrite_shape_exactly_once() {
    let pass = pass();
    let actual: BTreeSet<_> = pass
        .manifest()
        .cases
        .iter()
        .flat_map(|case| case.shapes.iter().map(String::as_str))
        .chain(
            pass.manifest()
                .omissions
                .iter()
                .flat_map(|case| case.shapes.iter().map(String::as_str)),
        )
        .collect();
    let expected: BTreeSet<_> = SUPPORTED_SHAPES.iter().copied().collect();

    assert_eq!(actual, expected);
    assert_eq!(
        pass.manifest()
            .cases
            .iter()
            .map(|case| case.shapes.len())
            .chain(
                pass.manifest()
                    .omissions
                    .iter()
                    .map(|case| case.shapes.len()),
            )
            .sum::<usize>(),
        actual.len()
    );

    let root = fixture_root();
    let mut referenced = BTreeSet::new();
    for case in &pass.manifest().cases {
        referenced.insert(case.input.clone());
        referenced.insert(case.expected.clone());
    }
    for case in &pass.manifest().unsupported {
        referenced.insert(case.input.clone());
    }
    for case in &pass.manifest().omissions {
        referenced.insert(case.input.clone());
    }
    assert_eq!(rust_fixture_paths(&root, &root), referenced);
}

#[test]
fn generated_first_set_artifact_is_omitted_instead_of_emptied() {
    let root = fixture_root();
    let pass = pass();
    let case = &pass.manifest().omissions[0];
    let input_path = root.join(&case.input);
    let input = fs::read_to_string(&input_path).unwrap();

    assert_eq!(
        pass.file_disposition(Path::new(&case.input)).unwrap(),
        FileDisposition::Omit
    );
    assert!(pass.edits(&input_path, &input).unwrap().is_empty());

    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("legacy");
    let new_repository = temporary.path().join("new-repository");
    let destination = new_repository.join("migrated");
    fs::create_dir_all(source.join("src/generated")).unwrap();
    fs::create_dir_all(&new_repository).unwrap();
    fs::write(source.join("src/generated/first_set.rs"), &input).unwrap();
    fs::write(source.join("src/kept.rs"), "pub struct Kept;\n").unwrap();
    rewrite_tree(RewriteTreeRequest {
        source_root: &source,
        destination_root: &destination,
        new_repository_root: &new_repository,
        passes: &[&pass],
    })
    .unwrap();
    assert!(!destination.join("src/generated/first_set.rs").exists());
    assert_eq!(
        fs::read_to_string(destination.join("src/kept.rs")).unwrap(),
        "pub struct Kept;\n"
    );
}

fn rust_fixture_paths(root: &Path, directory: &Path) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            paths.extend(rust_fixture_paths(root, &entry.path()));
        } else if entry.path().extension().and_then(|value| value.to_str()) == Some("rs") {
            paths.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    paths
}

#[test]
fn every_manifest_fixture_rewrites_to_its_reviewed_bytes() {
    let root = fixture_root();
    let pass = pass();

    for case in &pass.manifest().cases {
        let input_path = root.join(&case.input);
        let input = fs::read_to_string(&input_path).unwrap();
        let expected = fs::read_to_string(root.join(&case.expected)).unwrap();
        let actual = rewrite_source(&pass, &input_path, &input).unwrap();
        assert_eq!(actual, expected, "fixture {}", case.id);
        syn::parse_file(&actual)
            .unwrap_or_else(|error| panic!("fixture {} produced invalid Rust: {error}", case.id));
        assert_single_token_attachment(&input_path, &actual);
    }
}

#[test]
fn every_reviewed_output_is_a_second_pass_fixed_point() {
    let root = fixture_root();
    let pass = pass();

    for case in &pass.manifest().cases {
        let expected_path = root.join(&case.expected);
        let expected = fs::read_to_string(&expected_path).unwrap();
        let second_pass = rewrite_source(&pass, &expected_path, &expected).unwrap();
        assert_eq!(second_pass, expected, "fixture {}", case.id);
    }
}

#[test]
fn grammar_edits_are_validated_source_spans_and_are_byte_deterministic() {
    let root = fixture_root();
    let input_path = root.join("nodes.input.rs");
    let input = fs::read_to_string(&input_path).unwrap();
    let pass = pass();

    let first_edits = pass.edits(&input_path, &input).unwrap();
    let second_edits = pass.edits(&input_path, &input).unwrap();
    let first = rewrite_source(&pass, &input_path, &input).unwrap();
    let second = rewrite_source(&pass, &input_path, &input).unwrap();

    assert_eq!(first_edits, second_edits);
    assert_eq!(first.as_bytes(), second.as_bytes());
    assert!(
        first_edits
            .iter()
            .all(|edit| edit.start <= edit.end && edit.end <= input.len())
    );
    assert!(
        first_edits
            .iter()
            .all(|edit| { input.is_char_boundary(edit.start) && input.is_char_boundary(edit.end) })
    );
    assert!(
        first_edits
            .iter()
            .all(|edit| edit.end - edit.start < input.len())
    );
}

#[test]
fn optional_token_dispositions_require_the_exact_inventoried_path_span_and_shape() {
    let pass = pass();
    let unreviewed = "pub struct Unreviewed { pub unique: Option<UNIQUE>, }\n";
    assert!(
        pass.plan_edits(Path::new("src/unreviewed.rs"), unreviewed)
            .unwrap()
            .is_empty()
    );

    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let source = fs::read_to_string(repository.join("src/ast/ddl/index.rs")).unwrap();
    let drifted = source.replacen("Option<UNIQUE>", "Option<DEFAULT>", 1);
    let error = pass
        .plan_edits(Path::new("src/ast/ddl/index.rs"), &drifted)
        .unwrap_err();

    assert_eq!(error.code, "inventory.field-shape-drift");
    assert!(error.message.contains("CreateIndexStmt.unique"));
}

#[test]
fn comments_and_declaration_order_survive_the_span_rewrite() {
    let root = fixture_root();
    let input_path = root.join("nodes.input.rs");
    let input = fs::read_to_string(&input_path).unwrap();
    let output = rewrite_source(&pass(), &input_path, &input).unwrap();

    let comment = "// The declaration comment and field order must survive the rewrite.";
    assert!(output.contains(comment));
    assert!(
        output.find("pub struct SelectStmt").unwrap() < output.find("pub enum Choice").unwrap()
    );
    assert!(output.find("pub enum Choice").unwrap() < output.find("pub enum Expr").unwrap());
    assert!(output.find("pub items").unwrap() < output.find("pub values").unwrap());
    assert!(output.find("pub values").unwrap() < output.find("pub nested").unwrap());
}

#[test]
fn container_rewrites_preserve_every_intervening_comment_byte_in_order() {
    let root = fixture_root();
    let input_path = root.join("containers-comments.input.rs");
    let input = fs::read_to_string(&input_path).unwrap();
    let output = rewrite_source(&pass(), &input_path, &input).unwrap();
    let comments = [
        "// seq0-line\n",
        "/* seq1-item */",
        "/* seq1-path */",
        "/* trailing */",
        "/* surrounded-inner */",
        "/* nested-left */",
        "/* nested-separator */",
        "/* nested-right */",
    ];

    let mut previous = 0;
    for comment in comments {
        assert_eq!(output.matches(comment).count(), 1, "comment {comment:?}");
        let position = output.find(comment).unwrap();
        assert!(position >= previous, "comment order changed at {comment:?}");
        previous = position + comment.len();
    }
    syn::parse_file(&output).unwrap();
}

#[test]
fn unsupported_constructs_fail_with_manifested_structured_codes() {
    let root = fixture_root();
    let pass = pass();

    for case in &pass.manifest().unsupported {
        let input_path = root.join(&case.input);
        let input = fs::read_to_string(&input_path).unwrap();
        let error = pass.plan_edits(&input_path, &input).unwrap_err();
        assert_eq!(error.code, case.code, "fixture {}", case.id);
        assert!(error.offset.is_some(), "fixture {}", case.id);
        assert!(!error.message.is_empty(), "fixture {}", case.id);
    }
}

#[test]
fn analogous_noncanonical_type_paths_and_arguments_fail_closed() {
    let pass = pass();
    let cases = [
        (
            "leading-colon",
            "pub struct Bad { pub value: ::Seq0<Value, punct::Comma>, }",
            "unsupported.qualified-container",
        ),
        (
            "qualified-option",
            "pub struct Bad { pub value: std::option::Option<Seq0<Value, punct::Comma>>, }",
            "unsupported.qualified-container",
        ),
        (
            "lifetime-argument",
            "pub struct Bad<'a> { pub value: Seq0<Value, 'a>, }",
            "unsupported.non-type-generic-argument",
        ),
        (
            "wrong-token-module",
            "pub struct Bad { pub value: Seq0<Value, other::Comma>, }",
            "unsupported.token-path",
        ),
        (
            "parameterized-disposition",
            "pub struct Bad { pub value: Seq1<Value, punct::Comma, OptionalTrailing<Value>>, }",
            "unsupported.malformed-optional-trailing",
        ),
        (
            "qself-container",
            "pub struct Bad<T: Grammar> { pub value: <T as Grammar>::Seq0<Value, punct::Comma>, }",
            "unsupported.qualified-container",
        ),
    ];

    for (id, source, expected_code) in cases {
        let error = pass
            .plan_edits(Path::new("src/adversarial.rs"), source)
            .unwrap_err();
        assert_eq!(error.code, expected_code, "case {id}");
        assert!(!error.message.is_empty(), "case {id}");
    }
}

#[test]
fn obsolete_items_are_removed_only_when_their_full_reviewed_shape_matches() {
    let pass = pass();
    let cases = [
        (
            "changed callback body",
            "pub fn scan_dollar_string(lexer: &mut Lexer<'_>) -> Action { lexer.changed() }",
            "unsupported.obsolete-function-shape",
        ),
        (
            "changed handwritten parser body",
            "impl<'input> Parse<'input> for CustomOp<'input> { fn changed() {} }",
            "unsupported.handwritten-parser-shape",
        ),
    ];

    for (id, source, expected_code) in cases {
        let error = pass
            .plan_edits(Path::new("src/adversarial.rs"), source)
            .unwrap_err();
        assert_eq!(error.code, expected_code, "case {id}");
        assert!(!error.message.is_empty(), "case {id}");
    }
}

#[test]
fn obsolete_file_surface_requires_exact_shape_and_preserves_new_items() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let source = fs::read_to_string(repository.join("src/formatter.rs")).unwrap();
    let drifted = source.replace(
        "let mut output = String::new();",
        "let mut output = String::with_capacity(64);",
    );
    let error = pass()
        .plan_edits(Path::new("src/formatter.rs"), &drifted)
        .unwrap_err();
    assert_eq!(error.code, "unsupported.obsolete-file-surface-shape");

    let extended = format!("{source}\npub fn newly_added_formatter_api() {{}}\n");
    let rewritten = rewrite_source(&pass(), Path::new("src/formatter.rs"), &extended).unwrap();
    assert!(rewritten.contains("pub fn format_tokens_sql"));
    assert!(rewritten.contains("pub fn newly_added_formatter_api"));
    assert!(!rewritten.contains("pub fn format_file"));
}

#[test]
fn reviewed_string_literal_parser_is_removed_at_every_line_boundary() {
    const PARSER: &str = "impl Parse for StringLitSeq0 { fn parse() {} }";

    for (case, line_break) in [("lf", "\n"), ("crlf", "\r\n"), ("eof", "")] {
        let source = format!("{PARSER}{line_break}");
        let output = rewrite_source(&pass(), Path::new("src/string_lit_seq.rs"), &source).unwrap();

        assert_eq!(output, "", "case {case}");
    }
}

#[test]
fn obsolete_escape_hatches_are_rejected_independent_of_token_spacing() {
    let pass = pass();
    let cases = [
        (
            "inline callback",
            "pub struct Bad { #[lex(pattern=r\"x\",callback=crate::scan)] value: Word }",
            "unsupported.inline-callback",
        ),
        (
            "central callback",
            "recursa::tokens! { literals { Word => r\"x\" with\ncrate::scan, } }",
            "unsupported.central-callback",
        ),
        (
            "callback declarations",
            "recursa::tokens! { callbacks{scan=crate::scan} }",
            "unsupported.callback-declarations",
        ),
        (
            "post lex",
            "recursa::tokens! { post_lex=crate::repair }",
            "unsupported.post-lex-hook",
        ),
        (
            "parser postcondition",
            "#[recursa::parser (postcondition=crate::validate)] pub struct Bad;",
            "unsupported.parser-postcondition",
        ),
        (
            "custom parser option with spacing",
            "#[recursa :: parser ( custom = parse_special )] pub struct Bad;",
            "unsupported.custom-parser-option",
        ),
        (
            "remaining pratt parser attribute",
            "#[recursa :: parser ( pratt )] pub enum Bad { Value }",
            "unsupported.obsolete-parser-attribute",
        ),
        (
            "unknown parser option",
            "#[recursa::parser (future_option = enabled)] pub struct Bad;",
            "unsupported.obsolete-parser-attribute",
        ),
        (
            "first set module",
            "pub mod __firstset{}",
            "unsupported.obsolete-first-set-module",
        ),
        (
            "legacy container outside a field",
            "pub type Bad = Seq0<Value, punct::Comma>;",
            "rewrite.unhandled-legacy-shape",
        ),
    ];

    for (id, source, expected_code) in cases {
        let error = pass
            .plan_edits(Path::new("src/adversarial.rs"), source)
            .unwrap_err();
        assert_eq!(error.code, expected_code, "case {id}");
        assert!(!error.message.is_empty(), "case {id}");
    }

    let reviewed_rules = "#[recursa::parser (rules=SqlRules)] pub struct Reviewed;";
    assert_eq!(
        rewrite_source(&pass, Path::new("src/reviewed.rs"), reviewed_rules).unwrap(),
        " pub struct Reviewed;"
    );
    assert_eq!(
        rewrite_source(
            &pass,
            Path::new("src/reviewed.rs"),
            "use crate :: __firstset :: *;"
        )
        .unwrap(),
        ""
    );
}

#[test]
fn rewrite_needles_inside_comments_and_literals_are_not_edits() {
    let source = r####"pub struct SqlRules;
// pub struct SqlRules;
pub const REWRITE_LITERAL: &str = r#"pub struct SqlRules;
"#;
pub const ATTRIBUTE_LITERAL: &str = r#"#[recursa::parser(rules = SqlRules)]
"#;
// #[recursa::parser(rules = SqlRules)]
pub struct Kept;
"####;
    let path = Path::new("src/decoy_rewrites.rs");
    let pass = pass();

    let edits = pass.edits(path, source).unwrap();
    assert_eq!(
        edits.len(),
        1,
        "only the real SqlRules declaration is selected"
    );
    let output = rewrite_source(&pass, path, source).unwrap();

    assert!(!output.starts_with("pub struct SqlRules;"));
    assert!(output.contains("// pub struct SqlRules;"));
    assert!(output.contains("r#\"pub struct SqlRules;\n\"#"));
    assert!(output.contains("r#\"#[recursa::parser(rules = SqlRules)]\n\"#"));
    assert!(output.contains("// #[recursa::parser(rules = SqlRules)]"));
    syn::parse_file(&output).unwrap();
}

#[test]
fn unsupported_needles_inside_comments_and_literals_are_not_errors() {
    let source = r####"pub const UNKNOWN_LITERAL: &str = "UnknownGrammar<";
pub const CUSTOM_LITERAL: &str = "#[recursa::parser(custom";
pub const OPTIONAL_LITERAL: &str = "OptionalTrailing<";
// UnknownGrammar<
// #[recursa::parser(custom
// OptionalTrailing<
pub struct Kept;
"####;
    let path = Path::new("src/decoy_unsupported.rs");
    let pass = pass();

    assert!(pass.edits(path, source).unwrap().is_empty());
    assert_eq!(rewrite_source(&pass, path, source).unwrap(), source);
}
