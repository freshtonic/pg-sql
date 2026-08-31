#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    /// `CREATE TRANSFORM FOR Typename LANGUAGE name ( from_fn, to_fn )` —
    /// from the object_address.sql corpus, the only `CREATE TRANSFORM` in
    /// the regression suite. Exercises both `FROM SQL WITH FUNCTION ...`
    /// and `TO SQL WITH FUNCTION ...` element forms.
    #[test]
    fn parse_create_transform() {
        let lexed = crate::lex("CREATE TRANSFORM FOR int LANGUAGE SQL (\
             FROM SQL WITH FUNCTION prsd_lextype(internal),\
             TO SQL WITH FUNCTION int4recv(internal))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTransformStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_or_replace_transform_to_only() {
        let lexed = crate::lex("CREATE OR REPLACE TRANSFORM FOR text LANGUAGE plpgsql \
             (TO SQL WITH FUNCTION textrecv(internal))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTransformStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_transform() {
        let lexed = crate::lex("DROP TRANSFORM IF EXISTS FOR int LANGUAGE SQL CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTransformStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
}
