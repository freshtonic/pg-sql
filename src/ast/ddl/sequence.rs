//! SEQUENCE DDL statements (CREATE/ALTER/DROP).
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
/// Variant ordering: `None` (a single keyword) before `Name` (a dotted
/// identifier path) — they are disambiguated by first token, but the
/// declaration order matches the gram.y rule ordering.
#[derive(recursa::Node, Debug, Clone)]
pub enum OwnedByTarget<'input> {
    #[tok(NONE)] None,
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
pub struct SeqRestartOption<'input> {
    #[tok(RESTART, optional(WITH), this)]
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
    #[tok(NO, CYCLE)] NoCycle,
    #[tok(NO, MINVALUE)] NoMinvalue,
    #[tok(NO, MAXVALUE)] NoMaxvalue,
    As(SeqAsOption<'input>),
    Increment(SeqIncrementOption<'input>),
    Minvalue(SeqMinValueOption<'input>),
    Maxvalue(SeqMaxValueOption<'input>),
    Start(SeqStartOption<'input>),
    Cache(SeqCacheOption<'input>),
    OwnedBy(SeqOwnedByOption<'input>),
    Restart(SeqRestartOption<'input>),
    SequenceName(SeqSequenceNameOption<'input>),
    #[tok(CYCLE)] Cycle,
    #[tok(UNLOGGED)] Unlogged,
    // `LOGGED` is in Postgres' SeqOptElem but no corpus statement uses it
    // (it is the default); a `LOGGED` keyword token does not yet exist in
    // pg-sql. Add when first needed.
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateSequenceStmt<'input> {
    #[tok(CREATE, this)]
    /// Optional temporary persistence modifier: `TEMP`, `TEMPORARY`, or
    /// `UNLOGGED`. Postgres' `OptTemp` covers all three between `CREATE` and
    /// `SEQUENCE`.
    pub persistence: Option<CreatePersistence>,
    #[tok(SEQUENCE, this)]
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    pub options: Vec<SeqOption<'input>>,
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
    #[tok(TEMPORARY)] Temporary,
    #[tok(TEMP)] Temp,
    #[tok(UNLOGGED)] Unlogged,
}

/// `DROP SEQUENCE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropSequenceStmt<'input> {
    #[tok(DROP, SEQUENCE, this)]
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET LOGGED` — Postgres' `alter_table_cmd` SET LOGGED branch. Used by
/// ALTER SEQUENCE in the corpus (and by ALTER TABLE, modelled separately).
#[derive(recursa::Node, Debug, Clone)]
pub enum SetLoggedClause { #[tok(SET, LOGGED)] Value, }

/// `SET UNLOGGED` — Postgres' `alter_table_cmd` SET UNLOGGED branch.
/// `UNLOGGED` is the existing hard keyword token; `SET` precedes it here.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetUnloggedClause { #[tok(SET, UNLOGGED)] Value, }

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
    pub rest: Vec<SeqOption<'input>>,
}

/// `ALTER SEQUENCE [IF EXISTS] name action` — Postgres' `AlterSeqStmt`,
/// the sequence-applicable subset of ALTER TABLE's `alter_table_cmds`,
/// and `RenameStmt` / `AlterObjectSchemaStmt` branches for sequences.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSequenceStmt<'input> {
    #[tok(ALTER, SEQUENCE, this)]
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterSequenceAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_sequence_plain() {
        let lexed = crate::tokens::lex("CREATE SEQUENCE s1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.persistence.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_options() {
        let lexed = crate::tokens::lex("CREATE SEQUENCE s1 AS integer INCREMENT BY 2 MINVALUE 1 MAXVALUE 100 START WITH 5 CACHE 10 CYCLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 7);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_no_minvalue_owned_by() {
        let lexed = crate::tokens::lex("CREATE SEQUENCE s1 NO MINVALUE NO MAXVALUE NO CYCLE OWNED BY t.col");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 4);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_temp_if_not_exists() {
        let lexed = crate::tokens::lex("CREATE TEMPORARY SEQUENCE IF NOT EXISTS s1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.persistence,
            Some(CreatePersistence::Temporary(_))
        ));
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_sequence_owned_by_none() {
        let lexed = crate::tokens::lex("CREATE SEQUENCE s1 OWNED BY NONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(stmt.options[0], SeqOption::OwnedBy(_)));
        assert!(input.is_eof());
    }
}
