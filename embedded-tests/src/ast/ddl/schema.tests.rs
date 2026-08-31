#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_schema_named() {
        let lexed = crate::lex("CREATE SCHEMA s1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.head, SchemaNameClause::Named(_)));
        assert!(stmt.elements.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_schema_authorization_only() {
        let lexed = crate::lex("CREATE SCHEMA AUTHORIZATION alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.head, SchemaNameClause::Authorization(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_schema_if_not_exists() {
        let lexed = crate::lex("CREATE SCHEMA IF NOT EXISTS s1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_schema_with_named_and_auth() {
        let lexed = crate::lex("CREATE SCHEMA s1 AUTHORIZATION alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap().into_ast();
        match &stmt.head {
            SchemaNameClause::Named(n) => {
                assert_eq!(n.name.text(), "s1");
                assert!(n.authorization.is_some());
            }
            _ => panic!("expected Named"),
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_schema_cascade() {
        let lexed = crate::lex("DROP SCHEMA IF EXISTS s1, s2 CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropSchemaStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.names.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_schema_rename() {
        let lexed = crate::lex("ALTER SCHEMA test_ns_schema_1 RENAME TO test_ns_schema_renamed");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterSchemaStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_schema_owner() {
        let lexed = crate::lex("ALTER SCHEMA testns OWNER TO regress_schemauser2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterSchemaStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
