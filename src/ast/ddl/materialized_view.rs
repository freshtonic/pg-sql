//! MATERIALIZED VIEW DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::database::SetTablespaceClause;
use crate::ast::ddl::index::{AllInTablespaceBody, ResetReloptions, SetReloptions};
use crate::ast::ddl::trigger::DependsOnExtension;
use crate::ast::ddl::view::RenameColumnClause;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// Optional parenthesized materialized-view output-column list.
///
/// The wrapper owns the delimiters around the complete comma-separated list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct CreateMatViewColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<crate::tokens::ColId<'input>>,
);

/// `create_mv_target` — Postgres' materialized-view target clause:
/// `qualified_name [(col_list)] [USING am] [WITH (opts)] [TABLESPACE name]`.
///
/// Field order matches gram.y. Each trailer is optional and re-uses the
/// shared CREATE TABLE machinery (`UsingAccessMethodClause`, `WithStorage`,
/// `TablespaceClause`).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateMatViewTarget<'input> {
    pub name: QualifiedName<'input>,
    pub column_list: Option<CreateMatViewColumnList<'input>>,
    pub using: Option<crate::ast::ddl::table::UsingAccessMethodClause<'input>>,
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    pub tablespace: Option<crate::ast::ddl::table::TablespaceClause<'input>>,
}

/// `CREATE [UNLOGGED] MATERIALIZED VIEW [IF NOT EXISTS] target AS query
/// [WITH [NO] DATA]` — Postgres' `CreateMatViewStmt`. `target` carries the
/// optional column list, access method, storage options, and tablespace.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateMaterializedViewStmt<'input> {
    #[tok(CREATE, this, MATERIALIZED, VIEW)]
    #[presence(UNLOGGED)]
    pub unlogged: bool,
    pub if_not_exists: Option<IfNotExists>,
    pub target: CreateMatViewTarget<'input>,
    #[tok(AS, this)]
    pub query: Box<crate::ast::dml::values::Subquery<'input>>,
    pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
}

/// `DROP MATERIALIZED VIEW [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, MATERIALIZED, VIEW, this)]
pub struct DropMaterializedViewStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET ACCESS METHOD { name | DEFAULT }` — Postgres' alter_table_cmd
/// branch for changing a relation's table access method. Used by ALTER
/// MATERIALIZED VIEW (and ALTER TABLE).
///
/// Variant ordering: `Default` (keyword) before `Name` (`Ident`).
#[derive(recursa::Node, Debug, Clone)]
pub enum SetAccessMethodTarget<'input> {
    #[tok(DEFAULT)]
    Default,
    Name(crate::tokens::ColId<'input>),
}

/// `SET ACCESS METHOD { name | DEFAULT }` — Postgres' alter_table_cmd
/// branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetAccessMethodClause<'input> {
    #[tok(SET, ACCESS, METHOD, this)]
    pub target: SetAccessMethodTarget<'input>,
}

/// `COMPRESSION { name | DEFAULT }` — Postgres' `column_compression`.
///
/// Variant ordering: `Default` (keyword) before `Name` (`AliasName`).
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnCompressionTarget<'input> {
    #[tok(DEFAULT)]
    Default,
    Name(crate::tokens::ColId<'input>),
}

/// `ALTER [COLUMN] name SET COMPRESSION cm` — Postgres' alter_table_cmd
/// branch for changing a column's compression method. Used by ALTER
/// MATERIALIZED VIEW (and ALTER TABLE).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnSetCompression<'input> {
    #[tok(ALTER, optional(COLUMN), this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(SET, COMPRESSION, this)]
    pub target: ColumnCompressionTarget<'input>,
}

/// One `alter_table_cmd` from the ALTER MATERIALIZED VIEW corpus —
/// `SET TABLESPACE`, `SET ACCESS METHOD`, `ALTER COLUMN … SET COMPRESSION`,
/// or the schema/owner/rename actions. The list of cmds is comma-separated
/// (`alter_table_cmds`).
///
/// Variant ordering: keyword-disjoint variants — `SET TABLESPACE` /
/// `SET ACCESS METHOD` / `SET SCHEMA` all begin with `SET` but their second
/// tokens are distinct (`TABLESPACE` / `ACCESS` / `SCHEMA`); `Owner` /
/// `Rename` / `AlterColumn` start with distinct keywords. The
/// `RenameColumn` form precedes `Rename` because both start with `RENAME`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterMatViewCmd<'input> {
    SetTablespace(SetTablespaceClause<'input>),
    SetAccessMethod(SetAccessMethodClause<'input>),
    SetSchema(SetSchemaClause<'input>),
    SetReloptions(SetReloptions<'input>),
    ResetReloptions(ResetReloptions<'input>),
    AlterColumnCompression(AlterColumnSetCompression<'input>),
    RenameColumn(RenameColumnClause<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    Depends(DependsOnExtension<'input>),
}

/// Comma-separated `alter_table_cmds` on ALTER MATERIALIZED VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterMatViewCmds<'input> {
    #[sep(COMMA)]
    pub cmds: recursa::Vec1<AlterMatViewCmd<'input>>,
}

/// `[IF EXISTS] name action` — the per-matview branch of ALTER
/// MATERIALIZED VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterMaterializedViewSingle<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub cmds: AlterMatViewCmds<'input>,
}

/// `ALTER MATERIALIZED VIEW [IF EXISTS] name alter_table_cmds`
/// `ALTER MATERIALIZED VIEW ALL IN TABLESPACE name [OWNED BY role_list]
///   SET TABLESPACE new [NOWAIT]` — the two top-level shapes of Postgres'
/// `AlterTableStmt` branches that begin with `ALTER MATERIALIZED VIEW …`.
///
/// Variant ordering: `All` (`ALL`) before `Single` (`[IF EXISTS] name`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterMatViewBody<'input> {
    All(AllInTablespaceBody<'input>),
    Single(AlterMaterializedViewSingle<'input>),
}

/// `ALTER MATERIALIZED VIEW [IF EXISTS] name action` — corpus-exercised
/// subset of `alter_table_cmds` plus the bulk `ALL IN TABLESPACE` form.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterMaterializedViewStmt<'input> {
    #[tok(ALTER, MATERIALIZED, VIEW, this)]
    pub body: AlterMatViewBody<'input>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/materialized_view.tests.rs"
));
