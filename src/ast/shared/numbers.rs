/// Numeric-literal helpers shared across statement families.
use recursa_diagram::railroad;

use crate::tokens::{literal, punct};

/// Leading `+` or `-` sign on a number.
#[derive(recursa::Node, Debug, Clone)]
pub enum NumericSignToken {
    #[tok(MINUS)] Neg,
    #[tok(PLUS)] Pos,
}

/// Either an integer or a numeric (decimal/exponent) literal.
#[derive(recursa::Node, Debug, Clone)]
pub enum UnsignedNumberLit<'input> {
    /// Decimal/exponent forms come first so longest-match-wins picks them
    /// over a bare integer when a `.` or `e` is present.
    Numeric(#[lex(matcher)] literal::NumericLit<'input>),
    Integer(#[lex(matcher)] literal::IntegerLit<'input>),
}

/// Postgres' `NumericOnly`: optionally signed integer or floating-point
/// literal. Used by CREATE DATABASE options like `CONNECTION LIMIT n` and
/// `ENCODING -1`.
#[derive(recursa::Node, Debug, Clone)]
pub struct NumericOnly<'input> {
    pub sign: Option<NumericSignToken>,
    pub value: UnsignedNumberLit<'input>,
}

/// Postgres' `SignedIconst`: optionally signed integer literal. Used by
/// `SYSID n` in CREATE ROLE.
#[derive(recursa::Node, Debug, Clone)]
pub struct SignedIconst<'input> {
    pub sign: Option<NumericSignToken>,
    pub value: literal::IntegerLit<'input>,
}
