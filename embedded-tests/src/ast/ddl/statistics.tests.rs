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
        let _stmt = AlterStatisticsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_statistics_set_statistics_negative() {
        let lexed = crate::lex("ALTER STATISTICS ab1_a_b_stats SET STATISTICS -1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterStatisticsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_statistics_full() {
        let lexed = crate::lex("CREATE STATISTICS s ON a, b FROM ext_stats_test");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateStatisticsStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_none());
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
        let stmt = CreateStatisticsStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(stmt.stat_types.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_statistics_paren_expr() {
        let lexed = crate::lex("CREATE STATISTICS s ON (a + b), c FROM ext_stats_test");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateStatisticsStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.on.is_some());
        assert!(input.is_eof());
    }
}
