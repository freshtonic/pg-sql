#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_sequence_plain() {
        let lexed = crate::lex("CREATE SEQUENCE s1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.persistence.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_options() {
        let lexed = crate::lex(
            "CREATE SEQUENCE s1 AS integer INCREMENT BY 2 MINVALUE 1 MAXVALUE 100 START WITH 5 CACHE 10 CYCLE",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 7);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_no_minvalue_owned_by() {
        let lexed = crate::lex(
            "CREATE SEQUENCE s1 NO MINVALUE NO MAXVALUE NO CYCLE OWNED BY t.col",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 4);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_temp_if_not_exists() {
        let lexed = crate::lex("CREATE TEMPORARY SEQUENCE IF NOT EXISTS s1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.persistence,
            Some(CreatePersistence::Temporary)
        ));
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_owned_by_none() {
        let lexed = crate::lex("CREATE SEQUENCE s1 OWNED BY NONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(stmt.options[0], SeqOption::OwnedBy(_)));
        assert!(input.is_eof());
    }
}
