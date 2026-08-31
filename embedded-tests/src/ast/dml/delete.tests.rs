#[cfg(test)]
mod tests {
    use crate::ast::dml::delete::{DeleteStmt, DeleteTableAlias};

    #[test]
    fn parse_delete_qualified_table() {
        let lexed = crate::lex("DELETE FROM pg_catalog.pg_class");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "pg_class");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_simple() {
        let lexed = crate::lex("DELETE FROM delete_test WHERE a > 25");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_with_as_alias() {
        let lexed = crate::lex("DELETE FROM delete_test AS dt WHERE dt.a > 75");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(matches!(
            stmt.alias.as_deref(),
            Some(DeleteTableAlias::WithAs(_))
        ));
        assert_eq!(stmt.alias.as_ref().unwrap().name(), "dt");
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_with_bare_alias() {
        let lexed = crate::lex("DELETE FROM delete_test dt WHERE delete_test.a > 25");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(matches!(
            stmt.alias.as_deref(),
            Some(DeleteTableAlias::Bare(_))
        ));
        assert_eq!(stmt.alias.as_ref().unwrap().name(), "dt");
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_no_where() {
        let lexed = crate::lex("DELETE FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "t");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_none());
        assert!(input.is_eof());
    }

    /// `DELETE FROM ONLY tab` excludes inheritance children — `relation_expr`
    /// in `gram.y`. The `ONLY` qualifier appears immediately before the
    /// target table name.
    #[test]
    fn parse_delete_from_only() {
        let lexed = crate::lex("DELETE FROM ONLY c WHERE aa = 'new'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.only, "ONLY qualifier should be parsed");
        assert_eq!(stmt.table_name.object(), "c");
        assert!(input.is_eof());
    }
}
