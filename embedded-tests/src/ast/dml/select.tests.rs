#[cfg(test)]
mod tests {
    use crate::ast::dml::select::{GroupByItem, SelectDistinct, SelectItem, SelectStmt};
    use crate::ast::shared::expr::Expr;

    /// Parse `src` as a complete `SELECT` through the logos lex pass.
    fn parse_select_classified(src: &'static str) -> SelectStmt<'static> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
            .into_ast();
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        stmt
    }

    /// A JOIN's `ON`/`USING` may be deferred: `a JOIN b JOIN c ON x ON y`
    /// parses as `a JOIN (b JOIN c ON x) ON y`. The left-deep form must be
    /// unchanged, and the recursion must also work inside parentheses.
    #[test]
    fn parse_stacked_on_joins() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        let right_recursive_src = "SELECT * FROM a JOIN b JOIN c ON x ON y";
        let right_recursive = parse_select_classified(right_recursive_src);
        let [table] = right_recursive
            .from_clause()
            .expect("FROM clause")
            .tables
            .as_slice()
        else {
            panic!("expected one FROM item");
        };
        let [outer_join] = table.joins.as_slice() else {
            panic!("right-recursive form must have one outer JOIN");
        };
        let [inner_join] = outer_join.table.joins.as_slice() else {
            panic!("right operand must own its nested JOIN");
        };
        assert_eq!(
            format_tokens_sql(
                inner_join.condition.as_ref().expect("inner ON condition"),
                PrettyConfig::default(),
            )
            .trim(),
            "ON x",
        );
        assert_eq!(
            format_tokens_sql(
                outer_join.condition.as_ref().expect("outer ON condition"),
                PrettyConfig::default(),
            )
            .trim(),
            "ON y",
        );
        assert_eq!(
            format_tokens_sql(&right_recursive, PrettyConfig::default()).trim(),
            right_recursive_src,
        );

        let left_deep_src = "SELECT * FROM a JOIN b ON x JOIN c ON y";
        let left_deep = parse_select_classified(left_deep_src);
        let [table] = left_deep
            .from_clause()
            .expect("FROM clause")
            .tables
            .as_slice()
        else {
            panic!("expected one FROM item");
        };
        let [first_join, second_join] = table.joins.as_slice() else {
            panic!("left-deep form must keep two top-level JOINs");
        };
        assert!(first_join.table.joins.is_empty());
        assert!(second_join.table.joins.is_empty());
        assert_eq!(
            format_tokens_sql(
                first_join.condition.as_ref().expect("first ON condition"),
                PrettyConfig::default(),
            )
            .trim(),
            "ON x",
        );
        assert_eq!(
            format_tokens_sql(
                second_join.condition.as_ref().expect("second ON condition"),
                PrettyConfig::default(),
            )
            .trim(),
            "ON y",
        );
        assert_eq!(
            format_tokens_sql(&left_deep, PrettyConfig::default()).trim(),
            left_deep_src,
        );

        for src in [
            "SELECT * FROM t1 LEFT JOIN \
             (t2 LEFT JOIN t3 FULL JOIN t4 ON p ON q) \
             LEFT JOIN t5 ON r ON s", // parenthesised
            "SELECT * FROM a CROSS JOIN b JOIN c ON x", // unqualified mixed in
        ] {
            parse_select_classified(src);
        }
    }

    #[test]
    fn parse_rows_from() {
        for src in [
            "SELECT * FROM ROWS FROM(f(1), g(2)) WITH ORDINALITY AS z(a, b, c, ord)",
            "SELECT * FROM ROWS FROM(getf(1) AS (id int, nm text)) AS z(a, b)",
        ] {
            parse_select_classified(src);
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
            parse_select_classified(src);
        }
    }

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
            parse_select_classified(src);
        }
    }

    #[test]
    fn parse_simple_select() {
        let lexed = crate::lex("SELECT 1 AS one");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
            let stmt = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();

            match (stmt.distinct(), expect_on) {
                (Some(SelectDistinct::On(_)), true) | (Some(SelectDistinct::All), false) => {}
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
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();

        assert!(stmt.distinct().is_none());
        assert_eq!(stmt.item_count(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_empty_items() {
        let lexed = crate::lex("SELECT FROM emp");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.item_count(), 0);
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_paren_join_cross() {
        let lexed = crate::lex("SELECT * FROM (a CROSS JOIN b) AS tx");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_paren_join_using() {
        let lexed = crate::lex("SELECT * FROM (a JOIN b USING (i)) AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_paren_join_with_col_aliases() {
        let lexed =
            crate::lex("SELECT * FROM (a t1 (x, y) CROSS JOIN b t2 (p, q)) AS tx (a, b, c, d)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
            let _stmt = SelectStmt::parse(&mut input)
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
    fn parse_select_from_where() {
        let lexed = crate::lex("SELECT f1 FROM BOOLTBL1 WHERE f1 = true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.item_count(), 1);
        assert!(stmt.from_clause().is_some());
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn parse_select_star() {
        let lexed = crate::lex("SELECT * FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        let first = stmt.items().next().unwrap();
        let SelectItem::Expr(first) = first else {
            panic!("expected an expression SELECT item");
        };
        let alias = first.alias.as_ref().unwrap();
        assert_eq!(alias.name(), "true");

        for (src, expected_alias) in [("SELECT 1 TABLE", "TABLE"), ("SELECT 1 TRUE", "TRUE")] {
            let stmt = parse_select_classified(src);
            let SelectItem::Expr(item) = stmt.items().next().expect("SELECT item") else {
                panic!("expected expression target for {src:?}");
            };
            assert_eq!(
                item.alias.as_ref().map(|alias| alias.name()),
                Some(expected_alias),
                "bare keyword alias for {src:?}",
            );
        }

        let stmt = parse_select_classified("SELECT value IS NULL");
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
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
            let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_asc() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 ASC");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_desc() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 DESC");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_nulls_first() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 NULLS FIRST");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_order_by_desc_nulls_last() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY f1 DESC NULLS LAST");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    // --- OFFSET/LIMIT ---

    #[test]
    fn parse_select_offset() {
        let lexed = crate::lex("SELECT 1 OFFSET 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.limit_offset.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_limit() {
        let lexed = crate::lex("SELECT 1 LIMIT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.limit_offset.is_some());
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
            let parsed = SelectStmt::parse(&mut input);
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
            assert_eq!(roundtrip::<SelectStmt>(src), src);
        }
    }

    // --- FOR UPDATE ---

    #[test]
    fn parse_select_from_only() {
        let lexed = crate::lex("SELECT f1 FROM ONLY t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_only_with_alias() {
        let lexed = crate::lex("SELECT f1 FROM ONLY t AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_qualified_name() {
        let lexed = crate::lex("SELECT * FROM myschema.mytable");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_window_clause_standalone() {
        use super::WindowClause;
        let lexed = crate::lex("WINDOW w AS (PARTITION BY y ORDER BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let wc = WindowClause::parse(&mut input).unwrap().into_ast();
        assert_eq!(wc.defs.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_window_clause() {
        let lexed =
            crate::lex("SELECT sum(x) OVER w FROM t WINDOW w AS (PARTITION BY y ORDER BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_over_named() {
        let lexed = crate::lex("SELECT sum(x) OVER w FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_alias_with_column_list() {
        let lexed = crate::lex("SELECT * FROM tbl AS t (a, b, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_from_qualified_name_with_alias() {
        let lexed = crate::lex("SELECT * FROM s.t AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_join_using_alias() {
        let lexed = crate::lex("SELECT * FROM a JOIN b USING (i) AS x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_join_using_alias_where() {
        let lexed = crate::lex("SELECT * FROM a JOIN b USING (i) AS x WHERE x.i = 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_func_with_ordinality() {
        let lexed = crate::lex("SELECT * FROM rngfunct(1) WITH ORDINALITY AS z(a, b, ord)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_func_column_def_list() {
        let lexed = crate::lex("SELECT * FROM test_ret_set_rec_dyn(1500) AS (a int, b int, c int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Corpus: `select * from json_populate_recordset(row(0::int),'[...]') q (a text, b text)`
    /// — function-call FROM source with a bare-alias-name column-def list (no AS).
    ///
    /// DEFERRED: the `FuncTableAlias` enum disambiguates via first-token kind
    /// only, and both `ColumnDefList` and `Plain(TableAlias)` start with Ident
    /// (since `ColumnDefList::name` is `Option<AliasName>`). The codegen
    /// dispatcher commits on `Plain` for any non-`AS` first token, so the bare-
    /// name + column-def-list form (`q (a text, b text)`) falls into
    /// `Plain(TableAlias::Bare)` and the columns survive as leftover. Fixing
    /// this needs either a recursa-level longest-match-wins fallback for
    /// overlapping enum first-sets, or restructuring `FuncTableAlias` so each
    /// alternative has a distinct prefix (e.g. require the LParen first).
    /// Affects 8 `json_populate_recordset(...) q (a text, b text)` PG-accepts
    /// fallbacks across json.sql / jsonb.sql.
    #[test]
    #[ignore]
    fn parse_select_func_table_bare_alias_col_def() {
        let lexed = crate::lex(
            "select * from json_populate_recordset(row(0::int),'[{\"a\":\"1\"}]') q (a text, b text)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_select_func_with_ordinality_unnest() {
        let lexed = crate::lex("SELECT * FROM unnest(array['a','b']) WITH ORDINALITY AS z(a, ord)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_natural_join() {
        let lexed = crate::lex("SELECT * FROM a NATURAL JOIN b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
        assert!(join.natural, "NATURAL must belong to the JOIN suffix");
    }

    #[test]
    fn parse_select_natural_left_join() {
        let lexed = crate::lex("SELECT * FROM a NATURAL LEFT JOIN b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_left_outer_join_using() {
        let lexed = crate::lex("SELECT * FROM a LEFT OUTER JOIN b USING (i)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_full_outer_join_using() {
        let lexed = crate::lex("SELECT * FROM a FULL OUTER JOIN b USING (i)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_right_outer_join_on() {
        let lexed = crate::lex("SELECT * FROM a RIGHT OUTER JOIN b ON a.i = b.i");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_paren_join_simple() {
        let lexed = crate::lex("SELECT * FROM a LEFT JOIN (b JOIN c ON b.x = c.x) ON a.y = b.y");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_paren_join_with_subquery_inside() {
        let lexed = crate::lex(
            "SELECT * FROM a LEFT JOIN (b JOIN (SELECT 1 AS x) s ON b.x = s.x) ON a.y = b.y",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        SelectStmt::parse(&mut input).unwrap().into_ast();
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
                .into_ast();
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
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        let group_by = stmt.group_by.expect("GROUP BY clause");
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
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        let group_by = stmt.group_by.expect("GROUP BY clause");
        assert!(matches!(group_by.items.as_slice(), [GroupByItem::Expr(_)]));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_cube() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY CUBE (a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        let group_by = stmt.group_by.expect("GROUP BY clause");
        assert!(matches!(group_by.items.as_slice(), [GroupByItem::Expr(_)]));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_grouping_sets_nested() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY GROUPING SETS (ROLLUP(a), CUBE(b))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        let group_by = stmt.group_by.expect("GROUP BY clause");
        let [GroupByItem::GroupingSets(grouping_sets)] = group_by.items.as_slice() else {
            panic!("expected one GROUPING SETS item");
        };
        assert!(matches!(
            grouping_sets.groups.as_slice(),
            [rollup, cube]
                if matches!(rollup.as_ref(), GroupByItem::Expr(_))
                    && matches!(cube.as_ref(), GroupByItem::Expr(_))
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_group_by_mixed_primitives() {
        let lexed = crate::lex("SELECT sum(c) FROM t GROUP BY a, ROLLUP(b), CUBE(c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        let group_by = stmt.group_by.expect("GROUP BY clause");
        assert!(matches!(
            group_by.items.as_slice(),
            [
                GroupByItem::Expr(_),
                GroupByItem::Expr(_),
                GroupByItem::Expr(_)
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
            SelectStmt::parse(&mut input).unwrap().into_ast();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_select_for_update() {
        let lexed = crate::lex("SELECT f1 FROM t FOR UPDATE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.for_update.is_some());
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
            let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
            assert!(stmt.for_update.is_some(), "no locking clause: {src:?}");
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
            let _stmt = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("{src}: {e}"))
                .into_ast();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_select_unicode_alias() {
        let lexed = crate::lex(r#"SELECT U&'d\0061t\+000061' AS U&"d\0061t\+000061""#);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_unicode_alias_uescape() {
        let lexed = crate::lex(
            r#"SELECT U&'d!0061t\+000061' UESCAPE '!' AS U&"d*0061t\+000061" UESCAPE '*'"#,
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    // --- LIMIT / OFFSET / FETCH FIRST ---

    #[test]
    fn parse_offset_before_limit() {
        // Standard SQL order: OFFSET before LIMIT.
        let lexed = crate::lex("SELECT * FROM t ORDER BY x OFFSET 10 LIMIT 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_limit_before_offset() {
        // Postgres order: LIMIT before OFFSET.
        let lexed = crate::lex("SELECT * FROM t ORDER BY x LIMIT 5 OFFSET 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_first_rows_only() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST 5 ROWS ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_first_row_with_ties() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST 2 ROW WITH TIES");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_first_rows_no_count() {
        // FETCH FIRST ROWS WITH TIES — count is omitted (defaults to 1).
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST ROWS WITH TIES");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_next_row_only() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH NEXT 1 ROW ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_offset_then_fetch() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x OFFSET 10 FETCH FIRST 5 ROWS ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_fetch_then_offset() {
        let lexed = crate::lex("SELECT * FROM t ORDER BY x FETCH FIRST 5 ROWS WITH TIES OFFSET 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
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
            let _stmt = SelectStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }
}
