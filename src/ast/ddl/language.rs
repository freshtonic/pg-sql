//! LANGUAGE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `INLINE name` — optional inline handler in `CREATE LANGUAGE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageInlineHandler<'input> {
    #[tok(INLINE, this)]
    pub name: QualifiedName<'input>,
}

/// `VALIDATOR name | NO VALIDATOR` — Postgres' `validator_clause`.
///
/// Variant ordering: the two-token `NO VALIDATOR` before the
/// `VALIDATOR name` so the longer match wins on a leading `NO`.
#[derive(recursa::Node, Debug, Clone)]
pub enum LanguageValidatorClause<'input> {
    #[tok(NO, VALIDATOR)]
    None,
    Some(LanguageValidator<'input>),
}

/// `VALIDATOR name` — the populated validator branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageValidator<'input> {
    #[tok(VALIDATOR, this)]
    pub name: QualifiedName<'input>,
}

/// `HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]` — the
/// populated CREATE LANGUAGE handler clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageHandlerClause<'input> {
    #[tok(HANDLER, this)]
    pub name: QualifiedName<'input>,
    pub inline: Option<LanguageInlineHandler<'input>>,
    pub validator: Option<LanguageValidatorClause<'input>>,
}

/// `CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE name
/// [HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]]` —
/// Postgres' `CreatePLangStmt`. The handler-less form is silently treated as
/// `CREATE EXTENSION` by PG; structurally it is still a CREATE LANGUAGE.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateLanguageStmt<'input> {
    #[tok(CREATE, this)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    #[tok(this, optional(PROCEDURAL), LANGUAGE)]
    #[presence(TRUSTED)]
    pub trusted: bool,
    pub name: crate::tokens::ColId<'input>,
    pub handler: Option<LanguageHandlerClause<'input>>,
}

/// `DROP [PROCEDURAL] LANGUAGE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, optional(PROCEDURAL), LANGUAGE, this)]
pub struct DropLanguageStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER [PROCEDURAL] LANGUAGE name action` — covers
/// Postgres' `RenameStmt` and `AlterOwnerStmt` branches for
/// procedural languages. Languages have no SET SCHEMA action.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterLanguageAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER [PROCEDURAL] LANGUAGE name action` — Postgres' `RenameStmt`
/// and `AlterOwnerStmt` branches for procedural languages.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterLanguageStmt<'input> {
    #[tok(ALTER, optional(PROCEDURAL), LANGUAGE, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterLanguageAction<'input>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/language.tests.rs"
));
