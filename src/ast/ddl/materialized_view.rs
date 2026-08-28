//! MATERIALIZED VIEW DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::database::SetTablespaceClause;
use crate::ast::ddl::index::{AllInTablespaceBody, ResetReloptions, SetReloptions};
use crate::ast::ddl::trigger::DependsOnExtension;
use crate::ast::ddl::view::RenameColumnClause;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `create_mv_target` — Postgres' materialized-view target clause:
/// `qualified_name [(col_list)] [USING am] [WITH (opts)] [TABLESPACE name]`.
///
/// Field order matches gram.y. Each trailer is optional and re-uses the
/// shared CREATE TABLE machinery (`UsingAccessMethodClause`, `WithStorage`,
/// `TablespaceClause`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateMatViewTarget<'input> {
    pub name: QualifiedName<'input>,
    pub column_list: Option<
        Surrounded<punct::LParen, Seq1<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    >,
    pub using: Option<crate::ast::ddl::table::UsingAccessMethodClause<'input>>,
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    pub tablespace: Option<crate::ast::ddl::table::TablespaceClause<'input>>,
}

/// `CREATE [UNLOGGED] MATERIALIZED VIEW [IF NOT EXISTS] target AS query
/// [WITH [NO] DATA]` — Postgres' `CreateMatViewStmt`. `target` carries the
/// optional column list, access method, storage options, and tablespace.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateMaterializedViewStmt<'input> {
    pub create: CREATE,
    pub unlogged: Option<UNLOGGED>,
    pub materialized: MATERIALIZED,
    pub view: VIEW,
    pub if_not_exists: Option<IfNotExists>,
    pub target: CreateMatViewTarget<'input>,
    pub r#as: AS,
    pub query: Box<crate::ast::dml::values::Subquery<'input>>,
    pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
}

/// `DROP MATERIALIZED VIEW [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropMaterializedViewStmt<'input> {
    pub drop: DROP,
    pub materialized: MATERIALIZED,
    pub view: VIEW,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET ACCESS METHOD { name | DEFAULT }` — Postgres' alter_table_cmd
/// branch for changing a relation's table access method. Used by ALTER
/// MATERIALIZED VIEW (and ALTER TABLE).
///
/// Variant ordering: `Default` (keyword) before `Name` (`Ident`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetAccessMethodTarget<'input> {
    Default(DEFAULT),
    Name(literal::AliasName<'input>),
}

/// `SET ACCESS METHOD { name | DEFAULT }` — Postgres' alter_table_cmd
/// branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetAccessMethodClause<'input> {
    pub set: SET,
    pub access: ACCESS,
    pub method: METHOD,
    pub target: SetAccessMethodTarget<'input>,
}

/// `COMPRESSION { name | DEFAULT }` — Postgres' `column_compression`.
///
/// Variant ordering: `Default` (keyword) before `Name` (`AliasName`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ColumnCompressionTarget<'input> {
    Default(DEFAULT),
    Name(literal::AliasName<'input>),
}

/// `ALTER [COLUMN] name SET COMPRESSION cm` — Postgres' alter_table_cmd
/// branch for changing a column's compression method. Used by ALTER
/// MATERIALIZED VIEW (and ALTER TABLE).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColumnSetCompression<'input> {
    pub alter: ALTER,
    pub column: Option<COLUMN>,
    pub name: crate::tokens::ColId<'input>,
    pub set: SET,
    pub compression: COMPRESSION,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterMatViewCmds<'input> {
    pub cmds: Seq1<AlterMatViewCmd<'input>, punct::Comma>,
}

/// `[IF EXISTS] name action` — the per-matview branch of ALTER
/// MATERIALIZED VIEW.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterMatViewBody<'input> {
    All(AllInTablespaceBody<'input>),
    Single(AlterMaterializedViewSingle<'input>),
}

/// `ALTER MATERIALIZED VIEW [IF EXISTS] name action` — corpus-exercised
/// subset of `alter_table_cmds` plus the bulk `ALL IN TABLESPACE` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterMaterializedViewStmt<'input> {
    pub alter: ALTER,
    pub materialized: MATERIALIZED,
    pub view: VIEW,
    pub body: AlterMatViewBody<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_materialized_view_minimal() {
        let stmt: CreateMaterializedViewStmt =
            parse_stmt("CREATE MATERIALIZED VIEW mv AS SELECT 1");
        assert_eq!(stmt.target.name.object(), "mv");
        assert!(stmt.unlogged.is_none());
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.target.column_list.is_none());
    }

    #[test]
    fn create_materialized_view_with_no_data_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv AS SELECT * FROM t WITH NO DATA",
        );
    }

    #[test]
    fn create_materialized_view_if_not_exists_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW IF NOT EXISTS mv AS SELECT 1",
        );
    }

    #[test]
    fn create_materialized_view_using_access_method_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv USING heap2 AS SELECT * FROM t",
        );
    }

    #[test]
    fn create_materialized_view_column_list_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv (ii, jj) AS SELECT i, j FROM t",
        );
    }

    #[test]
    fn create_unlogged_materialized_view_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE UNLOGGED MATERIALIZED VIEW mv AS SELECT 1",
        );
    }

    #[test]
    fn create_materialized_view_with_storage_tablespace_roundtrips() {
        reparse_stable::<CreateMaterializedViewStmt>(
            "CREATE MATERIALIZED VIEW mv WITH (fillfactor = 50) TABLESPACE ts AS SELECT 1",
        );
    }
}
