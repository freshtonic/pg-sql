//! REFRESH MATERIALIZED VIEW statement.

use crate::ast::shared::names::QualifiedName;

// --- REFRESH ---

/// ```sql
/// REFRESH MATERIALIZED VIEW [CONCURRENTLY] name [WITH [NO] DATA]
/// ```
///
/// Reuses the `WithDataClause` from `create_table.rs` (also used by
/// `CREATE TABLE AS … WITH [NO] DATA`).
#[derive(recursa::Node, Debug)]
#[tok(REFRESH, MATERIALIZED, VIEW, this)]
pub struct RefreshStmt<'input> {
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub name: QualifiedName<'input>,
    pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
}
