#[cfg(test)]
mod tests {
    use crate::ast::utility::analyze::AnalyzeStmt;

    #[test]
    fn parse_analyze() {
        let lexed = crate::lex("ANALYZE onek2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = AnalyzeStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.targets.as_ref().unwrap().first().relation_name().object(), "onek2");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_analyze_bare() {
        let lexed = crate::lex("ANALYZE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AnalyzeStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_analyze_columns() {
        let lexed = crate::lex("ANALYZE atacc1(a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AnalyzeStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
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

    // Added in 18: `vacuum_relation: relation_expr opt_name_list`
    // (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
    // item 9).
    #[cfg(feature = "since-pg18")]
    #[test]
    fn analyze_relation_expr_of_18() {
        let stmt = crate::ast::test_support::parse_stmt::<AnalyzeStmt>(
            "ANALYZE (VERBOSE) ONLY t (a), u *",
        );
        let stmt = stmt.ast();
        let targets = stmt.targets.as_ref().unwrap();
        assert!(targets.first().table_name.is_only());
        assert_eq!(targets.first().relation_name().object(), "t");
        for src in ["ANALYZE ONLY t", "ANALYZE VERBOSE ONLY (t)", "ANALYZE t * (a)"] {
            crate::ast::test_support::reparse_stable::<AnalyzeStmt>(src);
        }
        assert!(!statement_parses("ANALYZE ONLY t *"));
    }

    #[cfg(not(feature = "since-pg18"))]
    #[test]
    fn analyze_relation_expr_is_rejected_before_18() {
        for src in ["ANALYZE ONLY t", "ANALYZE t *"] {
            assert!(!statement_parses(src), "{src:?} must not parse before 18");
        }
    }
}
