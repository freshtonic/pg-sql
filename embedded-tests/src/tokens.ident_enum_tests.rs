#[cfg(test)]
mod ident_enum_tests {
    use super::literal::*;
    #[test]
    fn ident_peek_rejects_from_keyword() {
        let lexed = crate::lex("FROM");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(
            Ident::parse(&mut input).is_err(),
            "Ident should reject FROM"
        );
    }
}
