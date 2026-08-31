#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_foreign_data_wrapper() {
        let lexed = crate::lex("DROP FOREIGN DATA WRAPPER fdw1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropForeignStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.kind, ForeignObjectKind::DataWrapper));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_foreign_table() {
        let lexed = crate::lex("DROP FOREIGN TABLE IF EXISTS ft1, ft2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropForeignStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.kind, ForeignObjectKind::Table));
        assert_eq!(stmt.names.len(), 2);
        assert!(input.is_eof());
    }

    /// Bare `ALTER FOREIGN DATA WRAPPER name` — PG itself rejects this
    /// (`AlterFdwStmt` requires at least one fdw_option or
    /// alter_generic_options), but the parser is over-permissive to avoid
    /// surfacing as a file-level parse error; the differential
    /// oracle accepts the PG-rejected case because pg-sql's reformat is
    /// also PG-rejected.
    #[test]
    fn parse_alter_fdw_bare() {
        let lexed = crate::lex("ALTER FOREIGN DATA WRAPPER foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterForeignStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Bare `ALTER SERVER name` — similar over-permissive acceptance for
    /// the bare form (gram.y `AlterForeignServerStmt` requires version,
    /// options, or both).
    #[test]
    fn parse_alter_server_bare() {
        let lexed = crate::lex("ALTER SERVER s0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterServerStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn alter_server_option_list_roundtrips() {
        reparse_stable::<AlterServerStmt>(
            "ALTER SERVER s0 OPTIONS (ADD host 'localhost', DROP obsolete)",
        );
    }

    #[test]
    fn alter_foreign_table_without_if_exists_roundtrips() {
        reparse_stable::<AlterForeignStmt>("ALTER FOREIGN TABLE ft SET SCHEMA archive");
    }

    #[test]
    fn import_foreign_schema_qualification_lists_roundtrip() {
        reparse_stable::<ImportForeignSchemaStmt>(
            "IMPORT FOREIGN SCHEMA remote LIMIT TO (t1, public.t2) FROM SERVER srv INTO local",
        );
        reparse_stable::<ImportForeignSchemaStmt>(
            "IMPORT FOREIGN SCHEMA remote EXCEPT (t1, public.t2) FROM SERVER srv INTO local",
        );
    }

    #[test]
    fn create_foreign_data_wrapper_bare_roundtrips() {
        let stmt: CreateForeignStmt = parse_stmt("CREATE FOREIGN DATA WRAPPER foo");
        if let CreateForeignBody::Fdw(b) = &stmt.body {
            assert_eq!(b.name.text(), "foo");
            assert!(b.fdw_options.is_empty());
            assert!(b.options.is_none());
        } else {
            panic!("expected Fdw body");
        }
        reparse_stable::<CreateForeignStmt>("CREATE FOREIGN DATA WRAPPER foo");
    }

    #[test]
    fn create_foreign_data_wrapper_handler_validator_options_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN DATA WRAPPER test_fdw HANDLER test_fdw_handler VALIDATOR postgresql_fdw_validator OPTIONS (testing '1', another '2')",
        );
    }

    #[test]
    fn create_foreign_data_wrapper_no_handler_no_validator_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN DATA WRAPPER foo NO HANDLER NO VALIDATOR",
        );
    }

    #[test]
    fn create_foreign_table_columns_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft2 (c1 integer NOT NULL, c2 text, c3 date) SERVER s0 OPTIONS (delimiter ',', quote '\"', \"be quoted\" 'value')",
        );
    }

    #[test]
    fn create_foreign_table_with_column_options_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft1 (c1 integer OPTIONS (\"param 1\" 'val1') NOT NULL, c2 text OPTIONS (param2 'val2') CHECK (c2 <> ''), c3 date) SERVER s0 OPTIONS (delimiter ',')",
        );
    }

    #[test]
    fn create_foreign_table_inherits_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft2 () INHERITS (fd_pt1) SERVER s0 OPTIONS (delimiter ',')",
        );
    }

    #[test]
    fn create_foreign_table_partition_of_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft_part1 PARTITION OF lt1 FOR VALUES FROM (0) TO (1000) SERVER s0",
        );
    }

    #[test]
    fn create_foreign_table_if_not_exists_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE IF NOT EXISTS ft1 (a INT) SERVER s0",
        );
    }

    #[test]
    fn create_server_minimal_roundtrips() {
        let stmt: CreateServerStmt = parse_stmt("CREATE SERVER s1 FOREIGN DATA WRAPPER foo");
        assert_eq!(stmt.name.text(), "s1");
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.server_type.is_none());
        assert!(stmt.version.is_none());
        assert_eq!(stmt.fdw.name.text(), "foo");
        assert!(stmt.options.is_none());
        reparse_stable::<CreateServerStmt>("CREATE SERVER s1 FOREIGN DATA WRAPPER foo");
    }

    #[test]
    fn create_server_if_not_exists_roundtrips() {
        reparse_stable::<CreateServerStmt>(
            "CREATE SERVER IF NOT EXISTS s1 FOREIGN DATA WRAPPER foo",
        );
    }

    #[test]
    fn create_server_type_version_options_roundtrips() {
        reparse_stable::<CreateServerStmt>(
            "CREATE SERVER s7 TYPE 'oracle' VERSION '17.0' FOREIGN DATA WRAPPER foo OPTIONS (host 'a', dbname 'b')",
        );
    }

    #[test]
    fn create_server_version_null_roundtrips() {
        reparse_stable::<CreateServerStmt>("CREATE SERVER s VERSION NULL FOREIGN DATA WRAPPER foo");
    }
}
