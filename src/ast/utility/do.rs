//! DO anonymous code block.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::literal;

// --- DO ---

/// `DO [LANGUAGE lang] $$ ... $$` anonymous code block.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["procedural"])]
pub struct DoStmt<'input> {
    pub r#do: DO,
    pub language: Option<DoLanguage<'input>>,
    pub body: literal::DollarStringLit<'input>,
    pub trailing_language: Option<DoLanguage<'input>>,
}

/// `LANGUAGE lang` clause on a `DO` block (may appear before or after body).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DoLanguage<'input> {
    pub language: LANGUAGE,
    pub name: crate::tokens::ColId<'input>,
}
