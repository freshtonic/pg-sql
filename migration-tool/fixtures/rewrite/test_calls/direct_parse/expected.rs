#[cfg(test)]
mod tests {
    #[test]
    fn parses_statement() {
        let lexed = crate::tokens::lex("SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        assert_eq!(stmt.columns.len(), 1);
    }
}
