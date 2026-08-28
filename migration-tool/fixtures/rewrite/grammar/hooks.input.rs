// Reviewed callbacks are replaced by closed declarations in tokens.rs.
pub fn scan_dollar_string(lexer: &mut Lexer<'_>) -> Action {
    lexer.scan_same_delimiter()
}

pub fn reject_trailing_word(lexer: &mut Lexer<'_>) -> Action {
    lexer.reject_word_character()
}

pub fn skip_block_comment(lexer: &mut Lexer<'_>) -> Action {
    lexer.skip_nested_comment()
}

// Both token repair passes have declarative replacements.
pub fn pg_lex(source: &str) -> LexResult {
    let mut result = lex(source);
    split_psql_var_keyword_tokens(source, &mut result.tokens);
    split_bang_eq_minus_before_dash_comment(source, &mut result.tokens);
    result
}

fn split_psql_var_keyword_tokens(source: &str, tokens: &mut Vec<TokenRecord>) {
    repair_psql_variables(source, tokens);
}

fn split_bang_eq_minus_before_dash_comment(source: &str, tokens: &mut Vec<TokenRecord>) {
    repair_operator_comment_fence(source, tokens);
}

fn not_frame_unit(value: &Ident<'_>) -> Result<(), ParseError> {
    reject_frame_unit(value)
}

#[derive(recursa::Node)]
#[recursa::parser(postcondition = crate::tokens::not_frame_unit_wrapper)]
pub enum WindowRefNameIdent<'input> {
    Ident(Ident<'input>),
}

pub fn not_frame_unit_wrapper(value: &WindowRefNameIdent<'_>) -> Result<(), ParseError> {
    let WindowRefNameIdent::Ident(value) = value;
    not_frame_unit(value)
}

pub const UNRELATED_BYTES_SURVIVE: &str = "unchanged";
