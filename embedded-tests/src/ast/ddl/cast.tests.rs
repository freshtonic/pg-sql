#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_cast() {
        let lexed = crate::lex("DROP CAST IF EXISTS (text AS text) RESTRICT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_cast_without_function() {
        let lexed = crate::lex("CREATE CAST (text AS casttesttype) WITHOUT FUNCTION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.r#impl, CastImpl::WithoutFunction));
        assert!(stmt.context.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_cast_with_inout_implicit() {
        let lexed = crate::lex("CREATE CAST (int4 AS casttesttype) WITH INOUT AS IMPLICIT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.r#impl, CastImpl::WithInout));
        assert!(matches!(
            stmt.context.as_ref().unwrap().kind,
            CastContextKind::Implicit
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_cast_with_function_assignment() {
        let lexed = crate::lex(
            "CREATE CAST (int4 AS casttesttype) WITH FUNCTION int4_casttesttype(int4) AS ASSIGNMENT",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.r#impl, CastImpl::WithFunction(_)));
        assert!(matches!(
            stmt.context.as_ref().unwrap().kind,
            CastContextKind::Assignment
        ));
        assert!(input.is_eof());
    }
}
