//! DOMAIN DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::trigger::ConstraintAttributeElem;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `COLLATE name` clause on a domain.
#[derive(recursa::Node, Debug, Clone)]
pub struct DomainCollate<'input> {
    #[tok(COLLATE, this)]
    pub name: QualifiedName<'input>,
}

/// `[CONSTRAINT name]` prefix on a domain constraint.
#[derive(recursa::Node, Debug, Clone)]
pub struct DomainConstraintName<'input> {
    #[tok(CONSTRAINT, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `NOT NULL` domain constraint body.
#[derive(recursa::Node, Debug, Clone)]
pub enum DomainNotNull {
    #[tok(NOT, NULL)]
    Value,
}

/// `CHECK (expr)` domain constraint body.
#[derive(recursa::Node, Debug, Clone)]
pub struct DomainCheckBody<'input> {
    #[tok(CHECK, LPAREN, this, RPAREN)]
    pub expr: Box<Expr<'input>>,
}

/// `DEFAULT expr` clause — domain default value.
#[derive(recursa::Node, Debug, Clone)]
pub struct DomainDefault<'input> {
    /// Greedy: the expression keeps extending on NOT instead of yielding to what may follow `DomainDefault`.
    #[greedy(NOT)]
    #[tok(DEFAULT, this)]
    pub expr: Box<Expr<'input>>,
}

/// Body of a domain constraint — Postgres' `DomainConstraintElem` plus the
/// `DEFAULT expr` form (which is split out from `ColConstraintElem` by
/// `SplitColQualList` in gram.y).
///
/// Variant ordering: `NotNull` (`NOT NULL`, 2 tokens) before `Null`; `Check`
/// and `Default` are keyword-led and unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum DomainConstraintBody<'input> {
    NotNull(DomainNotNull),
    #[tok(NULL)]
    Null,
    Check(DomainCheckBody<'input>),
    Default(DomainDefault<'input>),
}

/// A single domain constraint — `[CONSTRAINT name] body`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DomainConstraint<'input> {
    pub name: Option<DomainConstraintName<'input>>,
    pub body: DomainConstraintBody<'input>,
}

/// `CREATE DOMAIN name [AS] Typename [COLLATE name] [constraint_list]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateDomainStmt<'input> {
    #[tok(CREATE, DOMAIN, this)]
    pub name: QualifiedName<'input>,
    #[tok(optional(AS), this)]
    pub type_name: CastType<'input>,
    pub collate: Option<DomainCollate<'input>>,
    /// Greedy: a leading token from any of 5 kinds starts this element instead of ending `CreateDomainStmt` (bison shift preference).
    #[greedy(CHECK, CONSTRAINT, DEFAULT, NOT, NULL)]
    pub constraints: Vec<DomainConstraint<'input>>,
}

/// `DROP DOMAIN [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, DOMAIN, this)]
pub struct DropDomainStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub types: TypeNameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `CHECK (expr) ConstraintAttributeSpec` — Postgres' CHECK arm of
/// `DomainConstraintElem` (the ALTER DOMAIN-specific form). Differs from
/// CREATE DOMAIN's `DomainCheckBody` by carrying the optional trailing
/// `ConstraintAttributeSpec` (e.g. `NOT VALID`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainCheckConstraint<'input> {
    #[tok(CHECK, LPAREN, this, RPAREN)]
    pub expr: Box<Expr<'input>>,
    /// Greedy: a leading DEFERRABLE, INITIALLY, NO, NOT starts this element instead of ending `AlterDomainCheckConstraint` (bison shift preference).
    #[greedy(DEFERRABLE, INITIALLY, NO, NOT)]
    pub attrs: Vec<ConstraintAttributeElem>,
}

/// `NOT NULL ConstraintAttributeSpec` — Postgres' NOT NULL arm of
/// `DomainConstraintElem` (ALTER DOMAIN-specific form). The corpus
/// exercises the bare `NOT NULL` form as well as the optional
/// `ConstraintAttributeSpec` trailer the grammar allows.
///
/// The `NOT NULL` keywords sit on the struct, not on the `attrs` field: a
/// field-level `#[tok(...)]` on a repeated field binds to each element, so
/// it would demand one `NOT NULL` per attribute and reject the bare form
/// (which has no attributes at all).
#[derive(recursa::Node, Debug, Clone)]
#[tok(NOT, NULL, this)]
pub struct AlterDomainNotNullConstraint {
    /// Greedy: a leading DEFERRABLE, INITIALLY, NO, NOT starts this element instead of ending `AlterDomainNotNullConstraint` (bison shift preference).
    #[greedy(DEFERRABLE, INITIALLY, NO, NOT)]
    pub attrs: Vec<ConstraintAttributeElem>,
}

/// One body of an ALTER DOMAIN ADD constraint — Postgres'
/// `DomainConstraintElem`.
///
/// Variant ordering: variants begin with distinct keywords (`CHECK` /
/// `NOT`), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterDomainConstraintElem<'input> {
    Check(AlterDomainCheckConstraint<'input>),
    NotNull(AlterDomainNotNullConstraint),
}

/// `[CONSTRAINT name] DomainConstraintElem` on ALTER DOMAIN ADD —
/// reuses the shared `DomainConstraintName` prefix from CREATE DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainConstraint<'input> {
    pub name: Option<DomainConstraintName<'input>>,
    pub elem: AlterDomainConstraintElem<'input>,
}

/// `ADD [CONSTRAINT name] DomainConstraintElem` — ADD action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainAdd<'input> {
    #[tok(ADD, this)]
    pub constraint: AlterDomainConstraint<'input>,
}

/// `DROP CONSTRAINT [IF EXISTS] name [CASCADE | RESTRICT]` — DROP CONSTRAINT
/// action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, CONSTRAINT, this)]
pub struct AlterDomainDropConstraint<'input> {
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `VALIDATE CONSTRAINT name` — VALIDATE action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainValidate<'input> {
    #[tok(VALIDATE, CONSTRAINT, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `RENAME CONSTRAINT old TO new` — RenameStmt branch for domain constraints.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainRenameConstraint<'input> {
    #[tok(RENAME, CONSTRAINT, this)]
    pub old_name: crate::tokens::ColId<'input>,
    #[tok(TO, this)]
    pub new_name: crate::tokens::ColId<'input>,
}

/// `SET DEFAULT expr` — SET DEFAULT action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainSetDefault<'input> {
    #[tok(SET, DEFAULT, this)]
    pub expr: Box<Expr<'input>>,
}

/// `DROP DEFAULT` — DROP DEFAULT action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterDomainDropDefault {
    #[tok(DROP, DEFAULT)]
    Value,
}

/// `SET NOT NULL` — SET NOT NULL action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterDomainSetNotNull {
    #[tok(SET, NOT, NULL)]
    Value,
}

/// `DROP NOT NULL` — DROP NOT NULL action on ALTER DOMAIN.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterDomainDropNotNull {
    #[tok(DROP, NOT, NULL)]
    Value,
}

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
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterDomainAction<'input> {
    Add(AlterDomainAdd<'input>),
    DropConstraint(AlterDomainDropConstraint<'input>),
    DropNotNull(AlterDomainDropNotNull),
    DropDefault(AlterDomainDropDefault),
    SetNotNull(AlterDomainSetNotNull),
    SetDefault(AlterDomainSetDefault<'input>),
    SetSchema(SetSchemaClause<'input>),
    Validate(AlterDomainValidate<'input>),
    RenameConstraint(AlterDomainRenameConstraint<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER DOMAIN any_name action` — Postgres' `AlterDomainStmt`,
/// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDomainStmt<'input> {
    #[tok(ALTER, DOMAIN, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterDomainAction<'input>,
}
