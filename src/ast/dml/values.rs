/// VALUES statement, TABLE statement, and set operation support (UNION, EXCEPT, INTERSECT).
use crate::ast::dml::select::SelectBody;

/// TABLE statement: gram.y `simple_select: TABLE relation_expr`. Its ORDER BY
/// and LIMIT tails belong to the enclosing [`QueryBody`], as they do for
/// every `simple_select`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableStmt<'input> {
    #[tok(TABLE, this)]
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
}

/// A set operation keyword with its optional `ALL` / `DISTINCT` quantifier.
///
/// Variant ordering: the two-token forms before the bare keywords.
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

/// `UNION | EXCEPT | INTERSECT [ALL | DISTINCT] select_clause` — gram.y
/// `simple_select: select_clause UNION set_quantifier select_clause`; the
/// right operand is a `select_clause`, never a sorted or limited query.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetOpCombiner<'input> {
    pub op: SetOp,
    pub right: Box<SelectClause<'input>>,
}

/// gram.y `SelectStmt`: `select_no_parens` with its `with_clause` forms, the
/// query every subquery position accepts (`(WITH ... SELECT ... ORDER BY 1)`).
/// The statement level keeps its `with_clause` factored in
/// `WithStatement`, because there the CTE list is also the prefix of
/// `INSERT`, `UPDATE`, `DELETE` and `MERGE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct Subquery<'input> {
    pub with: Option<crate::ast::shared::with_clause::WithClause<'input>>,
    pub body: QueryBody<'input>,
}

/// gram.y `select_no_parens` without `with_clause`: a `select_clause`
/// followed by `opt_sort_clause`, `opt_select_limit` and
/// `opt_for_locking_clause`. The tails apply to the whole set-operation
/// chain and belong to no `simple_select` member, so
/// `SELECT 1 UNION SELECT 2 ORDER BY 1` sorts the union.
#[derive(recursa::Node, Debug, Clone)]
pub struct QueryBody<'input> {
    pub clause: SelectClause<'input>,
    #[pretty(break_before = soft)]
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    /// LIMIT / OFFSET / FETCH FIRST tail. Postgres allows one limiting
    /// clause (`LIMIT` or `FETCH FIRST`) and one `OFFSET`, in either order.
    #[pretty(break_before = soft)]
    pub limit_offset: Option<Box<crate::ast::dml::select::LimitOffsetClause<'input>>>,
    #[pretty(break_before = soft)]
    pub for_update: Option<Box<crate::ast::dml::select::ForUpdateClause<'input>>>,
}

/// gram.y `select_with_parens` (gram.y:12682):
///
/// ```text
/// select_with_parens: '(' select_no_parens ')' | '(' select_with_parens ')'
/// ```
///
/// The second production needs no separate variant here: [`Subquery`] is
/// `select_no_parens`, and its `select_clause` admits a further
/// `SelectWithParens`, so `((SELECT 1))` reduces through this one rule.
///
/// This is the single nonterminal that [`SelectClause`] and [`SimpleSelect`]
/// share. gram.y factors it for the same reason (gram.y:12658): spelling the
/// parenthesized query once is what stops the two from carrying duplicate
/// copies of every production below them.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectWithParens<'input> {
    pub open: crate::ast::shared::expr::ParenthesizedOpen,
    pub inner: Box<Subquery<'input>>,
    pub close: crate::ast::shared::expr::ParenthesizedClose,
}

/// A parenthesized left operand with its required set operation:
/// gram.y `simple_select: select_clause UNION set_quantifier select_clause`
/// (gram.y:12844) where the left `select_clause` is a `select_with_parens`,
/// as in `(SELECT 1) UNION SELECT 2`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ParenthesizedSetOp<'input> {
    pub left: SelectWithParens<'input>,
    pub set_op: SetOpCombiner<'input>,
}

/// gram.y `simple_select` (gram.y:12790): the set-operation members of a
/// query, which are exactly the `select_clause` forms that carry no outer
/// parentheses of their own.
///
/// Variant ordering: `ParenthesizedSet` starts with `(`, `Table` with
/// `TABLE`, `Body` with `SELECT` or `VALUES`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SimpleSelect<'input> {
    ParenthesizedSet(ParenthesizedSetOp<'input>),
    Table(TableStmt<'input>),
    Body(CompoundBody<'input>),
}

/// gram.y `select_clause: simple_select | select_with_parens`
/// (gram.y:12757).
///
/// Variant ordering: both alternatives can begin with `(`, and they part
/// company after the balanced group — a set operation there continues a
/// `simple_select`, and anything else ends the `select_with_parens`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectClause<'input> {
    Simple(SimpleSelect<'input>),
    Parens(SelectWithParens<'input>),
}

/// A `SELECT` or `VALUES` `simple_select` with an optional set operation.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompoundBody<'input> {
    pub body: SelectBody<'input>,
    pub set_op: Option<SetOpCombiner<'input>>,
}
