#[cfg(test)]
mod tests {
    #[test]
    fn ambiguous_result_use() {
        let mut input = crate::tokens::test_input("SELECT 1");
        consume(SelectStmt::parse(&mut input));
    }
}
