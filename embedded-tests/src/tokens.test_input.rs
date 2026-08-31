/// Lexes `src` into the grammar-local input type used by generated parsers.
#[cfg(test)]
pub fn test_input(src: &'static str) -> crate::Input<'static> {
    crate::lex(src).input()
}
