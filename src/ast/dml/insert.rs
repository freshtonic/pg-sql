/// INSERT INTO statement AST.
///
/// Supports: `INSERT INTO table [(cols)] source [ON CONFLICT ...] [RETURNING ...]`
/// where source is DEFAULT VALUES, VALUES rows, or SELECT query.
use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::{ReturningClause, SetAssignment};
use crate::ast::dml::values::Subquery;
use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;

/// `[AS] alias` on INSERT target table, e.g. `INSERT INTO t AS x`.
#[derive(recursa::Node, Debug, Clone)]
pub struct InsertTableAlias<'input> {
    #[tok(AS, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `OVERRIDING {SYSTEM | USER} VALUE` clause on an INSERT statement.
///
/// Variant ordering: distinct first tokens (`SYSTEM` vs `USER`), so
/// declaration order is cosmetic.
#[derive(recursa::Node, Debug, Clone)]
pub struct OverridingClause {
    #[tok(OVERRIDING, this, VALUE)]
    pub which: OverridingKind,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum OverridingKind {
    #[tok(SYSTEM)]
    System,
    #[tok(USER)]
    User,
}

/// Multiple value rows: `VALUES (row1), (row2), ...`
#[derive(recursa::Node, Debug, Clone)]
#[tok(VALUES, this)]
pub struct InsertValueRows<'input> {
    #[sep(COMMA)]
    pub rows: Vec<ValueList<'input>>,
}

/// Insert source: `DEFAULT VALUES` or a query.
///
/// PostgreSQL's query form includes `VALUES`, so the `Subquery` AST retains
/// the distinction between SELECT, VALUES, TABLE, and parenthesized sources
/// without declaring the VALUES language twice at this enum boundary.
#[derive(recursa::Node, Debug, Clone)]
pub enum InsertSource<'input> {
    #[tok(DEFAULT, VALUES)]
    Default,
    Select(Box<Subquery<'input>>),
}

/// DO UPDATE SET ... [WHERE ...] action.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DO, UPDATE, SET, this)]
pub struct DoUpdateAction<'input> {
    #[sep(COMMA)]
    pub assignments: recursa::Vec1<SetAssignment<'input>>,
    pub where_clause: Option<WhereClause<'input>>,
}

/// ON CONFLICT action: DO UPDATE SET ... [WHERE ...] or DO NOTHING.
///
/// Variant ordering: DoUpdate (`DO UPDATE SET`) is longer than
/// DoNothing (`DO NOTHING`), but both start with `DO` and diverge
/// at the next keyword, so the regex disambiguates.
#[derive(recursa::Node, Debug, Clone)]
pub enum ConflictAction<'input> {
    DoUpdate(Box<DoUpdateAction<'input>>),
    #[tok(DO, NOTHING)]
    DoNothing,
}

/// One entry in an `ON CONFLICT (...)` target list.
///
/// Matches the index-element grammar: an expression (plain column name,
/// qualified name, parenthesized expression, or function call) optionally
/// followed by a `COLLATE "name"` clause and an optional opclass ident.
#[derive(recursa::Node, Debug, Clone)]
pub struct ConflictTargetItem<'input> {
    pub target: crate::ast::ddl::index::IndexTarget<'input>,
    pub collate: Option<crate::ast::ddl::table::CollateClause<'input>>,
    pub opclass: Option<crate::tokens::ColId<'input>>,
}

/// `ON CONSTRAINT name` arbiter form of `opt_conf_expr` — names a unique
/// or exclusion constraint directly instead of inferring from column list.
/// Per gram.y `opt_conf_expr: ON CONSTRAINT name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OnConflictConstraint<'input> {
    #[tok(ON, CONSTRAINT, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// Parenthesized `index_params` arbiter list on `ON CONFLICT`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ConflictTargetList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<ConflictTargetItem<'input>>,
);

/// Arbiter specification for `ON CONFLICT` — Postgres' `opt_conf_expr`.
///
/// Variant ordering: each variant has a distinct first token (`(` vs `ON`),
/// so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum ConflictTarget<'input> {
    /// `( index_params )` — the inferring-by-columns form.
    Index(ConflictTargetList<'input>),
    /// `ON CONSTRAINT name` — the named-constraint form.
    Constraint(OnConflictConstraint<'input>),
}

/// ON CONFLICT clause: `ON CONFLICT [(col, ...) | ON CONSTRAINT name]
/// DO UPDATE SET ... | DO NOTHING`
#[derive(recursa::Node, Debug, Clone)]
#[tok(ON, CONFLICT, this)]
pub struct OnConflictClause<'input> {
    pub target: Option<ConflictTarget<'input>>,
    /// `WHERE predicate` after the arbiter target list, restricting the
    /// partial-index arbiter to matching rows. Only valid for the
    /// index-params form (gram.y attaches `where_clause` to that branch
    /// only); attached at the outer struct so the enum stays simple.
    pub where_clause: Option<WhereClause<'input>>,
    pub action: ConflictAction<'input>,
}

/// INSERT INTO statement with optional ON CONFLICT and RETURNING.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(group = consistent)]
pub struct InsertStmt<'input> {
    #[tok(INSERT, INTO, this)]
    pub table_name: QualifiedName<'input>,
    /// Optional `[AS] alias` after the target table, used to rebind the
    /// target in ON CONFLICT DO UPDATE expressions.
    pub alias: Option<InsertTableAlias<'input>>,
    pub columns: Option<Box<ColumnList<'input>>>,
    /// `OVERRIDING {SYSTEM|USER} VALUE` between the column list and the
    /// source. Controls whether explicit values override GENERATED ALWAYS
    /// identity columns.
    pub overriding: Option<OverridingClause>,
    #[pretty(break_before = soft)]
    pub source: Box<InsertSource<'input>>,
    /// Greedy: a leading ON starts this element instead of ending `InsertStmt` (bison shift preference).
    #[greedy(ON)]
    #[pretty(break_before = soft)]
    pub on_conflict: Option<Box<OnConflictClause<'input>>>,
    /// Greedy: a leading RETURNING starts this element instead of ending `InsertStmt` (bison shift preference).
    #[greedy(RETURNING)]
    #[pretty(break_before = soft)]
    pub returning: Option<Box<ReturningClause<'input>>>,
}

/// One target column of an INSERT column list: a column name plus an
/// optional indirection chain — `f2[1]`, `f3.if1`, `a[1:5]` (Postgres
/// `insert_column_item: ColId opt_indirection`).
#[derive(recursa::Node, Debug, Clone)]
pub struct InsertColumnItem<'input> {
    pub name: crate::tokens::ColId<'input>,
    /// Greedy: a leading DOT, LBRACKET starts this element instead of ending `InsertColumnItem` (bison shift preference).
    #[greedy(DOT, LBRACKET)]
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
}

/// Column list: `(col1, col2[1], col3.field, ...)`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<InsertColumnItem<'input>>,
);

/// Value list: `(col1, col2, ...)`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ValueList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<Expr<'input>>,
);
