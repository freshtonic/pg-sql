#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_language() {
        let lexed = crate::lex("CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateLanguageStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_plain() {
        let lexed = crate::lex("CREATE LANGUAGE plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateLanguageStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.text(), "plpgsql");
        assert!(stmt.handler.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_handler() {
        let lexed = crate::lex("CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateLanguageStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let h = stmt.handler.as_ref().expect("handler present");
        assert_eq!(h.name.object(), "plpgsql_call_handler");
        assert!(h.inline.is_none());
        assert!(h.validator.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_or_replace_trusted_procedural_with_validator() {
        let lexed = crate::lex(
            "CREATE OR REPLACE TRUSTED PROCEDURAL LANGUAGE plpgsql \
             HANDLER plpgsql_call_handler \
             INLINE plpgsql_inline_handler \
             VALIDATOR plpgsql_validator",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateLanguageStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.or_replace);
        assert!(stmt.trusted);
        let h = stmt.handler.as_ref().unwrap();
        assert!(h.inline.is_some());
        assert!(matches!(
            h.validator.as_ref().unwrap(),
            LanguageValidatorClause::Some(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_no_validator() {
        let lexed = crate::lex("CREATE LANGUAGE plpgsql HANDLER h NO VALIDATOR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateLanguageStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(
            stmt.handler.as_ref().unwrap().validator.as_ref().unwrap(),
            LanguageValidatorClause::None,
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_language_owner() {
        let lexed = crate::lex("ALTER LANGUAGE plpgsql OWNER TO foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterLanguageStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_language() {
        let lexed = crate::lex("DROP LANGUAGE plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropLanguageStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_procedural_language() {
        let lexed = crate::lex("DROP PROCEDURAL LANGUAGE IF EXISTS plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropLanguageStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_language_procedural_rename() {
        let lexed = crate::lex("ALTER PROCEDURAL LANGUAGE alt_lang1 RENAME TO alt_lang3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterLanguageStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }
}
