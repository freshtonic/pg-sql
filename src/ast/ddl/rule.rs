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
#[derive(recursa::Node, Debug)]
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
#[derive(recursa::Node, Debug)]
pub enum RuleInsteadAlso {
    #[tok(INSTEAD)]
    Instead,
    #[tok(ALSO)]
    Also,
}

/// `WHERE expr` clause on a rule. Postgres allows any `a_expr`.
#[derive(recursa::Node, Debug)]
pub struct RuleWhereClause<'input> {
    #[tok(WHERE, this)]
    pub expr: recursa::ArenaBox<'input, Expr<'input>>,
}

/// A single statement that may appear as a rule action — gram.y
/// `RuleActionStmt`: `SelectStmt`, `InsertStmt`, `UpdateStmt`, `DeleteStmt`
/// or `NotifyStmt`. gram.y gives the three DML statements an
/// `opt_with_clause` and `SelectStmt` its `with_clause`; pg-sql factors that
/// prefix once as `With`, so a `WITH`-led action is decided after the CTE
/// list.
///
/// Each variant boxes its underlying statement type so the parent
/// `RuleActions::Single` enum stays small. We reuse the existing statement
/// AST types directly.
///
/// Query actions deliberately exclude a parenthesized outer query because
/// parentheses at this level delimit a multi-action rule list.
#[derive(recursa::Node, Debug)]
pub enum RuleActionStmt<'input> {
    Query(recursa::ArenaBox<'input, RuleQuery<'input>>),
    With(recursa::ArenaBox<'input, RuleWithAction<'input>>),
    Insert(recursa::ArenaBox<'input, crate::ast::dml::insert::InsertStmt<'input>>),
    Update(recursa::ArenaBox<'input, crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(recursa::ArenaBox<'input, crate::ast::dml::delete::DeleteStmt<'input>>),
    Notify(NotifyStmt<'input>),
}

/// gram.y `with_clause` followed by the rule action it prefixes: a query
/// (`select_no_parens`), `insert_rest`, `update` or `delete`. `MERGE` is
/// not a rule action in gram.y, so this is not `WithStatement`.
#[derive(recursa::Node, Debug)]
pub struct RuleWithAction<'input> {
    pub with_clause: crate::ast::shared::with_clause::WithClause<'input>,
    pub body: RuleWithBody<'input>,
}

/// The statement after a rule action's `WITH` clause.
///
/// Variant ordering: `Query` leads with `SELECT`, `VALUES` or `TABLE`; the
/// three DML variants have disjoint leading keywords.
#[derive(recursa::Node, Debug)]
pub enum RuleWithBody<'input> {
    Query(recursa::ArenaBox<'input, RuleQuery<'input>>),
    Insert(recursa::ArenaBox<'input, crate::ast::dml::insert::InsertStmt<'input>>),
    Update(recursa::ArenaBox<'input, crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(recursa::ArenaBox<'input, crate::ast::dml::delete::DeleteStmt<'input>>),
}

/// gram.y `select_no_parens` less its `with_clause`: a non-parenthesized
/// `select_clause` with its `opt_sort_clause`, `select_limit` and
/// `for_locking_clause` tails.
#[derive(recursa::Node, Debug)]
pub struct RuleQuery<'input> {
    pub clause: RuleSelectClause<'input>,
    #[pretty(break_before = soft)]
    pub order_by: Option<recursa::ArenaBox<'input, crate::ast::dml::select::OrderByClause<'input>>>,
    #[pretty(break_before = soft)]
    pub limit_offset:
        Option<recursa::ArenaBox<'input, crate::ast::dml::select::LimitOffsetClause<'input>>>,
    #[pretty(break_before = soft)]
    pub for_update:
        Option<recursa::ArenaBox<'input, crate::ast::dml::select::ForUpdateClause<'input>>>,
}

/// Non-parenthesized query forms accepted as a single rule action.
#[derive(recursa::Node, Debug)]
pub enum RuleSelectClause<'input> {
    Table(crate::ast::dml::values::TableStmt<'input>),
    Body(crate::ast::dml::values::CompoundBody<'input>),
}

/// Rule actions — Postgres' `RuleActionList`:
/// `NOTHING | RuleActionStmt | '(' RuleActionMulti ')'`.
///
/// Variant ordering: distinct first tokens (`NOTHING` keyword, `(` punct, or
/// statement-leading keyword) so disambiguation is unambiguous.
#[derive(recursa::Node, Debug)]
pub enum RuleActions<'input> {
    #[tok(NOTHING)]
    Nothing,
    /// `'(' stmt; stmt; … ')'` — RuleActionMulti, accepting empty statements
    /// between semicolons. We use `Seq0` with `Semi` separator and an
    /// optional trailing separator so `(stmt;)` and `(stmt; stmt;)` both
    /// round-trip.
    Multi(RuleActionList<'input>),
    Single(recursa::ArenaBox<'input, RuleActionStmt<'input>>),
}

/// Parenthesized, semicolon-separated rule action list.
#[derive(recursa::Node, Debug)]
#[tok(LPAREN, this, RPAREN)]
pub struct RuleActionList<'input> {
    #[sep(SEMI, trailing)]
    pub actions: recursa::ArenaVec<'input, RuleActionStmt<'input>>,
}

/// Required `DO [INSTEAD | ALSO] actions` tail of a `CREATE RULE` statement.
///
/// The `DO` keyword belongs to this required wrapper rather than either child:
/// attaching it to the optional modifier would make `DO` optional, while
/// attaching it to `actions` would place it after the modifier.
#[derive(recursa::Node, Debug)]
#[tok(DO, this)]
pub struct RuleDoClause<'input> {
    pub instead_also: Option<RuleInsteadAlso>,
    pub actions: RuleActions<'input>,
}

/// `CREATE [OR REPLACE] RULE name AS ON event TO qualified_name [WHERE expr]
/// DO [INSTEAD|ALSO] RuleActionList` — Postgres' `RuleStmt`.
#[derive(recursa::Node, Debug)]
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
#[derive(recursa::Node, Debug)]
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
#[derive(recursa::Node, Debug)]
pub struct AlterRuleStmt<'input> {
    #[tok(ALTER, RULE, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub rename_to: RenameTo<'input>,
}
