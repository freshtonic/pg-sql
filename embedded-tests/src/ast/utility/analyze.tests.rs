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
        assert_eq!(stmt.targets.as_ref().unwrap().first().table_name.object(), "onek2");
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
}
