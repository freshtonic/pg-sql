/// ANALYZE statement AST: `ANALYZE [table [(col, ...)]]`.
use crate::tokens::literal;

/// ANALYZE statement with optional qualified table name and column list.
///
/// ```sql
/// ANALYZE [VERBOSE] [table_name [(column, ...)]]
/// ```
#[derive(recursa::Node, Debug)]
#[tok(ANALYZE, this)]
pub struct AnalyzeStmt<'input> {
    #[presence(VERBOSE)]
    /// Optional `VERBOSE` keyword (legacy bareword form).
    pub verbose: bool,
    /// Optional parenthesized options list, e.g.
    /// `(VERBOSE, SKIP_LOCKED, BUFFER_USAGE_LIMIT '512 kB')`.
    pub options: Option<AnalyzeOptions<'input>>,
    #[sep(COMMA)]
    pub targets: Option<recursa::ArenaVec1<'input, AnalyzeTarget<'input>>>,
}

/// Parenthesized options owned as one comma-separated list.
#[derive(recursa::Node, Debug, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct AnalyzeOptions<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::ArenaVec1<'input, AnalyzeOption<'input>>,
);

/// One option inside the parenthesized `ANALYZE (...)` options list.
///
/// Each option is a keyword-ish name (so we use `AliasName` to tolerate
/// identifiers that happen to collide with keywords) followed by an optional
/// value (string literal, integer, or ON/OFF-style AliasName).
#[derive(recursa::Node, Debug)]
pub struct AnalyzeOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<AnalyzeOptionValue<'input>>,
}

#[derive(recursa::Node, Debug)]
pub enum AnalyzeOptionValue<'input> {
    // Use the canonical content token from `tokens::literal`. Keeping an
    // inline lexer declaration here would create a second `StringLit`
    // definition and prevent lookahead filters from naming it uniquely.
    String(literal::StringLit<'input>),
    Integer(literal::IntegerLit<'input>),
    Name(literal::AliasName<'input>),
}

/// Optional parenthesized column list on an ANALYZE target.
#[derive(recursa::Node, Debug, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct AnalyzeColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::ArenaVec1<'input, crate::tokens::ColId<'input>>,
);

/// `table_name [(column, ...)]` target of an ANALYZE statement.
#[derive(recursa::Node, Debug)]
pub struct AnalyzeTarget<'input> {
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub columns: Option<AnalyzeColumnList<'input>>,
}
