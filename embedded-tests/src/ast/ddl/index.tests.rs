#[cfg(test)]
mod tests {
    use crate::ast::ddl::index::{CreateIndexStmt, DropIndexStmt};

    #[test]
    fn parse_create_unique_index_nulls_distinct() {
        let lexed = crate::lex("CREATE UNIQUE INDEX i ON t (i) NULLS NOT DISTINCT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let lexed = crate::lex("CREATE UNIQUE INDEX i ON t (i) NULLS DISTINCT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.as_ref().unwrap().text(), "fooi");
        assert_eq!(stmt.table_name.object(), "foo");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_with_desc() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo (f1 DESC)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_desc_nulls_last() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo (f1 DESC NULLS LAST)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_if_not_exists() {
        let lexed = crate::lex("CREATE INDEX IF NOT EXISTS fooi ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_concurrently() {
        let lexed = crate::lex("CREATE INDEX CONCURRENTLY fooi ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.concurrently);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_on_only() {
        let lexed = crate::lex("CREATE INDEX idx ON ONLY ptif_test (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_unnamed() {
        let lexed = crate::lex("CREATE INDEX ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.name.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_using_btree() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo USING btree (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.using.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_using_gin() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo USING gin (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_opclass() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo (f1 int4_ops)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let _ = stmt;
    }

    #[test]
    fn parse_create_index_opclass_desc() {
        let lexed = crate::lex("CREATE INDEX fooi ON foo (f1 text_pattern_ops DESC)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_opclass_options() {
        let lexed = crate::lex(
            "CREATE INDEX fooi ON foo (f1 text_pattern_ops (locale = 'C', deterministic = true))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_expr_column() {
        let lexed = crate::lex("CREATE INDEX i ON t ((lower(name)))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// A bare SQL/JSON function is a valid index element (Postgres allows
    /// any `func_expr_windowless`). It must not require extra parentheses.
    #[test]
    fn parse_create_index_bare_json_expr() {
        let sql = "CREATE INDEX ON t (JSON_QUERY(js, '$' PASSING 1 AS x))";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {sql:?}: {e}"))
            .into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_create_index_include() {
        let lexed = crate::lex("CREATE INDEX i ON t (a) INCLUDE (b, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.include.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_where_predicate() {
        let lexed = crate::lex("CREATE INDEX i ON t (a) WHERE a > 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_with_storage() {
        let lexed = crate::lex("CREATE INDEX i ON t (a) WITH (fillfactor = 70)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_storage.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_keyword_value_off() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::lex(
            "CREATE TABLE target (tid integer, balance integer) WITH (autovacuum_enabled=off)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_string_value() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::lex("CREATE TABLE t (a int) WITH (foo = 'bar')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_signed_numeric_value() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::lex("CREATE TABLE t (a int) WITH (fillfactor = -30.1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_signed_integer_value() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::lex("CREATE TABLE t (a int) WITH (fillfactor = +30)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Corpus regression for `reloptions.sql`: `(i INT) WITH (fillfactor=-30.1)`.
    /// Requires both the signed-numeric storage-param value and the PG
    /// operator-boundary rule on the lexer (`=-` lexes as Eq + Minus, not
    /// as one 2-char CustomOp).
    #[test]
    fn parse_create_table_with_storage_no_space_signed_numeric() {
        use crate::ast::ddl::table::CreateTableStmt;
        for src in [
            "CREATE TABLE t (i INT) WITH (fillfactor=-30.1)",
            "CREATE TABLE reloptions_test2(i INT) WITH (fillfactor=-30.1)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
            assert!(input.is_eof(), "{src:?}: parser cursor {}", input.cursor());
        }
    }

    #[test]
    fn parse_create_unique_index() {
        let lexed = crate::lex("CREATE UNIQUE INDEX i ON t (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.unique);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_full_kitchen_sink() {
        let lexed = crate::lex(
            "CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS idx ON t USING btree (a int4_ops ASC, (lower(b))) INCLUDE (c) WITH (fillfactor = 70) WHERE c > 0",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.unique);
        assert!(stmt.concurrently);
        assert!(stmt.if_not_exists.is_some());
        assert!(stmt.using.is_some());
        assert!(stmt.include.is_some());
        assert!(stmt.with_storage.is_some());
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_opclass_on_second_col() {
        let lexed = crate::lex(
            "create unique index op_index_key on insertconflicttest(key, fruit text_pattern_ops)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_collate() {
        let lexed = crate::lex(
            "create unique index collation_index_key on insertconflicttest(key, fruit collate \"C\")",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_collate_and_opclass() {
        let lexed = crate::lex(
            "create unique index both_index_key on insertconflicttest(key, fruit collate \"C\" text_pattern_ops)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_func_target_collate_opclass() {
        let lexed = crate::lex(
            "create unique index both_index_expr_key on insertconflicttest(key, lower(fruit) collate \"C\" text_pattern_ops)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index() {
        let lexed = crate::lex("DROP INDEX fooi");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_if_exists() {
        let lexed = crate::lex("DROP INDEX IF EXISTS fooi");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_concurrently() {
        let lexed = crate::lex("DROP INDEX CONCURRENTLY fooi");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.concurrently);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_multiple() {
        let lexed = crate::lex("DROP INDEX a, b, c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_cascade() {
        let lexed = crate::lex("DROP INDEX fooi CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
}
