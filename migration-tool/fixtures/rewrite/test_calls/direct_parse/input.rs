#[cfg(test)]
mod tests {
    #[test]
    fn parses_statement() {
        let mut input = crate::tokens::test_input("SELECT 1");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
        assert_eq!(stmt.columns.len(), 1);
    }
}
