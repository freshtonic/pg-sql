#[derive(recursa::Node)]
pub struct Bad<'input> {
    #[lex(pattern = r"[a-z]+", callback = crate::scan)]
    value: Word<'input>,
}
