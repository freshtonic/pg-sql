#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_type_list() {
        let lexed = crate::lex("DROP TYPE IF EXISTS t1, t2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.types.types.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_rename_to() {
        let lexed = crate::lex("ALTER TYPE bogus RENAME TO bogon");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_set_schema() {
        let lexed = crate::lex("ALTER TYPE alter1.ctype SET SCHEMA alter2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_set_def() {
        let lexed = crate::lex("ALTER TYPE myvarchar SET (storage = extended)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_add_value() {
        let lexed = crate::lex("ALTER TYPE planets ADD VALUE 'uranus'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_add_value_if_not_exists_after() {
        let lexed = crate::lex("ALTER TYPE planets ADD VALUE IF NOT EXISTS 'pluto' AFTER 'neptune'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_rename_value() {
        let lexed = crate::lex("ALTER TYPE rainbow RENAME VALUE 'red' TO 'crimson'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_add_attribute() {
        let lexed = crate::lex("ALTER TYPE test_type ADD ATTRIBUTE b text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_drop_attribute_cascade() {
        let lexed = crate::lex("ALTER TYPE test_type2 DROP ATTRIBUTE b CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_alter_attribute_set_data_type() {
        let lexed = crate::lex("ALTER TYPE test_type ALTER ATTRIBUTE b SET DATA TYPE varchar");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_multi_cmd() {
        let lexed = crate::lex("ALTER TYPE test_type DROP ATTRIBUTE a, ADD ATTRIBUTE d boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_rename_attribute() {
        let lexed = crate::lex("ALTER TYPE test_type RENAME ATTRIBUTE a TO aa");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_shell() {
        let lexed = crate::lex("CREATE TYPE shell");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.body.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_composite() {
        let lexed = crate::lex("CREATE TYPE row1 AS (f1 text, f2 int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Composite(_))));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_enum() {
        let lexed = crate::lex("CREATE TYPE color AS ENUM ('red', 'green', 'blue')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Enum(_))));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_range() {
        let lexed = crate::lex("CREATE TYPE intrange AS RANGE (subtype = int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Range(_))));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_base() {
        let lexed = crate::lex("CREATE TYPE widget (internallength = 24, input = widget_in, output = widget_out)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Base(_))));
        assert!(input.is_eof());
    }
}
