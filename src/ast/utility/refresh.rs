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
    #[tok(REFRESH, MATERIALIZED, VIEW, this)]
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub name: QualifiedName<'input>,
    pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/utility/refresh.tests.rs"
));
