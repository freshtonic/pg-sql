/// ANALYZE statement AST: `ANALYZE [table [(col, ...)]]`.
use crate::tokens::literal;

/// ANALYZE statement with optional qualified table name and column list.
///
/// ```sql
/// ANALYZE [VERBOSE] [table_name [(column, ...)]]
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(ANALYZE, this)]
pub struct AnalyzeStmt<'input> {
    /// Greedy: a leading VERBOSE starts this element instead of ending `AnalyzeStmt` (bison shift preference).
    #[greedy(VERBOSE)]
    #[presence(VERBOSE)]
    /// Optional `VERBOSE` keyword (legacy bareword form).
    pub verbose: bool,
    /// Optional parenthesized options list, e.g.
    /// `(VERBOSE, SKIP_LOCKED, BUFFER_USAGE_LIMIT '512 kB')`.
    pub options: Option<AnalyzeOptions<'input>>,
    #[sep(COMMA)]
    pub targets: Option<recursa::Vec1<AnalyzeTarget<'input>>>,
}

/// Parenthesized options owned as one comma-separated list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct AnalyzeOptions<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<AnalyzeOption<'input>>,
);

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

/// Optional parenthesized column list on an ANALYZE target.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct AnalyzeColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<crate::tokens::ColId<'input>>,
);

/// `table_name [(column, ...)]` target of an ANALYZE statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct AnalyzeTarget<'input> {
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub columns: Option<AnalyzeColumnList<'input>>,
}
