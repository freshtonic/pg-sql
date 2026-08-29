/// ANALYZE statement AST: `ANALYZE [table [(col, ...)]]`.
use recursa::seq::{Seq0, Seq1};

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// ANALYZE statement with optional qualified table name and column list.
///
/// ```sql
/// ANALYZE [VERBOSE] [table_name [(column, ...)]]
/// ```
#[cfg_attr(feature = "arbitrary", derive(crate::arbitrary::Arbitrary))]
#[derive(recursa::Node, Debug, Clone)]
pub struct AnalyzeStmt<'input> {
    #[tok(ANALYZE, this)]
    #[presence(VERBOSE)]
    /// Optional `VERBOSE` keyword (legacy bareword form).
    pub verbose: bool,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    /// Optional parenthesized options list, e.g.
    /// `(VERBOSE, SKIP_LOCKED, BUFFER_USAGE_LIMIT '512 kB')`.
    pub options:
        Option< Vec<AnalyzeOption<'input> > >,
    #[sep(COMMA)]
    pub targets: Option<recursa::Vec1<AnalyzeTarget<'input> >>,
}

/// One option inside the parenthesized `ANALYZE (...)` options list.
///
/// Each option is a keyword-ish name (so we use `AliasName` to tolerate
/// identifiers that happen to collide with keywords) followed by an optional
/// value (string literal, integer, or ON/OFF-style AliasName).
#[derive(recursa::Node, Debug, Clone)]
pub struct AnalyzeOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<AnalyzeOptionValue<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum AnalyzeOptionValue<'input> {
    String(#[lex(pattern = r"'[^']*(?:''[^']*)*'")] literal::StringLit<'input>),
    Integer(literal::IntegerLit<'input>),
    Name(literal::AliasName<'input>),
}

/// `table_name [(column, ...)]` target of an ANALYZE statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct AnalyzeTarget<'input> {
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<
         Vec<crate::tokens::ColId<'input> > ,
    >,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::utility::analyze::AnalyzeStmt;

    #[test]
    fn parse_analyze() {
        let lexed = crate::tokens::lex("ANALYZE onek2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = AnalyzeStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.targets.unwrap().first().table_name.object(), "onek2");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_analyze_bare() {
        let lexed = crate::tokens::lex("ANALYZE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AnalyzeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_analyze_columns() {
        let lexed = crate::tokens::lex("ANALYZE atacc1(a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AnalyzeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
