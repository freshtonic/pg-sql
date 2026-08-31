#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_large_object() {
        let lexed = crate::lex("ALTER LARGE OBJECT 42 OWNER TO regress_lo_user");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterLargeObjectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
