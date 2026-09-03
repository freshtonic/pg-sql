//! TYPE  DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::publication::SetDefinitionClause;
use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// A single column in `CREATE TYPE name AS (col_list)` — Postgres'
/// `TableFuncElement`: `ColId Typename [COLLATE name]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompositeTypeColumn<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub type_name: CastType<'input>,
    pub collate: Option<CompositeTypeCollate<'input>>,
}

/// `COLLATE name` clause on a composite-type column.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompositeTypeCollate<'input> {
    #[tok(COLLATE, this)]
    pub name: QualifiedName<'input>,
}

/// `AS (col_list)` — composite-type definition body.
#[derive(recursa::Node, Debug, Clone)]
#[tok(AS, LPAREN, this, RPAREN)]
pub struct CreateTypeComposite<'input> {
    #[sep(COMMA)]
    pub columns: Vec<CompositeTypeColumn<'input>>,
}

/// `AS ENUM ('label', ...)` — enum-type definition body. The label list may
/// be empty (Postgres allows `AS ENUM ()` to create a shell-only enum).
#[derive(recursa::Node, Debug, Clone)]
#[tok(AS, ENUM, LPAREN, this, RPAREN)]
pub struct CreateTypeEnum<'input> {
    #[sep(COMMA)]
    pub labels: Vec<literal::StringLit<'input>>,
}

/// `AS RANGE (def_list)` — range-type definition body.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTypeRange<'input> {
    #[tok(AS, RANGE, this)]
    pub definition: DefList<'input>,
}

/// The body of a `CREATE TYPE name ‹body›` statement.
///
/// Variant ordering: multi-keyword forms (`AS ENUM`, `AS RANGE`) before
/// `Composite` (`AS` + paren list) so the longer match wins. `Base` is the
/// `(def_list)` form (no `AS`); it begins with `(` and so cannot collide
/// with the `AS …` variants.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateTypeBody<'input> {
    Enum(CreateTypeEnum<'input>),
    Range(CreateTypeRange<'input>),
    Composite(CreateTypeComposite<'input>),
    Base(DefList<'input>),
}

/// `CREATE TYPE name [body]`.
///
/// - `CREATE TYPE name` — shell type
/// - `CREATE TYPE name AS (col_list)` — composite
/// - `CREATE TYPE name AS ENUM (labels)` — enum
/// - `CREATE TYPE name AS RANGE (def_list)` — range
/// - `CREATE TYPE name (def_list)` — base type
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTypeStmt<'input> {
    #[tok(CREATE, TYPE, this)]
    pub name: QualifiedName<'input>,
    pub body: Option<CreateTypeBody<'input>>,
}

/// `DROP TYPE [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, TYPE, this)]
pub struct DropTypeStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub types: TypeNameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `RENAME ATTRIBUTE old TO new [CASCADE | RESTRICT]` — Postgres'
/// `RenameStmt` branch for composite-type attribute renames.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeRenameAttribute<'input> {
    #[tok(RENAME, ATTRIBUTE, this)]
    pub old_name: crate::tokens::ColId<'input>,
    #[tok(TO, this)]
    pub new_name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `RENAME VALUE old_value TO new_value` — Postgres' `AlterEnumStmt`
/// branch for renaming enum values. Both literals are string literals.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeRenameValue<'input> {
    #[tok(RENAME, VALUE, this)]
    pub old_value: literal::StringLit<'input>,
    #[tok(TO, this)]
    pub new_value: literal::StringLit<'input>,
}

/// `BEFORE 'value'` or `AFTER 'value'` — neighbor anchor on
/// `ALTER TYPE name ADD VALUE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterEnumValuePosition<'input> {
    Before(AlterEnumValueBefore<'input>),
    After(AlterEnumValueAfter<'input>),
}

/// `BEFORE 'neighbor'` — neighbour anchor on
/// `ALTER TYPE name ADD VALUE ... BEFORE 'neighbor'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterEnumValueBefore<'input> {
    #[tok(BEFORE, this)]
    pub neighbor: literal::StringLit<'input>,
}

/// `AFTER 'neighbor'` — neighbour anchor on
/// `ALTER TYPE name ADD VALUE ... AFTER 'neighbor'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterEnumValueAfter<'input> {
    #[tok(AFTER, this)]
    pub neighbor: literal::StringLit<'input>,
}

/// `ADD VALUE [IF NOT EXISTS] 'val' [{BEFORE|AFTER} 'neighbour']` —
/// Postgres' `AlterEnumStmt` ADD VALUE branch.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ADD, VALUE, this)]
pub struct AlterTypeAddValue<'input> {
    pub if_not_exists: Option<IfNotExists>,
    pub new_value: literal::StringLit<'input>,
    pub position: Option<AlterEnumValuePosition<'input>>,
}

/// `ADD ATTRIBUTE column_def [CASCADE | RESTRICT]` — one `alter_type_cmd`
/// in Postgres. `column_def` is modelled as the same `CompositeTypeColumn`
/// used by `CREATE TYPE name AS (...)` (Postgres' `TableFuncElement`):
/// `name typename [COLLATE qualified_name]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeAddAttribute<'input> {
    #[tok(ADD, ATTRIBUTE, this)]
    pub column: CompositeTypeColumn<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP ATTRIBUTE [IF EXISTS] name [CASCADE | RESTRICT]` — one
/// `alter_type_cmd` in Postgres.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, ATTRIBUTE, this)]
pub struct AlterTypeDropAttribute<'input> {
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `[SET DATA]` modifier preceding `TYPE` in
/// `ALTER ATTRIBUTE name [SET DATA] TYPE typename`. Postgres'
/// `opt_set_data`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetDataClause {
    #[tok(SET, DATA)]
    Value,
}

/// `ALTER ATTRIBUTE name [SET DATA] TYPE typename [COLLATE qual] [CASCADE
/// | RESTRICT]` — one `alter_type_cmd` in Postgres. The typename uses the
/// same `CastType` enum as `CREATE TYPE name AS (col_list)` column types.
/// The optional `COLLATE` clause reuses [`CompositeTypeCollate`] (Postgres'
/// `opt_collate_clause` — `COLLATE any_name`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeAlterAttribute<'input> {
    #[tok(ALTER, ATTRIBUTE, this)]
    pub name: crate::tokens::ColId<'input>,
    pub set_data: Option<SetDataClause>,
    #[tok(TYPE, this)]
    pub type_name: CastType<'input>,
    pub collate: Option<CompositeTypeCollate<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// One `alter_type_cmd` in Postgres — an `ADD ATTRIBUTE`, `DROP ATTRIBUTE`,
/// or `ALTER ATTRIBUTE` action on `ALTER TYPE name action [, action ...]`.
///
/// Variant ordering: each variant has a distinct leading keyword (`ADD`,
/// `DROP`, `ALTER`) followed by `ATTRIBUTE`. Order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTypeCmd<'input> {
    AddAttribute(AlterTypeAddAttribute<'input>),
    DropAttribute(AlterTypeDropAttribute<'input>),
    AlterAttribute(AlterTypeAlterAttribute<'input>),
}

/// One or more comma-separated `alter_type_cmd`s — Postgres'
/// `alter_type_cmds` non-terminal.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeCmdList<'input> {
    #[sep(COMMA)]
    pub cmds: recursa::Vec1<AlterTypeCmd<'input>>,
}

/// One action on `ALTER TYPE any_name action` — covers Postgres'
/// `RenameStmt` (RENAME TO, RENAME ATTRIBUTE), `AlterOwnerStmt`
/// (OWNER TO), `AlterObjectSchemaStmt` (SET SCHEMA), `AlterTypeStmt`
/// (SET (...)), `AlterEnumStmt` (ADD VALUE, RENAME VALUE), and
/// `alter_type_cmds` (ADD/DROP/ALTER ATTRIBUTE, comma-separated).
///
/// Variant ordering: variants with two-keyword prefixes go before
/// single-keyword variants that share the same first token.
/// `RENAME ATTRIBUTE` / `RENAME VALUE` (two tokens) before `RENAME TO`
/// (also two tokens — distinct second). `SET SCHEMA` / `SET (` (the
/// def-list form starts with `SET LPAREN`) — distinct second tokens.
/// `ADD VALUE` (two tokens) before `Cmds` (which can start with `ADD
/// ATTRIBUTE`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTypeAction<'input> {
    RenameAttribute(AlterTypeRenameAttribute<'input>),
    RenameValue(AlterTypeRenameValue<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    SetDef(SetDefinitionClause<'input>),
    AddValue(AlterTypeAddValue<'input>),
    Cmds(AlterTypeCmdList<'input>),
}

/// `ALTER TYPE any_name action` — Postgres' `AlterTypeStmt` /
/// `AlterEnumStmt` / `RenameStmt` / `AlterOwnerStmt` /
/// `AlterObjectSchemaStmt` branches for types, plus the composite-type
/// `alter_type_cmds` set (ADD/DROP/ALTER ATTRIBUTE, comma-separated).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeStmt<'input> {
    #[tok(ALTER, TYPE, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterTypeAction<'input>,
}
