#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn refresh_materialized_view_is_modelled() {
        let stmt = parse_stmt::<RefreshStmt>("REFRESH MATERIALIZED VIEW mvtest_tm");
        let stmt = stmt.ast();
        assert_eq!(stmt.name.object(), "mvtest_tm");
        assert!(!stmt.concurrently);
        assert!(stmt.with_data.is_none());
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW mvtest_tm");
    }

    #[test]
    fn refresh_materialized_view_concurrently_roundtrips() {
        let stmt = parse_stmt::<RefreshStmt>("REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tm");
        let stmt = stmt.ast();
        assert!(stmt.concurrently);
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tm");
    }

    #[test]
    fn refresh_materialized_view_with_no_data_roundtrips() {
        let stmt = parse_stmt::<RefreshStmt>("REFRESH MATERIALIZED VIEW CONCURRENTLY mvtest_tvmm WITH NO DATA");
        let stmt = stmt.ast();
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
        let stmt = parse_stmt::<RefreshStmt>("REFRESH MATERIALIZED VIEW matview_schema.mv_withdata2");
        let stmt = stmt.ast();
        assert_eq!(stmt.name.object(), "mv_withdata2");
        reparse_stable::<RefreshStmt>("REFRESH MATERIALIZED VIEW matview_schema.mv_withdata2");
    }
}
