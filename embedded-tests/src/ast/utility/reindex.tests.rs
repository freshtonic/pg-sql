#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_reindex_tablespace_table() {
        let lexed = crate::lex("REINDEX (TABLESPACE ts) TABLE tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ReindexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reindex_verbose_index() {
        let lexed = crate::lex("REINDEX (VERBOSE) INDEX i");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ReindexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn reindex_table_is_modelled() {
        let stmt: ReindexStmt = parse_stmt("REINDEX TABLE concur_heap");
        assert!(stmt.options.is_none());
        reparse_stable::<ReindexStmt>("REINDEX TABLE concur_heap");
    }

    #[test]
    fn reindex_index_concurrently_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX INDEX CONCURRENTLY brin_insert_optimization_idx");
    }

    #[test]
    fn reindex_schema_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX SCHEMA concur_reindex_schema");
    }

    #[test]
    fn reindex_schema_concurrently_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX SCHEMA CONCURRENTLY pg_catalog");
    }

    #[test]
    fn reindex_database_with_name_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX DATABASE not_current_database");
    }

    #[test]
    fn reindex_system_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX (CONCURRENTLY) SYSTEM");
    }

    #[test]
    fn reindex_system_concurrently_with_name_roundtrips() {
        // Unparenthesized CONCURRENTLY between kind and name. PG rejects this
        // at runtime ("not allowed for SYSTEM") but the grammar accepts it.
        reparse_stable::<ReindexStmt>("REINDEX SYSTEM CONCURRENTLY postgres");
    }

    #[test]
    fn reindex_options_table_roundtrips() {
        let stmt: ReindexStmt = parse_stmt("REINDEX (TABLESPACE ts) TABLE tbl");
        assert!(stmt.options.is_some());
        reparse_stable::<ReindexStmt>("REINDEX (TABLESPACE ts) TABLE tbl");
    }

    #[test]
    fn reindex_qualified_name_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX INDEX CONCURRENTLY pg_toast.pg_toast_1260_index");
    }
}
