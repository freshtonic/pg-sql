//! DOMAIN DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::table::{
    CheckConstraint, ColumnConstraintAttr, GeneratedConstraint, PrimaryKeyConstraint,
    ReferencesConstraint, UniqueConstraint,
};
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `COLLATE name` clause on a domain.
    #[derive(Debug)]
    pub struct DomainCollate {
        #[tok(COLLATE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `[CONSTRAINT name]` prefix on a domain constraint — gram.y
    /// `ColConstraint: CONSTRAINT name ColConstraintElem`.
    #[derive(Debug)]
    pub struct DomainConstraintName {
        #[tok(CONSTRAINT, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `DEFAULT expr` clause — domain default value.
    #[derive(Debug)]
    pub struct DomainDefault {
        /// gram.y `ColConstraintElem: DEFAULT b_expr`, the restricted
        /// expression, so `DEFAULT 1 NOT NULL` ends the default at `NOT`.
        #[tok(DEFAULT, this)]
        pub expr: boxed!(BExpr),
    }
}

recursa::ast_node! {
    /// Body of a domain constraint — gram.y `ColConstraintElem`, which
    /// `CreateDomainStmt` reaches through `ColQualList`. The same element set
    /// serves a column of a table, so the arms reuse the nodes of
    /// [`crate::ast::ddl::table`]. Execution, not the raw parser, rejects the
    /// kinds that a domain cannot hold.
    ///
    /// Variant ordering: `NotNullNoInherit` (4 tokens) before `NotNull`; every
    /// other arm has its own leading keyword.
    #[derive(Debug)]
    pub enum DomainConstraintBody {
        Generated(GeneratedConstraint),
        PrimaryKey(PrimaryKeyConstraint),
        /// Added in 18: gram.y `ColConstraintElem: NOT NULL_P opt_no_inherit`
        /// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
        /// item 7; REL_18_6 gram.y `ColConstraintElem`).
        #[config(since = pg18)]
        #[tok(NOT, NULL, NO, INHERIT)]
        NotNullNoInherit,
        #[tok(NOT, NULL)]
        NotNull,
        #[tok(NULL)]
        Null,
        Unique(UniqueConstraint),
        References(ReferencesConstraint),
        Default(DomainDefault),
        Check(CheckConstraint),
    }
}

recursa::ast_node! {
    /// A named or unnamed domain constraint — gram.y `ColConstraint`'s two
    /// `ColConstraintElem` arms.
    #[derive(Debug)]
    pub struct DomainConstraint {
        pub name: Option<DomainConstraintName>,
        pub body: DomainConstraintBody,
    }
}

recursa::ast_node! {
    /// One entry of gram.y `ColQualList` — `ColConstraint`. A `ConstraintAttr`
    /// (`[NOT] DEFERRABLE`, `INITIALLY …`, and from 18 `[NOT] ENFORCED`) is an
    /// entry of its own, so it takes no `CONSTRAINT name` prefix.
    ///
    /// Variant ordering: `Attr` and `Constraint` share the `NOT` keyword and
    /// part at the token after it (`DEFERRABLE` or `ENFORCED` against `NULL`).
    #[derive(Debug)]
    pub enum DomainQual {
        Constraint(DomainConstraint),
        Attr(ColumnConstraintAttr),
    }
}

recursa::ast_node! {
    /// `CREATE DOMAIN name [AS] Typename [COLLATE name] ColQualList` — gram.y
    /// `CreateDomainStmt`.
    #[derive(Debug)]
    pub struct CreateDomainStmt {
        #[tok(CREATE, DOMAIN, this)]
        pub name: QualifiedName,
        #[tok(optional(AS), this)]
        pub type_name: CastType,
        pub collate: Option<DomainCollate>,
        pub quals: zero_or_many!(DomainQual),
    }
}

recursa::ast_node! {
    /// `DROP DOMAIN [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, DOMAIN, this)]
    pub struct DropDomainStmt {
        pub if_exists: Option<IfExists>,
        pub types: TypeNameList,
        pub behavior: Option<DropBehavior>,
    }
}

// Added in 17: `DomainConstraint` (research, PostgreSQL 17, "Changes to existing
// statements"; REL_17_11 gram.y 4254-4307, 11552).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y's `DomainConstraintElem`
    /// accepts after `CHECK (expr)`. `processCASbits` gets no `deferrable` and
    /// no `initdeferred` pointer, so `DEFERRABLE` and `INITIALLY DEFERRED` are
    /// raw-parser errors; from 18 it gets no `is_enforced` pointer either
    /// (REL_17_11 gram.y 4286-4288; REL_18_6 gram.y 4384-4386).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum DomainCheckAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(NOT, VALID)]
        NotValid,
        #[tok(NO, INHERIT)]
        NoInherit,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
    }
}

// Added in 17: see `DomainCheckAttr`.
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y's `DomainConstraintElem`
    /// accepts after `NOT NULL`. `processCASbits` gets only a `no_inherit`
    /// pointer (REL_17_11 gram.y 4300-4302), and from 18 no pointer at all, so
    /// `NO INHERIT` is a raw-parser error there (REL_18_6 gram.y 4399-4401).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum DomainNotNullAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        /// Removed in 18: REL_18_6 gram.y `DomainConstraintElem` passes no
        /// `no_inherit` pointer for `NOT NULL` (commit 14e87ffa5).
        #[config(before = pg18)]
        #[tok(NO, INHERIT)]
        NoInherit,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
    }
}

// Added in 17: `DomainConstraint` (research, PostgreSQL 17, "Changes to existing
// statements"; REL_17_11 gram.y 4254-4307, 11552).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `CHECK (expr) ConstraintAttributeSpec` — Postgres' CHECK arm of
    /// `DomainConstraintElem` (the ALTER DOMAIN-specific form). Differs from
    /// CREATE DOMAIN's `ColConstraintElem` CHECK by carrying the full
    /// `ConstraintAttributeSpec` (e.g. `NOT VALID`) instead of `opt_no_inherit`.
    #[derive(Debug)]
    pub struct AlterDomainCheckConstraint {
        #[tok(CHECK, LPAREN, this, RPAREN)]
        pub expr: boxed!(Expr),
        pub attrs: zero_or_many!(DomainCheckAttr),
    }
}

// Added in 17: `DomainConstraint` (research, PostgreSQL 17, "Changes to existing
// statements"; REL_17_11 gram.y 4254-4307, 11552).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `NOT NULL ConstraintAttributeSpec` — Postgres' NOT NULL arm of
    /// `DomainConstraintElem` (ALTER DOMAIN-specific form). The corpus
    /// exercises the bare `NOT NULL` form as well as the optional
    /// `ConstraintAttributeSpec` trailer the grammar allows.
    ///
    /// The `NOT NULL` keywords sit on the struct, not on the `attrs` field: a
    /// field-level `#[tok(...)]` on a repeated field binds to each element, so
    /// it would demand one `NOT NULL` per attribute and reject the bare form
    /// (which has no attributes at all).
    #[derive(Debug)]
    #[tok(NOT, NULL, this)]
    pub struct AlterDomainNotNullConstraint {
        pub attrs: zero_or_many!(DomainNotNullAttr),
    }
}

// Added in 17: `DomainConstraint` (research, PostgreSQL 17, "Changes to existing
// statements"; REL_17_11 gram.y 4254-4307, 11552).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// One body of an ALTER DOMAIN ADD constraint — Postgres'
    /// `DomainConstraintElem`.
    ///
    /// Variant ordering: variants begin with distinct keywords (`CHECK` /
    /// `NOT`), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterDomainConstraintElem {
        Check(AlterDomainCheckConstraint),
        /// Added in 17: the shape that the widening of `ALTER DOMAIN ... ADD`
        /// adds. Before 17 the body is a `TableConstraint`, whose
        /// `ConstraintElem` has no bare `NOT NULL` (REL_16_15 gram.y 11392;
        /// REL_17_11 gram.y 4254-4307, 11552).
        #[config(since = pg17)]
        NotNull(AlterDomainNotNullConstraint),
    }
}

// Added in 17: `DomainConstraint` (research, PostgreSQL 17, "Changes to existing
// statements"; REL_17_11 gram.y 4254-4307, 11552).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `[CONSTRAINT name] DomainConstraintElem` on ALTER DOMAIN ADD —
    /// reuses the shared `DomainConstraintName` prefix from CREATE DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainConstraint {
        pub name: Option<DomainConstraintName>,
        pub elem: AlterDomainConstraintElem,
    }
}

recursa::ast_node! {
    /// `ADD [CONSTRAINT name] DomainConstraintElem` — ADD action on ALTER DOMAIN.
    /// Before 17 it is `ADD TableConstraint`.
    #[derive(Debug)]
    pub struct AlterDomainAdd {
        // Changed in 17: research, PostgreSQL 17, "Changes to existing
        // statements". REL_17_11 gram.y 11552 takes `DomainConstraint`;
        // REL_16_15 gram.y 11392 takes `TableConstraint`, and execution
        // rejects the kinds other than `CHECK`.
        // A widening, not an addition (ADR 0010, `docs/minimum-version.md`):
        // the newer type accepts everything the older one does, so both arms
        // only remove and neither records. The gate for what 17 adds belongs
        // to those shapes.
        #[cfg(feature = "since-pg17")]
        #[tok(ADD, this)]
        pub constraint: AlterDomainConstraint,
        #[cfg(not(feature = "since-pg17"))]
        #[tok(ADD, this)]
        pub constraint: crate::ast::ddl::table::TableConstraint,
    }
}

recursa::ast_node! {
    /// `DROP CONSTRAINT [IF EXISTS] name [CASCADE | RESTRICT]` — DROP CONSTRAINT
    /// action on ALTER DOMAIN.
    #[derive(Debug)]
    #[tok(DROP, CONSTRAINT, this)]
    pub struct AlterDomainDropConstraint {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `VALIDATE CONSTRAINT name` — VALIDATE action on ALTER DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainValidate {
        #[tok(VALIDATE, CONSTRAINT, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `RENAME CONSTRAINT old TO new` — RenameStmt branch for domain constraints.
    #[derive(Debug)]
    pub struct AlterDomainRenameConstraint {
        #[tok(RENAME, CONSTRAINT, this)]
        pub old_name: crate::tokens::ColId,
        #[tok(TO, this)]
        pub new_name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `SET DEFAULT expr` — SET DEFAULT action on ALTER DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainSetDefault {
        #[tok(SET, DEFAULT, this)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `DROP DEFAULT` — DROP DEFAULT action on ALTER DOMAIN.
    #[derive(Debug)]
    pub enum AlterDomainDropDefault {
        #[tok(DROP, DEFAULT)]
        Value,
    }
}

recursa::ast_node! {
    /// `SET NOT NULL` — SET NOT NULL action on ALTER DOMAIN.
    #[derive(Debug)]
    pub enum AlterDomainSetNotNull {
        #[tok(SET, NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
    /// `DROP NOT NULL` — DROP NOT NULL action on ALTER DOMAIN.
    #[derive(Debug)]
    pub enum AlterDomainDropNotNull {
        #[tok(DROP, NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
    /// One action on `ALTER DOMAIN any_name action` — Postgres' `AlterDomainStmt`,
    /// `RenameStmt`, `AlterOwnerStmt` and `AlterObjectSchemaStmt` branches for
    /// domains.
    ///
    /// Variant ordering:
    /// - `SetNotNull` / `SetDefault` / `SetSchema` all begin with `SET`; the
    ///   two-token forms (`SetNotNull` = `SET NOT NULL`, `SetSchema` = `SET
    ///   SCHEMA`, `SetDefault` = `SET DEFAULT`) have distinct second tokens.
    /// - `DropNotNull` / `DropDefault` / `DropConstraint` all begin with
    ///   `DROP`; their second tokens (`NOT`, `DEFAULT`, `CONSTRAINT`) are
    ///   distinct.
    /// - `RenameConstraint` (two-token `RENAME CONSTRAINT`) must precede the
    ///   single-keyword `Rename` (`RENAME TO`) since both start with `RENAME`.
    #[derive(Debug)]
    pub enum AlterDomainAction {
        Add(AlterDomainAdd),
        DropConstraint(AlterDomainDropConstraint),
        DropNotNull(AlterDomainDropNotNull),
        DropDefault(AlterDomainDropDefault),
        SetNotNull(AlterDomainSetNotNull),
        SetDefault(AlterDomainSetDefault),
        SetSchema(SetSchemaClause),
        Validate(AlterDomainValidate),
        RenameConstraint(AlterDomainRenameConstraint),
        Rename(RenameTo),
        Owner(OwnerTo),
    }
}

recursa::ast_node! {
    /// `ALTER DOMAIN any_name action` — Postgres' `AlterDomainStmt`,
    /// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches.
    #[derive(Debug)]
    pub struct AlterDomainStmt {
        #[tok(ALTER, DOMAIN, this)]
        pub name: QualifiedName,
        pub action: AlterDomainAction,
    }
}
