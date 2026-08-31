/// MERGE statement AST.
///
/// ```sql
/// MERGE INTO [ONLY] target [[AS] alias]
/// USING source [[AS] alias] ON condition
/// WHEN [NOT] MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN
///     { UPDATE SET ... | DELETE | DO NOTHING
///     | INSERT [INTO target] [(cols)] { VALUES (...) [, (...)] | DEFAULT VALUES } }
/// [RETURNING ...]
/// ```
use crate::ast::dml::select::{PlainTable, TableRef};
use crate::ast::dml::update::{ReturningClause, SetAssignment};
use crate::ast::shared::expr::Expr;
use crate::tokens::literal;

/// `AND cond` qualifier on a WHEN clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct AndCondition<'input> {
    #[tok(AND, this)]
    pub condition: Expr<'input>,
}

/// `BY SOURCE` or `BY TARGET`.
#[derive(recursa::Node, Debug, Clone)]
pub enum NotMatchedBy {
    #[tok(BY, SOURCE)]
    Source,
    #[tok(BY, TARGET)]
    Target,
}

/// `UPDATE SET col = expr, ...` action body (the part after THEN).
#[derive(recursa::Node, Debug, Clone)]
pub struct UpdateAction<'input> {
    #[tok(UPDATE, SET, this)]
    #[sep(COMMA)]
    pub assignments: Vec<SetAssignment<'input>>,
}

/// Action allowed after `WHEN MATCHED ... THEN`.
///
/// Variant ordering: `DoNothing` (`DO NOTHING`) and `Update` (`UPDATE`) and
/// `Delete` (`DELETE`) all start with distinct keywords, so order is by
/// declaration only.
#[derive(recursa::Node, Debug, Clone)]
pub enum MatchedAction<'input> {
    Update(UpdateAction<'input>),
    #[tok(DELETE)]
    Delete,
    #[tok(DO, NOTHING)]
    DoNothing,
}

/// A single row of values: `(expr, ...)`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct ValueRow<'input>(#[sep(COMMA)] pub Vec<Expr<'input>>);

/// `VALUES (row), (row), ...` body.
#[derive(recursa::Node, Debug, Clone)]
#[tok(VALUES, this)]
pub struct InsertValuesBody<'input> {
    #[sep(COMMA)]
    pub rows: Vec<ValueRow<'input>>,
}

/// Body of an INSERT inside MERGE: `VALUES ...` or `DEFAULT VALUES`.
///
/// Variant ordering: `Default` (`DEFAULT VALUES`) is matched before
/// `Values` (`VALUES`) since they begin with different keywords.
#[derive(recursa::Node, Debug, Clone)]
pub enum InsertBody<'input> {
    #[tok(DEFAULT, VALUES)]
    Default,
    Values(InsertValuesBody<'input>),
}

/// Optional `INTO target_name` after `INSERT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct InsertInto<'input> {
    #[tok(INTO, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `INSERT [INTO target] [(cols)] { VALUES ... | DEFAULT VALUES }`
#[derive(recursa::Node, Debug, Clone)]
#[tok(INSERT, this)]
pub struct InsertAction<'input> {
    pub into: Option<InsertInto<'input>>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<recursa::Vec1<literal::AliasName<'input>>>,
    /// `OVERRIDING {SYSTEM|USER} VALUE` between the columns and the body.
    pub overriding: Option<crate::ast::dml::insert::OverridingClause>,
    pub body: InsertBody<'input>,
}

/// Action allowed after `WHEN NOT MATCHED ... THEN`.
///
/// `WHEN NOT MATCHED [BY TARGET]` takes an `INSERT` (or `DO NOTHING`);
/// `WHEN NOT MATCHED BY SOURCE` takes an `UPDATE` / `DELETE` instead
/// (the target row exists, the source row does not). Which `by` form
/// permits which action is a semantic rule, so all four are accepted
/// grammatically.
#[derive(recursa::Node, Debug, Clone)]
pub enum NotMatchedAction<'input> {
    Insert(InsertAction<'input>),
    Update(UpdateAction<'input>),
    #[tok(DELETE)]
    Delete,
    #[tok(DO, NOTHING)]
    DoNothing,
}

/// `WHEN NOT MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN action`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WHEN, NOT, MATCHED, this)]
pub struct WhenNotMatched<'input> {
    pub by: Option<NotMatchedBy>,
    pub and: Option<AndCondition<'input>>,
    #[tok(THEN, this)]
    pub action: NotMatchedAction<'input>,
}

/// `WHEN MATCHED [AND cond] THEN action`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WHEN, MATCHED, this)]
pub struct WhenMatched<'input> {
    pub and: Option<AndCondition<'input>>,
    #[tok(THEN, this)]
    pub action: MatchedAction<'input>,
}

/// A WHEN clause in MERGE.
///
/// Variant ordering: `NotMatched` (`WHEN NOT MATCHED`) is longer than
/// `Matched` (`WHEN MATCHED`); list it first.
#[derive(recursa::Node, Debug, Clone)]
pub enum WhenClause<'input> {
    NotMatched(WhenNotMatched<'input>),
    Matched(WhenMatched<'input>),
}

/// MERGE statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct MergeStmt<'input> {
    #[tok(MERGE, INTO, this)]
    pub target: Box<PlainTable<'input>>,
    #[tok(USING, this)]
    pub source: Box<TableRef<'input>>,
    #[tok(ON, this)]
    pub condition: Box<Expr<'input>>,
    /// PostgreSQL's `merge_when_list` is one-or-more.
    pub when_clauses: recursa::Vec1<WhenClause<'input>>,
    pub returning: Option<Box<ReturningClause<'input>>>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/dml/merge.tests.rs"
));
