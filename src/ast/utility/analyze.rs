/// ANALYZE statement AST: `ANALYZE [table [(col, ...)]]`.
use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// ANALYZE statement with optional qualified table name and column list.
///
/// ```sql
/// ANALYZE [VERBOSE] [table_name [(column, ...)]]
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(crate::arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct AnalyzeStmt<'input> {
    pub analyze: ANALYZE,
    /// Optional `VERBOSE` keyword (legacy bareword form).
    pub verbose: Option<VERBOSE>,
    /// Optional parenthesized options list, e.g.
    /// `(VERBOSE, SKIP_LOCKED, BUFFER_USAGE_LIMIT '512 kB')`.
    pub options:
        Option<Surrounded<punct::LParen, Seq0<AnalyzeOption<'input>, punct::Comma>, punct::RParen>>,
    pub targets: Option<Seq1<AnalyzeTarget<'input>, punct::Comma>>,
}

/// One option inside the parenthesized `ANALYZE (...)` options list.
///
/// Each option is a keyword-ish name (so we use `AliasName` to tolerate
/// identifiers that happen to collide with keywords) followed by an optional
/// value (string literal, integer, or ON/OFF-style AliasName).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AnalyzeOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<AnalyzeOptionValue<'input>>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AnalyzeOptionValue<'input> {
    String(literal::StringLit<'input>),
    Integer(literal::IntegerLit<'input>),
    Name(literal::AliasName<'input>),
}

/// `table_name [(column, ...)]` target of an ANALYZE statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AnalyzeTarget<'input> {
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    >,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::utility::analyze::AnalyzeStmt;

    #[test]
    fn parse_analyze() {
        let mut input = crate::tokens::test_input("ANALYZE onek2");
        let stmt = AnalyzeStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.targets.unwrap().first().table_name.object(), "onek2");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_analyze_bare() {
        let mut input = crate::tokens::test_input("ANALYZE");
        let _stmt = AnalyzeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_analyze_columns() {
        let mut input = crate::tokens::test_input("ANALYZE atacc1(a, b)");
        let _stmt = AnalyzeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
