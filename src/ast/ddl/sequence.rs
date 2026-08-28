//! SEQUENCE DDL statements (CREATE/ALTER/DROP).
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

/// `AS TypeName` sequence option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqAsOption<'input> {
    pub r#as: AS,
    pub type_name: CastType<'input>,
}

/// `INCREMENT [BY] N` sequence option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqIncrementOption<'input> {
    pub increment: INCREMENT,
    pub by: Option<BY>,
    pub value: NumericOnly<'input>,
}

/// `MINVALUE N` sequence option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqMinValueOption<'input> {
    pub minvalue: MINVALUE,
    pub value: NumericOnly<'input>,
}

/// `MAXVALUE N` sequence option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqMaxValueOption<'input> {
    pub maxvalue: MAXVALUE,
    pub value: NumericOnly<'input>,
}

/// `START [WITH] N` sequence option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqStartOption<'input> {
    pub start: START,
    pub with: Option<WITH>,
    pub value: NumericOnly<'input>,
}

/// `CACHE N` sequence option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqCacheOption<'input> {
    pub cache: CACHE,
    pub value: NumericOnly<'input>,
}

/// `OWNED BY { NONE | qualified_name }` sequence option.
///
/// Variant ordering: `None` (a single keyword) before `Name` (a dotted
/// identifier path) — they are disambiguated by first token, but the
/// declaration order matches the gram.y rule ordering.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OwnedByTarget<'input> {
    None(NONE),
    Name(QualifiedName<'input>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOwnedByOption<'input> {
    pub owned: OWNED,
    pub by: BY,
    pub target: OwnedByTarget<'input>,
}

/// `RESTART [[WITH] N]` sequence option (used by ALTER SEQUENCE). The
/// `RESTART` keyword is a soft keyword so it remains reclaimable as an
/// identifier in non-sequence positions.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqRestartOption<'input> {
    pub restart: RESTART,
    pub with: Option<(Option<WITH>, NumericOnly<'input>)>,
}

/// `SEQUENCE NAME qualified_name` sequence option — used to set the
/// underlying sequence relation's `relname` during pg_dump restores.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqSequenceNameOption<'input> {
    pub sequence: SEQUENCE,
    pub name_kw: NAME,
    pub name: QualifiedName<'input>,
}

/// A single sequence option — Postgres' `SeqOptElem`.
///
/// Variant ordering: multi-token forms (`NoCycle`, `NoMinvalue`, `NoMaxvalue`,
/// `OwnedBy`, `SequenceName`) before any single-token form they share a first
/// token with so longest-match-wins picks the longer spelling.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SeqOption<'input> {
    NoCycle((NO, CYCLE)),
    NoMinvalue((NO, MINVALUE)),
    NoMaxvalue((NO, MAXVALUE)),
    As(SeqAsOption<'input>),
    Increment(SeqIncrementOption<'input>),
    Minvalue(SeqMinValueOption<'input>),
    Maxvalue(SeqMaxValueOption<'input>),
    Start(SeqStartOption<'input>),
    Cache(SeqCacheOption<'input>),
    OwnedBy(SeqOwnedByOption<'input>),
    Restart(SeqRestartOption<'input>),
    SequenceName(SeqSequenceNameOption<'input>),
    Cycle(CYCLE),
    Unlogged(UNLOGGED),
    // `LOGGED` is in Postgres' SeqOptElem but no corpus statement uses it
    // (it is the default); a `LOGGED` keyword token does not yet exist in
    // pg-sql. Add when first needed.
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateSequenceStmt<'input> {
    pub create: CREATE,
    /// Optional temporary persistence modifier: `TEMP`, `TEMPORARY`, or
    /// `UNLOGGED`. Postgres' `OptTemp` covers all three between `CREATE` and
    /// `SEQUENCE`.
    pub persistence: Option<CreatePersistence>,
    pub sequence: SEQUENCE,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreatePersistence {
    Temporary(TEMPORARY),
    Temp(TEMP),
    Unlogged(UNLOGGED),
}

/// `DROP SEQUENCE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropSequenceStmt<'input> {
    pub drop: DROP,
    pub sequence: SEQUENCE,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET LOGGED` — Postgres' `alter_table_cmd` SET LOGGED branch. Used by
/// ALTER SEQUENCE in the corpus (and by ALTER TABLE, modelled separately).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetLoggedClause {
    pub set: SET,
    pub logged: LOGGED,
}

/// `SET UNLOGGED` — Postgres' `alter_table_cmd` SET UNLOGGED branch.
/// `UNLOGGED` is the existing hard keyword token; `SET` precedes it here.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetUnloggedClause {
    pub set: SET,
    pub unlogged: UNLOGGED,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOptList<'input> {
    pub head: SeqOption<'input>,
    pub rest: Vec<SeqOption<'input>>,
}

/// `ALTER SEQUENCE [IF EXISTS] name action` — Postgres' `AlterSeqStmt`,
/// the sequence-applicable subset of ALTER TABLE's `alter_table_cmds`,
/// and `RenameStmt` / `AlterObjectSchemaStmt` branches for sequences.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterSequenceStmt<'input> {
    pub alter: ALTER,
    pub sequence: SEQUENCE,
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
        let mut input = crate::tokens::test_input("CREATE SEQUENCE s1");
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap();
        assert!(stmt.persistence.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_sequence_options() {
        let mut input = crate::tokens::test_input(
            "CREATE SEQUENCE s1 AS integer INCREMENT BY 2 MINVALUE 1 MAXVALUE 100 START WITH 5 CACHE 10 CYCLE",
        );
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 7);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_sequence_no_minvalue_owned_by() {
        let mut input = crate::tokens::test_input(
            "CREATE SEQUENCE s1 NO MINVALUE NO MAXVALUE NO CYCLE OWNED BY t.col",
        );
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 4);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_sequence_temp_if_not_exists() {
        let mut input = crate::tokens::test_input("CREATE TEMPORARY SEQUENCE IF NOT EXISTS s1");
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.persistence,
            Some(CreatePersistence::Temporary(_))
        ));
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_sequence_owned_by_none() {
        let mut input = crate::tokens::test_input("CREATE SEQUENCE s1 OWNED BY NONE");
        let stmt = CreateSequenceStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(stmt.options[0], SeqOption::OwnedBy(_)));
        assert!(input.is_empty());
    }
}
