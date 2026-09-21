#[cfg(test)]
mod tests {
    use crate::ast::dml::values::{QueryBody, SelectClause, TableStmt};

    #[test]
    fn parse_table_stmt() {
        let lexed = crate::lex("TABLE int8_tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = TableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.table_name.object(), "int8_tbl");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_standalone() {
        let lexed = crate::lex("VALUES (1,2), (3,4), (7,8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body_parsed = SelectClause::parse(&mut input).unwrap();
        let body = body_parsed.ast();
        assert!(matches!(body, SelectClause::Values(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_union_all_select() {
        let lexed = crate::lex("VALUES (1,2) UNION ALL SELECT 3, 4");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body_parsed = SelectClause::parse(&mut input).unwrap();
        let body = body_parsed.ast();
        assert!(matches!(body, SelectClause::Union(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_union_all_table() {
        let lexed = crate::lex("VALUES (1,2) UNION ALL TABLE t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body_parsed = SelectClause::parse(&mut input).unwrap();
        let body = body_parsed.ast();
        assert!(matches!(body, SelectClause::Union(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_stmt_order_by() {
        let lexed = crate::lex(
            "TABLE information_schema.enabled_roles ORDER BY role_name COLLATE \"C\"",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_stmt_order_by_multiple_items() {
        let lexed = crate::lex("TABLE t ORDER BY a, b DESC");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = QueryBody::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.order_by.as_ref().unwrap().items.len(), 2);
        assert!(input.is_eof());
    }

    /// A `TABLE` query accepts at most one limiting clause (`LIMIT` or
    /// `FETCH FIRST`) and at most one `OFFSET` clause, like `SELECT`; the
    /// tail belongs to the enclosing `QueryBody`.
    #[test]
    fn reject_table_stmt_duplicate_limit_offset_clauses() {
        for src in [
            "TABLE t LIMIT 1 LIMIT 1",
            "TABLE t OFFSET 1 OFFSET 1",
            "TABLE t FETCH FIRST 1 ROWS ONLY FETCH FIRST 1 ROWS ONLY",
            "TABLE t LIMIT 1 FETCH FIRST 1 ROWS ONLY",
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

    /// Both PostgreSQL clause orders and each bare clause round-trip on a
    /// `TABLE` statement with the written order preserved.
    #[test]
    fn parse_table_stmt_limit_offset_orders_roundtrip() {
        use crate::ast::test_support::roundtrip;
        for src in [
            "TABLE t LIMIT 2 OFFSET 3",
            "TABLE t OFFSET 3 LIMIT 2",
            "TABLE t OFFSET 3 FETCH FIRST 2 ROWS ONLY",
            "TABLE t LIMIT 2",
            "TABLE t OFFSET 3",
            "TABLE t FETCH FIRST 2 ROWS ONLY",
        ] {
            assert_eq!(roundtrip::<QueryBody>(src), src);
        }
    }

    /// A parenthesized set-operation form accepts at most one limiting clause
    /// and at most one `OFFSET` clause after the closing parenthesis.
    #[test]
    fn reject_compound_paren_duplicate_limit_offset_clauses() {
        for src in [
            "(SELECT 1) LIMIT 1 LIMIT 1",
            "(SELECT 1) OFFSET 1 OFFSET 1",
            "(SELECT 1) FETCH FIRST 1 ROWS ONLY FETCH FIRST 1 ROWS ONLY",
            "(SELECT 1) LIMIT 1 FETCH FIRST 1 ROWS ONLY",
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

    /// Both PostgreSQL clause orders and each bare clause round-trip on the
    /// parenthesized set-operation form with the written order preserved.
    #[test]
    fn parse_compound_paren_limit_offset_orders_roundtrip() {
        use crate::ast::test_support::roundtrip;
        for src in [
            "(SELECT 1) LIMIT 2 OFFSET 3",
            "(SELECT 1) OFFSET 3 LIMIT 2",
            "(SELECT 1) OFFSET 3 FETCH FIRST 2 ROWS ONLY",
            "(SELECT 1) LIMIT 2",
            "(SELECT 1) OFFSET 3",
            "(SELECT 1) FETCH FIRST 2 ROWS ONLY",
        ] {
            assert_eq!(roundtrip::<QueryBody>(src), src);
        }
    }
    /// The shape of a set-operation tree, with `s` for a `SELECT`, `v` for
    /// `VALUES`, `t` for `TABLE` and `(...)` for a parenthesized query.
    fn shape(clause: &SelectClause<'_>) -> String {
        use crate::ast::dml::values::SetQuantifier;

        fn quantifier(quantifier: &Option<SetQuantifier>) -> &'static str {
            match quantifier {
                None => "",
                Some(SetQuantifier::All) => "All",
                Some(SetQuantifier::Distinct) => "Distinct",
            }
        }
        match clause {
            SelectClause::Union(left, op, right) => format!(
                "Union{}({},{})",
                quantifier(&op.quantifier),
                shape(left),
                shape(right)
            ),
            SelectClause::Except(left, op, right) => format!(
                "Except{}({},{})",
                quantifier(&op.quantifier),
                shape(left),
                shape(right)
            ),
            SelectClause::Intersect(left, op, right) => format!(
                "Intersect{}({},{})",
                quantifier(&op.quantifier),
                shape(left),
                shape(right)
            ),
            SelectClause::Select(_) => "s".to_owned(),
            SelectClause::Values(_) => "v".to_owned(),
            SelectClause::Table(_) => "t".to_owned(),
            SelectClause::Parens(parens) => format!("({})", shape(&parens.inner.body.clause)),
        }
    }

    /// gram.y:830-831 `%left UNION EXCEPT` below `%left INTERSECT`, applied to
    /// gram.y:12844-12854 `select_clause UNION set_quantifier select_clause`
    /// and its twins: `INTERSECT` binds tighter and every operator associates
    /// to the left. This is PostgreSQL's `SetOperationStmt` tree. The
    /// differential oracle cannot see it, because both nestings render the
    /// same text; `a INTERSECT b UNION c` and `a INTERSECT (b UNION c)` are
    /// different queries all the same.
    #[test]
    fn set_operations_nest_as_gram_y_declares() {
        let cases = [
            ("SELECT 1 UNION SELECT 2", "Union(s,s)"),
            ("SELECT 1 UNION SELECT 2 UNION SELECT 3", "Union(Union(s,s),s)"),
            ("SELECT 1 EXCEPT SELECT 2 EXCEPT SELECT 3", "Except(Except(s,s),s)"),
            ("SELECT 1 UNION SELECT 2 EXCEPT SELECT 3", "Except(Union(s,s),s)"),
            ("SELECT 1 EXCEPT SELECT 2 UNION SELECT 3", "Union(Except(s,s),s)"),
            (
                "SELECT 1 INTERSECT SELECT 2 INTERSECT SELECT 3",
                "Intersect(Intersect(s,s),s)",
            ),
            ("SELECT 1 INTERSECT SELECT 2 UNION SELECT 3", "Union(Intersect(s,s),s)"),
            ("SELECT 1 UNION SELECT 2 INTERSECT SELECT 3", "Union(s,Intersect(s,s))"),
            ("SELECT 1 EXCEPT SELECT 2 INTERSECT SELECT 3", "Except(s,Intersect(s,s))"),
            (
                "SELECT 1 UNION SELECT 2 INTERSECT SELECT 3 UNION SELECT 4",
                "Union(Union(s,Intersect(s,s)),s)",
            ),
            (
                "SELECT 1 UNION ALL SELECT 2 UNION DISTINCT SELECT 3",
                "UnionDistinct(UnionAll(s,s),s)",
            ),
            (
                "SELECT 1 INTERSECT ALL SELECT 2 EXCEPT DISTINCT SELECT 3",
                "ExceptDistinct(IntersectAll(s,s),s)",
            ),
            // Only parentheses override the declared nesting.
            ("SELECT 1 INTERSECT (SELECT 2 UNION SELECT 3)", "Intersect(s,(Union(s,s)))"),
            ("(SELECT 1 UNION SELECT 2) INTERSECT SELECT 3", "Intersect((Union(s,s)),s)"),
            ("SELECT 1 UNION (SELECT 2 UNION SELECT 3)", "Union(s,(Union(s,s)))"),
            ("((SELECT 1)) UNION SELECT 2", "Union(((s)),s)"),
            // Every `simple_select` is an operand.
            ("VALUES (1) UNION TABLE t INTERSECT VALUES (2)", "Union(v,Intersect(t,v))"),
            ("TABLE t", "t"),
            ("(SELECT 1)", "(s)"),
        ];
        for (src, expected) in cases {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = SelectClause::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            assert_eq!(shape(parsed.ast()), expected, "for {src:?}");
        }
    }

    /// gram.y `select_no_parens`: the sort, limit and locking tails follow
    /// the whole chain and belong to no member, and an operand is a
    /// `select_clause`, which has none of them.
    #[test]
    fn set_operation_tails_belong_to_the_whole_chain() {
        let lexed = crate::lex("SELECT 1 INTERSECT SELECT 2 UNION SELECT 3 ORDER BY 1 LIMIT 1");
        let mut input = lexed.input();
        let parsed = QueryBody::parse(&mut input).unwrap();
        assert!(input.is_eof());
        let query = parsed.ast();
        assert_eq!(shape(&query.clause), "Union(Intersect(s,s),s)");
        assert!(query.order_by.is_some());
        assert!(query.limit_offset.is_some());

        for src in [
            "SELECT 1 ORDER BY 1 UNION SELECT 2",
            "SELECT 1 LIMIT 1 UNION SELECT 2",
            "SELECT 1 UNION",
            "SELECT 1 UNION ALL DISTINCT SELECT 2",
            "UNION SELECT 1",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = QueryBody::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

}
