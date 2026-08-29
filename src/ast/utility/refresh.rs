//! REFRESH MATERIALIZED VIEW statement.

use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn refresh_materialized_view_is_modelled() {
        let stmt: RefreshStmt = parse_stmt("REFRESH MATERIALIZED VIEW mvtest_tm");
        assert_eq!(stmt.name.object(), "mvtest_tm");
        assert!(stmt.concurrently.is_none());
        assert!(stmt.with_data.is_none());
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW mvtest_tm");
    }

    #[test]
    fn refresh_materialized_view_concurrently_roundtrips() {
        let stmt: RefreshStmt = parse_stmt("REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tm");
        assert!(stmt.concurrently.is_some());
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tm");
    }

    #[test]
    fn refresh_materialized_view_with_no_data_roundtrips() {
        let stmt: RefreshStmt =
            parse_stmt("REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tvmm WITH NO DATA");
        assert!(stmt.with_data.is_some());
        reparse_stable::<RefreshStmt>(
            "REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tvmm WITH NO DATA",
        );
    }

    #[test]
    fn refresh_materialized_view_with_data_roundtrips() {
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW mv WITH DATA");
    }

    #[test]
    fn refresh_materialized_view_qualified_name_roundtrips() {
        let stmt: RefreshStmt = parse_stmt("REFRESH MATERIALIZED VIEW matview_schema.mv_withdata2");
        assert_eq!(stmt.name.object(), "mv_withdata2");
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW matview_schema.mv_withdata2");
    }
}
