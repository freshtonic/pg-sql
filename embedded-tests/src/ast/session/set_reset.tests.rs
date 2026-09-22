#[cfg(test)]
mod tests {
    use crate::ast::session::set_reset::{ResetStmt, ShowStmt, VariableSetRest, VariableSetStmt};

    #[test]
    fn parse_set_to() {
        let lexed = crate::lex("SET enable_seqscan TO off");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = VariableSetStmt::parse(&mut input).unwrap();
        let VariableSetStmt::Unscoped(body) = parsed.ast()
        else {
            panic!("unscoped SET expected");
        };
        let VariableSetRest::Generic(rest) = &body.rest else {
            panic!("generic SET expected");
        };
        assert_eq!(rest.param.object(), "enable_seqscan");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_eq() {
        let lexed = crate::lex("SET enable_sort = false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = VariableSetStmt::parse(&mut input).unwrap();
        let VariableSetStmt::Unscoped(body) = parsed.ast()
        else {
            panic!("unscoped SET expected");
        };
        let VariableSetRest::Generic(rest) = &body.rest else {
            panic!("generic SET expected");
        };
        assert_eq!(rest.param.object(), "enable_sort");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_integer_value() {
        let lexed = crate::lex("SET work_mem = 4096");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = VariableSetStmt::parse(&mut input).unwrap();
        let VariableSetStmt::Unscoped(body) = parsed.ast()
        else {
            panic!("unscoped SET expected");
        };
        let VariableSetRest::Generic(rest) = &body.rest else {
            panic!("generic SET expected");
        };
        assert_eq!(rest.values.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_numeric_value() {
        let lexed = crate::lex("SET seq_page_cost = 1.5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = VariableSetStmt::parse(&mut input).unwrap();
        let VariableSetStmt::Unscoped(body) = parsed.ast()
        else {
            panic!("unscoped SET expected");
        };
        let VariableSetRest::Generic(rest) = &body.rest else {
            panic!("generic SET expected");
        };
        assert_eq!(rest.values.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_multi_value() {
        let lexed = crate::lex("SET search_path TO public, pg_catalog");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = VariableSetStmt::parse(&mut input).unwrap();
        let VariableSetStmt::Unscoped(body) = parsed.ast()
        else {
            panic!("unscoped SET expected");
        };
        let VariableSetRest::Generic(rest) = &body.rest else {
            panic!("generic SET expected");
        };
        assert_eq!(rest.values.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_scope() {
        let lexed = crate::lex("SET SESSION enable_seqscan TO off");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = VariableSetStmt::parse(&mut input).unwrap();
        let VariableSetStmt::Session(body) = parsed.ast()
        else {
            panic!("SESSION-scoped SET expected");
        };
        assert!(matches!(body.rest, VariableSetRest::Generic(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset() {
        let lexed = crate::lex("RESET enable_seqscan");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = ResetStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(input.is_eof());
        let _ = stmt;
    }

    #[test]
    fn parse_reset_all() {
        let lexed = crate::lex("RESET ALL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ResetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_role() {
        let lexed = crate::lex("RESET ROLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ResetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_session_authorization() {
        let lexed = crate::lex("RESET SESSION AUTHORIZATION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ResetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_time_zone() {
        let lexed = crate::lex("RESET TIME ZONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ResetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_default() {
        let lexed = crate::lex("SET ROLE DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_none() {
        let lexed = crate::lex("SET ROLE NONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_name() {
        let lexed = crate::lex("SET ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_local_role() {
        let lexed = crate::lex("SET LOCAL ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_authorization_default() {
        let lexed = crate::lex("SET SESSION AUTHORIZATION DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_authorization_string() {
        let lexed = crate::lex("SET SESSION AUTHORIZATION 'alice'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_scope_session_authorization() {
        // gram.y `VariableSetStmt: SET SESSION set_rest` (gram.y:1617) over
        // one `set_rest`, whose `set_rest_more` holds `SESSION AUTHORIZATION
        // NonReservedWord_or_Sconst` (gram.y:1761): the scope keyword and the
        // `SESSION` of `SESSION AUTHORIZATION` are separate, so both appear.
        let lexed = crate::lex("SET SESSION SESSION AUTHORIZATION alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_local_scope_session_authorization() {
        let lexed = crate::lex("SET LOCAL SESSION AUTHORIZATION DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_string() {
        let lexed = crate::lex("SET TIME ZONE 'UTC'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_negative() {
        let lexed = crate::lex("SET TIME ZONE -8");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_default() {
        let lexed = crate::lex("SET TIME ZONE DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_param() {
        let lexed = crate::lex("SHOW TimeZone");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ShowStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_ident() {
        let lexed = crate::lex("SHOW transaction_read_only");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ShowStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_all() {
        let lexed = crate::lex("SHOW ALL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ShowStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_time_zone() {
        let lexed = crate::lex("SHOW TIME ZONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ShowStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_transaction_isolation_level() {
        let lexed = crate::lex("SHOW TRANSACTION ISOLATION LEVEL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = ShowStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_local() {
        let lexed = crate::lex("SET TIME ZONE LOCAL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VariableSetStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// Added in 19: `generic_set: var_name TO NULL_P | var_name '=' NULL_P`
    /// (gram.y b73d13c:1727, 1737; research PostgreSQL 19, "Changes to
    /// existing statements", commit ff4597acd). `NULL` is one value, not a
    /// list element. Every `generic_set` position takes it.
    #[cfg(feature = "since-pg19")]
    #[test]
    fn set_var_to_null_from_19() {
        crate::ast::test_support::check_statement_forms(
            &[
                "SET search_path TO NULL",
                "SET search_path = NULL",
                "SET LOCAL search_path TO NULL",
                "SET SESSION s.x = NULL",
                "ALTER ROLE r IN DATABASE d SET search_path = NULL",
                "ALTER FUNCTION f() SET search_path TO NULL",
                "CREATE FUNCTION f() RETURNS int LANGUAGE sql SET search_path TO NULL RETURN 1",
            ],
            &[
                "SET search_path TO NULL, public",
                "SET search_path TO public, NULL",
                "SET TIME ZONE NULL",
                "CREATE FUNCTION f() RETURNS int LANGUAGE sql SET search_path TO NULL, x RETURN 1",
            ],
        );
    }

    /// Before 19, `SET var TO NULL` is a syntax error (the same research
    /// entry).
    #[cfg(not(feature = "since-pg19"))]
    #[test]
    fn reject_set_var_to_null_before_19() {
        crate::ast::test_support::check_statement_forms(
            &["SET search_path TO DEFAULT"],
            &[
                "SET search_path TO NULL",
                "SET search_path = NULL",
                "ALTER ROLE r SET search_path TO NULL",
                "ALTER FUNCTION f() SET search_path TO NULL",
            ],
        );
    }
}
