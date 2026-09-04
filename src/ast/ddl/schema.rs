//! SCHEMA DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::sequence::CreateSequenceStmt;
use crate::ast::ddl::trigger::CreateTriggerStmt;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::grant::GrantStmt;
use crate::tokens::{literal, punct};

/// `AUTHORIZATION role_spec` clause on `CREATE SCHEMA`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SchemaAuthorization<'input> {
    #[tok(AUTHORIZATION, this)]
    pub role: RoleSpec<'input>,
}

/// `name [AUTHORIZATION role]` — schema name with optional authorization.
#[derive(recursa::Node, Debug, Clone)]
pub struct SchemaNameAndAuth<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub authorization: Option<SchemaAuthorization<'input>>,
}

/// The name/authorization clause of `CREATE SCHEMA` — Postgres allows either
/// a schema name (with optional `AUTHORIZATION`) or just `AUTHORIZATION role`
/// (which implicitly names the schema after the role).
///
/// Variant ordering: the `Authorization` form must come first so a leading
/// `AUTHORIZATION` keyword does not get consumed as the schema-name `Ident`
/// (soft keywords are reclaimable as identifiers, and `AUTHORIZATION` is
/// soft).
#[derive(recursa::Node, Debug, Clone)]
pub enum SchemaNameClause<'input> {
    Authorization(SchemaAuthorization<'input>),
    Named(SchemaNameAndAuth<'input>),
}

/// A single statement nested inside `CREATE SCHEMA … schema_element*`.
///
/// Postgres' `OptSchemaEltList` accepts a small subset of statements:
/// `CreateStmt | IndexStmt | CreateSeqStmt | CreateTrigStmt | GrantStmt |
/// ViewStmt`. We type each one with the relevant statement struct rather
/// than a general `Statement` to mirror the grammar precisely.
///
/// Variant ordering: multi-keyword `GRANT` distinct from the `CREATE …`
/// family; among the CREATE-led variants, `CreateView` (matches `CREATE [OR
/// REPLACE] [TEMP|TEMPORARY] [RECURSIVE] VIEW …`) is the most specific.
/// The other CREATE variants disambiguate on their `CREATE { TABLE | INDEX
/// | SEQUENCE | TRIGGER }` second-token.
#[derive(recursa::Node, Debug, Clone)]
pub enum SchemaElement<'input> {
    Grant(GrantStmt<'input>),
    CreateView(Box<crate::ast::ddl::view::CreateViewStmt<'input>>),
    CreateTable(Box<crate::ast::ddl::table::CreateTableStmt<'input>>),
    CreateIndex(Box<crate::ast::ddl::index::CreateIndexStmt<'input>>),
    CreateSequence(CreateSequenceStmt<'input>),
    CreateTrigger(Box<CreateTriggerStmt<'input>>),
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, SCHEMA, this)]
pub struct CreateSchemaStmt<'input> {
    pub if_not_exists: Option<crate::ast::shared::flags::IfNotExists>,
    /// Schema name and/or `AUTHORIZATION` clause. Postgres requires at least
    /// one — the enum forces that structurally.
    pub head: SchemaNameClause<'input>,
    /// Greedy: a leading CREATE, GRANT starts this element instead of ending `CreateSchemaStmt` (bison shift preference).
    #[greedy(CREATE, GRANT)]
    /// Nested `schema_element` statements — `OptSchemaEltList`. Each element
    /// is a top-level statement type; the surrounding semicolons live on the
    /// enclosing statement, not on the nested ones.
    pub elements: Vec<SchemaElement<'input>>,
}

/// `DROP SCHEMA [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, SCHEMA, this)]
pub struct DropSchemaStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER SCHEMA name action` — covers Postgres'
/// `RenameStmt` and `AlterOwnerStmt` branches for schemas. Schemas
/// have no SET SCHEMA action.
///
/// Variant ordering: variants begin with distinct keywords (`RENAME`,
/// `OWNER`), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterSchemaAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER SCHEMA name action` — Postgres' `RenameStmt` /
/// `AlterOwnerStmt` branches for schemas.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSchemaStmt<'input> {
    #[tok(ALTER, SCHEMA, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterSchemaAction<'input>,
}
