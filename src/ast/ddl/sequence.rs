//! SEQUENCE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `AS TypeName` sequence option.
    #[derive(Debug)]
    pub struct SeqAsOption {
        #[tok(AS, this)]
        pub type_name: CastType,
    }
}

recursa::ast_node! {
    /// `INCREMENT [BY] N` sequence option.
    #[derive(Debug)]
    pub struct SeqIncrementOption {
        #[tok(INCREMENT, optional(BY), this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `MINVALUE N` sequence option.
    #[derive(Debug)]
    pub struct SeqMinValueOption {
        #[tok(MINVALUE, this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `MAXVALUE N` sequence option.
    #[derive(Debug)]
    pub struct SeqMaxValueOption {
        #[tok(MAXVALUE, this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `START [WITH] N` sequence option.
    #[derive(Debug)]
    pub struct SeqStartOption {
        #[tok(START, optional(WITH), this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `CACHE N` sequence option.
    #[derive(Debug)]
    pub struct SeqCacheOption {
        #[tok(CACHE, this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `OWNED BY { NONE | qualified_name }` sequence option.
    ///
    /// `NONE` is an unreserved identifier in this position, so
    /// [`QualifiedName`] already accepts both grammar branches. Keeping a
    /// separate fixed-token `None` arm would describe the same token stream
    /// twice and create two LR derivations for the same input.
    #[derive(Debug)]
    pub enum OwnedByTarget {
        Name(QualifiedName),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SeqOwnedByOption {
        #[tok(OWNED, BY, this)]
        pub target: OwnedByTarget,
    }
}

recursa::ast_node! {
    /// `RESTART [[WITH] N]` sequence option (used by ALTER SEQUENCE). The
    /// `RESTART` keyword is a soft keyword so it remains reclaimable as an
    /// identifier in non-sequence positions.
    #[derive(Debug)]
    #[tok(RESTART, this)]
    pub struct SeqRestartOption {
        #[tok(optional(WITH), this)]
        pub with: Option<NumericOnly>,
    }
}

recursa::ast_node! {
    /// `SEQUENCE NAME qualified_name` sequence option — used to set the
    /// underlying sequence relation's `relname` during pg_dump restores.
    #[derive(Debug)]
    pub struct SeqSequenceNameOption {
        #[tok(SEQUENCE, NAME, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// A single sequence option — Postgres' `SeqOptElem`.
    ///
    /// Variant ordering: multi-token forms (`NoCycle`, `NoMinvalue`, `NoMaxvalue`,
    /// `OwnedBy`, `SequenceName`) before any single-token form they share a first
    /// token with so longest-match-wins picks the longer spelling.
    #[derive(Debug)]
    pub enum SeqOption {
        #[tok(NO, CYCLE)]
        NoCycle,
        #[tok(NO, MINVALUE)]
        NoMinvalue,
        #[tok(NO, MAXVALUE)]
        NoMaxvalue,
        As(SeqAsOption),
        Increment(SeqIncrementOption),
        Minvalue(SeqMinValueOption),
        Maxvalue(SeqMaxValueOption),
        Start(SeqStartOption),
        Cache(SeqCacheOption),
        OwnedBy(SeqOwnedByOption),
        Restart(SeqRestartOption),
        SequenceName(SeqSequenceNameOption),
        #[tok(CYCLE)]
        Cycle,
        /// Added in 15: REL_15_19 gram.y `SeqOptElem: ... | LOGGED` (4741).
        /// The research lists it under PostgreSQL 17, "Changes to existing
        /// statements" (commit f7567f9e53d, back-patched to 15 and 16).
        /// REL_14_24 gram.y `SeqOptElem` has no `LOGGED` or `UNLOGGED`.
        #[cfg(feature = "since-pg15")]
        #[tok(LOGGED)]
        Logged,
        /// Added in 15: REL_15_19 gram.y `SeqOptElem: ... | UNLOGGED` (4781).
        /// Same commit and research entry as [`SeqOption::Logged`].
        #[cfg(feature = "since-pg15")]
        #[tok(UNLOGGED)]
        Unlogged,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(CREATE, this)]
    pub struct CreateSequenceStmt {
        /// Optional temporary persistence modifier: `TEMP`, `TEMPORARY`, or
        /// `UNLOGGED`. Postgres' `OptTemp` covers all three between `CREATE` and
        /// `SEQUENCE`.
        pub persistence: Option<CreatePersistence>,
        pub sequence: SequenceKeyword,
        pub if_not_exists: Option<IfNotExists>,
        pub name: QualifiedName,
        pub options: zero_or_many!(SeqOption),
    }
}

recursa::ast_node! {
    /// Required `SEQUENCE` keyword after the optional persistence modifier.
    #[derive(Debug)]
    pub enum SequenceKeyword {
        #[tok(SEQUENCE)]
        Sequence,
    }
}

recursa::ast_node! {
    /// Persistence modifier between `CREATE` and an object keyword: `TEMP`,
    /// `TEMPORARY`, `UNLOGGED`, or the longer `GLOBAL TEMPORARY`/`LOCAL TEMPORARY`
    /// forms (deprecated but still accepted).
    ///
    /// Variant ordering: multi-keyword forms (`GLOBAL TEMP[ORARY]`,
    /// `LOCAL TEMP[ORARY]`) — none of which are exercised by the sequence corpus
    /// but kept for forward-compat — would come first; today only `Temporary`,
    /// `Temp`, `Unlogged` are modelled.
    #[derive(Debug)]
    pub enum CreatePersistence {
        #[tok(TEMPORARY)]
        Temporary,
        #[tok(TEMP)]
        Temp,
        #[tok(UNLOGGED)]
        Unlogged,
    }
}

recursa::ast_node! {
    /// `DROP SEQUENCE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, SEQUENCE, this)]
    pub struct DropSequenceStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `SET LOGGED` — Postgres' `alter_table_cmd` SET LOGGED branch. Used by
    /// ALTER SEQUENCE in the corpus (and by ALTER TABLE, modelled separately).
    #[derive(Debug)]
    pub enum SetLoggedClause {
        #[tok(SET, LOGGED)]
        Value,
    }
}

recursa::ast_node! {
    /// `SET UNLOGGED` — Postgres' `alter_table_cmd` SET UNLOGGED branch.
    /// `UNLOGGED` is the existing hard keyword token; `SET` precedes it here.
    #[derive(Debug)]
    pub enum SetUnloggedClause {
        #[tok(SET, UNLOGGED)]
        Value,
    }
}

recursa::ast_node! {
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
    ///   `AS`, `CACHE`, `CYCLE`, `INCREMENT`, `LOGGED`, `MAXVALUE`,
    ///   `MINVALUE`, `NO …`, `OWNED`, `RESTART`, `SEQUENCE`, `START`,
    ///   `UNLOGGED` — none of which conflict with the keyword-led variants
    ///   above.
    #[derive(Debug)]
    pub enum AlterSequenceAction {
        SetLogged(SetLoggedClause),
        SetUnlogged(SetUnloggedClause),
        SetSchema(SetSchemaClause),
        Rename(RenameTo),
        Opts(SeqOptList),
    }
}

recursa::ast_node! {
    /// Non-empty list of `SeqOptElem`s — Postgres' `SeqOptList`. A `Vec` would
    /// allow the empty case (gram.y requires at least one), but recursa's
    /// `Vec` is implemented as a `Seq0` and cannot be empty here without an
    /// alternation that already covers the no-options case. We use a struct
    /// with a single `Seq1`-style field instead so the action enum can peek
    /// on a non-empty SeqOpt and commit. The leading `UNLOGGED` SeqOption is
    /// the bare `UNLOGGED` keyword form — distinct from the `SET UNLOGGED`
    /// branch above (which has the leading `SET`).
    #[derive(Debug)]
    pub struct SeqOptList {
        pub head: SeqOption,
        pub rest: zero_or_many!(SeqOption),
    }
}

recursa::ast_node! {
    /// `ALTER SEQUENCE [IF EXISTS] name action` — Postgres' `AlterSeqStmt`,
    /// the sequence-applicable subset of ALTER TABLE's `alter_table_cmds`,
    /// and `RenameStmt` / `AlterObjectSchemaStmt` branches for sequences.
    #[derive(Debug)]
    #[tok(ALTER, SEQUENCE, this)]
    pub struct AlterSequenceStmt {
        pub if_exists: Option<IfExists>,
        pub name: QualifiedName,
        pub action: AlterSequenceAction,
    }
}
