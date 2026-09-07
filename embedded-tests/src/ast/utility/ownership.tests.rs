#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_owned_by() {
        let lexed = crate::lex("DROP OWNED BY r1, r2 CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropOwnedStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.roles.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn reassign_owned_is_modelled() {
        let stmt = parse_stmt::<ReassignStmt>("REASSIGN OWNED BY a TO b");
        let stmt = stmt.ast();
        assert_eq!(stmt.roles.len(), 1);
        assert_eq!(
            roundtrip::<ReassignStmt>("REASSIGN OWNED BY a TO b"),
            "REASSIGN OWNED BY a TO b"
        );
    }

    #[test]
    fn reassign_owned_multiple_roles_roundtrips() {
        let stmt = parse_stmt::<ReassignStmt>("REASSIGN OWNED BY a, b TO c");
        let stmt = stmt.ast();
        assert_eq!(stmt.roles.len(), 2);
        assert_eq!(
            roundtrip::<ReassignStmt>("REASSIGN OWNED BY a, b TO c"),
            "REASSIGN OWNED BY a, b TO c"
        );
    }
}
