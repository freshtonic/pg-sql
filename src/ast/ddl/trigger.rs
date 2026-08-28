//! TRIGGER DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TriggerActionTime {
    InsteadOf((INSTEAD, OF)),
    Before(BEFORE),
    After(AFTER),
}

/// `UPDATE OF col[, col …]` — column list following an UPDATE trigger event.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerUpdateOfColumns<'input> {
    pub of: OF,
    pub columns: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
}

/// `UPDATE [OF cols]` — UPDATE trigger event with optional column list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerUpdateEvent<'input> {
    pub update: UPDATE,
    pub of: Option<TriggerUpdateOfColumns<'input>>,
}

/// One trigger event — Postgres' `TriggerOneEvent`:
/// `INSERT | DELETE | UPDATE [OF cols] | TRUNCATE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TriggerOneEvent<'input> {
    Insert(INSERT),
    Delete(DELETE),
    Update(TriggerUpdateEvent<'input>),
    Truncate(TRUNCATE),
}

/// `ROW | STATEMENT` — granularity selector after `FOR [EACH]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TriggerForType {
    Row(ROW),
    Statement(STATEMENT),
}

/// `FOR [EACH] {ROW | STATEMENT}` — Postgres' `TriggerForSpec`. When omitted
/// PG defaults to `STATEMENT`, but we preserve absence in the AST so the
/// formatter round-trips source verbatim.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerForSpec {
    pub r#for: FOR,
    pub each: Option<EACH>,
    pub kind: TriggerForType,
}

/// `WHEN (expr)` — Postgres' `TriggerWhen` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerWhenClause<'input> {
    pub when: WHEN,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `NEW | OLD` — Postgres' `TransitionOldOrNew`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TransitionOldOrNew {
    Old(OLD),
    New(NEW),
}

/// `ROW | TABLE` — Postgres' `TransitionRowOrTable`. ROW is permitted by
/// gram.y though semantically only TABLE makes sense for transition tables.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TransitionRowOrTable {
    Table(TABLE),
    Row(ROW),
}

/// A single `REFERENCING` transition: `{OLD|NEW} {TABLE|ROW} [AS] name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerTransition<'input> {
    pub old_or_new: TransitionOldOrNew,
    pub row_or_table: TransitionRowOrTable,
    pub r#as: Option<AS>,
    pub name: crate::tokens::ColId<'input>,
}

/// `REFERENCING transition+` — one or more transition-table clauses.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerReferencing<'input> {
    pub referencing: REFERENCING,
    pub transitions: Vec<TriggerTransition<'input>>,
}

/// `FUNCTION | PROCEDURE` — Postgres' `FUNCTION_or_PROCEDURE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FunctionOrProcedure {
    Function(FUNCTION),
    Procedure(PROCEDURE),
}

/// A single trigger function argument — Postgres' `TriggerFuncArg`:
/// integer, numeric, string, or ColLabel (identifier-or-keyword) literal.
///
/// Variant ordering: numeric forms before integer (NumericLit longest-match
/// wins on `.` / `e`); literal `StringLit` before identifier `AliasName`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TriggerFuncArg<'input> {
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    String(literal::StringLit<'input>),
    Ident(literal::AliasName<'input>),
}

/// `(arg, …)` argument list passed to the trigger's EXECUTE FUNCTION/PROCEDURE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerExecArgs<'input> {
    pub args: Surrounded<punct::LParen, Seq0<TriggerFuncArg<'input>, punct::Comma>, punct::RParen>,
}

/// `EXECUTE {FUNCTION | PROCEDURE} func_name(args)` — Postgres' trigger
/// action clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TriggerExecuteClause<'input> {
    pub execute: EXECUTE,
    pub kind: FunctionOrProcedure,
    pub func_name: QualifiedName<'input>,
    pub args: TriggerExecArgs<'input>,
}

/// `CREATE [OR REPLACE] TRIGGER name {BEFORE|AFTER|INSTEAD OF} events ON
/// qualified_name [REFERENCING …] [FOR [EACH] {ROW|STATEMENT}] [WHEN (expr)]
/// EXECUTE {FUNCTION|PROCEDURE} func_name(args)` — Postgres' `CreateTrigStmt`
/// (non-constraint form).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateTriggerStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub trigger: TRIGGER,
    pub name: crate::tokens::ColId<'input>,
    pub timing: TriggerActionTime,
    pub events: Seq1<TriggerOneEvent<'input>, OR>,
    pub on: ON,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DependsOnExtension<'input> {
    pub no: Option<NO>,
    pub depends: DEPENDS,
    pub on: ON,
    pub extension: EXTENSION,
    pub name: crate::tokens::ColId<'input>,
}

/// One action on `ALTER TRIGGER name ON qualified_name action` — Postgres'
/// `RenameStmt` and `AlterObjectDependsStmt` branches for triggers.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`RENAME` / `NO` / `DEPENDS`), so order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterTriggerAction<'input> {
    Rename(RenameTo<'input>),
    Depends(DependsOnExtension<'input>),
}

/// `ALTER TRIGGER name ON qualified_name { RENAME TO new |
/// [NO] DEPENDS ON EXTENSION name }` — Postgres' `RenameStmt` and
/// `AlterObjectDependsStmt` branches for triggers. There is no OWNER /
/// SET SCHEMA / ENABLE branch on triggers in gram.y.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterTriggerStmt<'input> {
    pub alter: ALTER,
    pub trigger: TRIGGER,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
    pub action: AlterTriggerAction<'input>,
}

/// `FROM qualified_name` — Postgres' `OptConstrFromTable` (the referenced
/// table on a constraint trigger).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ConstrFromTable<'input> {
    pub from: FROM,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ConstraintAttributeElem {
    NotDeferrable((NOT, DEFERRABLE)),
    NotValid((NOT, VALID)),
    NoInherit((NO, INHERIT)),
    InitiallyImmediate((INITIALLY, IMMEDIATE)),
    InitiallyDeferred((INITIALLY, DEFERRED)),
    Deferrable(DEFERRABLE),
}

/// `CREATE [OR REPLACE] CONSTRAINT TRIGGER name AFTER events ON table
/// [FROM ref_table] ConstraintAttributeSpec FOR EACH ROW [WHEN (expr)]
/// EXECUTE {FUNCTION|PROCEDURE} func_name(args)` — Postgres'
/// `CreateTrigStmt` (constraint form).
///
/// PG rejects `OR REPLACE` semantically here, but gram.y accepts it; we
/// mirror the grammar.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateConstraintTriggerStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub constraint: CONSTRAINT,
    pub trigger: TRIGGER,
    pub name: crate::tokens::ColId<'input>,
    pub after: AFTER,
    pub events: Seq1<TriggerOneEvent<'input>, OR>,
    pub on: ON,
    pub table: QualifiedName<'input>,
    pub from_table: Option<ConstrFromTable<'input>>,
    pub constraint_attrs: Vec<ConstraintAttributeElem>,
    pub r#for: FOR,
    pub each: EACH,
    pub row: ROW,
    pub when_clause: Option<TriggerWhenClause<'input>>,
    pub execute_clause: TriggerExecuteClause<'input>,
}

/// `DROP TRIGGER [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropTriggerStmt<'input> {
    pub drop: DROP,
    pub trigger: TRIGGER,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// A single `event_trigger_when_item`: `tag IN ('a', 'b', …)`. The
/// filter-tag name is a `ColId` (identifier or unreserved keyword); the
/// values are `Sconst` (single-quoted strings).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EventTriggerWhenItem<'input> {
    pub tag: literal::AliasName<'input>,
    pub r#in: IN,
    pub values:
        Surrounded<punct::LParen, Seq1<literal::StringLit<'input>, punct::Comma>, punct::RParen>,
}

/// `WHEN item AND item AND …` — Postgres' `event_trigger_when_list`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EventTriggerWhenClause<'input> {
    pub when: WHEN,
    pub items: Seq1<EventTriggerWhenItem<'input>, AND>,
}

/// `CREATE EVENT TRIGGER name ON event_name [WHEN filters]
/// EXECUTE {FUNCTION|PROCEDURE} func_name()` — Postgres' `CreateEventTrigStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateEventTriggerStmt<'input> {
    pub create: CREATE,
    pub event: EVENT,
    pub trigger: TRIGGER,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    /// The event name (e.g. `sql_drop`, `ddl_command_start`) is a `ColLabel`
    /// in gram.y — any identifier-or-keyword.
    pub event_name: literal::AliasName<'input>,
    pub when_filters: Option<EventTriggerWhenClause<'input>>,
    pub execute: EXECUTE,
    pub kind: FunctionOrProcedure,
    pub func_name: QualifiedName<'input>,
    /// `()` — event triggers never take arguments. We use `Seq0` for the
    /// empty body so the `Surrounded` helper stays uniform with other
    /// EXECUTE clauses; PG only ever produces an empty list here.
    pub args: Surrounded<punct::LParen, Seq0<TriggerFuncArg<'input>, punct::Comma>, punct::RParen>,
}

/// `DROP EVENT TRIGGER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropEventTriggerStmt<'input> {
    pub drop: DROP,
    pub event: EVENT,
    pub trigger: TRIGGER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum EnableTrigger {
    EnableReplica((ENABLE, REPLICA)),
    EnableAlways((ENABLE, ALWAYS)),
    Enable(ENABLE),
    Disable(DISABLE),
}

/// One action on `ALTER EVENT TRIGGER name action` — Postgres'
/// `AlterEventTrigStmt` (`enable_trigger`) plus the event-trigger branches
/// of `RenameStmt` and `AlterOwnerStmt`.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`ENABLE`/`DISABLE`/`RENAME`/`OWNER`), so order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterEventTriggerAction<'input> {
    Enable(EnableTrigger),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER EVENT TRIGGER name action` — Postgres' `AlterEventTrigStmt` plus
/// the event-trigger branches of `RenameStmt` / `AlterOwnerStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterEventTriggerStmt<'input> {
    pub alter: ALTER,
    pub event: EVENT,
    pub trigger: TRIGGER,
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
        let mut input = crate::tokens::test_input("DROP TRIGGER IF EXISTS trg ON my_table CASCADE");
        let stmt = DropTriggerStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.name.text(), "trg");
        assert_eq!(stmt.table.object(), "my_table");
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_event_trigger() {
        let mut input = crate::tokens::test_input("DROP EVENT TRIGGER et1");
        let stmt = DropEventTriggerStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_trigger_rename() {
        let mut input = crate::tokens::test_input(
            "ALTER TRIGGER modified_a ON main_table RENAME TO modified_modified_a",
        );
        let _stmt = AlterTriggerStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
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
