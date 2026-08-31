#[cfg(test)]
mod tests {
    use crate::ast::session::set_reset::{
        ResetStmt, SetRoleStmt, SetSessionAuthStmt, SetStmt, SetTimeZoneStmt, ShowStmt,
    };

    #[test]
    fn parse_set_to() {
        let lexed = crate::lex("SET enable_seqscan TO off");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.param.object(), "enable_seqscan");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_eq() {
        let lexed = crate::lex("SET enable_sort = false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.param.object(), "enable_sort");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_integer_value() {
        let lexed = crate::lex("SET work_mem = 4096");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.values.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_numeric_value() {
        let lexed = crate::lex("SET seq_page_cost = 1.5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.values.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_multi_value() {
        let lexed = crate::lex("SET search_path TO public, pg_catalog");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.values.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_scope() {
        let lexed = crate::lex("SET SESSION enable_seqscan TO off");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.scope.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset() {
        let lexed = crate::lex("RESET enable_seqscan");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let _ = stmt;
    }

    #[test]
    fn parse_reset_all() {
        let lexed = crate::lex("RESET ALL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_role() {
        let lexed = crate::lex("RESET ROLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_session_authorization() {
        let lexed = crate::lex("RESET SESSION AUTHORIZATION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_time_zone() {
        let lexed = crate::lex("RESET TIME ZONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_default() {
        let lexed = crate::lex("SET ROLE DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_none() {
        let lexed = crate::lex("SET ROLE NONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_name() {
        let lexed = crate::lex("SET ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_local_role() {
        let lexed = crate::lex("SET LOCAL ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_authorization_default() {
        let lexed = crate::lex("SET SESSION AUTHORIZATION DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetSessionAuthStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_authorization_string() {
        let lexed = crate::lex("SET SESSION AUTHORIZATION 'alice'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetSessionAuthStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_string() {
        let lexed = crate::lex("SET TIME ZONE 'UTC'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_negative() {
        let lexed = crate::lex("SET TIME ZONE -8");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_default() {
        let lexed = crate::lex("SET TIME ZONE DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_param() {
        let lexed = crate::lex("SHOW TimeZone");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_ident() {
        let lexed = crate::lex("SHOW transaction_read_only");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_all() {
        let lexed = crate::lex("SHOW ALL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_time_zone() {
        let lexed = crate::lex("SHOW TIME ZONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_transaction_isolation_level() {
        let lexed = crate::lex("SHOW TRANSACTION ISOLATION LEVEL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_local() {
        let lexed = crate::lex("SET TIME ZONE LOCAL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
