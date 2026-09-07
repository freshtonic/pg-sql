#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn close_cursor_is_modelled() {
        let stmt = parse_stmt::<CloseStmt>("CLOSE foo1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.target, CloseTarget::Cursor(_)));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE foo1"), "CLOSE foo1");
    }

    #[test]
    fn close_all_is_modelled() {
        let stmt = parse_stmt::<CloseStmt>("CLOSE ALL");
        let stmt = stmt.ast();
        assert!(matches!(stmt.target, CloseTarget::All));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE ALL"), "CLOSE ALL");
    }
}
