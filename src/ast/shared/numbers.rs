/// Numeric-literal helpers shared across statement families.
use crate::tokens::literal;

recursa::ast_node! {
    /// Leading `+` or `-` sign on a number.
    #[derive(Debug)]
    pub enum NumericSignToken {
        #[tok(MINUS)]
        Neg,
        #[tok(PLUS)]
        Pos,
    }
}

recursa::ast_node! {
    /// Either an integer or a numeric (decimal/exponent) literal.
    #[derive(Debug)]
    pub enum UnsignedNumberLit {
        /// Decimal/exponent forms come first so longest-match-wins picks them
        /// over a bare integer when a `.` or `e` is present.
        Numeric(#[lex(matcher)] literal::NumericLit),
        Integer(#[lex(matcher)] literal::IntegerLit),
    }
}

recursa::ast_node! {
    /// Postgres' `NumericOnly`: optionally signed integer or floating-point
    /// literal. Used by CREATE DATABASE options like `CONNECTION LIMIT n` and
    /// `ENCODING -1`.
    #[derive(Debug)]
    pub struct NumericOnly {
        pub sign: Option<NumericSignToken>,
        pub value: UnsignedNumberLit,
    }
}

recursa::ast_node! {
    /// Postgres' `SignedIconst`: optionally signed integer literal. Used by
    /// `SYSID n` in CREATE ROLE.
    #[derive(Debug)]
    pub struct SignedIconst {
        pub sign: Option<NumericSignToken>,
        pub value: literal::IntegerLit,
    }
}
