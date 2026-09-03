//! COLLATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// Body of `CREATE COLLATION` after the name: either a `def_list` of options
/// (`LOCALE`/`LC_COLLATE`/`PROVIDER`/...), or `FROM existing_collation_name`.
///
/// Variant ordering: `From` (keyword-led) before `Options` (paren-led) — they
/// begin with different tokens so peek disambiguation is unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateCollationBody<'input> {
    From(CollationFromClause<'input>),
    Options(DefList<'input>),
}

/// `FROM existing_collation_name` — copy an existing collation.
#[derive(recursa::Node, Debug, Clone)]
pub struct CollationFromClause<'input> {
    #[tok(FROM, this)]
    pub name: QualifiedName<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, COLLATION, this)]
pub struct CreateCollationStmt<'input> {
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    pub body: CreateCollationBody<'input>,
}

/// `DROP COLLATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, COLLATION, this)]
pub struct DropCollationStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `REFRESH VERSION` — Postgres' `AlterCollationStmt` action.
#[derive(recursa::Node, Debug, Clone)]
pub enum CollationRefreshVersion {
    #[tok(REFRESH, VERSION)]
    Value,
}

/// One action on `ALTER COLLATION any_name action` — Postgres'
/// `RenameStmt`, `AlterOwnerStmt`, `AlterObjectSchemaStmt`, and
/// `AlterCollationStmt` branches for collations.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`, `REFRESH`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterCollationAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    RefreshVersion(CollationRefreshVersion),
}

/// `ALTER COLLATION any_name action` — Postgres' `AlterCollationStmt`
/// (REFRESH VERSION) plus the collation branches of `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterCollationStmt<'input> {
    #[tok(ALTER, COLLATION, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterCollationAction<'input>,
}
