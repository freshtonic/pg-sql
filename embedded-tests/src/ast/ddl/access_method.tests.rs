#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_access_method_index() {
        let lexed = crate::lex("CREATE ACCESS METHOD gist2 TYPE INDEX HANDLER gisthandler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAccessMethodStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert_eq!(stmt.name.text(), "gist2");
        assert!(matches!(stmt.am_type, AccessMethodType::Index));
        assert_eq!(stmt.handler_name.object(), "gisthandler");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_access_method_table() {
        let lexed = crate::lex(
            "CREATE ACCESS METHOD heap2 TYPE TABLE HANDLER heap_tableam_handler",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAccessMethodStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert_eq!(stmt.name.text(), "heap2");
        assert!(matches!(stmt.am_type, AccessMethodType::Table));
        assert!(input.is_eof());
    }
}
