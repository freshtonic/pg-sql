/// WITH clause (Common Table Expressions) AST.
///
/// Supports `WITH [RECURSIVE] name [(col, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (query)
///   [SEARCH DEPTH|BREADTH FIRST BY col, ... SET col]
///   [CYCLE col, ... SET col [TO val DEFAULT val] USING col]
///   [, ...] SELECT|INSERT|UPDATE|DELETE|MERGE`
use crate::ast::shared::expr::Expr;
use crate::tokens::literal;

/// Materialization option: `MATERIALIZED` or `NOT MATERIALIZED`.
#[derive(recursa::Node, Debug, Clone)]
pub enum MaterializedOption {
    #[tok(NOT, MATERIALIZED)]
    NotMaterialized,
    #[tok(MATERIALIZED)]
    Materialized,
}

/// Required `AS` separator between a CTE name and its optional materialization mode.
#[derive(recursa::Node, Debug, Clone)]
pub enum CteAs {
    #[tok(AS)]
    Value,
}

/// SEARCH direction: DEPTH or BREADTH
#[derive(recursa::Node, Debug, Clone)]
pub enum SearchDirection {
    #[tok(DEPTH)]
    Depth,
    #[tok(BREADTH)]
    Breadth,
}

/// `FIRST BY col, ...` list in a recursive CTE SEARCH clause.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(FIRST, BY, this)]
pub struct SearchColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<literal::AliasName<'input>>,
);

/// SEARCH clause: `SEARCH DEPTH|BREADTH FIRST BY col, ... SET col`
#[derive(recursa::Node, Debug, Clone)]
pub struct SearchClause<'input> {
    #[tok(SEARCH, this)]
    pub direction: SearchDirection,
    pub columns: SearchColumnList<'input>,
    #[tok(SET, this)]
    pub set_column: literal::AliasName<'input>,
}

/// CYCLE clause: `CYCLE col, ... SET col [TO val DEFAULT val] USING col`
#[derive(recursa::Node, Debug, Clone)]
#[tok(CYCLE, this)]
pub struct CycleClause<'input> {
    #[sep(COMMA)]
    pub columns: Vec<literal::AliasName<'input>>,
    #[tok(SET, this)]
    pub set_column: CycleSetColumn<'input>,
    #[tok(USING, this)]
    pub using_column: literal::AliasName<'input>,
}

/// SET column with optional TO/DEFAULT values.
#[derive(recursa::Node, Debug, Clone)]
pub struct CycleSetColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub to_default: Option<CycleToDefault<'input>>,
}

/// TO value DEFAULT value
#[derive(recursa::Node, Debug, Clone)]
pub struct CycleToDefault<'input> {
    #[tok(TO, this)]
    pub to_value: Expr<'input>,
    #[tok(DEFAULT, this)]
    pub default_value: Expr<'input>,
}

/// Optional parenthesized output-column list on a CTE definition.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct CteColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<literal::AliasName<'input>>,
);

/// A single CTE definition: `name [(col, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (query)
///   [SEARCH ...] [CYCLE ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct CteDefinition<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub columns: Option<CteColumnList<'input>>,
    pub r#as: CteAs,
    pub materialized: Option<MaterializedOption>,
    #[tok(LPAREN, this, RPAREN)]
    pub query: Box<crate::ast::Statement<'input>>,
    pub search: Option<SearchClause<'input>>,
    pub cycle: Option<CycleClause<'input>>,
}

/// WITH clause: `WITH [RECURSIVE] cte_def, ...`
#[derive(recursa::Node, Debug, Clone)]
pub struct WithClause<'input> {
    #[tok(WITH)]
    #[presence(RECURSIVE)]
    pub recursive: bool,
    #[sep(COMMA)]
    pub ctes: recursa::Vec1<CteDefinition<'input>>,
}

/// WITH statement: WITH clause followed by a body statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithStatement<'input> {
    pub with_clause: WithClause<'input>,
    pub body: Box<crate::ast::Statement<'input>>,
}
