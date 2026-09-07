//! TRIGGER DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `BEFORE | AFTER | INSTEAD OF` — Postgres' `TriggerActionTime`.
///
/// Variant ordering: multi-word `InsteadOf` first so the longer match wins
/// when `INSTEAD` is followed by `OF`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerActionTime {
    #[tok(INSTEAD, OF)]
    InsteadOf,
    #[tok(BEFORE)]
    Before,
    #[tok(AFTER)]
    After,
}

/// `UPDATE OF col[, col …]` — column list following an UPDATE trigger event.
#[derive(recursa::Node, Debug, Clone)]
#[tok(OF, this)]
pub struct TriggerUpdateOfColumns<'input> {
    #[sep(COMMA)]
    pub columns: recursa::Vec1<crate::tokens::ColId<'input>>,
}

/// `UPDATE [OF cols]` — UPDATE trigger event with optional column list.
#[derive(recursa::Node, Debug, Clone)]
#[tok(UPDATE, this)]
pub struct TriggerUpdateEvent<'input> {
    pub of: Option<TriggerUpdateOfColumns<'input>>,
}

/// One trigger event — Postgres' `TriggerOneEvent`:
/// `INSERT | DELETE | UPDATE [OF cols] | TRUNCATE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerOneEvent<'input> {
    #[tok(INSERT)]
    Insert,
    #[tok(DELETE)]
    Delete,
    Update(TriggerUpdateEvent<'input>),
    #[tok(TRUNCATE)]
    Truncate,
}

/// One or more trigger events separated by `OR`.
///
/// The wrapper owns the separator policy for the list as a whole. Keeping the
/// repeated field directly on each statement can make generated formatting
/// concatenate a unit event and its following separator (for example,
/// `INSERTOR`).
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerEventList<'input> {
    #[sep(OR)]
    pub events: recursa::Vec1<TriggerOneEvent<'input>>,
}

/// `ROW | STATEMENT` — granularity selector after `FOR [EACH]`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerForType {
    #[tok(ROW)]
    Row,
    #[tok(STATEMENT)]
    Statement,
}

/// `FOR [EACH] {ROW | STATEMENT}` — Postgres' `TriggerForSpec`. When omitted
/// PG defaults to `STATEMENT`, but we preserve absence in the AST so the
/// formatter round-trips source verbatim.
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerForSpec {
    #[tok(FOR, optional(EACH), this)]
    pub kind: TriggerForType,
}

/// `WHEN (expr)` — Postgres' `TriggerWhen` clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerWhenClause<'input> {
    #[tok(WHEN, LPAREN, this, RPAREN)]
    pub expr: Box<Expr<'input>>,
}

/// `NEW | OLD` — Postgres' `TransitionOldOrNew`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransitionOldOrNew {
    #[tok(OLD)]
    Old,
    #[tok(NEW)]
    New,
}

/// `ROW | TABLE` — Postgres' `TransitionRowOrTable`. ROW is permitted by
/// gram.y though semantically only TABLE makes sense for transition tables.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransitionRowOrTable {
    #[tok(TABLE)]
    Table,
    #[tok(ROW)]
    Row,
}

/// A single `REFERENCING` transition: `{OLD|NEW} {TABLE|ROW} [AS] name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerTransition<'input> {
    pub old_or_new: TransitionOldOrNew,
    pub row_or_table: TransitionRowOrTable,
    #[tok(optional(AS), this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `REFERENCING transition+` — one or more transition-table clauses.
#[derive(recursa::Node, Debug, Clone)]
#[tok(REFERENCING, this)]
pub struct TriggerReferencing<'input> {
    pub transitions: Vec<TriggerTransition<'input>>,
}

/// `FUNCTION | PROCEDURE` — Postgres' `FUNCTION_or_PROCEDURE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionOrProcedure {
    #[tok(FUNCTION)]
    Function,
    #[tok(PROCEDURE)]
    Procedure,
}

/// A single trigger function argument — Postgres' `TriggerFuncArg`:
/// integer, numeric, string, or ColLabel (identifier-or-keyword) literal.
///
/// Variant ordering: numeric forms before integer (NumericLit longest-match
/// wins on `.` / `e`); literal `StringLit` before identifier `AliasName`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerFuncArg<'input> {
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    String(literal::StringLit<'input>),
    Ident(literal::AliasName<'input>),
}

/// `(arg, …)` argument list passed to the trigger's EXECUTE FUNCTION/PROCEDURE.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct TriggerExecArgs<'input> {
    #[sep(COMMA)]
    pub args: Vec<TriggerFuncArg<'input>>,
}

/// `EXECUTE {FUNCTION | PROCEDURE} func_name(args)` — Postgres' trigger
/// action clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerExecuteClause<'input> {
    #[tok(EXECUTE, this)]
    pub kind: FunctionOrProcedure,
    pub func_name: QualifiedName<'input>,
    pub args: TriggerExecArgs<'input>,
}

/// `CREATE [OR REPLACE] TRIGGER name {BEFORE|AFTER|INSTEAD OF} events ON
/// qualified_name [REFERENCING …] [FOR [EACH] {ROW|STATEMENT}] [WHEN (expr)]
/// EXECUTE {FUNCTION|PROCEDURE} func_name(args)` — Postgres' `CreateTrigStmt`
/// (non-constraint form).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTriggerStmt<'input> {
    #[tok(CREATE, this, TRIGGER)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub name: crate::tokens::ColId<'input>,
    pub timing: TriggerActionTime,
    pub events: TriggerEventList<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub referencing: Option<TriggerReferencing<'input>>,
    pub for_spec: Option<TriggerForSpec>,
    pub when_clause: Option<TriggerWhenClause<'input>>,
    pub execute_clause: TriggerExecuteClause<'input>,
}

/// `[NO] DEPENDS ON EXTENSION name` — Postgres' `AlterObjectDependsStmt`
/// action shared by ALTER TRIGGER / ALTER MATERIALIZED VIEW / ALTER INDEX
/// (and several others). The optional `NO` toggles whether the extension
/// dependency is added (`DEPENDS ...`) or removed (`NO DEPENDS ...`).
#[derive(recursa::Node, Debug, Clone)]
pub struct DependsOnExtension<'input> {
    #[tok(this, DEPENDS, ON, EXTENSION)]
    #[presence(NO)]
    pub no: bool,
    pub name: crate::tokens::ColId<'input>,
}

/// One action on `ALTER TRIGGER name ON qualified_name action` — Postgres'
/// `RenameStmt` and `AlterObjectDependsStmt` branches for triggers.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`RENAME` / `NO` / `DEPENDS`), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTriggerAction<'input> {
    Rename(RenameTo<'input>),
    Depends(DependsOnExtension<'input>),
}

/// `ALTER TRIGGER name ON qualified_name { RENAME TO new |
/// [NO] DEPENDS ON EXTENSION name }` — Postgres' `RenameStmt` and
/// `AlterObjectDependsStmt` branches for triggers. There is no OWNER /
/// SET SCHEMA / ENABLE branch on triggers in gram.y.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTriggerStmt<'input> {
    #[tok(ALTER, TRIGGER, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub action: AlterTriggerAction<'input>,
}

/// `FROM qualified_name` — Postgres' `OptConstrFromTable` (the referenced
/// table on a constraint trigger).
#[derive(recursa::Node, Debug, Clone)]
pub struct ConstrFromTable<'input> {
    #[tok(FROM, this)]
    pub table: QualifiedName<'input>,
}

/// A single `ConstraintAttributeElem` — one of
/// `NOT DEFERRABLE | DEFERRABLE | INITIALLY IMMEDIATE | INITIALLY DEFERRED`.
///
/// The `NOT VALID` / `NO INHERIT` forms are also in gram.y but never appear
/// on a CONSTRAINT TRIGGER in practice; PG accepts them syntactically. We
/// include them so the union matches gram.y faithfully.
///
/// Variant ordering: longer/multi-keyword forms first
/// (`NOT DEFERRABLE`/`NOT VALID`/`INITIALLY …`/`NO INHERIT`).
#[derive(recursa::Node, Debug, Clone)]
pub enum ConstraintAttributeElem {
    #[tok(NOT, DEFERRABLE)]
    NotDeferrable,
    #[tok(NOT, VALID)]
    NotValid,
    #[tok(NO, INHERIT)]
    NoInherit,
    #[tok(INITIALLY, IMMEDIATE)]
    InitiallyImmediate,
    #[tok(INITIALLY, DEFERRED)]
    InitiallyDeferred,
    #[tok(DEFERRABLE)]
    Deferrable,
}

/// `CREATE [OR REPLACE] CONSTRAINT TRIGGER name AFTER events ON table
/// [FROM ref_table] ConstraintAttributeSpec FOR EACH ROW [WHEN (expr)]
/// EXECUTE {FUNCTION|PROCEDURE} func_name(args)` — Postgres'
/// `CreateTrigStmt` (constraint form).
///
/// PG rejects `OR REPLACE` semantically here, but gram.y accepts it; we
/// mirror the grammar.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateConstraintTriggerStmt<'input> {
    #[tok(CREATE, this, CONSTRAINT, TRIGGER)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    #[tok(this, AFTER)]
    pub name: crate::tokens::ColId<'input>,
    pub events: TriggerEventList<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub from_table: Option<ConstrFromTable<'input>>,
    pub constraint_attrs: Vec<ConstraintAttributeElem>,
    pub for_each_row: ForEachRow,
    pub when_clause: Option<TriggerWhenClause<'input>>,
    pub execute_clause: TriggerExecuteClause<'input>,
}

/// Mandatory row-level marker on a constraint trigger.
#[derive(recursa::Node, Debug, Clone)]
pub enum ForEachRow {
    #[tok(FOR, EACH, ROW)]
    Value,
}

/// `DROP TRIGGER [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, TRIGGER, this)]
pub struct DropTriggerStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One or more event-trigger filter values enclosed in parentheses.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct EventTriggerValueList<'input> {
    #[sep(COMMA)]
    pub values: recursa::Vec1<literal::StringLit<'input>>,
}

/// A single `event_trigger_when_item`: `tag IN ('a', 'b', …)`. The
/// filter-tag name is a `ColId` (identifier or unreserved keyword); the
/// values are `Sconst` (single-quoted strings).
#[derive(recursa::Node, Debug, Clone)]
pub struct EventTriggerWhenItem<'input> {
    #[tok(this, IN)]
    pub tag: literal::AliasName<'input>,
    pub values: EventTriggerValueList<'input>,
}

/// `WHEN item AND item AND …` — Postgres' `event_trigger_when_list`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WHEN, this)]
pub struct EventTriggerWhenClause<'input> {
    #[sep(AND)]
    pub items: recursa::Vec1<EventTriggerWhenItem<'input>>,
}

/// `CREATE EVENT TRIGGER name ON event_name [WHEN filters]
/// EXECUTE {FUNCTION|PROCEDURE} func_name()` — Postgres' `CreateEventTrigStmt`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(this, RPAREN)]
pub struct CreateEventTriggerStmt<'input> {
    #[tok(CREATE, EVENT, TRIGGER, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    /// The event name (e.g. `sql_drop`, `ddl_command_start`) is a `ColLabel`
    /// in gram.y — any identifier-or-keyword.
    pub event_name: literal::AliasName<'input>,
    pub when_filters: Option<EventTriggerWhenClause<'input>>,
    #[tok(EXECUTE, this)]
    pub kind: FunctionOrProcedure,
    #[tok(this, LPAREN)]
    pub func_name: QualifiedName<'input>,
    #[sep(COMMA)]
    /// `()` — event triggers never take arguments; the list is empty for
    /// PostgreSQL-valid inputs.
    pub args: Vec<TriggerFuncArg<'input>>,
}

/// `DROP EVENT TRIGGER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, EVENT, TRIGGER, this)]
pub struct DropEventTriggerStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `enable_trigger` — Postgres' four-way enable/disable toggle on an event
/// trigger (and on regular triggers in ALTER TABLE).
///
/// Variant ordering: the two-token `ENABLE REPLICA` / `ENABLE ALWAYS` forms
/// come before bare `ENABLE` so longest-match-wins picks the longer spelling
/// first. `DISABLE` is keyword-disjoint, so its position is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum EnableTrigger {
    #[tok(ENABLE, REPLICA)]
    EnableReplica,
    #[tok(ENABLE, ALWAYS)]
    EnableAlways,
    #[tok(ENABLE)]
    Enable,
    #[tok(DISABLE)]
    Disable,
}

/// One action on `ALTER EVENT TRIGGER name action` — Postgres'
/// `AlterEventTrigStmt` (`enable_trigger`) plus the event-trigger branches
/// of `RenameStmt` and `AlterOwnerStmt`.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`ENABLE`/`DISABLE`/`RENAME`/`OWNER`), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterEventTriggerAction<'input> {
    Enable(EnableTrigger),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER EVENT TRIGGER name action` — Postgres' `AlterEventTrigStmt` plus
/// the event-trigger branches of `RenameStmt` / `AlterOwnerStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterEventTriggerStmt<'input> {
    #[tok(ALTER, EVENT, TRIGGER, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterEventTriggerAction<'input>,
}
