/// EXPLAIN statement AST.
use recursa::seq::Seq0;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// An explain option value: ON, OFF, TRUE, FALSE, numeric, string, or identifier.
///
/// Per PG's `explain_option_arg` rule (gram.y), the value is an
/// `opt_boolean_or_string` / `NumericOnly`, so it accepts `ON`/`OFF`,
/// `TRUE`/`FALSE`, bare identifiers, numeric literals, and string literals
/// (e.g. `format 'json'`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExplainOptValue<'input> {
    On(ON),
    Off(OFF),
    True(TRUE),
    False(FALSE),
    // Numeric literal (e.g. `WAL on, ROWS 100`). `NumericLit` requires a
    // decimal/exponent (longer match), so it must come before `IntegerLit`.
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    // Quoted string (e.g. `FORMAT 'json'`).
    String(literal::StringLit<'input>),
    Ident(crate::tokens::ColId<'input>),
}

/// A single explain option: `name value` (e.g., `costs off`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExplainOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<ExplainOptValue<'input>>,
}

/// Explain options: `(opt, ...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
#[recursa::parser(rules = SqlRules)]
pub struct ExplainOptions<'input>(
    #[deref] pub Surrounded<punct::LParen, Seq0<ExplainOption<'input>, punct::Comma>, punct::RParen>,
);

/// EXPLAIN statement: `EXPLAIN [(options)] statement`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct ExplainStmt<'input> {
    pub explain: EXPLAIN,
    pub options: Option<ExplainOptions<'input>>,
    pub body: Box<crate::ast::Statement<'input>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::utility::explain::ExplainStmt;

    #[test]
    fn parse_explain_costs_off() {
        let mut input = crate::tokens::test_input("explain (costs off) select * from t");
        let stmt = ExplainStmt::parse(&mut input).unwrap();
        assert!(stmt.options.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_explain_multiple_options() {
        let mut input = crate::tokens::test_input(
            "explain (costs off, analyze on, timing off, summary off) select * from t",
        );
        let stmt = ExplainStmt::parse(&mut input).unwrap();
        assert!(stmt.options.is_some());
        assert!(input.is_empty());
    }

    /// `EXPLAIN (VERBOSE TRUE, COSTS FALSE)` — PG's `explain_option_arg` accepts
    /// `opt_boolean_or_string` (gram.y), so `TRUE` / `FALSE` are valid option
    /// values alongside `ON` / `OFF` / identifier. The fast_default regression
    /// fixture relies on `EXPLAIN (VERBOSE TRUE, COSTS FALSE) SELECT ...`.
    #[test]
    fn parse_explain_bool_option_value() {
        for src in [
            "EXPLAIN (VERBOSE TRUE, COSTS FALSE) SELECT 1",
            "EXPLAIN (VERBOSE true) SELECT 1",
            "EXPLAIN (BUFFERS false) SELECT 1",
        ] {
            let mut input = crate::tokens::test_input(src);
            let stmt =
                ExplainStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(stmt.options.is_some());
            assert!(
                input.is_empty(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }
}
