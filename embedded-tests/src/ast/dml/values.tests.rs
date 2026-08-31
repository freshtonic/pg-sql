#[cfg(test)]
mod tests {
    use crate::ast::dml::values::{CompoundBody, TableStmt};

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
}
