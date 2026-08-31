#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_group() {
        let lexed = crate::lex("CREATE GROUP g1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateGroupStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_group_with_users() {
        let lexed = crate::lex("CREATE GROUP g1 WITH USER u1, u2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateGroupStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_group_add_user() {
        let lexed = crate::lex("ALTER GROUP g1 ADD USER u1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterGroupStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_group_drop_user() {
        let lexed = crate::lex("ALTER GROUP g1 DROP USER u1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterGroupStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_plain() {
        let lexed = crate::lex("CREATE ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "alice");
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_with_attributes() {
        let lexed = crate::lex(
            "CREATE ROLE alice WITH SUPERUSER CREATEDB CREATEROLE NOINHERIT \
             REPLICATION BYPASSRLS LOGIN",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 7);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_negated_attributes() {
        let lexed = crate::lex(
            "CREATE ROLE alice NOSUPERUSER NOCREATEDB NOCREATEROLE NOLOGIN \
             NOREPLICATION NOBYPASSRLS",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 6);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_password() {
        let lexed = crate::lex("CREATE ROLE alice PASSWORD 'secret'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::Password(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_encrypted_password_null() {
        let lexed = crate::lex("CREATE ROLE alice ENCRYPTED PASSWORD NULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_connection_limit() {
        let lexed = crate::lex("CREATE ROLE alice CONNECTION LIMIT 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::ConnectionLimit(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_valid_until() {
        let lexed = crate::lex("CREATE ROLE alice VALID UNTIL '2030-01-01'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::ValidUntil(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_in_role() {
        let lexed = crate::lex("CREATE ROLE bob IN ROLE alice, charlie");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::InRole(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_admin() {
        let lexed = crate::lex("CREATE ROLE bob ADMIN alice, charlie");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::Admin(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_role_sysid() {
        let lexed = crate::lex("CREATE ROLE bob SYSID 12345");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::SysId(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_user_with_login() {
        let lexed = crate::lex("CREATE USER alice WITH NOLOGIN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateUserStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "alice");
        assert_eq!(stmt.options.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_group_role_members() {
        let lexed = crate::lex("CREATE GROUP g1 ROLE alice, bob");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateGroupStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::Role(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_group_user_members() {
        let lexed = crate::lex("CREATE GROUP g1 WITH USER u1, u2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateGroupStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::User(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_group() {
        let lexed = crate::lex("DROP GROUP g1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropGroupStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.roles.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_role_if_exists_multi() {
        let lexed = crate::lex("DROP ROLE IF EXISTS a, b, c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.roles.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_def_arg_custom_op() {
        // The smallest reproducer: a `def_arg` value that is a 3-char custom
        // operator. Used as the RHS of `COMMUTATOR =` etc.
        let lexed = crate::lex("===");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _arg = DefArg::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_def_arg_at_eq() {
        let lexed = crate::lex("@=");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _arg = DefArg::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_def_arg_bang_eq_eq() {
        let lexed = crate::lex("!==");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _arg = DefArg::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_def_arg_signed_numeric_still_parses_as_numeric() {
        // Regression: `Numeric` and `QualOp` share `+`/`-` as a leading
        // token. `Numeric` is declared first so a signed integer must still
        // win — `+1` and `-2` are common `def_arg` values (`default = -1`,
        // `internallength = +24`) and must not silently demote to QualOp.
        let lexed = crate::lex("+1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let arg = DefArg::parse(&mut input).unwrap().into_ast();
        assert!(
            matches!(arg, DefArg::Numeric(_)),
            "expected Numeric for `+1`, got {arg:?}"
        );
        assert!(input.is_eof());

        let lexed = crate::lex("-2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let arg = DefArg::parse(&mut input).unwrap().into_ast();
        assert!(
            matches!(arg, DefArg::Numeric(_)),
            "expected Numeric for `-2`, got {arg:?}"
        );
        assert!(input.is_eof());
    }
}
