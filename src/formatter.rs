//! PostgreSQL rendering through Recursa's structured Pretty capability.

use recursa::{Pretty, PrettyConfig};

/// Renders an AST node with caller-selected width and indentation.
pub fn format_tokens_sql(root: &impl Pretty, config: PrettyConfig) -> String {
    root.render_with(config)
}
