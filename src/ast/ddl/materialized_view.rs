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

recursa::ast_node! {
    /// Optional parenthesized materialized-view output-column list.
    ///
    /// The wrapper owns the delimiters around the complete comma-separated list.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CreateMatViewColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
    );
}

recursa::ast_node! {
    /// `create_mv_target` — Postgres' materialized-view target clause:
    /// `qualified_name [(col_list)] [USING am] [WITH (opts)] [TABLESPACE name]`.
    ///
    /// Field order matches gram.y. Each trailer is optional and re-uses the
    /// shared CREATE TABLE machinery (`UsingAccessMethodClause`, `WithStorage`,
    /// `TablespaceClause`).
    #[derive(Debug)]
    pub struct CreateMatViewTarget {
        pub name: QualifiedName,
        pub column_list: Option<CreateMatViewColumnList>,
        pub using: Option<crate::ast::ddl::table::UsingAccessMethodClause>,
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        pub tablespace: Option<crate::ast::ddl::table::TablespaceClause>,
    }
}

recursa::ast_node! {
    /// `CREATE [UNLOGGED] MATERIALIZED VIEW [IF NOT EXISTS] target AS query
    /// [WITH [NO] DATA]` — Postgres' `CreateMatViewStmt`. `target` carries the
    /// optional column list, access method, storage options, and tablespace.
    #[derive(Debug)]
    pub struct CreateMaterializedViewStmt {
        #[tok(CREATE, this, MATERIALIZED, VIEW)]
        #[presence(UNLOGGED)]
        pub unlogged: bool,
        pub if_not_exists: Option<IfNotExists>,
        pub target: CreateMatViewTarget,
        #[tok(AS, this)]
        pub query: boxed!(crate::ast::dml::values::Subquery),
        pub with_data: Option<crate::ast::ddl::table::WithDataClause>,
    }
}

recursa::ast_node! {
    /// `DROP MATERIALIZED VIEW [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, MATERIALIZED, VIEW, this)]
    pub struct DropMaterializedViewStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `alter_table_cmd: SET ACCESS METHOD name`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `SET ACCESS METHOD { name | DEFAULT }` — Postgres' alter_table_cmd
    /// branch for changing a relation's table access method. Used by ALTER
    /// MATERIALIZED VIEW (and ALTER TABLE).
    ///
    /// Variant ordering: `Default` (keyword) before `Name` (`Ident`).
    #[derive(Debug)]
    pub enum SetAccessMethodTarget {
        // Added in 17: research, PostgreSQL 17, "Changes to existing statements"
        // (REL_17_11 gram.y 3098 `set_access_method_name`). REL_16_15 gram.y
        // 2838 takes only `name`.
        #[cfg(feature = "since-pg17")]
        #[tok(DEFAULT)]
        Default,
        Name(crate::tokens::ColId),
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `alter_table_cmd: SET ACCESS METHOD name`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `SET ACCESS METHOD { name | DEFAULT }` — Postgres' alter_table_cmd
    /// branch.
    #[derive(Debug)]
    pub struct SetAccessMethodClause {
        #[tok(SET, ACCESS, METHOD, this)]
        pub target: SetAccessMethodTarget,
    }
}

recursa::ast_node! {
    /// `COMPRESSION { name | DEFAULT }` — Postgres' `column_compression`.
    ///
    /// Variant ordering: `Default` (keyword) before `Name` (`AliasName`).
    #[derive(Debug)]
    pub enum ColumnCompressionTarget {
        #[tok(DEFAULT)]
        Default,
        Name(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `ALTER [COLUMN] name SET COMPRESSION cm` — Postgres' alter_table_cmd
    /// branch for changing a column's compression method. Used by ALTER
    /// MATERIALIZED VIEW (and ALTER TABLE).
    #[derive(Debug)]
    pub struct AlterColumnSetCompression {
        #[tok(ALTER, optional(COLUMN), this)]
        pub name: crate::tokens::ColId,
        #[tok(SET, COMPRESSION, this)]
        pub target: ColumnCompressionTarget,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterMatViewCmd {
        SetTablespace(SetTablespaceClause),
        /// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
        /// (REL_15_19 gram.y `alter_table_cmd: SET ACCESS METHOD name`).
        #[cfg(feature = "since-pg15")]
        SetAccessMethod(SetAccessMethodClause),
        SetSchema(SetSchemaClause),
        SetReloptions(SetReloptions),
        ResetReloptions(ResetReloptions),
        AlterColumnCompression(AlterColumnSetCompression),
        RenameColumn(RenameColumnClause),
        Rename(RenameTo),
        Owner(OwnerTo),
        Depends(DependsOnExtension),
    }
}

recursa::ast_node! {
    /// Comma-separated `alter_table_cmds` on ALTER MATERIALIZED VIEW.
    #[derive(Debug)]
    pub struct AlterMatViewCmds {
        #[sep(COMMA)]
        pub cmds: one_or_many!(AlterMatViewCmd),
    }
}

recursa::ast_node! {
    /// `[IF EXISTS] name action` — the per-matview branch of ALTER
    /// MATERIALIZED VIEW.
    #[derive(Debug)]
    pub struct AlterMaterializedViewSingle {
        pub if_exists: Option<IfExists>,
        pub name: QualifiedName,
        pub cmds: AlterMatViewCmds,
    }
}

recursa::ast_node! {
    /// `ALTER MATERIALIZED VIEW [IF EXISTS] name alter_table_cmds`
    /// `ALTER MATERIALIZED VIEW ALL IN TABLESPACE name [OWNED BY role_list]
    ///   SET TABLESPACE new [NOWAIT]` — the two top-level shapes of Postgres'
    /// `AlterTableStmt` branches that begin with `ALTER MATERIALIZED VIEW …`.
    ///
    /// Variant ordering: `All` (`ALL`) before `Single` (`[IF EXISTS] name`).
    #[derive(Debug)]
    pub enum AlterMatViewBody {
        All(AllInTablespaceBody),
        Single(AlterMaterializedViewSingle),
    }
}

recursa::ast_node! {
    /// `ALTER MATERIALIZED VIEW [IF EXISTS] name action` — corpus-exercised
    /// subset of `alter_table_cmds` plus the bulk `ALL IN TABLESPACE` form.
    #[derive(Debug)]
    pub struct AlterMaterializedViewStmt {
        #[tok(ALTER, MATERIALIZED, VIEW, this)]
        pub body: AlterMatViewBody,
    }
}
