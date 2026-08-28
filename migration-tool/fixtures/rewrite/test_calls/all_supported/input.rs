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

        let mut input = crate::tokens::test_input("SELECT 5");
        let parsed = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());

        let mut reinput = crate::tokens::test_input(formatted.as_str());
        let reparsed = <SelectStmt>::parse(&mut reinput).expect("reparse succeeds");
        assert!(reinput.is_empty(), "must consume EOF");
        let explained = SelectStmt::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse failed: {error}"));

        let already_converted = SelectStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(SelectStmt::parse(&mut input).is_err());
        let error = <SelectStmt as Parse>::parse(&mut input).expect_err("must reject");
        let _discarded_result = SelectStmt::parse(&mut input);

        consume(inferred, generic, parsed, reparsed, explained, already_converted, error);
    }
}
