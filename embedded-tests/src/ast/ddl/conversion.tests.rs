#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_conversion_set_schema() {
        let lexed = crate::lex("ALTER CONVERSION alt_conv2 SET SCHEMA alt_nsp2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_default_conversion() {
        let lexed = crate::lex(
            "CREATE DEFAULT CONVERSION mydef FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_conversion_without_default() {
        let lexed = crate::lex(
            "CREATE CONVERSION myconv FOR 'UTF8' TO 'LATIN1' FROM utf8_to_iso8859_1",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_conversion_plain() {
        let lexed = crate::lex(
            "CREATE CONVERSION myconv FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(!stmt.default);
        assert_eq!(stmt.name.object(), "myconv");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_default_conversion_structured() {
        let lexed = crate::lex(
            "CREATE DEFAULT CONVERSION public.mydef FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.default);
        assert!(input.is_eof());
    }
}
