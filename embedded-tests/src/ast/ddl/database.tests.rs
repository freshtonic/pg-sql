#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_database_plain() {
        let lexed = crate::lex("CREATE DATABASE mydb");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDatabaseStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.text(), "mydb");
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_database_with_options() {
        let lexed = crate::lex(
            "CREATE DATABASE mydb ENCODING utf8 LC_COLLATE \"C\" LC_CTYPE \"C\" TEMPLATE template0",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDatabaseStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.options.len(), 4);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_database_with_equals_and_connection_limit() {
        let lexed = crate::lex(
            "CREATE DATABASE mydb WITH OWNER = alice CONNECTION LIMIT = 5 IS_TEMPLATE = TRUE",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDatabaseStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_eof());
    }

    /// gram.y `AlterDatabaseSetStmt: ALTER DATABASE name SetResetClause`
    /// (REL_17_11:11441). `SetResetClause` is `SET set_rest` and
    /// `VariableResetStmt`, so every special form of an unscoped top-level
    /// `SET` belongs here, but neither `LOCAL` nor `SESSION` does. The
    /// dedicated `AlterDatabaseStmt: ... SET TABLESPACE name` branch still
    /// wins over `generic_set`, whose `var_name` would take `TABLESPACE`.
    #[test]
    fn alter_database_set_reset_clause() {
        check_statement_forms(
            &[
                "ALTER DATABASE d SET search_path = a, b",
                "ALTER DATABASE d SET search_path TO DEFAULT",
                "ALTER DATABASE d SET a.b = 1",
                "ALTER DATABASE d SET TIME ZONE 'UTC'",
                "ALTER DATABASE d SET TIME ZONE LOCAL",
                "ALTER DATABASE d SET TIME ZONE -7",
                "ALTER DATABASE d SET SESSION AUTHORIZATION bob",
                "ALTER DATABASE d SET SESSION AUTHORIZATION DEFAULT",
                "ALTER DATABASE d SET ROLE bob",
                "ALTER DATABASE d SET XML OPTION DOCUMENT",
                "ALTER DATABASE d SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
                "ALTER DATABASE d SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY",
                "ALTER DATABASE d SET TRANSACTION SNAPSHOT '000'",
                "ALTER DATABASE d SET TABLESPACE ts",
                "ALTER DATABASE d RESET work_mem",
                "ALTER DATABASE d RESET a.b",
                "ALTER DATABASE d RESET ALL",
                "ALTER DATABASE d RESET TIME ZONE",
                "ALTER DATABASE d RESET SESSION AUTHORIZATION",
                "ALTER DATABASE d RESET TRANSACTION ISOLATION LEVEL",
                "ALTER DATABASE d RESET TABLESPACE",
            ],
            &[
                "ALTER DATABASE d SET x",
                "ALTER DATABASE d SET LOCAL x = 1",
                "ALTER DATABASE d SET SESSION x = 1",
                // `set_rest_more: CATALOG_P Sconst` exists, but its action is
                // an unconditional `ereport(ERROR)` (REL_17_11:1723), so the
                // raw parser rejects it.
                "ALTER DATABASE d SET CATALOG 'x'",
                "ALTER DATABASE d RESET",
            ],
        );
    }

    #[test]
    fn parse_drop_database_force() {
        let lexed = crate::lex("DROP DATABASE IF EXISTS db1 WITH (FORCE)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropDatabaseStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
    }
}
