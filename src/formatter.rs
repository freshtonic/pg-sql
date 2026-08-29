//! SQL pretty-printer using derived FormatTokens.

use recursa::fmt::{FormatStyle, PrintEngine};

/// Format an AST node into pretty-printed SQL.
pub fn format_tokens_sql(root: &impl recursa::FormatTokens, style: FormatStyle) -> String {
    let mut tokens = Vec::new();
    root.format_tokens(&mut tokens);
    let engine = PrintEngine::new(style);
    engine.print(&tokens)
}
