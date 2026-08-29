// Reviewed callbacks are replaced by closed declarations in tokens.rs.



// Both token repair passes have declarative replacements.




#[derive(recursa::Node)]
pub enum WindowRefNameIdent<'input> {
    Ident(#[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(WindowRefName))] WindowRefNameText<'input>),
}


pub const UNRELATED_BYTES_SURVIVE: &str = "unchanged";
