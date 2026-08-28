//! SQL pretty-printer using derived FormatTokens.

use recursa::fmt::{FormatStyle, PrintEngine};

/// Format an AST node into pretty-printed SQL.
pub fn format_tokens_sql(root: &impl recursa::FormatTokens, style: FormatStyle) -> String {
    let mut tokens = Vec::new();
    root.format_tokens(&mut tokens);
    let engine = PrintEngine::new(style);
    engine.print(&tokens)
}

/// Format a list of parsed file items into SQL text.
///
/// [`FileItem::ParseError`](crate::ast::FileItem::ParseError) items are skipped (they have no structured AST
/// to format). Callers that need to preserve unparseable text should use
/// [`crate::ast::parse_sql_file_with_spans`] and slice the source by span.
pub fn format_file(items: &[crate::ast::FileItem], style: FormatStyle) -> String {
    let mut output = String::new();
    for item in items {
        match item {
            crate::ast::FileItem::Command(cmd) => {
                output.push_str(&format_tokens_sql(cmd, style.clone()));
                output.push('\n');
            }
            crate::ast::FileItem::RawLines(text) => {
                output.push_str(text);
            }
            crate::ast::FileItem::ParseError { .. } => {
                // No structured AST → no reformatted output. Caller can use
                // `parse_sql_file_with_spans` if they need to preserve the
                // failing source verbatim.
            }
        }
    }
    output
}
