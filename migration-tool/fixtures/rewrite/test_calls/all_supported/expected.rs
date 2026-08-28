pub enum GrammarRegion {
    Keep,
}

impl GrammarRegion {
    fn parser_code(input: &mut Input) {
        // Grammar code is outside a test region and must remain byte-identical.
        let _node = GrammarNode::parse(input).unwrap();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn helper_and_direct_shapes() {
        // Keep this comment and source order.
        let inferred: SelectStmt = parse_stmt("SELECT 1");
        let generic = parse_stmt::<SelectStmt>("SELECT 2");
        reparse_stable::<SelectStmt>("SELECT 3");
        let formatted = roundtrip::<SelectStmt>("SELECT 4");

        let lexed = crate::tokens::lex("SELECT 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let parsed = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());

        let relexed = crate::tokens::lex(formatted.as_str());
        assert_eq!(relexed.errors().count(), 0, "lex errors in reinput");
        let mut reinput = relexed.input();
        let reparsed = <SelectStmt>::parse(&mut reinput).expect("reparse succeeds").into_ast();
        assert!(reinput.is_eof(), "must consume EOF");
        let explained = SelectStmt::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse failed: {error}")).into_ast();

        let already_converted = SelectStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(SelectStmt::parse(&mut input).is_err());
        let error = <SelectStmt as Parse>::parse(&mut input).expect_err("must reject");
        let _discarded_result = SelectStmt::parse(&mut input);

        consume(inferred, generic, parsed, reparsed, explained, already_converted, error);
    }
}
