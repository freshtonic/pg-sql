/// EXPLAIN statement AST.
use recursa::seq::Seq0;

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// An explain option value: ON, OFF, TRUE, FALSE, numeric, string, or identifier.
///
/// Per PG's `explain_option_arg` rule (gram.y), the value is an
/// `opt_boolean_or_string` / `NumericOnly`, so it accepts `ON`/`OFF`,
/// `TRUE`/`FALSE`, bare identifiers, numeric literals, and string literals
/// (e.g. `format 'json'`).
#[derive(recursa::Node, Debug, Clone)]
pub enum ExplainOptValue<'input> {
    #[tok(ON)] On,
    #[tok(OFF)] Off,
    #[tok(TRUE)] True,
    #[tok(FALSE)] False,
    // Numeric literal (e.g. `WAL on, ROWS 100`). `NumericLit` requires a
    // decimal/exponent (longer match), so it must come before `IntegerLit`.
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    // Quoted string (e.g. `FORMAT 'json'`).
    String(literal::StringLit<'input>),
    Ident(crate::tokens::ColId<'input>),
}

/// A single explain option: `name value` (e.g., `costs off`).
#[derive(recursa::Node, Debug, Clone)]
pub struct ExplainOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<ExplainOptValue<'input>>,
}

/// Explain options: `(opt, ...)`.
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
pub struct ExplainOptions<'input>(
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    #[deref] pub  Vec<ExplainOption<'input> > ,
);

/// EXPLAIN statement: `EXPLAIN [(options)] statement`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExplainStmt<'input> {
    #[tok(EXPLAIN, this)]
    pub options: Option<ExplainOptions<'input>>,
    pub body: Box<crate::ast::Statement<'input>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::utility::explain::ExplainStmt;

    #[test]
    fn parse_explain_costs_off() {
        let lexed = crate::tokens::lex("explain (costs off) select * from t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = ExplainStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_explain_multiple_options() {
        let lexed = crate::tokens::lex("explain (costs off, analyze on, timing off, summary off) select * from t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = ExplainStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
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
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt =
                ExplainStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(stmt.options.is_some());
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }
}
