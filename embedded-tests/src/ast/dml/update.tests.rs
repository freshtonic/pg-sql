#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_update_qualified_table() {
        let lexed = crate::lex("UPDATE pg_catalog.pg_class SET relname = '123'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "pg_class");
        assert!(input.is_eof());
    }

    /// A SET target may carry an indirection chain — `[idx]`, `[lo:hi]`,
    /// `.field` (Postgres `set_target: ColId opt_indirection`).
    #[test]
    fn parse_update_set_target_indirection() {
        for src in [
            "UPDATE t SET a[1:2] = '{16,25}'",
            "UPDATE t SET b[1:1][1:1][1:2] = '{1}', c[2:2] = '{x}'",
            "UPDATE t SET alias.col = 1",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            UpdateStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn parse_update_simple() {
        let lexed = crate::lex("UPDATE y SET a = a + 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "y");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_target_aliases() {
        for (src, expected_alias) in [
            ("UPDATE t target SET value = 1", "target"),
            ("UPDATE t format SET value = 1", "format"),
            ("UPDATE t \"set\" SET value = 1", "\"set\""),
            ("UPDATE t AS target SET value = 1", "target"),
            ("UPDATE t AS set SET value = 1", "set"),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt = UpdateStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert_eq!(stmt.alias.as_ref().unwrap().name(), expected_alias);
            assert_eq!(stmt.assignments.len(), 1);
            assert!(input.is_eof());
        }
    }

    #[test]
    fn parse_update_set_as_assignment_target_not_alias() {
        let src = "UPDATE t SET set = 1";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.alias.is_none());
        assert_eq!(stmt.assignments.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn reject_bare_set_as_update_alias() {
        let src = "UPDATE t SET SET value = 1";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(UpdateStmt::parse(&mut input).is_err());
    }

    #[test]
    fn parse_update_multiple_assignments_and_returning_items() {
        let src = "UPDATE t SET a = 1, b = 2 RETURNING a, b";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.alias.is_none());
        assert_eq!(stmt.assignments.len(), 2);
        assert_eq!(stmt.returning.as_ref().unwrap().items.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_with_returning() {
        let lexed = crate::lex("UPDATE y SET a = a + 1 RETURNING *");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.returning.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_with_from_where() {
        let lexed = crate::lex(
            "UPDATE y SET a = y.a - 10 FROM t WHERE y.a > 20 AND t.a = y.a RETURNING y.a",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause.is_some());
        assert!(stmt.where_clause.is_some());
        assert!(stmt.returning.is_some());
        assert!(input.is_eof());
    }

    /// PostgreSQL requires at least one from-list item after UPDATE's
    /// `FROM`; the bare clause must not parse to the end of the input.
    #[test]
    fn reject_update_from_without_from_list() {
        let lexed = crate::lex("UPDATE t SET a = 1 FROM");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = UpdateStmt::parse(&mut input);
        assert!(
            parsed.is_err() || !input.is_eof(),
            "empty UPDATE from-list parsed to EOF"
        );
    }

    #[test]
    fn parse_update_subscript_assignment() {
        let lexed = crate::lex("UPDATE t SET e[0] = '1.1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_subscript_assignment_one() {
        let lexed = crate::lex("UPDATE t SET e[1] = '2.2'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_plain_assignment_still_parses() {
        let lexed = crate::lex("UPDATE t SET col = 'x'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_tuple_set() {
        let lexed = crate::lex(
            "UPDATE parent SET (k, v) = (SELECT k, v FROM simpletup WHERE simpletup.k = parent.k)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// A multi-column SET target list may carry an indirection chain on
    /// individual items — Postgres `set_target: ColId opt_indirection`,
    /// admitted by `'(' set_target_list ')' '=' a_expr`. The rules.sql
    /// regression has `SET (f2[1], f1, tag) = (...)`.
    #[test]
    fn parse_update_tuple_set_with_indirection() {
        let lexed =
            crate::lex("UPDATE trgt SET (f2[1], f1, tag) = (SELECT 1, 2, 'updated'::varchar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// `UPDATE ONLY tab SET ...` excludes inheritance children — Postgres'
    /// `relation_expr` in `gram.y`. The `ONLY` qualifier appears immediately
    /// before the target table name (after the `UPDATE` keyword).
    #[test]
    fn parse_update_only() {
        let lexed = crate::lex("UPDATE ONLY a SET aa = 'zzzzz' WHERE aa = 'aaaaa'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.only, "ONLY qualifier should be parsed");
        assert_eq!(stmt.table_name.object(), "a");
        assert!(input.is_eof());
    }
}
