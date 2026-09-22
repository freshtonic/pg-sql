#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_reindex_tablespace_table() {
        let lexed = crate::lex("REINDEX (TABLESPACE ts) TABLE tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ReindexStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reindex_verbose_index() {
        let lexed = crate::lex("REINDEX (VERBOSE) INDEX i");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ReindexStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn reindex_table_is_modelled() {
        let stmt = parse_stmt::<ReindexStmt>("REINDEX TABLE concur_heap");
        let stmt = stmt.ast();
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

    // The name is optional from 16: research, PostgreSQL 16, "Changes to
    // existing statements".
    #[cfg(feature = "since-pg16")]
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
        let stmt = parse_stmt::<ReindexStmt>("REINDEX (TABLESPACE ts) TABLE tbl");
        let stmt = stmt.ast();
        assert!(stmt.options.is_some());
        reparse_stable::<ReindexStmt>("REINDEX (TABLESPACE ts) TABLE tbl");
    }

    #[test]
    fn reindex_qualified_name_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX INDEX CONCURRENTLY pg_toast.pg_toast_1260_index");
    }

    // Added in 16: research, PostgreSQL 16, "Changes to existing statements"
    // (REL_16_15 gram.y `reindex_target_all opt_concurrently opt_single_name`).
    #[cfg(feature = "since-pg16")]
    #[test]
    fn reindex_database_without_a_name() {
        crate::ast::test_support::assert_statements_parse(&[
            "REINDEX DATABASE",
            "REINDEX SYSTEM",
            "REINDEX DATABASE CONCURRENTLY",
            "REINDEX (VERBOSE) DATABASE",
        ]);
    }

    // Added in 16, so rejected before 16: REL_15_19 gram.y has
    // `REINDEX reindex_target_multitable opt_concurrently name`.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn reindex_database_needs_a_name_before_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "REINDEX DATABASE",
            "REINDEX SYSTEM",
            "REINDEX DATABASE CONCURRENTLY",
            "REINDEX (VERBOSE) DATABASE",
            "REINDEX (CONCURRENTLY) SYSTEM",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "REINDEX DATABASE d",
            "REINDEX SYSTEM d",
            "REINDEX DATABASE CONCURRENTLY d",
            "REINDEX (VERBOSE) SYSTEM s",
        ]);
    }
}
