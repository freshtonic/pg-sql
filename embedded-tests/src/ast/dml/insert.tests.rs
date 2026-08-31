#[cfg(test)]
mod tests {
    use crate::ast::dml::insert::InsertStmt;

    #[test]
    fn parse_insert_qualified_table() {
        let lexed = crate::lex("INSERT INTO pg_catalog.foo VALUES (1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "foo");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_insert_with_columns() {
        let lexed = crate::lex("INSERT INTO BOOLTBL1 (f1) VALUES (bool 't')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "BOOLTBL1");
        assert!(stmt.columns.is_some());
        assert_eq!(stmt.columns.as_ref().unwrap().len(), 1);
        assert!(input.is_eof());
    }

    /// An INSERT column-list target may carry an indirection chain —
    /// `f2[1]`, `f3.if1`, `a[1:5]` (Postgres `insert_column_item`).
    #[test]
    fn parse_insert_column_indirection() {
        for src in [
            "INSERT INTO t (f2[1], f2[2]) VALUES (1, 2)",
            "INSERT INTO t (f3.if1, f3.if2) VALUES (1, '{foo}')",
            "INSERT INTO t (a[1:5], b[1:1][1:2]) VALUES ('{1}', '{2}')",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            InsertStmt::parse(&mut input)
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
    fn parse_insert_multiple_columns() {
        let lexed = crate::lex("INSERT INTO BOOLTBL3 (d, b, o) VALUES ('true', true, 1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.columns.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn parse_insert_without_columns() {
        let lexed = crate::lex("INSERT INTO booltbl4 VALUES (false, true, null)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.columns.is_none());
        assert!(matches!(*stmt.source, super::InsertSource::Select(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_insert_default_values_returning() {
        let lexed = crate::lex("INSERT INTO t DEFAULT VALUES RETURNING *");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(*stmt.source, super::InsertSource::Default));
        assert!(stmt.returning.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_insert_select() {
        let lexed = crate::lex("INSERT INTO y SELECT generate_series(1, 10)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(*stmt.source, super::InsertSource::Select(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_insert_on_conflict_do_nothing() {
        let lexed = crate::lex("INSERT INTO t VALUES (1) ON CONFLICT (k) DO NOTHING");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_insert_on_conflict_do_update() {
        let lexed = crate::lex(
            "INSERT INTO t VALUES (1) ON CONFLICT (k) DO UPDATE SET v = 'updated'",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_eof());
    }

    /// `ON CONFLICT ON CONSTRAINT name DO …` — the arbiter-by-constraint form
    /// of `opt_conf_expr` (gram.y `ON CONSTRAINT name`). Distinct from the
    /// `( index_params )` form.
    #[test]
    fn parse_insert_on_conflict_on_constraint_do_nothing() {
        let lexed = crate::lex(
            "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_insert_on_conflict_on_constraint_do_update() {
        let lexed = crate::lex(
            "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO UPDATE SET v = 'x'",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = InsertStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_eof());
    }
}
