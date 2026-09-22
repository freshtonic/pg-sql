#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_statistics_set_statistics_default() {
        let lexed = crate::lex("ALTER STATISTICS IF EXISTS ab1_a_b_stats SET STATISTICS 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterStatisticsStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_statistics_set_statistics_negative() {
        let lexed = crate::lex("ALTER STATISTICS ab1_a_b_stats SET STATISTICS -1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterStatisticsStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_statistics_full() {
        let lexed = crate::lex("CREATE STATISTICS s ON a, b FROM ext_stats_test");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateStatisticsStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_not_exists.is_none());
        // The name is a required field before 16.
        #[cfg(feature = "since-pg16")]
        assert!(stmt.name.is_some());
        assert!(stmt.on.is_some());
        assert!(stmt.from.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_statistics_with_kinds_and_if_not_exists() {
        let lexed = crate::lex("CREATE STATISTICS IF NOT EXISTS s (ndistinct, dependencies) ON a, b FROM tab");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateStatisticsStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(stmt.stat_types.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_statistics_paren_expr() {
        let lexed = crate::lex("CREATE STATISTICS s ON (a + b), c FROM ext_stats_test");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateStatisticsStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.on.is_some());
        assert!(input.is_eof());
    }

    // `SET STATISTICS DEFAULT` is added in 17: research, PostgreSQL 17, "Changes to existing statements"
    // (REL_17_11 gram.y 4662, 4671).
    #[cfg(feature = "since-pg17")]
    #[test]
    fn alter_statistics_set_statistics_default() {
        crate::ast::test_support::assert_statements_parse(&[
            "ALTER STATISTICS s SET STATISTICS DEFAULT",
            "ALTER STATISTICS IF EXISTS s SET STATISTICS DEFAULT",
        ]);
    }

    // Added in 17, so rejected before 17: research, PostgreSQL 17, "Changes to existing statements".
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn alter_statistics_set_statistics_default_is_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "ALTER STATISTICS s SET STATISTICS DEFAULT",
            "ALTER STATISTICS IF EXISTS s SET STATISTICS DEFAULT",
        ]);
        crate::ast::test_support::assert_statements_parse(&["ALTER STATISTICS s SET STATISTICS 10"]);
    }

    // Added in 16: research, PostgreSQL 16, "Changes to existing statements"
    // (REL_16_15 gram.y `CreateStatsStmt`, commit 624aa2a13).
    #[cfg(feature = "since-pg16")]
    #[test]
    fn create_statistics_without_a_name() {
        crate::ast::test_support::assert_statements_parse(&[
            "CREATE STATISTICS ON a, b FROM t",
            "CREATE STATISTICS (ndistinct) ON a, b FROM t",
        ]);
    }

    // Added in 16, so rejected before 16: REL_15_19 gram.y has
    // `CREATE STATISTICS any_name`.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn create_statistics_needs_a_name_before_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "CREATE STATISTICS ON a, b FROM t",
            "CREATE STATISTICS (ndistinct) ON a, b FROM t",
            "CREATE STATISTICS IF NOT EXISTS ON a, b FROM t",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "CREATE STATISTICS s ON a, b FROM t",
            "CREATE STATISTICS IF NOT EXISTS s (ndistinct) ON a, b FROM t",
        ]);
    }
}
