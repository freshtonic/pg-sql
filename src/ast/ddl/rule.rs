//! RULE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::session::notify::NotifyStmt;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// Rule event — Postgres' `event` rule (a strict subset of trigger events):
/// `SELECT | INSERT | UPDATE | DELETE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleEvent {
    #[tok(SELECT)]
    Select,
    #[tok(INSERT)]
    Insert,
    #[tok(UPDATE)]
    Update,
    #[tok(DELETE)]
    Delete,
}

/// `INSTEAD | ALSO` — Postgres' `opt_instead`. Either keyword is optional;
/// when absent the rule fires alongside the original command (`ALSO`).
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleInsteadAlso {
    #[tok(INSTEAD)]
    Instead,
    #[tok(ALSO)]
    Also,
}

/// `WHERE expr` clause on a rule. Postgres allows any `a_expr`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RuleWhereClause<'input> {
    #[tok(WHERE, this)]
    pub expr: Box<Expr<'input>>,
}

/// A single statement that may appear as a rule action — Postgres'
/// `RuleActionStmt`: SELECT, INSERT, UPDATE, DELETE, or NOTIFY.
///
/// Each variant boxes its underlying statement type so the parent
/// `RuleActions::Single` enum stays small. We reuse the existing statement
/// AST types directly.
///
/// PG accepts a few additional forms in a rule body that gram.y models
/// elsewhere but that the rules.sql / with.sql regression corpora exercise:
/// - `WITH cte AS (...) {SELECT|INSERT|UPDATE|DELETE} ...` — a
///   CTE-prefixed statement. Query-shaped `WITH` actions use the same
///   `RuleQuery::Body` / `SelectBody::WithBody` path as other query bodies.
/// - `VALUES (row), (row), ...` — a bare values clause, represented by the
///   same consolidated query shape used by `Statement::Query`.
///
/// Query actions deliberately exclude a parenthesized outer query because
/// parentheses at this level delimit a multi-action rule list.
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleActionStmt<'input> {
    Query(Box<RuleQuery<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Notify(NotifyStmt<'input>),
}

/// Non-parenthesized query forms accepted as a single rule action.
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleQuery<'input> {
    Table(crate::ast::dml::values::TableStmt<'input>),
    Body(crate::ast::dml::values::CompoundBody<'input>),
}

/// Rule actions — Postgres' `RuleActionList`:
/// `NOTHING | RuleActionStmt | '(' RuleActionMulti ')'`.
///
/// Variant ordering: distinct first tokens (`NOTHING` keyword, `(` punct, or
/// statement-leading keyword) so disambiguation is unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleActions<'input> {
    #[tok(NOTHING)]
    Nothing,
    /// `'(' stmt; stmt; … ')'` — RuleActionMulti, accepting empty statements
    /// between semicolons. We use `Seq0` with `Semi` separator and an
    /// optional trailing separator so `(stmt;)` and `(stmt; stmt;)` both
    /// round-trip.
    Multi(RuleActionList<'input>),
    Single(Box<RuleActionStmt<'input>>),
}

/// Parenthesized, semicolon-separated rule action list.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct RuleActionList<'input> {
    #[sep(SEMI, trailing)]
    pub actions: Vec<RuleActionStmt<'input>>,
}

/// Required `DO [INSTEAD | ALSO] actions` tail of a `CREATE RULE` statement.
///
/// The `DO` keyword belongs to this required wrapper rather than either child:
/// attaching it to the optional modifier would make `DO` optional, while
/// attaching it to `actions` would place it after the modifier.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DO, this)]
pub struct RuleDoClause<'input> {
    pub instead_also: Option<RuleInsteadAlso>,
    pub actions: RuleActions<'input>,
}

/// `CREATE [OR REPLACE] RULE name AS ON event TO qualified_name [WHERE expr]
/// DO [INSTEAD|ALSO] RuleActionList` — Postgres' `RuleStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateRuleStmt<'input> {
    #[tok(CREATE, this, RULE)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub name: crate::tokens::ColId<'input>,
    #[tok(AS, ON, this)]
    pub event: RuleEvent,
    #[tok(TO, this)]
    pub table: QualifiedName<'input>,
    pub where_clause: Option<RuleWhereClause<'input>>,
    pub do_clause: RuleDoClause<'input>,
}

/// `DROP RULE [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, RULE, this)]
pub struct DropRuleStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `ALTER RULE name ON qualified_name RENAME TO new` — Postgres'
/// `RenameStmt` branch for rules. Rules have no OWNER / SET SCHEMA
/// actions in gram.y, so RENAME is the only branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterRuleStmt<'input> {
    #[tok(ALTER, RULE, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub rename_to: RenameTo<'input>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/rule.tests.rs"
));
