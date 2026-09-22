#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    /// `ALTER PUBLICATION ... ADD TABLES IN SCHEMA name (cols)` — PG rejects
    /// this syntactically (`TABLES IN SCHEMA ColId` has no opt_column_list),
    /// but pg-sql accepts it over-permissively so the publication.sql corpus
    /// statement parses into a structured AST.
    #[test]
    fn parse_alter_publication_add_tables_in_schema_with_columns() {
        let lexed = crate::lex("ALTER PUBLICATION testpub1_forschema ADD TABLES IN SCHEMA foo (a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterPublicationStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// Sanity: `ALTER PUBLICATION ... ADD TABLES IN SCHEMA name` (bare,
    /// PG-accepted form) still parses.
    #[test]
    fn parse_alter_publication_add_tables_in_schema_bare() {
        let lexed = crate::lex("ALTER PUBLICATION p ADD TABLES IN SCHEMA foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterPublicationStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn create_publication_bare_roundtrips() {
        let stmt = parse_stmt::<CreatePublicationStmt>("CREATE PUBLICATION testpub_default");
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "testpub_default");
        assert!(stmt.r#for.is_none());
        assert!(stmt.with.is_none());
        reparse_stable::<CreatePublicationStmt>("CREATE PUBLICATION testpub_default");
    }

    #[test]
    fn create_publication_for_all_tables_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION testpub_foralltables FOR ALL TABLES WITH (publish = 'insert')",
        );
    }

    #[test]
    fn create_publication_for_table_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION testpub_fortable FOR TABLE testpub_tbl1",
        );
    }

    #[test]
    fn create_publication_for_table_only_where_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION p FOR TABLE testpub_rf_tbl1, ONLY testpub_rf_tbl3 WHERE (e < 999) WITH (publish = 'insert')",
        );
    }

    #[test]
    fn create_publication_for_tables_in_schema_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION testpub_forschema FOR TABLES IN SCHEMA pub_test",
        );
    }

    #[test]
    fn create_publication_mixed_tables_and_schema_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION p FOR TABLES IN SCHEMA pub_test, TABLE pub_test.testpub_nopk",
        );
    }

    #[test]
    fn create_publication_with_columns_and_where_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION p FOR TABLE testpub_rf_tbl1 (c, d) WHERE (c <> 'test' AND d < 5)",
        );
    }

    /// Added in 19: `FOR pub_all_obj_type_list` and `SET
    /// pub_all_obj_type_list` (gram.y b73d13c:10781, 11003,
    /// `PublicationAllObjSpec` 10907, `opt_pub_except_clause` 10902; research
    /// PostgreSQL 19, "Changes to existing statements"). Each kind comes at
    /// most one time (`preprocess_pub_all_objtype_list`).
    #[cfg(feature = "since-pg19")]
    #[test]
    fn publication_all_object_lists_from_19() {
        crate::ast::test_support::check_statement_forms(
            &[
                "CREATE PUBLICATION p FOR ALL TABLES",
                "CREATE PUBLICATION p FOR ALL SEQUENCES",
                "CREATE PUBLICATION p FOR ALL TABLES, ALL SEQUENCES",
                "CREATE PUBLICATION p FOR ALL SEQUENCES, ALL TABLES WITH (publish = 'insert')",
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT (TABLE t1, TABLE ONLY t2, s.t3 *)",
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT (TABLE t1), ALL SEQUENCES",
                "ALTER PUBLICATION p SET ALL SEQUENCES",
                "ALTER PUBLICATION p SET ALL TABLES EXCEPT (TABLE t1), ALL SEQUENCES",
            ],
            &[
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT (t1)",
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT TABLE t1",
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT ()",
                "CREATE PUBLICATION p FOR ALL TABLES, ALL TABLES",
                "CREATE PUBLICATION p FOR ALL SEQUENCES, ALL SEQUENCES",
                "CREATE PUBLICATION p FOR ALL TABLES, TABLE t",
                "CREATE PUBLICATION p FOR ALL SEQUENCES EXCEPT (TABLE t)",
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT (TABLE t1 (a))",
                "ALTER PUBLICATION p ADD ALL TABLES",
            ],
        );
    }

    /// Before 19 there is only `FOR ALL TABLES` (the same research entry).
    #[cfg(not(feature = "since-pg19"))]
    #[test]
    fn reject_publication_all_object_lists_before_19() {
        crate::ast::test_support::check_statement_forms(
            &["CREATE PUBLICATION p FOR ALL TABLES"],
            &[
                "CREATE PUBLICATION p FOR ALL SEQUENCES",
                "CREATE PUBLICATION p FOR ALL TABLES, ALL SEQUENCES",
                "CREATE PUBLICATION p FOR ALL TABLES EXCEPT (TABLE t1)",
                "ALTER PUBLICATION p SET ALL TABLES",
            ],
        );
    }
}
