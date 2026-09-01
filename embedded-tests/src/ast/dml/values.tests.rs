#[cfg(test)]
mod tests {
    use crate::ast::dml::values::{CompoundBody, CompoundParen, TableStmt};

    #[test]
    fn parse_table_stmt() {
        let lexed = crate::lex("TABLE int8_tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = TableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "int8_tbl");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_standalone() {
        let lexed = crate::lex("VALUES (1,2), (3,4), (7,8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body = CompoundBody::parse(&mut input).unwrap().into_ast();
        assert!(body.set_op.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_union_all_select() {
        let lexed = crate::lex("VALUES (1,2) UNION ALL SELECT 3, 4");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body = CompoundBody::parse(&mut input).unwrap().into_ast();
        assert!(body.set_op.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_union_all_table() {
        let lexed = crate::lex("VALUES (1,2) UNION ALL TABLE t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body = CompoundBody::parse(&mut input).unwrap().into_ast();
        assert!(body.set_op.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_stmt_order_by() {
        let lexed = crate::lex(
            "TABLE information_schema.enabled_roles ORDER BY role_name COLLATE \"C\"",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = TableStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_stmt_order_by_multiple_items() {
        let lexed = crate::lex("TABLE t ORDER BY a, b DESC");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = TableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.order_by.unwrap().items.len(), 2);
        assert!(input.is_eof());
    }

    /// A `TABLE` statement accepts at most one limiting clause (`LIMIT` or
    /// `FETCH FIRST`) and at most one `OFFSET` clause, like `SELECT`.
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
            let parsed = TableStmt::parse(&mut input);
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
            assert_eq!(roundtrip::<TableStmt>(src), src);
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
            let parsed = CompoundParen::parse(&mut input);
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
            assert_eq!(roundtrip::<CompoundParen>(src), src);
        }
    }
}
