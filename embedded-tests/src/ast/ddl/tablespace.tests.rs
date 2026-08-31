#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_create_tablespace_basic() {
        let lexed = crate::lex("CREATE TABLESPACE ts1 LOCATION '/tmp'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_tablespace_with_options() {
        let lexed =
            crate::lex("CREATE TABLESPACE ts1 LOCATION '' WITH (random_page_cost = 3.0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_tablespace_owner() {
        let lexed = crate::lex("CREATE TABLESPACE ts1 OWNER foo LOCATION '/tmp'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_tablespace() {
        let lexed = crate::lex("DROP TABLESPACE ts1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_tablespace_if_exists() {
        let lexed = crate::lex("DROP TABLESPACE IF EXISTS ts1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_set() {
        let lexed = crate::lex("ALTER TABLESPACE ts SET (random_page_cost = 1.0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_reset() {
        let lexed = crate::lex(
            "ALTER TABLESPACE ts RESET (random_page_cost, effective_io_concurrency)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_rename() {
        let lexed = crate::lex("ALTER TABLESPACE ts RENAME TO ts2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_owner() {
        let lexed = crate::lex("ALTER TABLESPACE ts OWNER TO foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
