use std::path::Path;

use pg_sql_migrate::rewrite::{SourceRewritePass, rewrite_source};
use pg_sql_migrate::test_call_rewrite::TestCallRewritePass;

fn fixture(case: &str, name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/rewrite/test_calls")
            .join(case)
            .join(name),
    )
    .unwrap()
}

#[test]
fn rewrites_a_successful_direct_parse_through_the_public_pass_seam() {
    let input = fixture("direct_parse", "input.rs");
    let expected = fixture("direct_parse", "expected.rs");

    let rewritten =
        rewrite_source(&TestCallRewritePass, Path::new("src/ast/query.rs"), &input).unwrap();

    assert_eq!(rewritten, expected);
}

#[test]
fn rewrites_every_supported_test_call_shape_and_preserves_grammar_bytes() {
    let input = fixture("all_supported", "input.rs");
    let expected = fixture("all_supported", "expected.rs");
    let path = Path::new("src/ast/query.rs");

    let edits = TestCallRewritePass.edits(path, &input).unwrap();
    assert!(edits.windows(2).all(|pair| pair[0].start < pair[1].start));
    let rewritten = rewrite_source(&TestCallRewritePass, path, &input).unwrap();

    assert_eq!(rewritten, expected);
    let grammar_end = input.find("#[cfg(test)]").unwrap();
    assert_eq!(&rewritten[..grammar_end], &input[..grammar_end]);
}

#[test]
fn repeating_the_pass_is_byte_deterministic() {
    let input = fixture("all_supported", "input.rs");
    let path = Path::new("src/ast/query.rs");

    let first = rewrite_source(&TestCallRewritePass, path, &input).unwrap();
    let second = rewrite_source(&TestCallRewritePass, path, &input).unwrap();

    assert_eq!(first.as_bytes(), second.as_bytes());
}

#[test]
fn rejects_an_ambiguous_direct_parse_instead_of_guessing() {
    let input = fixture("ambiguous", "input.rs");

    let error =
        rewrite_source(&TestCallRewritePass, Path::new("src/ast/query.rs"), &input).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unsupported ambiguous direct parse")
    );
    assert!(error.to_string().contains("src/ast/query.rs"));
}

#[test]
fn every_inventoried_imported_test_call_is_supported() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut rejected = Vec::new();
    for entry in walkdir::WalkDir::new(repository.join("src")) {
        let entry = entry.unwrap();
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|extension| extension.to_str()) != Some("rs")
            || path
                .components()
                .any(|component| component.as_os_str() == "generated")
        {
            continue;
        }
        let source = std::fs::read_to_string(path).unwrap();
        let relative = path.strip_prefix(repository).unwrap();
        if let Err(error) = rewrite_source(&TestCallRewritePass, relative, &source) {
            rejected.push(format!("{}: {error}", relative.display()));
        }
    }
    assert!(rejected.is_empty(), "{}", rejected.join("\n"));
}
