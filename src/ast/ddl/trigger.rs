//! TRIGGER DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `BEFORE | AFTER | INSTEAD OF` — Postgres' `TriggerActionTime`.
///
/// Variant ordering: multi-word `InsteadOf` first so the longer match wins
/// when `INSTEAD` is followed by `OF`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerActionTime {
    #[tok(INSTEAD, OF)] InsteadOf,
    #[tok(BEFORE)] Before,
    #[tok(AFTER)] After,
}

/// `UPDATE OF col[, col …]` — column list following an UPDATE trigger event.
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerUpdateOfColumns<'input> {
    #[tok(OF, this)]
    #[sep(COMMA)]
    pub columns: recursa::Vec1<crate::tokens::ColId<'input> >,
}

/// `UPDATE [OF cols]` — UPDATE trigger event with optional column list.
#[derive(recursa::Node, Debug, Clone)]
pub struct TriggerUpdateEvent<'input> {
    #[tok(UPDATE, this)]
    pub of: Option<TriggerUpdateOfColumns<'input>>,
}

/// One trigger event — Postgres' `TriggerOneEvent`:
/// `INSERT | DELETE | UPDATE [OF cols] | TRUNCATE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerOneEvent<'input> {
    #[tok(INSERT)] Insert,
    #[tok(DELETE)] Delete,
    Update(TriggerUpdateEvent<'input>),
    #[tok(TRUNCATE)] Truncate,
}

/// `ROW | STATEMENT` — granularity selector after `FOR [EACH]`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerForType {
    #[tok(ROW)] Row,
    #[tok(STATEMENT)] Statement,
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
    pub expr:  Box<Expr<'input>> ,
}

/// `NEW | OLD` — Postgres' `TransitionOldOrNew`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransitionOldOrNew {
    #[tok(OLD)] Old,
    #[tok(NEW)] New,
}

/// `ROW | TABLE` — Postgres' `TransitionRowOrTable`. ROW is permitted by
/// gram.y though semantically only TABLE makes sense for transition tables.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransitionRowOrTable {
    #[tok(TABLE)] Table,
    #[tok(ROW)] Row,
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
pub struct TriggerReferencing<'input> {
    #[tok(REFERENCING, this)]
    pub transitions: Vec<TriggerTransition<'input>>,
}

/// `FUNCTION | PROCEDURE` — Postgres' `FUNCTION_or_PROCEDURE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionOrProcedure {
    #[tok(FUNCTION)] Function,
    #[tok(PROCEDURE)] Procedure,
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
pub struct TriggerExecArgs<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:  Vec<TriggerFuncArg<'input> > ,
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
    #[sep(OR)]
    pub events: recursa::Vec1<TriggerOneEvent<'input> >,
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
    #[tok(NOT, DEFERRABLE)] NotDeferrable,
    #[tok(NOT, VALID)] NotValid,
    #[tok(NO, INHERIT)] NoInherit,
    #[tok(INITIALLY, IMMEDIATE)] InitiallyImmediate,
    #[tok(INITIALLY, DEFERRED)] InitiallyDeferred,
    #[tok(DEFERRABLE)] Deferrable,
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
    pub name: crate::tokens::ColId<'input>,
    #[tok(AFTER, this)]
    #[sep(OR)]
    pub events: recursa::Vec1<TriggerOneEvent<'input> >,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub from_table: Option<ConstrFromTable<'input>>,
    pub constraint_attrs: Vec<ConstraintAttributeElem>,
    #[tok(FOR, EACH, ROW, this)]
    pub when_clause: Option<TriggerWhenClause<'input>>,
    pub execute_clause: TriggerExecuteClause<'input>,
}

/// `DROP TRIGGER [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropTriggerStmt<'input> {
    #[tok(DROP, TRIGGER, this)]
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// A single `event_trigger_when_item`: `tag IN ('a', 'b', …)`. The
/// filter-tag name is a `ColId` (identifier or unreserved keyword); the
/// values are `Sconst` (single-quoted strings).
#[derive(recursa::Node, Debug, Clone)]
pub struct EventTriggerWhenItem<'input> {
    pub tag: literal::AliasName<'input>,
    #[tok(IN, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub values:
         recursa::Vec1<literal::StringLit<'input> > ,
}

/// `WHEN item AND item AND …` — Postgres' `event_trigger_when_list`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EventTriggerWhenClause<'input> {
    #[tok(WHEN, this)]
    #[sep(AND)]
    pub items: recursa::Vec1<EventTriggerWhenItem<'input> >,
}

/// `CREATE EVENT TRIGGER name ON event_name [WHEN filters]
/// EXECUTE {FUNCTION|PROCEDURE} func_name()` — Postgres' `CreateEventTrigStmt`.
#[derive(recursa::Node, Debug, Clone)]
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
    pub func_name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    /// `()` — event triggers never take arguments. We use `Seq0` for the
    /// empty body so the `Surrounded` helper stays uniform with other
    /// EXECUTE clauses; PG only ever produces an empty list here.
    pub args:  Vec<TriggerFuncArg<'input> > ,
}

/// `DROP EVENT TRIGGER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropEventTriggerStmt<'input> {
    #[tok(DROP, EVENT, TRIGGER, this)]
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
    #[tok(ENABLE, REPLICA)] EnableReplica,
    #[tok(ENABLE, ALWAYS)] EnableAlways,
    #[tok(ENABLE)] Enable,
    #[tok(DISABLE)] Disable,
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

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_trigger_on_table() {
        let lexed = crate::tokens::lex("DROP TRIGGER IF EXISTS trg ON my_table CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTriggerStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.name.text(), "trg");
        assert_eq!(stmt.table.object(), "my_table");
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_event_trigger() {
        let lexed = crate::tokens::lex("DROP EVENT TRIGGER et1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropEventTriggerStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_trigger_rename() {
        let lexed = crate::tokens::lex("ALTER TRIGGER modified_a ON main_table RENAME TO modified_modified_a");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTriggerStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_trigger_minimal() {
        let stmt: CreateTriggerStmt =
            parse_stmt("CREATE TRIGGER t BEFORE INSERT ON tbl FOR EACH ROW EXECUTE PROCEDURE f()");
        assert_eq!(stmt.name.text(), "t");
        assert!(matches!(stmt.timing, TriggerActionTime::Before(_)));
        assert_eq!(stmt.table.object(), "tbl");
        assert!(stmt.or_replace.is_none());
        assert!(stmt.referencing.is_none());
        assert!(stmt.when_clause.is_none());
    }

    #[test]
    fn parse_create_or_replace_trigger_modelled() {
        let stmt: CreateTriggerStmt = parse_stmt(
            "CREATE OR REPLACE TRIGGER my_trig BEFORE INSERT ON my_table FOR EACH ROW EXECUTE PROCEDURE funcB()",
        );
        assert!(stmt.or_replace.is_some());
        assert_eq!(stmt.name.text(), "my_trig");
    }

    #[test]
    fn create_trigger_after_update_or_delete_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER UPDATE OR DELETE ON tbl FOR EACH STATEMENT EXECUTE FUNCTION f()",
        );
    }

    #[test]
    fn create_trigger_update_of_columns_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t BEFORE UPDATE OF a, b ON tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_instead_of_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_truncate_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t BEFORE TRUNCATE ON tbl FOR EACH STATEMENT EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_when_clause_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER INSERT ON tbl FOR EACH ROW WHEN (NEW.a = 123) EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_referencing_old_new_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER UPDATE ON tbl REFERENCING OLD TABLE AS oldtable NEW TABLE AS newtable FOR EACH STATEMENT EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_with_args_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t BEFORE INSERT ON tbl FOR EACH STATEMENT EXECUTE PROCEDURE f('hello', 42)",
        );
    }

    #[test]
    fn create_trigger_default_for_each_roundtrips() {
        // `FOR EACH ROW`/`STATEMENT` is optional — when omitted, defaults to
        // STATEMENT per the SQL standard. Our AST mirrors source verbatim.
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER UPDATE ON tbl EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn parse_create_constraint_trigger_minimal() {
        let stmt: CreateConstraintTriggerStmt = parse_stmt(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
        assert_eq!(stmt.name.text(), "t");
        assert_eq!(stmt.table.object(), "tbl");
        assert!(stmt.constraint_attrs.is_empty());
    }

    #[test]
    fn create_constraint_trigger_initially_deferred_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER UPDATE ON tbl INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION f()",
        );
    }

    #[test]
    fn create_constraint_trigger_deferrable_initially_deferred_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_constraint_trigger_multi_event_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT OR UPDATE OR DELETE ON s.tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_constraint_trigger_from_table_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FROM other DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn parse_create_event_trigger_minimal() {
        let stmt: CreateEventTriggerStmt = parse_stmt(
            "CREATE EVENT TRIGGER undroppable ON sql_drop EXECUTE PROCEDURE undroppable()",
        );
        assert_eq!(stmt.name.text(), "undroppable");
        assert!(stmt.when_filters.is_none());
    }

    #[test]
    fn create_event_trigger_when_tag_in_roundtrips() {
        reparse_stable::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER t ON sql_drop WHEN TAG IN ('drop table', 'drop function') EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_event_trigger_when_multi_filter_roundtrips() {
        reparse_stable::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER t ON ddl_command_start WHEN TAG IN ('CREATE TABLE') AND TAG IN ('ALTER TABLE') EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_event_trigger_execute_function_roundtrips() {
        reparse_stable::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER t ON ddl_command_end EXECUTE FUNCTION f()",
        );
    }
}
