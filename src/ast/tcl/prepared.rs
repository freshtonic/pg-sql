//! PREPARE / EXECUTE / DEALLOCATE statements (including the two-phase-commit
//! `PREPARE TRANSACTION 'gid'` form).

use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::TypeNameList;
use crate::tokens::literal;

// --- PREPARE / EXECUTE / DEALLOCATE ---

/// A statement that can be the body of `PREPARE name AS ...` — Postgres'
/// `PreparableStmt`: `SELECT | INSERT | UPDATE | DELETE | MERGE`.
///
/// `Query` uses `Subquery`, which already models Postgres' full `SelectStmt`
/// grammar (`SELECT`, set operations, `VALUES`, `TABLE`, and `WITH`). The
/// other four variants have disjoint leading keywords, so variant order does
/// not affect disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum PreparableStmt<'input> {
    Query(Box<crate::ast::dml::values::Subquery<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Merge(Box<crate::ast::dml::merge::MergeStmt<'input>>),
}

/// `( typename [, ...] )` parameter-type list on a `PREPARE` statement
/// (`prep_type_clause` in `gram.y`).
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareTypes<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub types: TypeNameList<'input>,
}

/// Body of a standard `PREPARE name [(types)] AS stmt` statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareStandardBody<'input> {
    pub name: literal::AliasName<'input>,
    pub types: Option<PrepareTypes<'input>>,
    #[tok(AS, this)]
    pub body: PreparableStmt<'input>,
}

/// `PREPARE TRANSACTION 'gid'` — the two-phase-commit transaction-prepare
/// form (`gram.y::TransactionStmt: PREPARE TRANSACTION Sconst`). Distinct
/// from the `PREPARE name … AS stmt` form modelled by
/// [`PrepareStandardBody`].
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareTransactionBody<'input> {
    #[tok(TRANSACTION, this)]
    pub gid: literal::StringLit<'input>,
}

/// Body of a `PREPARE` statement.
///
/// Variant ordering: both variants share a 1-token first-set. `Transaction`
/// matches only the `TRANSACTION` keyword; `Standard` matches any word
/// (`AliasName::Bare` accepts every keyword including `TRANSACTION` — see
/// `tokens::literal::BareAliasName::peek`). Declaration order is the
/// tiebreaker, so `Transaction` must come first to win when the source is
/// literally `PREPARE TRANSACTION 'gid'`.
#[derive(recursa::Node, Debug, Clone)]
pub enum PrepareStmtBody<'input> {
    Transaction(PrepareTransactionBody<'input>),
    Standard(PrepareStandardBody<'input>),
}

/// ```sql
/// PREPARE name [ (typename [, ...]) ] AS PreparableStmt
/// PREPARE TRANSACTION 'gid'
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareStmt<'input> {
    #[tok(PREPARE, this)]
    pub body: PrepareStmtBody<'input>,
}

/// `( expr [, ...] )` argument list on an `EXECUTE` statement
/// (`execute_param_clause` in `gram.y`).
#[derive(recursa::Node, Debug, Clone)]
pub struct ExecuteParams<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params: Vec<Expr<'input>>,
}

/// ```sql
/// EXECUTE name [ (expr [, ...]) ]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct ExecuteStmt<'input> {
    #[tok(EXECUTE, this)]
    pub name: literal::AliasName<'input>,
    pub params: Option<ExecuteParams<'input>>,
}

/// Target of a `DEALLOCATE` statement: a named prepared statement or `ALL`.
///
/// Variant ordering: `All` (the `ALL` keyword) before `Name` so the reserved
/// word is not swallowed as a statement name.
#[derive(recursa::Node, Debug, Clone)]
pub enum DeallocateTarget<'input> {
    #[tok(ALL)]
    All,
    Name(literal::Ident<'input>),
}

/// ```sql
/// DEALLOCATE [PREPARE] { name | ALL }
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct DeallocateStmt<'input> {
    #[tok(DEALLOCATE, this)]
    #[presence(PREPARE)]
    pub prepare: bool,
    pub target: DeallocateTarget<'input>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/tcl/prepared.tests.rs"
));
