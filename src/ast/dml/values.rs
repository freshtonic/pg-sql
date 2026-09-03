/// VALUES statement, TABLE statement, and set operation support (UNION, EXCEPT, INTERSECT).
use crate::ast::dml::select::SelectBody;

/// TABLE statement: `TABLE tablename [ORDER BY ...] [LIMIT ...] [OFFSET ...]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableStmt<'input> {
    #[tok(TABLE, this)]
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset: Option<Box<crate::ast::dml::select::LimitOffsetClause<'input>>>,
}

/// Set operation type.
///
/// Variant ordering: longer keyword sequences first within each group
/// so longest-match-wins picks UNION ALL over bare UNION, etc.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetOp {
    #[tok(UNION, ALL)]
    UnionAll,
    #[tok(UNION, DISTINCT)]
    UnionDistinct,
    #[tok(EXCEPT, ALL)]
    ExceptAll,
    #[tok(INTERSECT, ALL)]
    IntersectAll,
    #[tok(UNION)]
    Union,
    #[tok(EXCEPT)]
    Except,
    #[tok(INTERSECT)]
    Intersect,
}

/// A set operation combiner: `UNION [ALL|DISTINCT] | EXCEPT [ALL] | INTERSECT [ALL]`
/// followed by the right-hand query.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetOpCombiner<'input> {
    pub op: SetOp,
    pub right: Box<Subquery<'input>>,
}

/// A compound query: a query body optionally followed by a set operation.
/// This allows chaining: `VALUES ... UNION ALL SELECT ... EXCEPT TABLE ...`
/// Paren variant handles `(WITH ... SELECT ... UNION ...)` grouping.
#[derive(recursa::Node, Debug, Clone)]
pub enum Subquery<'input> {
    Paren(CompoundParen<'input>),
    Table(TableStmt<'input>),
    Body(CompoundBody<'input>),
}

/// Parenthesized compound query with optional set operation continuation.
/// e.g., `(SELECT ... UNION ALL ...) EXCEPT ...`
#[derive(recursa::Node, Debug, Clone)]
pub struct CompoundParen<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub inner: Box<Subquery<'input>>,
    pub set_op: Option<SetOpCombiner<'input>>,
    /// Optional trailing `ORDER BY ...` applied to the parenthesized query.
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset: Option<Box<crate::ast::dml::select::LimitOffsetClause<'input>>>,
}

/// A SELECT or VALUES body with optional set operation continuation.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompoundBody<'input> {
    pub body: SelectBody<'input>,
    pub set_op: Option<SetOpCombiner<'input>>,
}
