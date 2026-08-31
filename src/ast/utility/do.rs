//! DO anonymous code block.

use crate::tokens::literal;

// --- DO ---

/// `DO [LANGUAGE lang] $$ ... $$` anonymous code block.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DO, this)]
pub struct DoStmt<'input> {
    pub language: Option<DoLanguage<'input>>,
    #[lex(matcher)]
    pub body: literal::DollarStringLit<'input>,
    pub trailing_language: Option<DoLanguage<'input>>,
}

/// `LANGUAGE lang` clause on a `DO` block (may appear before or after body).
#[derive(recursa::Node, Debug, Clone)]
pub struct DoLanguage<'input> {
    #[tok(LANGUAGE, this)]
    pub name: crate::tokens::ColId<'input>,
}
