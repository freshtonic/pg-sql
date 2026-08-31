#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn close_cursor_is_modelled() {
        let stmt: CloseStmt = parse_stmt("CLOSE foo1");
        assert!(matches!(stmt.target, CloseTarget::Cursor(_)));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE foo1"), "CLOSE foo1");
    }

    #[test]
    fn close_all_is_modelled() {
        let stmt: CloseStmt = parse_stmt("CLOSE ALL");
        assert!(matches!(stmt.target, CloseTarget::All));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE ALL"), "CLOSE ALL");
    }
}
