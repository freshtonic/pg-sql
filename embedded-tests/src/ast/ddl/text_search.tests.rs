#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_text_search_configuration() {
        let lexed = crate::lex("DROP TEXT SEARCH CONFIGURATION IF EXISTS tsc1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTextSearchStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.kind, TextSearchObjectKind::Configuration));
        assert!(stmt.if_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_text_search_dictionary() {
        let lexed =
            crate::lex("CREATE TEXT SEARCH DICTIONARY alt_ts_dict1 (template=simple)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTextSearchStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_text_search_configuration_rename() {
        let lexed = crate::lex(
            "ALTER TEXT SEARCH CONFIGURATION alt_ts_conf1 RENAME TO alt_ts_conf2",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTextSearchStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_text_search_parser() {
        let lexed = crate::lex("DROP TEXT SEARCH PARSER my_parser");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropTextSearchStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_text_search_dictionary_structured() {
        let lexed = crate::lex(
            "CREATE TEXT SEARCH DICTIONARY ispell (Template=ispell, DictFile=ispell_sample)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTextSearchStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.kind, TextSearchObjectKind::Dictionary));
        assert_eq!(stmt.name.object(), "ispell");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_text_search_configuration() {
        let lexed =
            crate::lex("CREATE TEXT SEARCH CONFIGURATION ispell_tst (PARSER = default)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTextSearchStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.kind, TextSearchObjectKind::Configuration));
        assert!(input.is_eof());
    }
}
