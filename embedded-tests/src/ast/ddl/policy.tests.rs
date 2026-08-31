#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_policy_on_table() {
        let lexed = crate::lex("DROP POLICY p1 ON document");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropPolicyStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "p1");
        assert_eq!(stmt.table.object(), "document");
        assert!(input.is_eof());
    }

    #[test]
    fn create_policy_minimal_roundtrips() {
        let stmt: CreatePolicyStmt = parse_stmt("CREATE POLICY p1 ON document");
        assert_eq!(stmt.name.text(), "p1");
        assert_eq!(stmt.table.object(), "document");
        assert!(stmt.permissive.is_none());
        assert!(stmt.for_cmd.is_none());
        assert!(stmt.to_roles.is_none());
        assert!(stmt.using.is_none());
        assert!(stmt.with_check.is_none());
        reparse_stable::<CreatePolicyStmt>("CREATE POLICY p1 ON document");
    }

    #[test]
    fn create_policy_as_permissive_using_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p1 ON document AS PERMISSIVE USING (true)",
        );
    }

    #[test]
    fn create_policy_as_restrictive_to_role_using_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p1r ON document AS RESTRICTIVE TO regress_rls_dave USING (cid <> 44)",
        );
    }

    #[test]
    fn create_policy_for_insert_with_check_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p ON document FOR INSERT WITH CHECK (dauthor = current_user)",
        );
    }

    #[test]
    fn create_policy_for_all_to_public_using_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p ON t FOR ALL TO PUBLIC USING (a % 2 = 0)",
        );
    }

    #[test]
    fn create_policy_for_update_using_with_check_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p3 ON document FOR UPDATE USING (true) WITH CHECK (true)",
        );
    }
}
