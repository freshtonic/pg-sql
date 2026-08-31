#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_extension_plain() {
        let lexed = crate::lex("CREATE EXTENSION hstore");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "hstore");
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_extension_if_not_exists_with_options() {
        let lexed = crate::lex("CREATE EXTENSION IF NOT EXISTS hstore WITH SCHEMA public VERSION '1.6' CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_update() {
        let lexed = crate::lex("ALTER EXTENSION my_ext UPDATE TO '1.1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_update_no_to() {
        let lexed = crate::lex("ALTER EXTENSION my_ext UPDATE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_set_schema() {
        let lexed = crate::lex("ALTER EXTENSION my_ext SET SCHEMA new_schema");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_add_table() {
        let lexed = crate::lex("ALTER EXTENSION my_ext ADD TABLE my_table");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
