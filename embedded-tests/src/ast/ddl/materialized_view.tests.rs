#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_materialized_view_minimal() {
        let stmt: CreateMaterializedViewStmt =
            parse_stmt("CREATE MATERIALIZED VIEW mv AS SELECT 1");
        assert_eq!(stmt.target.name.object(), "mv");
        assert!(!stmt.unlogged);
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.target.column_list.is_none());
    }

    #[test]
    fn create_materialized_view_with_no_data_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv AS SELECT * FROM t WITH NO DATA",
        );
    }

    #[test]
    fn create_materialized_view_if_not_exists_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW IF NOT EXISTS mv AS SELECT 1",
        );
    }

    #[test]
    fn create_materialized_view_using_access_method_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv USING heap2 AS SELECT * FROM t",
        );
    }

    #[test]
    fn create_materialized_view_column_list_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv (ii, jj) AS SELECT i, j FROM t",
        );
    }

    #[test]
    fn create_unlogged_materialized_view_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE UNLOGGED MATERIALIZED VIEW mv AS SELECT 1",
        );
    }

    #[test]
    fn create_materialized_view_with_storage_tablespace_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv WITH (fillfactor = 50) TABLESPACE ts AS SELECT 1",
        );
    }
}
