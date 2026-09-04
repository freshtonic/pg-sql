//! SEQUENCE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `AS TypeName` sequence option.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqAsOption<'input> {
    #[tok(AS, this)]
    pub type_name: CastType<'input>,
}

/// `INCREMENT [BY] N` sequence option.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqIncrementOption<'input> {
    #[tok(INCREMENT, optional(BY), this)]
    pub value: NumericOnly<'input>,
}

/// `MINVALUE N` sequence option.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqMinValueOption<'input> {
    #[tok(MINVALUE, this)]
    pub value: NumericOnly<'input>,
}

/// `MAXVALUE N` sequence option.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqMaxValueOption<'input> {
    #[tok(MAXVALUE, this)]
    pub value: NumericOnly<'input>,
}

/// `START [WITH] N` sequence option.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqStartOption<'input> {
    #[tok(START, optional(WITH), this)]
    pub value: NumericOnly<'input>,
}

/// `CACHE N` sequence option.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqCacheOption<'input> {
    #[tok(CACHE, this)]
    pub value: NumericOnly<'input>,
}

/// `OWNED BY { NONE | qualified_name }` sequence option.
///
/// `NONE` is an unreserved identifier in this position, so
/// [`QualifiedName`] already accepts both grammar branches. Keeping a
/// separate fixed-token `None` arm would describe the same token stream
/// twice and leave the generated dispatcher ambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum OwnedByTarget<'input> {
    Name(QualifiedName<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOwnedByOption<'input> {
    #[tok(OWNED, BY, this)]
    pub target: OwnedByTarget<'input>,
}

/// `RESTART [[WITH] N]` sequence option (used by ALTER SEQUENCE). The
/// `RESTART` keyword is a soft keyword so it remains reclaimable as an
/// identifier in non-sequence positions.
#[derive(recursa::Node, Debug, Clone)]
#[tok(RESTART, this)]
pub struct SeqRestartOption<'input> {
    #[tok(optional(WITH), this)]
    pub with: Option<NumericOnly<'input>>,
}

/// `SEQUENCE NAME qualified_name` sequence option — used to set the
/// underlying sequence relation's `relname` during pg_dump restores.
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqSequenceNameOption<'input> {
    #[tok(SEQUENCE, NAME, this)]
    pub name: QualifiedName<'input>,
}

/// A single sequence option — Postgres' `SeqOptElem`.
///
/// Variant ordering: multi-token forms (`NoCycle`, `NoMinvalue`, `NoMaxvalue`,
/// `OwnedBy`, `SequenceName`) before any single-token form they share a first
/// token with so longest-match-wins picks the longer spelling.
#[derive(recursa::Node, Debug, Clone)]
pub enum SeqOption<'input> {
    #[tok(NO, CYCLE)]
    NoCycle,
    #[tok(NO, MINVALUE)]
    NoMinvalue,
    #[tok(NO, MAXVALUE)]
    NoMaxvalue,
    As(SeqAsOption<'input>),
    Increment(SeqIncrementOption<'input>),
    Minvalue(SeqMinValueOption<'input>),
    Maxvalue(SeqMaxValueOption<'input>),
    Start(SeqStartOption<'input>),
    Cache(SeqCacheOption<'input>),
    OwnedBy(SeqOwnedByOption<'input>),
    Restart(SeqRestartOption<'input>),
    SequenceName(SeqSequenceNameOption<'input>),
    #[tok(CYCLE)]
    Cycle,
    #[tok(UNLOGGED)]
    Unlogged,
    // `LOGGED` is in Postgres' SeqOptElem but no corpus statement uses it
    // (it is the default); a `LOGGED` keyword token does not yet exist in
    // pg-sql. Add when first needed.
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, this)]
pub struct CreateSequenceStmt<'input> {
    /// Optional temporary persistence modifier: `TEMP`, `TEMPORARY`, or
    /// `UNLOGGED`. Postgres' `OptTemp` covers all three between `CREATE` and
    /// `SEQUENCE`.
    pub persistence: Option<CreatePersistence>,
    pub sequence: SequenceKeyword,
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    /// Greedy: a leading token from any of 12 kinds starts this element instead of ending `CreateSequenceStmt` (bison shift preference).
    #[greedy(
        AS, CACHE, CYCLE, INCREMENT, MAXVALUE, MINVALUE, NO, OWNED, RESTART, SEQUENCE, START,
        UNLOGGED
    )]
    pub options: Vec<SeqOption<'input>>,
}

/// Required `SEQUENCE` keyword after the optional persistence modifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum SequenceKeyword {
    #[tok(SEQUENCE)]
    Sequence,
}

/// Persistence modifier between `CREATE` and an object keyword: `TEMP`,
/// `TEMPORARY`, `UNLOGGED`, or the longer `GLOBAL TEMPORARY`/`LOCAL TEMPORARY`
/// forms (deprecated but still accepted).
///
/// Variant ordering: multi-keyword forms (`GLOBAL TEMP[ORARY]`,
/// `LOCAL TEMP[ORARY]`) — none of which are exercised by the sequence corpus
/// but kept for forward-compat — would come first; today only `Temporary`,
/// `Temp`, `Unlogged` are modelled.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreatePersistence {
    #[tok(TEMPORARY)]
    Temporary,
    #[tok(TEMP)]
    Temp,
    #[tok(UNLOGGED)]
    Unlogged,
}

/// `DROP SEQUENCE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, SEQUENCE, this)]
pub struct DropSequenceStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET LOGGED` — Postgres' `alter_table_cmd` SET LOGGED branch. Used by
/// ALTER SEQUENCE in the corpus (and by ALTER TABLE, modelled separately).
#[derive(recursa::Node, Debug, Clone)]
pub enum SetLoggedClause {
    #[tok(SET, LOGGED)]
    Value,
}

/// `SET UNLOGGED` — Postgres' `alter_table_cmd` SET UNLOGGED branch.
/// `UNLOGGED` is the existing hard keyword token; `SET` precedes it here.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetUnloggedClause {
    #[tok(SET, UNLOGGED)]
    Value,
}

/// One action on `ALTER SEQUENCE [IF EXISTS] name action` — Postgres'
/// `AlterSeqStmt` (`SeqOptList`), the sequence-specific subset of
/// `alter_table_cmds` (`SET LOGGED`/`SET UNLOGGED`), plus the sequence
/// branches of `RenameStmt` / `AlterObjectSchemaStmt`.
///
/// Variant ordering:
/// - `SetLogged` / `SetUnlogged` / `SetSchema` all begin with `SET`; the
///   second token disambiguates them (`LOGGED` / `UNLOGGED` / `SCHEMA`).
///   None of them conflicts with a `SeqOption` since `SET` is never a
///   `SeqOptElem` first token.
/// - `Rename` (`RENAME TO`) is keyword-disjoint from the others.
/// - `Opts` (`SeqOptList`) is listed last because it starts with any of
///   `AS`, `CACHE`, `CYCLE`, `INCREMENT`, `MAXVALUE`, `MINVALUE`,
///   `NO …`, `OWNED`, `RESTART`, `SEQUENCE`, `START`, `UNLOGGED` — none
///   of which conflict with the keyword-led variants above.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterSequenceAction<'input> {
    SetLogged(SetLoggedClause),
    SetUnlogged(SetUnloggedClause),
    SetSchema(SetSchemaClause<'input>),
    Rename(RenameTo<'input>),
    Opts(SeqOptList<'input>),
}

/// Non-empty list of `SeqOptElem`s — Postgres' `SeqOptList`. A `Vec` would
/// allow the empty case (gram.y requires at least one), but recursa's
/// `Vec` is implemented as a `Seq0` and cannot be empty here without an
/// alternation that already covers the no-options case. We use a struct
/// with a single `Seq1`-style field instead so the action enum can peek
/// on a non-empty SeqOpt and commit. The leading `UNLOGGED` SeqOption is
/// the bare `UNLOGGED` keyword form — distinct from the `SET UNLOGGED`
/// branch above (which has the leading `SET`).
#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOptList<'input> {
    pub head: SeqOption<'input>,
    /// Greedy: a leading token from any of 12 kinds starts this element instead of ending `SeqOptList` (bison shift preference).
    #[greedy(
        AS, CACHE, CYCLE, INCREMENT, MAXVALUE, MINVALUE, NO, OWNED, RESTART, SEQUENCE, START,
        UNLOGGED
    )]
    pub rest: Vec<SeqOption<'input>>,
}

/// `ALTER SEQUENCE [IF EXISTS] name action` — Postgres' `AlterSeqStmt`,
/// the sequence-applicable subset of ALTER TABLE's `alter_table_cmds`,
/// and `RenameStmt` / `AlterObjectSchemaStmt` branches for sequences.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ALTER, SEQUENCE, this)]
pub struct AlterSequenceStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterSequenceAction<'input>,
}
