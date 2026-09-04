/// UPDATE statement AST.
///
/// `UPDATE table SET col = expr [, ...] [FROM ...] [WHERE ...] [RETURNING ...]`
use crate::ast::dml::select::{FromClause, WhereClause};
use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

/// Single SET assignment: `col = expr`, `col[idx] = expr`,
/// `col[lo:hi] = expr`, `alias.col = expr`, or any chain thereof.
///
/// The target is a column name plus an optional indirection chain
/// (Postgres `set_target: ColId opt_indirection`). The `.field` form of
/// indirection also covers the `alias.col` left-hand side that
/// `ON CONFLICT DO UPDATE` permits inside `INSERT ... AS alias`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SingleAssignment<'input> {
    pub column: literal::Ident<'input>,
    /// Greedy: a leading DOT, LBRACKET starts this element instead of ending `SingleAssignment` (bison shift preference).
    #[greedy(DOT, LBRACKET)]
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
    #[tok(EQ, this)]
    pub value: Expr<'input>,
}

/// One entry in a multi-column SET target list — Postgres
/// `set_target: ColId opt_indirection`. The indirection chain admits the
/// same `[idx]`, `[lo:hi]`, and `.field` elements as `SingleAssignment`,
/// so `SET (f2[1], f1, tag) = (...)` (rules.sql) parses cleanly.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetTarget<'input> {
    pub column: literal::Ident<'input>,
    /// Greedy: a leading DOT, LBRACKET starts this element instead of ending `SetTarget` (bison shift preference).
    #[greedy(DOT, LBRACKET)]
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
}

/// Tuple SET assignment: `(col, ...) = expr` — Postgres
/// `'(' set_target_list ')' '=' a_expr`. Each item in the list is a
/// `set_target` (`ColId opt_indirection`), so subscripts and field
/// accessors are admitted on individual columns.
#[derive(recursa::Node, Debug, Clone)]
pub struct TupleAssignment<'input> {
    pub columns: SetTargetList<'input>,
    #[tok(EQ, this)]
    pub values: Expr<'input>,
}

/// Parenthesized, comma-separated targets on the left side of a tuple SET.
///
/// The delimiters wrap the list as a whole. Attaching them to the repeated
/// field would require a fresh pair of parentheses around every target.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct SetTargetList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<SetTarget<'input>>,
);

/// A single SET assignment: `col = expr` or `(col, ...) = (expr, ...)`
///
/// Variant ordering: Tuple starts with `(` which is longer than a bare
/// identifier, so longest-match-wins picks it when parens are present.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetAssignment<'input> {
    Tuple(TupleAssignment<'input>),
    Single(SingleAssignment<'input>),
}

/// RETURNING clause: `RETURNING expr, ...`
#[derive(recursa::Node, Debug, Clone)]
#[tok(RETURNING, this)]
pub struct ReturningClause<'input> {
    /// Greedy: any kind that can start this element continues it instead of ending `ReturningClause` (bison shift preference).
    #[greedy(all)]
    #[sep(COMMA)]
    pub items: Vec<crate::ast::dml::select::SelectItem<'input>>,
}

/// `AS alias` on an UPDATE target table.
#[derive(recursa::Node, Debug, Clone)]
pub struct UpdateTableAliasWithAs<'input> {
    #[tok(AS, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// Required `SET` plus its comma-separated assignments.
///
/// Keeping `SET` on this wrapper makes it occur once for the whole list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(SET, this)]
pub struct SetClause<'input>(
    /// Greedy: a leading ABSENT starts this element instead of ending `SetClause` (bison shift preference).
    #[greedy(ABSENT)]
    #[sep(COMMA)]
    #[pretty(indent)]
    #[deref]
    pub Vec<SetAssignment<'input>>,
);

/// Optional target-table alias before the required SET clause.
///
/// PostgreSQL admits `SET` as a `ColId`, but gives it precedence as the UPDATE
/// clause keyword in this position. The bare-alias admission excludes exactly
/// that keyword; the explicit `AS` form continues to accept it.
#[derive(recursa::Node, Debug, Clone)]
pub enum UpdateTableAlias<'input> {
    WithAs(UpdateTableAliasWithAs<'input>),
    Bare(literal::UpdateAliasName<'input>),
}

impl UpdateTableAlias<'_> {
    /// Raw alias text regardless of whether `AS` was present.
    pub fn name(&self) -> &str {
        match self {
            UpdateTableAlias::WithAs(alias) => alias.name.text(),
            UpdateTableAlias::Bare(name) => name.text(),
        }
    }
}

/// UPDATE statement: `UPDATE [ONLY] table [alias] SET assignments [FROM ...] [WHERE ...] [RETURNING ...]`
///
/// The optional `ONLY` modifier excludes inheritance children — Postgres'
/// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
/// not exercised by any UPDATE corpus statement, so it is not modelled (matches
/// the `TruncateRelation` / `LockRelation` shape).
#[derive(recursa::Node, Debug, Clone)]
#[pretty(group = consistent)]
pub struct UpdateStmt<'input> {
    #[tok(UPDATE, this)]
    #[presence(ONLY)]
    pub only: bool,
    pub table_name: QualifiedName<'input>,
    pub alias: Option<UpdateTableAlias<'input>>,
    #[pretty(break_before = soft)]
    pub assignments: SetClause<'input>,
    #[pretty(break_before = soft)]
    pub from_clause: Option<Box<FromClause<'input>>>,
    #[pretty(break_before = soft)]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    /// Greedy: a leading RETURNING starts this element instead of ending `UpdateStmt` (bison shift preference).
    #[greedy(RETURNING)]
    #[pretty(break_before = soft)]
    pub returning: Option<Box<ReturningClause<'input>>>,
}
