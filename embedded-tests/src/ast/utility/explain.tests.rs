#[cfg(test)]
mod tests {
    use crate::ast::utility::explain::ExplainStmt;

    #[test]
    fn parse_explain_costs_off() {
        let lexed = crate::lex("explain (costs off) select * from t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = ExplainStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.options().is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_explain_multiple_options() {
        let lexed = crate::lex(
            "explain (costs off, analyze on, timing off, summary off) select * from t",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = ExplainStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.options().is_some());
        assert!(input.is_eof());
    }

    /// `EXPLAIN (VERBOSE TRUE, COSTS FALSE)` — PG's `explain_option_arg` accepts
    /// `opt_boolean_or_string` (gram.y), so `TRUE` / `FALSE` are valid option
    /// values alongside `ON` / `OFF` / identifier. The fast_default regression
    /// fixture relies on `EXPLAIN (VERBOSE TRUE, COSTS FALSE) SELECT ...`.
    #[test]
    fn parse_explain_bool_option_value() {
        for src in [
            "EXPLAIN (VERBOSE TRUE, COSTS FALSE) SELECT 1",
            "EXPLAIN (VERBOSE true) SELECT 1",
            "EXPLAIN (BUFFERS false) SELECT 1",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt = ExplainStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(stmt.options().is_some());
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }
}
