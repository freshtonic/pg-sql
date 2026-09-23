#[cfg(test)]
mod tests {
    use crate::ast::dml::delete::{DeleteStmt, DeleteTableAlias};

    #[test]
    fn parse_delete_qualified_table() {
        let lexed = crate::lex("DELETE FROM pg_catalog.pg_class");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DeleteStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.relation.name().object(), "pg_class");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_simple() {
        let lexed = crate::lex("DELETE FROM delete_test WHERE a > 25");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DeleteStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.relation.name().object(), "delete_test");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_with_as_alias() {
        let lexed = crate::lex("DELETE FROM delete_test AS dt WHERE dt.a > 75");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DeleteStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.relation.name().object(), "delete_test");
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
        let stmt_parsed = DeleteStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.relation.name().object(), "delete_test");
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
        let stmt_parsed = DeleteStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.relation.name().object(), "t");
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
        let stmt_parsed = DeleteStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.relation.is_only(), "ONLY qualifier should be parsed");
        assert_eq!(stmt.relation.name().object(), "c");
        assert!(input.is_eof());
    }

    // gram.y gives the alias-free `relation_expr_opt_alias` rule the `UMINUS`
    // precedence, which is above `SET`'s, so the parser reduces the empty
    // alias rather than shift the keyword (REL_17_11 gram.y 13801-13810 and
    // `%nonassoc IDENT … SET …` 886-887). DELETE, UPDATE and MERGE share that
    // state. The `AS` form keeps `set`, and every other `ColId` keeps the bare
    // form.
    #[test]
    fn delete_bare_alias_never_takes_set() {
        crate::ast::test_support::check_statement_forms(
            &[
                "DELETE FROM t AS set",
                "DELETE FROM t value",
                "DELETE FROM t between",
            ],
            &[
                "DELETE FROM t set",
                "DELETE FROM ONLY t set",
                "DELETE FROM t set USING u WHERE true",
            ],
        );
    }

    // `returning_clause: RETURNING target_list` has no empty list, so a bare
    // `RETURNING` is neither an alias (the keyword is reserved) nor a clause
    // (REL_14_24 gram.y `returning_clause`; REL_18_6 gram.y
    // `returning_clause`).
    #[test]
    fn returning_takes_a_non_empty_target_list() {
        crate::ast::test_support::check_statement_forms(
            &["DELETE FROM t RETURNING *", "UPDATE t SET a = 1 RETURNING a"],
            &[
                "DELETE FROM t returning",
                "UPDATE t SET a = 1 RETURNING",
                "INSERT INTO t VALUES (1) RETURNING",
            ],
        );
    }
}
