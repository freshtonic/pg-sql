//! TRIGGER DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `BEFORE | AFTER | INSTEAD OF` — Postgres' `TriggerActionTime`.
    ///
    /// Variant ordering: multi-word `InsteadOf` first so the longer match wins
    /// when `INSTEAD` is followed by `OF`.
    #[derive(Debug)]
    pub enum TriggerActionTime {
        #[tok(INSTEAD, OF)]
        InsteadOf,
        #[tok(BEFORE)]
        Before,
        #[tok(AFTER)]
        After,
    }
}

recursa::ast_node! {
    /// `UPDATE OF col[, col …]` — column list following an UPDATE trigger event.
    #[derive(Debug)]
    #[tok(OF, this)]
    pub struct TriggerUpdateOfColumns {
        #[sep(COMMA)]
        pub columns: one_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `UPDATE [OF cols]` — UPDATE trigger event with optional column list.
    #[derive(Debug)]
    #[tok(UPDATE, this)]
    pub struct TriggerUpdateEvent {
        pub of: Option<TriggerUpdateOfColumns>,
    }
}

recursa::ast_node! {
    /// One trigger event — Postgres' `TriggerOneEvent`:
    /// `INSERT | DELETE | UPDATE [OF cols] | TRUNCATE`.
    #[derive(Debug)]
    pub enum TriggerOneEvent {
        #[tok(INSERT)]
        Insert,
        #[tok(DELETE)]
        Delete,
        Update(TriggerUpdateEvent),
        #[tok(TRUNCATE)]
        Truncate,
    }
}

recursa::ast_node! {
    /// One or more trigger events separated by `OR`.
    ///
    /// The wrapper owns the separator policy for the list as a whole. Keeping the
    /// repeated field directly on each statement can make generated formatting
    /// concatenate a unit event and its following separator (for example,
    /// `INSERTOR`).
    #[derive(Debug)]
    pub struct TriggerEventList {
        #[sep(OR)]
        pub events: one_or_many!(TriggerOneEvent),
    }
}

recursa::ast_node! {
    /// `ROW | STATEMENT` — granularity selector after `FOR [EACH]`.
    #[derive(Debug)]
    pub enum TriggerForType {
        #[tok(ROW)]
        Row,
        #[tok(STATEMENT)]
        Statement,
    }
}

recursa::ast_node! {
    /// `FOR [EACH] {ROW | STATEMENT}` — Postgres' `TriggerForSpec`. When omitted
    /// PG defaults to `STATEMENT`, but we preserve absence in the AST so the
    /// formatter round-trips source verbatim.
    #[derive(Debug)]
    pub struct TriggerForSpec {
        #[tok(FOR, optional(EACH), this)]
        pub kind: TriggerForType,
    }
}

recursa::ast_node! {
    /// `WHEN (expr)` — Postgres' `TriggerWhen` clause.
    #[derive(Debug)]
    pub struct TriggerWhenClause {
        #[tok(WHEN, LPAREN, this, RPAREN)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `NEW | OLD` — Postgres' `TransitionOldOrNew`.
    #[derive(Debug)]
    pub enum TransitionOldOrNew {
        #[tok(OLD)]
        Old,
        #[tok(NEW)]
        New,
    }
}

recursa::ast_node! {
    /// `ROW | TABLE` — Postgres' `TransitionRowOrTable`. ROW is permitted by
    /// gram.y though semantically only TABLE makes sense for transition tables.
    #[derive(Debug)]
    pub enum TransitionRowOrTable {
        #[tok(TABLE)]
        Table,
        #[tok(ROW)]
        Row,
    }
}

recursa::ast_node! {
    /// A single `REFERENCING` transition: `{OLD|NEW} {TABLE|ROW} [AS] name`.
    #[derive(Debug)]
    pub struct TriggerTransition {
        pub old_or_new: TransitionOldOrNew,
        pub row_or_table: TransitionRowOrTable,
        #[tok(optional(AS), this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `REFERENCING transition+` — one or more transition-table clauses.
    #[derive(Debug)]
    #[tok(REFERENCING, this)]
    pub struct TriggerReferencing {
        pub transitions: zero_or_many!(TriggerTransition),
    }
}

recursa::ast_node! {
    /// `FUNCTION | PROCEDURE` — Postgres' `FUNCTION_or_PROCEDURE`.
    #[derive(Debug)]
    pub enum FunctionOrProcedure {
        #[tok(FUNCTION)]
        Function,
        #[tok(PROCEDURE)]
        Procedure,
    }
}

recursa::ast_node! {
    /// A single trigger function argument — Postgres' `TriggerFuncArg`:
    /// integer, numeric, string, or ColLabel (identifier-or-keyword) literal.
    ///
    /// Variant ordering: numeric forms before integer (NumericLit longest-match
    /// wins on `.` / `e`); literal `StringLit` before identifier `AliasName`.
    #[derive(Debug)]
    pub enum TriggerFuncArg {
        Numeric(literal::NumericLit),
        Integer(literal::IntegerLit),
        String(literal::StringLit),
        Ident(literal::AliasName),
    }
}

recursa::ast_node! {
    /// `(arg, …)` argument list passed to the trigger's EXECUTE FUNCTION/PROCEDURE.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct TriggerExecArgs {
        #[sep(COMMA)]
        pub args: zero_or_many!(TriggerFuncArg),
    }
}

recursa::ast_node! {
    /// `EXECUTE {FUNCTION | PROCEDURE} func_name(args)` — Postgres' trigger
    /// action clause.
    #[derive(Debug)]
    pub struct TriggerExecuteClause {
        #[tok(EXECUTE, this)]
        pub kind: FunctionOrProcedure,
        pub func_name: QualifiedName,
        pub args: TriggerExecArgs,
    }
}

recursa::ast_node! {
    /// `CREATE [OR REPLACE] TRIGGER name {BEFORE|AFTER|INSTEAD OF} events ON
    /// qualified_name [REFERENCING …] [FOR [EACH] {ROW|STATEMENT}] [WHEN (expr)]
    /// EXECUTE {FUNCTION|PROCEDURE} func_name(args)` — Postgres' `CreateTrigStmt`
    /// (non-constraint form).
    #[derive(Debug)]
    pub struct CreateTriggerStmt {
        #[tok(CREATE, this, TRIGGER)]
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        pub name: crate::tokens::ColId,
        pub timing: TriggerActionTime,
        pub events: TriggerEventList,
        #[tok(ON, this)]
        pub table: QualifiedName,
        pub referencing: Option<TriggerReferencing>,
        pub for_spec: Option<TriggerForSpec>,
        pub when_clause: Option<TriggerWhenClause>,
        pub execute_clause: TriggerExecuteClause,
    }
}

recursa::ast_node! {
    /// `[NO] DEPENDS ON EXTENSION name` — Postgres' `AlterObjectDependsStmt`
    /// action shared by ALTER TRIGGER / ALTER MATERIALIZED VIEW / ALTER INDEX
    /// (and several others). The optional `NO` toggles whether the extension
    /// dependency is added (`DEPENDS ...`) or removed (`NO DEPENDS ...`).
    #[derive(Debug)]
    pub struct DependsOnExtension {
        #[tok(this, DEPENDS, ON, EXTENSION)]
        #[presence(NO)]
        pub no: bool,
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// One action on `ALTER TRIGGER name ON qualified_name action` — Postgres'
    /// `RenameStmt` and `AlterObjectDependsStmt` branches for triggers.
    ///
    /// Variant ordering: variants begin with distinct leading keywords
    /// (`RENAME` / `NO` / `DEPENDS`), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterTriggerAction {
        Rename(RenameTo),
        Depends(DependsOnExtension),
    }
}

recursa::ast_node! {
    /// `ALTER TRIGGER name ON qualified_name { RENAME TO new |
    /// [NO] DEPENDS ON EXTENSION name }` — Postgres' `RenameStmt` and
    /// `AlterObjectDependsStmt` branches for triggers. There is no OWNER /
    /// SET SCHEMA / ENABLE branch on triggers in gram.y.
    #[derive(Debug)]
    pub struct AlterTriggerStmt {
        #[tok(ALTER, TRIGGER, this)]
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
        pub action: AlterTriggerAction,
    }
}

recursa::ast_node! {
    /// `FROM qualified_name` — Postgres' `OptConstrFromTable` (the referenced
    /// table on a constraint trigger).
    #[derive(Debug)]
    pub struct ConstrFromTable {
        #[tok(FROM, this)]
        pub table: QualifiedName,
    }
}

recursa::ast_node! {
    /// A single `ConstraintAttributeElem` — one of
    /// `NOT DEFERRABLE | DEFERRABLE | INITIALLY IMMEDIATE | INITIALLY DEFERRED`.
    ///
    /// The `NOT VALID` / `NO INHERIT` forms are also in gram.y but never appear
    /// on a CONSTRAINT TRIGGER in practice; PG accepts them syntactically. We
    /// include them so the union matches gram.y faithfully.
    ///
    /// Variant ordering: longer/multi-keyword forms first
    /// (`NOT DEFERRABLE`/`NOT VALID`/`INITIALLY …`/`NO INHERIT`).
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `CREATE [OR REPLACE] CONSTRAINT TRIGGER name AFTER events ON table
    /// [FROM ref_table] ConstraintAttributeSpec FOR EACH ROW [WHEN (expr)]
    /// EXECUTE {FUNCTION|PROCEDURE} func_name(args)` — Postgres'
    /// `CreateTrigStmt` (constraint form).
    ///
    /// PG rejects `OR REPLACE` semantically here, but gram.y accepts it; we
    /// mirror the grammar.
    #[derive(Debug)]
    pub struct CreateConstraintTriggerStmt {
        #[tok(CREATE, this, CONSTRAINT, TRIGGER)]
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        #[tok(this, AFTER)]
        pub name: crate::tokens::ColId,
        pub events: TriggerEventList,
        #[tok(ON, this)]
        pub table: QualifiedName,
        pub from_table: Option<ConstrFromTable>,
        pub constraint_attrs: zero_or_many!(ConstraintAttributeElem),
        pub for_each_row: ForEachRow,
        pub when_clause: Option<TriggerWhenClause>,
        pub execute_clause: TriggerExecuteClause,
    }
}

recursa::ast_node! {
    /// Mandatory row-level marker on a constraint trigger.
    #[derive(Debug)]
    pub enum ForEachRow {
        #[tok(FOR, EACH, ROW)]
        Value,
    }
}

recursa::ast_node! {
    /// `DROP TRIGGER [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, TRIGGER, this)]
    pub struct DropTriggerStmt {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One or more event-trigger filter values enclosed in parentheses.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct EventTriggerValueList {
        #[sep(COMMA)]
        pub values: one_or_many!(literal::StringLit),
    }
}

recursa::ast_node! {
    /// A single `event_trigger_when_item`: `tag IN ('a', 'b', …)`. The
    /// filter-tag name is a `ColId` (identifier or unreserved keyword); the
    /// values are `Sconst` (single-quoted strings).
    #[derive(Debug)]
    pub struct EventTriggerWhenItem {
        #[tok(this, IN)]
        pub tag: literal::AliasName,
        pub values: EventTriggerValueList,
    }
}

recursa::ast_node! {
    /// `WHEN item AND item AND …` — Postgres' `event_trigger_when_list`.
    #[derive(Debug)]
    #[tok(WHEN, this)]
    pub struct EventTriggerWhenClause {
        #[sep(AND)]
        pub items: one_or_many!(EventTriggerWhenItem),
    }
}

recursa::ast_node! {
    /// `CREATE EVENT TRIGGER name ON event_name [WHEN filters]
    /// EXECUTE {FUNCTION|PROCEDURE} func_name()` — Postgres' `CreateEventTrigStmt`.
    #[derive(Debug)]
    #[tok(this, RPAREN)]
    pub struct CreateEventTriggerStmt {
        #[tok(CREATE, EVENT, TRIGGER, this)]
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        /// The event name (e.g. `sql_drop`, `ddl_command_start`) is a `ColLabel`
        /// in gram.y — any identifier-or-keyword.
        pub event_name: literal::AliasName,
        pub when_filters: Option<EventTriggerWhenClause>,
        #[tok(EXECUTE, this)]
        pub kind: FunctionOrProcedure,
        #[tok(this, LPAREN)]
        pub func_name: QualifiedName,
        #[sep(COMMA)]
        /// `()` — event triggers never take arguments; the list is empty for
        /// PostgreSQL-valid inputs.
        pub args: zero_or_many!(TriggerFuncArg),
    }
}

recursa::ast_node! {
    /// `DROP EVENT TRIGGER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, EVENT, TRIGGER, this)]
    pub struct DropEventTriggerStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `enable_trigger` — Postgres' four-way enable/disable toggle on an event
    /// trigger (and on regular triggers in ALTER TABLE).
    ///
    /// Variant ordering: the two-token `ENABLE REPLICA` / `ENABLE ALWAYS` forms
    /// come before bare `ENABLE` so longest-match-wins picks the longer spelling
    /// first. `DISABLE` is keyword-disjoint, so its position is for clarity only.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// One action on `ALTER EVENT TRIGGER name action` — Postgres'
    /// `AlterEventTrigStmt` (`enable_trigger`) plus the event-trigger branches
    /// of `RenameStmt` and `AlterOwnerStmt`.
    ///
    /// Variant ordering: variants begin with distinct leading keywords
    /// (`ENABLE`/`DISABLE`/`RENAME`/`OWNER`), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterEventTriggerAction {
        Enable(EnableTrigger),
        Rename(RenameTo),
        Owner(OwnerTo),
    }
}

recursa::ast_node! {
    /// `ALTER EVENT TRIGGER name action` — Postgres' `AlterEventTrigStmt` plus
    /// the event-trigger branches of `RenameStmt` / `AlterOwnerStmt`.
    #[derive(Debug)]
    pub struct AlterEventTriggerStmt {
        #[tok(ALTER, EVENT, TRIGGER, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterEventTriggerAction,
    }
}
