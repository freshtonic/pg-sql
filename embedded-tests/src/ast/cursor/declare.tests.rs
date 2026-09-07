#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn declare_plain_cursor_is_modelled() {
        let stmt = parse_stmt::<DeclareStmt>("DECLARE c CURSOR FOR SELECT 1");
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "c");
        assert!(stmt.options.is_empty());
        assert!(stmt.hold.is_none());
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE c CURSOR FOR SELECT 1"),
            "DECLARE c CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_scroll_cursor_keeps_option() {
        let stmt = parse_stmt::<DeclareStmt>("DECLARE foo1 SCROLL CURSOR FOR SELECT a FROM t");
        let stmt = stmt.ast();
        assert_eq!(stmt.options.len(), 1);
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo1 SCROLL CURSOR FOR SELECT a FROM t"),
            "DECLARE foo1 SCROLL CURSOR FOR SELECT a FROM t"
        );
    }

    #[test]
    fn declare_no_scroll_cursor_roundtrips() {
        let stmt = parse_stmt::<DeclareStmt>("DECLARE foo24 NO SCROLL CURSOR FOR SELECT 1");
        let stmt = stmt.ast();
        assert_eq!(stmt.options.len(), 1);
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo24 NO SCROLL CURSOR FOR SELECT 1"),
            "DECLARE foo24 NO SCROLL CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_binary_cursor_roundtrips() {
        let _stmt = parse_stmt::<DeclareStmt>("DECLARE bc BINARY CURSOR FOR SELECT 1");
        let _stmt = _stmt.ast();
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE bc BINARY CURSOR FOR SELECT 1"),
            "DECLARE bc BINARY CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_insensitive_cursor_roundtrips() {
        let _stmt = parse_stmt::<DeclareStmt>("DECLARE c1 INSENSITIVE CURSOR FOR SELECT 1");
        let _stmt = _stmt.ast();
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE c1 INSENSITIVE CURSOR FOR SELECT 1"),
            "DECLARE c1 INSENSITIVE CURSOR FOR SELECT 1"
        );
    }

    #[test]
    fn declare_cursor_with_hold_keeps_hold() {
        let stmt = parse_stmt::<DeclareStmt>("DECLARE foo25 SCROLL CURSOR WITH HOLD FOR SELECT 1");
        let stmt = stmt.ast();
        assert!(stmt.hold.is_some());
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo25 SCROLL CURSOR WITH HOLD FOR SELECT 1"),
            "DECLARE foo25 SCROLL CURSOR WITH HOLD FOR SELECT 1"
        );
    }

    #[test]
    fn declare_no_scroll_cursor_with_hold_roundtrips() {
        let _stmt = parse_stmt::<DeclareStmt>("DECLARE foo25ns NO SCROLL CURSOR WITH HOLD FOR SELECT 1");
        let _stmt = _stmt.ast();
        assert_eq!(
            roundtrip::<DeclareStmt>("DECLARE foo25ns NO SCROLL CURSOR WITH HOLD FOR SELECT 1"),
            "DECLARE foo25ns NO SCROLL CURSOR WITH HOLD FOR SELECT 1"
        );
    }
}
