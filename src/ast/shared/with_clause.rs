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
    /// gram.y `columnList`: one or more `ColId`.
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<crate::tokens::ColId<'input>>,
);

/// SEARCH clause: `SEARCH DEPTH|BREADTH FIRST BY col, ... SET col`
#[derive(recursa::Node, Debug, Clone)]
pub struct SearchClause<'input> {
    #[tok(SEARCH, this)]
    pub direction: SearchDirection,
    pub columns: SearchColumnList<'input>,
    #[tok(SET, this)]
    pub set_column: crate::tokens::ColId<'input>,
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
    pub query: Box<crate::ast::tcl::prepared::PreparableStmt<'input>>,
    pub search: Option<SearchClause<'input>>,
    pub cycle: Option<CycleClause<'input>>,
}

/// `WITH` with its optional `RECURSIVE` modifier. Keeping the alternatives
/// explicit lets the LR table encode PostgreSQL's shift preference: an
/// immediate `RECURSIVE` is always the modifier, never the first CTE name.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithClauseHead {
    Recursive(RecursiveWith),
    #[parse(lr_conflict(
        action = shift,
        against = ast::shared::with_clause::RecursiveWith,
        lookahead = { RECURSIVE },
        expect = 1
    ))]
    Plain(crate::ast::shared::flags::AnyWith),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct RecursiveWith {
    pub with: crate::ast::shared::flags::AnyWith,
    pub recursive: RecursiveKeyword,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum RecursiveKeyword {
    #[tok(RECURSIVE)]
    Value,
}

/// WITH clause: `WITH [RECURSIVE] cte_def, ...` — gram.y `with_clause:
/// WITH cte_list | WITH_LA cte_list | WITH RECURSIVE cte_list`, whose
/// `WITH_LA` twin lets the first CTE be named `time` or `ordinality`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithClause<'input> {
    pub head: WithClauseHead,
    #[sep(COMMA)]
    pub ctes: recursa::Vec1<CteDefinition<'input>>,
}

impl WithClause<'_> {
    pub fn is_recursive(&self) -> bool {
        matches!(self.head, WithClauseHead::Recursive(_))
    }
}

/// WITH statement: WITH clause followed by a query-shaped body.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithStatement<'input> {
    pub with_clause: WithClause<'input>,
    pub body: WithBody<'input>,
}
/// Query that follows a `WITH` clause. Mirrors `gram.y`: `with_clause` is
/// followed by `select_clause` (with its set operations), `insert_rest`,
/// `update`, `delete`, or `merge`. A second `WITH` clause is not admitted, so
/// the `Statement` FOLLOW set no longer inherits every query continuation.
///
/// Variant order: `Query` leads with `SELECT`, `VALUES`, `TABLE`, or `(`;
/// the four DML variants have disjoint leading keywords.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithBody<'input> {
    /// gram.y `select_no_parens: with_clause select_clause ...`.
    Query(Box<crate::ast::dml::values::QueryBody<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Merge(Box<crate::ast::dml::merge::MergeStmt<'input>>),
}
