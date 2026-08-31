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
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_database_force() {
        let lexed = crate::lex("DROP DATABASE IF EXISTS db1 WITH (FORCE)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropDatabaseStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
    }
}
