#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_collation_rename() {
        let lexed = crate::lex("ALTER COLLATION test1 RENAME TO test11");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_collation_refresh_version() {
        let lexed = crate::lex("ALTER COLLATION en_us REFRESH VERSION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_collation_def_list() {
        let lexed = crate::lex("CREATE COLLATION mycoll (LC_COLLATE = \"POSIX\", LC_CTYPE = \"POSIX\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCollationStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "mycoll");
        assert!(matches!(stmt.body, CreateCollationBody::Options(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_collation_from() {
        let lexed = crate::lex("CREATE COLLATION mycoll FROM \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, CreateCollationBody::From(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_collation_if_not_exists() {
        let lexed = crate::lex("CREATE COLLATION IF NOT EXISTS mycoll FROM \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }
}
