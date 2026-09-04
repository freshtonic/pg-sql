//! REFRESH MATERIALIZED VIEW statement.

use crate::ast::shared::names::QualifiedName;

// --- REFRESH ---

/// ```sql
/// REFRESH MATERIALIZED VIEW [CONCURRENTLY] name [WITH [NO] DATA]
/// ```
///
/// Reuses the `WithDataClause` from `create_table.rs` (also used by
/// `CREATE TABLE AS … WITH [NO] DATA`).
#[derive(recursa::Node, Debug, Clone)]
pub struct RefreshStmt<'input> {
    /// Greedy: a leading CONCURRENTLY starts this element instead of ending `RefreshStmt` (bison shift preference).
    #[greedy(CONCURRENTLY)]
    #[tok(REFRESH, MATERIALIZED, VIEW, this)]
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub name: QualifiedName<'input>,
    pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
}
