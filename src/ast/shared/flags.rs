/// DROP/CREATE option flags shared across statement families.
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;

/// `CASCADE | RESTRICT` drop behavior.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DropBehavior {
    Cascade(CASCADE),
    Restrict(RESTRICT),
}

/// `IF EXISTS` modifier, shared by every DROP statement that allows it.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IfExists {
    pub r#if: IF,
    pub exists: EXISTS,
}

/// `IF NOT EXISTS` modifier, shared by CREATE statements that allow it.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IfNotExists {
    pub r#if: IF,
    pub not: NOT,
    pub exists: EXISTS,
}
