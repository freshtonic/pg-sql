//! CONVERSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `CREATE [DEFAULT] CONVERSION name FOR 'src_enc' TO 'dst_enc' FROM func`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateConversionStmt<'input> {
    #[tok(CREATE, this, CONVERSION)]
    #[presence(DEFAULT)]
    pub default: bool,
    pub name: QualifiedName<'input>,
    #[tok(FOR, this)]
    pub src_encoding: literal::StringLit<'input>,
    #[tok(TO, this)]
    pub dest_encoding: literal::StringLit<'input>,
    #[tok(FROM, this)]
    pub func_name: QualifiedName<'input>,
}

/// `DROP CONVERSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, CONVERSION, this)]
pub struct DropConversionStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER CONVERSION any_name action` — Postgres'
/// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches
/// for conversions.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterConversionAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER CONVERSION any_name action` — Postgres' `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt` branches for conversions.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterConversionStmt<'input> {
    #[tok(ALTER, CONVERSION, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterConversionAction<'input>,
}
