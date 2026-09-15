//! REFRESH MATERIALIZED VIEW statement.

use crate::ast::shared::names::QualifiedName;

// --- REFRESH ---

recursa::ast_node! {
    /// ```sql
    /// REFRESH MATERIALIZED VIEW [CONCURRENTLY] name [WITH [NO] DATA]
    /// ```
    ///
    /// Reuses the `WithDataClause` from `create_table.rs` (also used by
    /// `CREATE TABLE AS … WITH [NO] DATA`).
    #[derive(Debug)]
    #[tok(REFRESH, MATERIALIZED, VIEW, this)]
    pub struct RefreshStmt {
        #[presence(CONCURRENTLY)]
        pub concurrently: bool,
        pub name: QualifiedName,
        pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
    }
}
