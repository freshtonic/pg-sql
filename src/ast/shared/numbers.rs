/// Numeric-literal helpers shared across statement families.
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::{literal, punct};

/// Leading `+` or `-` sign on a number.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NumericSignToken {
    Neg(punct::Minus),
    Pos(punct::Plus),
}

/// Either an integer or a numeric (decimal/exponent) literal.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum UnsignedNumberLit<'input> {
    /// Decimal/exponent forms come first so longest-match-wins picks them
    /// over a bare integer when a `.` or `e` is present.
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
}

/// Postgres' `NumericOnly`: optionally signed integer or floating-point
/// literal. Used by CREATE DATABASE options like `CONNECTION LIMIT n` and
/// `ENCODING -1`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NumericOnly<'input> {
    pub sign: Option<NumericSignToken>,
    pub value: UnsignedNumberLit<'input>,
}

/// Postgres' `SignedIconst`: optionally signed integer literal. Used by
/// `SYSID n` in CREATE ROLE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SignedIconst<'input> {
    pub sign: Option<NumericSignToken>,
    pub value: literal::IntegerLit<'input>,
}
