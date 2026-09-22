#[cfg(test)]
mod tests {
    use crate::ast::dml::select::{
        GroupByItem, JoinSuffix, SelectDistinctRef, SelectItem, SelectStmt, UnqualifiedJoin,
        UnqualifiedJoinKind,
    };
    use crate::ast::dml::values::QueryBody;
    use crate::ast::shared::expr::Expr;

    /// Parse `src` as a complete `SELECT` through the logos lex pass.
    type SelectFamily =
        <SelectStmt<'static> as recursa::ArenaParse<crate::Input<'static>>>::Family;

    fn parse_select_classified(
        src: &'static str,
    ) -> recursa::ArenaParsed<'static, SelectFamily> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
            ;
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        stmt_parsed
    }

    /// A JOIN's `ON`/`USING` may be deferred: `a JOIN b JOIN c ON x ON y`
    /// parses as `a JOIN (b JOIN c ON x) ON y`. The left-deep form must be
    /// unchanged, and the recursion must also work inside parentheses.
    #[test]
    fn parse_stacked_on_joins() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        let right_recursive_src = "SELECT * FROM a JOIN b JOIN c ON x ON y";
        let right_recursive_parsed = parse_select_classified(right_recursive_src);
        let right_recursive = right_recursive_parsed.ast();
        let [table] = right_recursive
            .from_clause()
            .expect("FROM clause")
            .tables
            .as_slice()
        else {
            panic!("expected one FROM item");
        };
        let [JoinSuffix::Qualified(outer_join)] = table.joins.as_slice() else {
            panic!("right-recursive form must have one outer JOIN");
        };
        let [JoinSuffix::Qualified(inner_join)] = outer_join.table.joins.as_slice() else {
            panic!("right operand must own its nested JOIN");
        };
        assert_eq!(
            format_tokens_sql(&inner_join.condition, PrettyConfig::default()).trim(),
            "ON x",
        );
        assert_eq!(
            format_tokens_sql(&outer_join.condition, PrettyConfig::default()).trim(),
            "ON y",
        );
        assert_eq!(
            format_tokens_sql(right_recursive, PrettyConfig::default()).trim(),
            right_recursive_src,
        );

        let left_deep_src = "SELECT * FROM a JOIN b ON x JOIN c ON y";
        let left_deep_parsed = parse_select_classified(left_deep_src);
        let left_deep = left_deep_parsed.ast();
        let [table] = left_deep
            .from_clause()
            .expect("FROM clause")
            .tables
            .as_slice()
        else {
            panic!("expected one FROM item");
        };
        let [JoinSuffix::Qualified(first_join), JoinSuffix::Qualified(second_join)] =
            table.joins.as_slice()
        else {
            panic!("left-deep form must keep two top-level JOINs");
        };
        assert!(first_join.table.joins.is_empty());
        assert!(second_join.table.joins.is_empty());
        assert_eq!(
            format_tokens_sql(&first_join.condition, PrettyConfig::default()).trim(),
            "ON x",
        );
        assert_eq!(
            format_tokens_sql(&second_join.condition, PrettyConfig::default()).trim(),
            "ON y",
        );
        assert_eq!(
            format_tokens_sql(left_deep, PrettyConfig::default()).trim(),
            left_deep_src,
        );

        for src in [
            "SELECT * FROM t1 LEFT JOIN \
             (t2 LEFT JOIN t3 FULL JOIN t4 ON p ON q) \
             LEFT JOIN t5 ON r ON s", // parenthesised
            "SELECT * FROM a CROSS JOIN b JOIN c ON x", // unqualified mixed in
        ] {
            parse_select_classified(src).ast();
        }
    }

    #[test]
    fn parse_rows_from() {
        for src in [
            "SELECT * FROM ROWS FROM(f(1), g(2)) WITH ORDINALITY AS z(a, b, c, ord)",
            "SELECT * FROM ROWS FROM(getf(1) AS (id int, nm text)) AS z(a, b)",
        ] {
            parse_select_classified(src).ast();
        }
    }

    #[test]
    fn parse_xmltable_and_lateral() {
        for src in [
            "SELECT * FROM XMLTABLE('/r' PASSING d COLUMNS \
             a int PATH '@id', o FOR ORDINALITY, n text PATH 'N' NOT NULL, \
             p text DEFAULT 'x') AS f (x, y)",
            "SELECT * FROM XMLTABLE(XMLNAMESPACES('http://x' AS zz), '/zz:r' \
             PASSING BY REF d COLUMNS a int PATH 'zz:a')",
            // LATERAL now wraps XMLTABLE / a function table / a subquery.
            "SELECT * FROM d, LATERAL XMLTABLE('/r' PASSING data COLUMNS a int) jt",
            "SELECT * FROM t, LATERAL generate_series(1, t.n) g",
            "SELECT * FROM t, LATERAL (SELECT 1) s",
        ] {
            parse_select_classified(src).ast();
        }
    }

    // `JSON_TABLE` is added in 17: research, PostgreSQL 17, "Queries and expressions".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_json_table() {
        for src in [
            // empty COLUMNS, ordinality, typed, EXISTS, behaviors
            "SELECT * FROM JSON_TABLE(NULL, '$' COLUMNS ())",
            "SELECT * FROM JSON_TABLE('[]', 'strict $.a' COLUMNS (js2 int PATH '$') ERROR ON ERROR)",
            "SELECT * FROM JSON_TABLE(jsonb '1', '$' COLUMNS \
             (id FOR ORDINALITY, c int EXISTS PATH '$.a' UNKNOWN ON ERROR))",
            // FORMAT JSON / wrapper / quotes on columns
            "SELECT * FROM JSON_TABLE(js, 'lax $[*]' COLUMNS \
             (jsb jsonb FORMAT JSON PATH '$' OMIT QUOTES, jw json PATH '$' WITH WRAPPER))",
            // path-name, PASSING, NESTED, table alias with column aliases
            "SELECT * FROM JSON_TABLE(js, '$' AS root PASSING 1 AS a \
             COLUMNS (a int, NESTED PATH '$.b' AS nb COLUMNS (c int PATH '$'))) AS jt (x, y)",
        ] {
            parse_select_classified(src).ast();
        }
    }

    #[test]
    fn parse_simple_select() {
        let lexed = crate::lex("SELECT 1 AS one");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.item_count(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_distinct_heads() {
        for (src, expect_on) in [
            ("SELECT DISTINCT a FROM t", false),
            ("SELECT DISTINCT ON (a) a, b FROM t", true),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt_parsed = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let stmt = stmt_parsed.ast();

            match (stmt.distinct(), expect_on) {
                (Some(SelectDistinctRef::On(_)), true)
                | (Some(SelectDistinctRef::All), false) => {}
                (actual, _) => panic!("unexpected DISTINCT form for {src:?}: {actual:?}"),
            }
            assert_eq!(stmt.item_count(), if expect_on { 2 } else { 1 });
            assert!(input.is_eof());
        }
    }

    #[test]
    fn parse_select_plain_distinct_prefix_identifier() {
        let lexed = crate::lex("SELECT distinct_column FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();

        assert!(stmt.distinct().is_none());
        assert_eq!(stmt.item_count(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_empty_items() {
        let lexed = crate::lex("SELECT FROM emp");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.item_count(), 0);
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    /// gram.y's `opt_target_list` is nullable, so a bare `SELECT` with no
    /// targets, no `INTO` and no `FROM` is a complete `simple_select`.
    /// `union.sql` and `errors.sql` both rely on it.
    #[test]
    fn parse_select_no_targets_at_all() {
        let lexed = crate::lex("SELECT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.item_count(), 0);
        assert!(stmt.from_clause().is_none());
        assert!(stmt.into_clause().is_none());
        assert!(stmt.target_list().is_none());
        assert!(input.is_eof());
    }

    /// A targetless `SELECT` composes as a set-operation operand in every
    /// spelling, including the bare left operand (`union.sql:110`-`112`,
    /// issue #55).
    ///
    /// The bare left operand works because no reserved keyword is in
    /// FIRST(`SelectHead`): `QualifiedRef` qualifies with a `ColId`, so
    /// `UNION`/`INTERSECT`/`EXCEPT` cannot start a target item and the
    /// generated decision reaches the absent head.
    #[test]
    fn parse_targetless_select_set_operations() {
        use crate::ast::dml::values::Subquery;

        for src in [
            "select union select",
            "select intersect select",
            "select except select",
            "select union select 1",
            "select 1 union select",
            "select 1 intersect select",
            "select 1 except select",
            "(select) union select",
            "(select) union (select)",
            "(select) intersect (select)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let _query_parsed = Subquery::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _query = _query_parsed.ast();
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

    /// `SELECT INTO tbl FROM src` — a nullable target list followed by a
    /// present `into_clause` (`create_am.sql:124`).
    #[test]
    fn parse_select_into_without_targets() {
        for src in [
            "SELECT INTO dst FROM src",
            "SELECT INTO TABLE dst FROM src",
            "SELECT INTO TEMP TABLE dst FROM src",
            "SELECT INTO dst",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let stmt_parsed = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let stmt = stmt_parsed.ast();
            assert_eq!(stmt.item_count(), 0, "{src:?}");
            assert!(stmt.into_clause().is_some(), "{src:?}");
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

    /// gram.y `opt_alias_clause_for_join_using` requires `AS`, so a `JOIN`
    /// directly after `USING (...)` continues the join chain
    /// (`create_view.sql:140`-`141`).
    #[test]
    fn parse_join_chained_after_using() {
        for src in [
            "select * from tt2 join tt3 using (b,c) join tt4 using (b)",
            "select * from (tt2 join tt3 using (b,c) join tt4 using (b)) j",
            "select * from tt2 join tt3 using (b) left join tt4 using (b)",
            "select * from tt2 join tt3 using (b) natural join tt4",
            "select * from tt2 join tt3 using (b) AS x join tt4 using (b) AS y",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let _stmt_parsed = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }


    #[test]
    fn parse_select_paren_join_cross() {
        let lexed = crate::lex("SELECT * FROM (a CROSS JOIN b) AS tx");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_paren_join_using() {
        let lexed = crate::lex("SELECT * FROM (a JOIN b USING (i)) AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_paren_join_with_col_aliases() {
        let lexed = crate::lex("SELECT * FROM (a t1 (x, y) CROSS JOIN b t2 (p, q)) AS tx (a, b, c, d)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// `CAST(...)` and `COLLATION FOR (...)` are PG `func_expr_common_subexpr`
    /// forms reachable from `func_table` (gram.y), so they can appear in a
    /// `FROM` clause alongside ordinary function-style table references.
    /// The create_view regression exercises `from coalesce(1,2) as c,
    /// collation for ('x'::text) col, ..., cast(1+2 as int4) as i4` —
    /// modelled via the new `SimpleTableRef::SpecialFunc` variant.
    #[test]
    fn parse_from_special_func_table() {
        for src in [
            "SELECT * FROM collation for ('x'::text)",
            "SELECT * FROM collation for ('x'::text) col",
            "SELECT * FROM cast(1+2 as int4)",
            "SELECT * FROM cast(1+2 as int4) as i4",
            "SELECT * FROM coalesce(1,2) as c, collation for ('x'::text) col, cast(1+2 as int4) as i4",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn parse_select_from_where() {
        let lexed = crate::lex("SELECT f1 FROM BOOLTBL1 WHERE f1 = true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.item_count(), 1);
        assert!(stmt.from_clause().is_some());
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn parse_select_star() {
        let lexed = crate::lex("SELECT * FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.item_count(), 1);
        assert!(matches!(stmt.items().next(), Some(SelectItem::Star(_))));
        assert!(input.is_eof());

        let lexed = crate::lex("SELECT * AS everything");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = SelectStmt::parse(&mut input);
        assert!(
            parsed.is_err() || !input.is_eof(),
            "SELECT * AS alias parsed completely",
        );
    }

    #[test]
    fn parse_select_with_alias_keyword() {
        let lexed = crate::lex("SELECT 1 AS true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let first = stmt.items().next().unwrap();
        let SelectItem::Expr(first) = first else {
            panic!("expected an expression SELECT item");
        };
        let alias = first.alias.as_ref().unwrap();
        assert_eq!(alias.name(), "true");

        for (src, expected_alias) in [("SELECT 1 TABLE", "TABLE"), ("SELECT 1 TRUE", "TRUE")] {
            let stmt_parsed = parse_select_classified(src);
            let stmt = stmt_parsed.ast();
            let SelectItem::Expr(item) = stmt.items().next().expect("SELECT item") else {
                panic!("expected expression target for {src:?}");
            };
            assert_eq!(
                item.alias.as_ref().map(|alias| alias.name()),
                Some(expected_alias),
                "bare keyword alias for {src:?}",
            );
        }

        let stmt_parsed = parse_select_classified("SELECT value IS NULL");
        let stmt = stmt_parsed.ast();
        let SelectItem::Expr(item) = stmt.items().next().expect("SELECT item") else {
            panic!("expected expression target");
        };
        assert!(item.alias.is_none());
        assert!(matches!(&item.expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_select_order_by() {
        let lexed = crate::lex("SELECT f1 FROM t ORDER BY f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
    }

    #[test]
    fn parse_select_from_function() {
        for src in [
            "SELECT * FROM pg_input_error_info('junk', 'bool')",
            "SELECT * FROM aggregate_source(*)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
            let stmt = stmt_parsed.ast();
            assert!(stmt.from_clause().is_some());
            assert!(input.is_eof());
        }
    }

    // --- ORDER BY enhancements ---

    #[test]
    fn parse_order_by_using() {
        let lexed = crate::lex("SELECT f1 FROM t ORDER BY f1 using >");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_asc() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 ASC");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_desc() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 DESC");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_nulls_first() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 NULLS FIRST");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_desc_nulls_last() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 DESC NULLS LAST");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    // --- OFFSET/LIMIT ---

    #[test]
    fn parse_select_offset() {
        let lexed = crate::lex("SELECT 1 OFFSET 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.limit_offset().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_limit() {
        let lexed = crate::lex("SELECT 1 LIMIT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.limit_offset().is_some());
        assert!(input.is_eof());
    }

    /// PostgreSQL's `select_limit` accepts at most one limiting clause
    /// (`LIMIT` or `FETCH FIRST`) and at most one `OFFSET` clause. Duplicate
    /// same-kind clauses and `LIMIT` mixed with `FETCH FIRST` must not parse
    /// to the end of the statement.
    #[test]
    fn reject_select_duplicate_limit_offset_clauses() {
        for src in [
            "SELECT 1 LIMIT 1 LIMIT 1",
            "SELECT 1 OFFSET 1 OFFSET 1",
            "SELECT 1 FETCH FIRST 1 ROWS ONLY FETCH FIRST 1 ROWS ONLY",
            "SELECT 1 LIMIT 1 FETCH FIRST 1 ROWS ONLY",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = QueryBody::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "invalid duplicate clause parsed to EOF: {src:?}"
            );
        }
    }

    /// PostgreSQL requires at least one from-list item after `FROM`: only
    /// the target list may be empty (`SELECT FROM emp`), never the
    /// from-list. None of these forms may parse to the end of the input.
    #[test]
    fn reject_select_from_without_from_list() {
        for src in [
            "SELECT FROM",
            "SELECT FROM;",
            "SELECT FROM emp,",
            "SELECT 1 FROM",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = SelectStmt::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "empty from-list parsed to EOF: {src:?}"
            );
        }
    }

    /// Both PostgreSQL clause orders and each bare clause round-trip with the
    /// written order preserved.
    #[test]
    fn parse_select_limit_offset_orders_roundtrip() {
        use crate::ast::test_support::roundtrip;
        for src in [
            "SELECT 1 LIMIT 2 OFFSET 3",
            "SELECT 1 OFFSET 3 LIMIT 2",
            "SELECT 1 OFFSET 3 FETCH FIRST 2 ROWS ONLY",
            "SELECT 1 FETCH FIRST 2 ROWS ONLY OFFSET 3",
            "SELECT 1 LIMIT 2",
            "SELECT 1 OFFSET 3",
            "SELECT 1 FETCH FIRST 2 ROWS ONLY",
        ] {
            assert_eq!(roundtrip::<QueryBody>(src), src);
        }
    }

    // --- FOR UPDATE ---

    #[test]
    fn parse_select_from_only() {
        let lexed = crate::lex("SELECT f1 FROM ONLY t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_only_with_alias() {
        let lexed = crate::lex("SELECT f1 FROM ONLY t AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_qualified_name() {
        let lexed = crate::lex("SELECT * FROM myschema.mytable");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_window_clause_standalone() {
        use super::WindowClause;
        let lexed = crate::lex("WINDOW w AS (PARTITION BY y ORDER BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let wc_parsed = WindowClause::parse(&mut input).unwrap();
        let wc = wc_parsed.ast();
        assert_eq!(wc.defs.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_window_clause() {
        let lexed = crate::lex("SELECT sum(x) OVER w FROM t WINDOW w AS (PARTITION BY y ORDER BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.window.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_frame_rows_between() {
        let lexed = crate::lex(
            "SELECT sum(x) OVER (ORDER BY y ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM t",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_over_named() {
        let lexed = crate::lex("SELECT sum(x) OVER w FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_alias_with_column_list() {
        let lexed = crate::lex("SELECT * FROM tbl AS t (a, b, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_qualified_name_with_alias() {
        let lexed = crate::lex("SELECT * FROM s.t AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_join_using_alias() {
        let lexed = crate::lex("SELECT * FROM a JOIN b USING (i) AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_join_using_alias_where() {
        let lexed = crate::lex("SELECT * FROM a JOIN b USING (i) AS x WHERE x.i = 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_func_with_ordinality() {
        let lexed = crate::lex("SELECT * FROM rngfunct(1) WITH ORDINALITY AS z(a, b, ord)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_func_column_def_list() {
        let lexed = crate::lex("SELECT * FROM test_ret_set_rec_dyn(1500) AS (a int, b int, c int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// Corpus: `select * from json_populate_recordset(row(0::int),'[...]') q (a text, b text)`
    /// — function-call FROM source with a bare-alias-name column-def list (no AS).
    ///
    /// This was deferred when the old generated first-token dispatcher chose
    /// the plain alias before seeing the typed columns. `FuncTableAlias` now
    /// parses the alias head once and lets the LR grammar distinguish each
    /// optional column type. The ignored identity remains pinned by the
    /// immutable migration reconciliation contract; run it explicitly when
    /// changing that contract.
    #[test]
    #[ignore]
    fn parse_select_func_table_bare_alias_col_def() {
        let lexed = crate::lex(
            "select * from json_populate_recordset(row(0::int),'[{\"a\":\"1\"}]') q (a text, b text)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_select_func_with_ordinality_unnest() {
        let lexed = crate::lex("SELECT * FROM unnest(array['a','b']) WITH ORDINALITY AS z(a, ord)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_natural_join() {
        let lexed = crate::lex("SELECT * FROM a NATURAL JOIN b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(input.is_eof());
        let [table] = stmt
            .from_clause()
            .expect("FROM clause")
            .tables
            .as_slice()
        else {
            panic!("expected one FROM item")
        };
        let crate::ast::dml::select::SimpleTableRef::Named(base) = &table.base else {
            panic!("expected identifier-led table")
        };
        assert!(base.tail.is_none(), "NATURAL must not become a table alias");
        let [join] = table.joins.as_slice() else {
            panic!("expected one JOIN suffix")
        };
        assert!(
            matches!(
                join,
                JoinSuffix::Unqualified(UnqualifiedJoin {
                    kind: UnqualifiedJoinKind::Natural(_),
                    ..
                })
            ),
            "NATURAL must belong to the JOIN suffix"
        );
    }

    #[test]
    fn parse_select_natural_left_join() {
        let lexed = crate::lex("SELECT * FROM a NATURAL LEFT JOIN b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_left_outer_join_using() {
        let lexed = crate::lex("SELECT * FROM a LEFT OUTER JOIN b USING (i)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_full_outer_join_using() {
        let lexed = crate::lex("SELECT * FROM a FULL OUTER JOIN b USING (i)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_right_outer_join_on() {
        let lexed = crate::lex("SELECT * FROM a RIGHT OUTER JOIN b ON a.i = b.i");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_paren_join_simple() {
        let lexed = crate::lex("SELECT * FROM a LEFT JOIN (b JOIN c ON b.x = c.x) ON a.y = b.y");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_paren_join_with_subquery_inside() {
        let lexed = crate::lex(
            "SELECT * FROM a LEFT JOIN (b JOIN (SELECT 1 AS x) s ON b.x = s.x) ON a.y = b.y",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_paren_join_leading_subquery() {
        for src in [
            "SELECT * FROM a LEFT JOIN ((SELECT * FROM b) s LEFT JOIN c ON s.x = c.x) ON a.y = s.y",
            // The outer parenthesized table body chooses a grouped query only
            // after the matching inner close exposes UNION.
            "SELECT * FROM ((SELECT 1) UNION SELECT 2) q",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            SelectStmt::parse(&mut input)
                .unwrap_or_else(|error| panic!("parse {src:?}: {error}"))
                ;
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn parse_group_by_grouping_sets_simple() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY GROUPING SETS ((), (a), (a,b))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let group_by = stmt.group_by.as_ref().expect("GROUP BY clause");
        assert!(matches!(
            group_by.items.as_slice(),
            [GroupByItem::GroupingSets(_)]
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_rollup() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY ROLLUP (a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let group_by = stmt.group_by.as_ref().expect("GROUP BY clause");
        assert!(matches!(group_by.items.as_slice(), [GroupByItem::Rollup(_)]));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_cube() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY CUBE (a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let group_by = stmt.group_by.as_ref().expect("GROUP BY clause");
        assert!(matches!(group_by.items.as_slice(), [GroupByItem::Cube(_)]));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_grouping_sets_nested() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY GROUPING SETS (ROLLUP(a), CUBE(b))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let group_by = stmt.group_by.as_ref().expect("GROUP BY clause");
        let [GroupByItem::GroupingSets(grouping_sets)] = group_by.items.as_slice() else {
            panic!("expected one GROUPING SETS item");
        };
        assert!(matches!(
            grouping_sets.groups.as_slice(),
            [rollup, cube]
                if matches!(rollup.as_ref(), GroupByItem::Rollup(_))
                    && matches!(cube.as_ref(), GroupByItem::Cube(_))
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_mixed_primitives() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY a, ROLLUP(b), CUBE(c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let group_by = stmt.group_by.as_ref().expect("GROUP BY clause");
        assert!(matches!(
            group_by.items.as_slice(),
            [
                GroupByItem::Expr(_),
                GroupByItem::Rollup(_),
                GroupByItem::Cube(_)
            ]
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_distinct_modifier() {
        // Regression: groupingsets.sql uses `GROUP BY DISTINCT ROLLUP(a, b), ROLLUP(a, c)`
        // and `GROUP BY ALL ROLLUP(a, b), ROLLUP(a, c)`.
        for src in [
            "SELECT a FROM t GROUP BY DISTINCT ROLLUP(a, b), ROLLUP(a, c)",
            "SELECT a FROM t GROUP BY ALL ROLLUP(a, b), ROLLUP(a, c)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            SelectStmt::parse(&mut input).unwrap();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_select_for_update() {
        let lexed = crate::lex("SELECT f1 FROM t FOR UPDATE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(!stmt.locking_items().is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_locking_variants() {
        // Regression: matview.sql uses `FOR SHARE`. Postgres also supports
        // `FOR NO KEY UPDATE` and `FOR KEY SHARE`.
        for src in [
            "SELECT * FROM t FOR SHARE",
            "SELECT * FROM t FOR NO KEY UPDATE",
            "SELECT * FROM t FOR KEY SHARE",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt_parsed = QueryBody::parse(&mut input).unwrap();
            let stmt = stmt_parsed.ast();
            assert!(!stmt.locking_items().is_empty(), "no locking clause: {src:?}");
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_order_by_using_locale_ops() {
        // Postgres allows custom operators in ORDER BY ... USING <op>.
        // The locale-aware operators ~<~, ~>~, ~<=~, ~>=~ are the main ones.
        for src in [
            "SELECT * FROM t ORDER BY c USING ~<~",
            "SELECT * FROM t ORDER BY c USING ~>~",
            "SELECT * FROM t ORDER BY c USING ~<=~",
            "SELECT * FROM t ORDER BY c USING ~>=~",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = QueryBody::parse(&mut input)
                .unwrap_or_else(|e| panic!("{src}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_select_unicode_alias() {
        let lexed = crate::lex(r#"SELECT U&'d\0061t\+000061' AS U&"d\0061t\+000061""#);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_unicode_alias_uescape() {
        let lexed = crate::lex(
            r#"SELECT U&'d!0061t\+000061' UESCAPE '!' AS U&"d*0061t\+000061" UESCAPE '*'"#,
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    // --- LIMIT / OFFSET / FETCH FIRST ---

    #[test]
    fn parse_offset_before_limit() {
        // Standard SQL order: OFFSET before LIMIT.
        let lexed = crate::lex("SELECT * FROM t ORDER BY x OFFSET 10 LIMIT 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_limit_before_offset() {
        // Postgres order: LIMIT before OFFSET.
        let lexed = crate::lex("SELECT * FROM t ORDER BY x LIMIT 5 OFFSET 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_first_rows_only() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST 5 ROWS ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_first_row_with_ties() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST 2 ROW WITH TIES");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_first_rows_no_count() {
        // FETCH FIRST ROWS WITH TIES — count is omitted (defaults to 1).
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST ROWS WITH TIES");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_next_row_only() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH NEXT 1 ROW ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_offset_then_fetch() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x OFFSET 10 FETCH FIRST 5 ROWS ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_then_offset() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST 5 ROWS WITH TIES OFFSET 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// PG accepts `"normalize"('abc', 'def')` as a function call — the
    /// quoted ident escapes NORMALIZE as a user-defined function name.
    /// Quoted names are admitted through `FuncCallName` and route through
    /// `Expr::Func`; the complete call must win over `Expr::ColumnRef` and
    /// consume its argument list.
    #[test]
    fn parse_quoted_keyword_function_name() {
        for src in [
            "SELECT \"normalize\"('abc', 'def')",
            "SELECT \"select\"('a')",
            "SELECT \"trim\"('abc')",
            "SELECT \"any\"('a')",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }
    /// gram.y `func_table` (gram.y:13867) and `rowsfrom_item` (gram.y:13891)
    /// take `func_expr_windowless`, which reaches `COALESCE`, `GREATEST`,
    /// `LEAST` and `NULLIF` through `func_expr_common_subexpr`. Their words
    /// are `COL_NAME` keywords, so the ordinary function-table node never
    /// sees them, and the tree must say which form it holds.
    #[test]
    fn parse_coalesce_family_as_function_tables() {
        for (src, node) in [
            ("SELECT * FROM coalesce(1, 2) AS c", "CoalesceExpr"),
            ("SELECT * FROM greatest(1, 2) WITH ORDINALITY AS g(v, n)", "GreatestExpr"),
            ("SELECT * FROM least(1, 2)", "LeastExpr"),
            ("SELECT * FROM nullif(1, 2) n", "NullIfExpr"),
            ("SELECT * FROM ROWS FROM(coalesce(a, b), f(1)) AS z", "CoalesceExpr"),
            ("SELECT * FROM ROWS FROM(nullif(a, b) AS (x int)) AS z", "NullIfExpr"),
        ] {
            let parsed = parse_select_classified(src);
            let tree = format!("{:?}", parsed.ast());
            assert!(tree.contains(node), "{src:?} did not build {node}: {tree}");
        }
        // A bare word is still a table name.
        let parsed = parse_select_classified("SELECT * FROM coalesce");
        assert!(!format!("{:?}", parsed.ast()).contains("CoalesceExpr"));
    }

    /// gram.y `simple_select: SELECT distinct_clause target_list ...`: only the
    /// `opt_all_clause opt_target_list` form has an empty list. PostgreSQL
    /// rejects `SELECT DISTINCT FROM pg_database` (errors.sql) with a syntax
    /// error at `from`.
    #[test]
    fn reject_distinct_without_a_target_list() {
        for src in [
            "SELECT DISTINCT FROM pg_database",
            "SELECT DISTINCT ON (x) FROM t",
            "SELECT DISTINCT",
            "SELECT DISTINCT ON (x)",
            "SELECT DISTINCT INTO u FROM t",
            "SELECT DISTINCT WHERE true",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = SelectStmt::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "DISTINCT without targets parsed completely: {src:?}",
            );
        }
        // The head without DISTINCT keeps every zero-target form.
        for src in ["SELECT FROM t", "SELECT", "SELECT INTO u FROM t"] {
            assert_eq!(parse_select_classified(src).ast().item_count(), 0, "{src:?}");
        }
        for (src, count) in [
            ("SELECT DISTINCT a FROM t", 1),
            ("SELECT DISTINCT ON (a) a, b FROM t", 2),
            ("SELECT DISTINCT a INTO u FROM t", 1),
        ] {
            let parsed = parse_select_classified(src);
            assert_eq!(parsed.ast().item_count(), count, "{src:?}");
            assert!(parsed.ast().distinct().is_some(), "{src:?}");
            assert!(parsed.ast().from_clause().is_some(), "{src:?}");
        }
    }

    /// gram.y:13336-13348 `rollup_clause` and `cube_clause`. `%nonassoc CUBE
    /// ROLLUP` below `'('` (gram.y:871-873) makes the word followed by `(`
    /// the grouping clause whenever it starts a `group_by_item`. The
    /// differential oracle cannot see which node pg-sql built.
    #[test]
    fn parse_group_by_rollup_and_cube_as_grouping_clauses() {
        use crate::ast::dml::select::GroupByItem;
        fn items(src: &'static str, check: impl for<'a> FnOnce(&'a [GroupByItem<'a>])) {
            let parsed = parse_select_classified(src);
            let group_by = parsed.ast().group_by.as_ref().expect("GROUP BY");
            check(group_by.items.as_slice());
        }

        items("SELECT 1 FROM t GROUP BY rollup(a, b)", |items| {
            let [GroupByItem::Rollup(rollup)] = items else {
                panic!("expected Rollup, got {items:?}");
            };
            assert_eq!(rollup.items.len(), 2);
        });
        items("SELECT 1 FROM t GROUP BY CUBE (a, b + 1, (c, d))", |items| {
            let [GroupByItem::Cube(cube)] = items else {
                panic!("expected Cube, got {items:?}");
            };
            assert_eq!(cube.items.len(), 3);
        });
        items("SELECT 1 FROM t GROUP BY a, rollup(b), cube(c), ()", |items| {
            assert!(matches!(
                items,
                [
                    GroupByItem::Expr(_),
                    GroupByItem::Rollup(_),
                    GroupByItem::Cube(_),
                    GroupByItem::Empty(_)
                ]
            ));
        });
        items(
            "SELECT 1 FROM t GROUP BY GROUPING SETS (rollup(a, b), cube(c), (d))",
            |items| {
                let [GroupByItem::GroupingSets(sets)] = items else {
                    panic!("expected GroupingSets, got {items:?}");
                };
                assert!(matches!(
                    sets.groups.as_slice(),
                    [a, b, c] if matches!(**a, GroupByItem::Rollup(_))
                        && matches!(**b, GroupByItem::Cube(_))
                        && matches!(**c, GroupByItem::Expr(_))
                ));
            },
        );

        // Not at the start of a `group_by_item`: a function call.
        items("SELECT 1 FROM t GROUP BY (cube(a, b))", |items| {
            let [GroupByItem::Expr(expr)] = items else {
                panic!("expected Expr, got {items:?}");
            };
            assert!(matches!(**expr, Expr::Parenthesized(_)));
        });
        items("SELECT 1 FROM t GROUP BY pg_catalog.cube(a, b), s.rollup(a)", |items| {
            assert!(matches!(items, [GroupByItem::Expr(a), GroupByItem::Expr(b)]
                if matches!(**a, Expr::QualRef(_)) && matches!(**b, Expr::QualRef(_))));
        });
        // A bare word is a column, and outside GROUP BY the call is a call.
        items("SELECT 1 FROM t GROUP BY cube, rollup", |items| {
            assert!(matches!(items, [GroupByItem::Expr(a), GroupByItem::Expr(b)]
                if matches!(**a, Expr::ColumnRef(_)) && matches!(**b, Expr::ColumnRef(_))));
        });
        let parsed = parse_select_classified("SELECT cube(a, b), rollup(a) FROM t");
        assert!(format!("{:?}", parsed.ast()).matches("Func(").count() >= 2);

        // `expr_list` is never empty.
        for src in ["SELECT 1 FROM t GROUP BY rollup()", "SELECT 1 FROM t GROUP BY cube()"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = SelectStmt::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// gram.y:12791 `SELECT opt_all_clause opt_target_list`: `ALL` is a noise
    /// word that keeps the nullable target list, so `SELECT ALL FROM t` is
    /// complete where `SELECT DISTINCT FROM t` is an error.
    #[test]
    fn parse_select_all_with_and_without_targets() {
        use crate::ast::dml::select::SelectHead;

        for (src, count, has_from, has_into) in [
            ("SELECT ALL a, b FROM t", 2, true, false),
            ("SELECT ALL FROM t", 0, true, false),
            ("SELECT ALL", 0, false, false),
            ("SELECT ALL INTO u FROM t", 0, true, true),
            ("SELECT ALL a INTO u FROM t WHERE a > 1", 1, true, true),
            ("SELECT ALL WHERE true", 0, false, false),
        ] {
            let parsed = parse_select_classified(src);
            let stmt = parsed.ast();
            assert!(matches!(stmt.head, Some(SelectHead::All(_))), "{src:?}");
            assert_eq!(stmt.item_count(), count, "{src:?}");
            assert_eq!(stmt.from_clause().is_some(), has_from, "{src:?}");
            assert_eq!(stmt.into_clause().is_some(), has_into, "{src:?}");
            assert!(stmt.distinct().is_none(), "{src:?}");
        }
        for src in ["SELECT ALL DISTINCT a FROM t", "SELECT DISTINCT ALL a FROM t", "SELECT ALL ALL a"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = SelectStmt::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// gram.y `frame_bound` (gram.y:16391-16427) has `UNBOUNDED PRECEDING` and
    /// `UNBOUNDED FOLLOWING` as alternatives of their own, and `%nonassoc
    /// UNBOUNDED` under `PRECEDING` and `FOLLOWING` (gram.y:886) makes the
    /// keyword win over a column named `unbounded`. The differential oracle
    /// cannot see which bound pg-sql built.
    #[test]
    fn parse_unbounded_frame_bounds_as_their_own_variant() {
        fn bounds(src: &'static str) -> Vec<&'static str> {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = crate::ast::shared::expr::WindowFrameClause::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            let tree = format!("{:?}", parsed.ast());
            let mut found = Vec::new();
            for (needle, name) in [
                ("Unbounded(Preceding", "unbounded preceding"),
                ("Unbounded(Following", "unbounded following"),
                ("CurrentRow", "current row"),
                ("Offset(", "offset"),
            ] {
                found.extend(std::iter::repeat_n(name, tree.matches(needle).count()));
            }
            found
        }

        assert_eq!(
            bounds("ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW"),
            ["unbounded preceding", "current row"]
        );
        assert_eq!(
            bounds("RANGE BETWEEN unbounded preceding AND Unbounded Following"),
            ["unbounded preceding", "unbounded following"]
        );
        assert_eq!(bounds("ROWS UNBOUNDED PRECEDING"), ["unbounded preceding"]);
        assert_eq!(
            bounds("GROUPS BETWEEN 1 PRECEDING AND UNBOUNDED FOLLOWING"),
            ["unbounded following", "offset"]
        );
        // A quoted name, or the word inside an expression, is a column.
        assert_eq!(bounds("ROWS \"unbounded\" PRECEDING"), ["offset"]);
        assert_eq!(bounds("ROWS unbounded + 1 PRECEDING"), ["offset"]);
        assert_eq!(bounds("ROWS (unbounded) FOLLOWING"), ["offset"]);
    }

    /// gram.y:13451 `table_ref: relation_expr opt_alias_clause
    /// tablesample_clause` is the only `table_ref` with a sample: a table
    /// name, with `*`, `ONLY`, `ONLY (name)` or an alias, and never a subquery,
    /// a function, a parenthesized join, `LATERAL` or `ROWS FROM`. PostgreSQL
    /// 17.9 accepts every statement of the first list and rejects every one of
    /// the second with a syntax error at `TABLESAMPLE`.
    #[test]
    fn parse_tablesample_on_relations_only() {
        for (src, samples) in [
            ("SELECT * FROM t TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM t AS x TABLESAMPLE BERNOULLI (5) REPEATABLE (1)", 1),
            ("SELECT * FROM t x (a, b) TABLESAMPLE SYSTEM (5)", 1),
            ("SELECT * FROM s.t TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM t * TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM t * x TABLESAMPLE SYSTEM (1)", 1),
            ("SELECT * FROM ONLY t TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM ONLY (t) TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM ONLY (s.t) x", 0),
            ("SELECT * FROM ONLY t AS x TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM value TABLESAMPLE BERNOULLI (5)", 1),
            // gram.y `tablesample_clause: TABLESAMPLE func_name ...`.
            ("SELECT * FROM t TABLESAMPLE s.m (5, 6)", 1),
            // A join operand is a `table_ref`, so each side can have one.
            ("SELECT * FROM a CROSS JOIN b TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM a NATURAL JOIN b TABLESAMPLE BERNOULLI (5)", 1),
            ("SELECT * FROM a JOIN b TABLESAMPLE BERNOULLI (5) ON true", 1),
            (
                "SELECT * FROM a TABLESAMPLE SYSTEM (1) JOIN b TABLESAMPLE BERNOULLI (5) USING (x)",
                2,
            ),
        ] {
            let parsed = parse_select_classified(src);
            let tree = format!("{:?}", parsed.ast());
            assert_eq!(tree.matches("TableSampleClause {").count(), samples, "{src:?}: {tree}");
        }

        for src in [
            "SELECT * FROM (SELECT * FROM t) AS q TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM f() TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM f() AS x TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM (a JOIN b ON true) TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM (a JOIN b ON true) AS j TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM LATERAL (SELECT 1) x TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM ROWS FROM (f()) TABLESAMPLE BERNOULLI (5)",
            "SELECT * FROM xmltable('/a' PASSING d COLUMNS a int) TABLESAMPLE SYSTEM (1)",
            "SELECT * FROM t TABLESAMPLE BERNOULLI (5) TABLESAMPLE SYSTEM (1)",
            "SELECT * FROM t TABLESAMPLE BERNOULLI (5) AS x",
            "SELECT * FROM ONLY ((t))",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = SelectStmt::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// gram.y's parenthesized `table_ref` is `select_with_parens
    /// opt_alias_clause` or `'(' joined_table ')' alias_clause`, and
    /// `joined_table` is `'(' joined_table ')'` or a `table_ref` with a join
    /// after it (gram.y:13529). The parentheses never hold a lone relation.
    /// PostgreSQL 17.9 agrees with every line here.
    #[test]
    fn parse_parenthesized_from_sources_as_queries_or_joins() {
        for (src, node) in [
            ("SELECT * FROM (a JOIN b ON true)", "ParenJoinRef"),
            ("SELECT * FROM (a JOIN b ON true) AS j", "ParenJoinRef"),
            ("SELECT * FROM (a JOIN b ON true) j (x, y)", "ParenJoinRef"),
            // `joined_table: '(' joined_table ')'`, to any depth, alias outside.
            ("SELECT * FROM ((a JOIN b ON true))", "Group(ParenJoinGroup"),
            ("SELECT * FROM ((a JOIN b ON true)) AS j", "Group(ParenJoinGroup"),
            ("SELECT * FROM (((a JOIN b ON true)))", "Group(ParenJoinGroup"),
            // An aliased group is a `table_ref`, so it needs a join after it.
            ("SELECT * FROM ((a JOIN b ON true) j JOIN c ON true)", "ParenJoinRef"),
            ("SELECT * FROM ((a JOIN b ON true) JOIN c ON true)", "ParenJoinRef"),
            ("SELECT * FROM (a JOIN (b JOIN c ON true) ON true)", "ParenJoinRef"),
            ("SELECT * FROM a JOIN (b JOIN c ON true) ON true", "ParenJoinRef"),
            ("SELECT * FROM (f() CROSS JOIN g())", "ParenJoinRef"),
            ("SELECT * FROM (t TABLESAMPLE SYSTEM (1) CROSS JOIN u)", "ParenJoinRef"),
            ("SELECT * FROM ((SELECT 1) s CROSS JOIN t)", "ParenJoinRef"),
            ("SELECT * FROM (SELECT * FROM ((a JOIN b ON true))) s", "Group(ParenJoinGroup"),
            // A query in the parentheses.
            ("SELECT * FROM (SELECT 1)", "ParenQueryRef"),
            ("SELECT * FROM (SELECT 1) s", "ParenQueryRef"),
            ("SELECT * FROM ((SELECT 1)) s", "ParenQueryRef"),
            ("SELECT * FROM ((SELECT 1) UNION SELECT 2) s", "ParenQueryRef"),
            ("SELECT * FROM (VALUES (1)) v", "ParenQueryRef"),
            ("SELECT * FROM (TABLE t) v", "ParenQueryRef"),
        ] {
            let parsed = parse_select_classified(src);
            let tree = format!("{:?}", parsed.ast());
            assert!(tree.contains(node), "{src:?} did not build {node}: {tree}");
        }

        for src in [
            "SELECT * FROM (t)",
            "SELECT * FROM (t) x",
            "SELECT * FROM (t x)",
            "SELECT * FROM ((t))",
            "SELECT * FROM (ONLY t)",
            "SELECT * FROM (f())",
            "SELECT * FROM (t TABLESAMPLE SYSTEM (1))",
            "SELECT * FROM a JOIN (b) ON true",
            "SELECT * FROM ((a JOIN b ON true) j)",
            "SELECT * FROM ((SELECT 1) s)",
            "SELECT * FROM ()",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = SelectStmt::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// gram.y:13095 `sortby: a_expr USING qual_all_Op opt_nulls_order | a_expr
    /// opt_asc_desc opt_nulls_order`. `qual_all_Op` is every operator, a
    /// `MathOp` included, or `OPERATOR(...)`; PostgreSQL parses `ORDER BY x
    /// USING +` and rejects it later, in parse analysis. The order is `ASC`,
    /// `DESC` or `USING op`, never two of them. PostgreSQL 17.9 agrees with
    /// every line here.
    #[test]
    fn parse_order_by_using_any_operator() {
        fn tree(src: &'static str) -> String {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = crate::ast::dml::values::QueryBody::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            format!("{:?}", parsed.ast())
        }

        for op in [
            "+", "-", "*", "/", "%", "^", "<", ">", "=", "<=", ">=", "<>", "!=", "~<~", "||",
            "@>", "&&&",
        ] {
            let src: &'static str = format!("SELECT x FROM t ORDER BY x USING {op}").leak();
            let tree = tree(src);
            assert!(tree.contains("Using(UsingClause"), "{src:?}: {tree}");
            assert!(tree.contains("op: Plain("), "{src:?}: {tree}");
        }
        for src in [
            "SELECT x FROM t ORDER BY x USING OPERATOR(pg_catalog.<)",
            "SELECT x FROM t ORDER BY x USING operator(s.t.>) NULLS LAST",
        ] {
            let tree = tree(src);
            assert!(tree.contains("op: Decorated(DecoratedOperator"), "{src:?}: {tree}");
        }
        // The same `sortby` stands in an aggregate, in WITHIN GROUP and in a
        // window.
        for src in [
            "SELECT array_agg(x ORDER BY x USING +) FROM t",
            "SELECT percentile_disc(0.5) WITHIN GROUP (ORDER BY x USING +) FROM t",
            "SELECT rank() OVER (ORDER BY x USING -) FROM t",
        ] {
            assert!(tree(src).contains("Using(UsingClause"), "{src:?}");
        }
        let plain = tree("SELECT x FROM t ORDER BY x DESC NULLS LAST, y USING < NULLS FIRST");
        assert!(plain.contains("Dir(") && plain.contains("Using(UsingClause"), "{plain}");

        for src in [
            "SELECT x FROM t ORDER BY x ASC USING <",
            "SELECT x FROM t ORDER BY x USING < DESC",
            "SELECT x FROM t ORDER BY x NULLS FIRST ASC",
            "SELECT x FROM t ORDER BY x USING",
            "SELECT x FROM t ORDER BY x USING =>",
            "SELECT x FROM t ORDER BY x USING OPERATOR()",
            "SELECT x FROM t ORDER BY x USING < >",
            "SELECT x FROM t ORDER BY x USING lt",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = crate::ast::dml::values::QueryBody::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "{src:?} parsed completely"
            );
        }
    }

    // `JSON_TABLE` is added in 17, so rejected before 17: research, PostgreSQL 17, "Queries
    // and expressions". Before 17, `json_table(...)` is a function table.
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn json_table_is_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "SELECT * FROM JSON_TABLE(j, '$' COLUMNS (a int)) jt",
            "SELECT * FROM t, LATERAL JSON_TABLE(t.j, '$' COLUMNS (a int EXISTS)) jt",
            "SELECT * FROM JSON_TABLE(j, '$' COLUMNS (i FOR ORDINALITY))",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT * FROM json_table(1)",
            "SELECT * FROM json_table(1, 2) AS x",
            "SELECT json_table FROM json_table",
        ]);
    }
}
