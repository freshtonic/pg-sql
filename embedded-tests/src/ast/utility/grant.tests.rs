#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn grant_single_privilege_on_bare_table_to_role() {
        let stmt = parse_stmt::<GrantStmt>("GRANT SELECT ON tbl1 TO u1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.privileges, Privileges::List(_)));
        assert!(matches!(stmt.body, GrantBody::Privilege(_)));
        reparse_stable::<GrantStmt>("GRANT SELECT ON tbl1 TO u1");
    }

    #[test]
    fn grant_multiple_privileges_on_table_to_list() {
        reparse_stable::<GrantStmt>("GRANT SELECT, INSERT, UPDATE ON tbl1 TO u1, u2");
    }

    #[test]
    fn grant_all_on_table_to_role() {
        let stmt = parse_stmt::<GrantStmt>("GRANT ALL ON tbl1 TO u1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.privileges, Privileges::All(_)));
        reparse_stable::<GrantStmt>("GRANT ALL ON tbl1 TO u1");
    }

    #[test]
    fn grant_all_privileges_on_table_to_role() {
        let stmt = parse_stmt::<GrantStmt>("GRANT ALL PRIVILEGES ON tbl1 TO u1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.privileges, Privileges::AllPrivileges(_)));
        reparse_stable::<GrantStmt>("GRANT ALL PRIVILEGES ON tbl1 TO u1");
    }

    #[test]
    fn grant_all_with_column_list() {
        let stmt = parse_stmt::<GrantStmt>("GRANT ALL (a) ON tbl1 TO u1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.privileges, Privileges::AllCols(_)));
        reparse_stable::<GrantStmt>("GRANT ALL (a) ON tbl1 TO u1");
    }

    #[test]
    fn grant_all_privileges_with_column_list() {
        let stmt = parse_stmt::<GrantStmt>("GRANT ALL PRIVILEGES (a, b) ON tbl1 TO u1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.privileges, Privileges::AllPrivilegesCols(_)));
        reparse_stable::<GrantStmt>("GRANT ALL PRIVILEGES (a, b) ON tbl1 TO u1");
    }

    #[test]
    fn grant_column_level_select_to_role() {
        reparse_stable::<GrantStmt>("GRANT SELECT (a, b) ON tbl1 TO u1");
    }

    #[test]
    fn grant_explicit_table_keyword() {
        reparse_stable::<GrantStmt>("GRANT SELECT ON TABLE tbl1 TO u1");
    }

    #[test]
    fn grant_all_tables_in_schema() {
        reparse_stable::<GrantStmt>("GRANT ALL ON ALL TABLES IN SCHEMA testns TO u1");
    }

    #[test]
    fn grant_usage_on_schema() {
        reparse_stable::<GrantStmt>("GRANT USAGE ON SCHEMA s TO u1");
    }

    #[test]
    fn grant_with_grant_option() {
        let stmt = parse_stmt::<GrantStmt>("GRANT CREATE ON DATABASE d TO u1 WITH GRANT OPTION");
        let stmt = stmt.ast();
        if let GrantBody::Privilege(body) = &stmt.body {
            assert!(body.grant_option.is_some());
        } else {
            panic!("expected privilege body");
        }
        reparse_stable::<GrantStmt>("GRANT CREATE ON DATABASE d TO u1 WITH GRANT OPTION");
    }

    #[test]
    fn grant_granted_by() {
        reparse_stable::<GrantStmt>("GRANT INSERT ON atest2 TO u4 GRANTED BY CURRENT_USER");
    }

    #[test]
    fn grant_function_signature() {
        reparse_stable::<GrantStmt>("GRANT EXECUTE ON FUNCTION f(int) TO u2");
    }

    #[test]
    fn grant_large_object_to_public() {
        reparse_stable::<GrantStmt>("GRANT ALL ON LARGE OBJECT 1001 TO PUBLIC");
    }

    #[test]
    fn grant_group_grantee() {
        reparse_stable::<GrantStmt>("GRANT DELETE ON atest3 TO GROUP regress_priv_group2");
    }

    #[test]
    fn grant_role_membership_simple() {
        let stmt = parse_stmt::<GrantStmt>("GRANT role1 TO role2");
        let stmt = stmt.ast();
        assert!(matches!(stmt.body, GrantBody::Role(_)));
        reparse_stable::<GrantStmt>("GRANT role1 TO role2");
    }

    #[test]
    fn grant_role_membership_with_admin_option() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH ADMIN OPTION");
    }

    // Added in 16: research, PostgreSQL 16, "Changes to existing statements".
    #[cfg(feature = "since-pg16")]
    #[test]
    fn grant_role_membership_with_inherit_false() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH INHERIT FALSE");
    }

    // Added in 16: research, PostgreSQL 16, "Changes to existing statements".
    #[cfg(feature = "since-pg16")]
    #[test]
    fn grant_role_membership_with_set_true() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH SET TRUE");
    }

    #[test]
    fn grant_role_membership_with_admin_option_granted_by() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH ADMIN OPTION GRANTED BY role3");
    }

    #[test]
    fn revoke_simple_privilege() {
        let stmt = parse_stmt::<RevokeStmt>("REVOKE SELECT ON tbl1 FROM u1");
        let stmt = stmt.ast();
        assert!(stmt.option_for.is_none());
        assert!(matches!(stmt.body, RevokeBody::Privilege(_)));
        reparse_stable::<RevokeStmt>("REVOKE SELECT ON tbl1 FROM u1");
    }

    #[test]
    fn revoke_grant_option_for_cascade() {
        let stmt = parse_stmt::<RevokeStmt>("REVOKE GRANT OPTION FOR SELECT ON tbl1 FROM u1 CASCADE");
        let stmt = stmt.ast();
        assert!(matches!(
            stmt.option_for,
            Some(RevokeOptionFor::GrantOption(_))
        ));
        reparse_stable::<RevokeStmt>("REVOKE GRANT OPTION FOR SELECT ON tbl1 FROM u1 CASCADE");
    }

    #[test]
    fn revoke_role_membership_cascade() {
        let stmt = parse_stmt::<RevokeStmt>("REVOKE role1 FROM u1 CASCADE");
        let stmt = stmt.ast();
        assert!(matches!(stmt.body, RevokeBody::Role(_)));
        reparse_stable::<RevokeStmt>("REVOKE role1 FROM u1 CASCADE");
    }

    #[test]
    fn revoke_admin_option_for_role() {
        let stmt = parse_stmt::<RevokeStmt>("REVOKE ADMIN OPTION FOR role1 FROM u1");
        let stmt = stmt.ast();
        assert!(matches!(
            stmt.option_for,
            Some(RevokeOptionFor::AdminOption(_))
        ));
        reparse_stable::<RevokeStmt>("REVOKE ADMIN OPTION FOR role1 FROM u1");
    }

    #[test]
    fn alter_default_privileges_in_schema_grant_tables() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES IN SCHEMA s GRANT SELECT ON TABLES TO u1",
        );
    }

    #[test]
    fn alter_default_privileges_for_role_revoke_functions() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES FOR ROLE r REVOKE EXECUTE ON FUNCTIONS FROM public",
        );
    }

    #[test]
    fn alter_default_privileges_grant_schemas() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES GRANT USAGE ON SCHEMAS TO u2",
        );
    }

    #[test]
    fn alter_default_privileges_for_role_in_schema_grant() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES FOR ROLE r IN SCHEMA s GRANT ALL ON TABLES TO u2",
        );
    }

    /// Whether `src` parses as one complete statement.
    fn statement_parses(src: &str) -> bool {
        let lexed = crate::lex(src);
        if lexed.errors().count() > 0 {
            return false;
        }
        let mut input = lexed.input();
        crate::ast::Statement::parse(&mut input).is_ok() && input.is_eof()
    }

    // Added in 18: `defacl_privilege_target: LARGE_P OBJECTS_P`
    // (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
    // item 8).
    #[cfg(feature = "since-pg18")]
    #[test]
    fn alter_default_privileges_on_large_objects() {
        for src in [
            "ALTER DEFAULT PRIVILEGES GRANT SELECT ON LARGE OBJECTS TO u1",
            "ALTER DEFAULT PRIVILEGES FOR ROLE r REVOKE GRANT OPTION FOR UPDATE ON LARGE OBJECTS FROM u1",
        ] {
            reparse_stable::<AlterDefaultPrivilegesStmt>(src);
        }
        assert!(!statement_parses(
            "ALTER DEFAULT PRIVILEGES GRANT SELECT ON LARGE OBJECT TO u1"
        ));
    }

    #[cfg(not(feature = "since-pg18"))]
    #[test]
    fn alter_default_privileges_on_large_objects_is_rejected_before_18() {
        assert!(!statement_parses(
            "ALTER DEFAULT PRIVILEGES GRANT SELECT ON LARGE OBJECTS TO u1"
        ));
    }

    // Added in 16: research, PostgreSQL 16, "Changes to existing statements"
    // (REL_16_15 gram.y `grant_role_opt_list` and `REVOKE ColId OPTION FOR`).
    #[cfg(feature = "since-pg16")]
    #[test]
    fn role_grant_options_from_16() {
        crate::ast::test_support::assert_statements_parse(&[
            "GRANT r TO u WITH ADMIN TRUE",
            "GRANT r TO u WITH INHERIT FALSE, SET TRUE, ADMIN OPTION",
            "REVOKE INHERIT OPTION FOR r FROM u",
            "REVOKE SET OPTION FOR r FROM u",
        ]);
    }

    // Added in 16, so rejected before 16: REL_15_19 gram.y has only
    // `opt_grant_admin_option: WITH ADMIN OPTION` and `REVOKE ADMIN OPTION FOR`.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn role_grant_options_before_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "GRANT r TO u WITH ADMIN TRUE",
            "GRANT r TO u WITH INHERIT FALSE",
            "GRANT r TO u WITH SET TRUE",
            "GRANT r TO u WITH ADMIN OPTION, INHERIT TRUE",
            "REVOKE INHERIT OPTION FOR r FROM u",
            "REVOKE SET OPTION FOR r FROM u",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "GRANT r TO u WITH ADMIN OPTION",
            "GRANT r1, r2 TO u1, u2 WITH ADMIN OPTION GRANTED BY v",
            "GRANT r TO u GRANTED BY CURRENT_USER",
            "REVOKE ADMIN OPTION FOR r FROM u CASCADE",
        ]);
    }
}
