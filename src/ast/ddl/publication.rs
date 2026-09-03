//! PUBLICATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::{DefElem, DefList};
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// Optional `(col, ...)` column-list on a publication table object —
/// Postgres' `opt_column_list`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct PublicationColumnList<'input> {
    #[sep(COMMA)]
    pub cols: recursa::Vec1<crate::tokens::ColId<'input>>,
}

/// `WHERE (a_expr)` row-filter on a publication table object —
/// Postgres' `OptWhereClause`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationWhereClause<'input> {
    #[tok(WHERE, LPAREN, this, RPAREN)]
    pub expr: Box<Expr<'input>>,
}

/// `TABLE [ONLY] name [*] [(cols)] [WHERE (expr)]` — the `TABLE`-prefixed
/// publication object.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjTable<'input> {
    #[tok(TABLE, this)]
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// `TABLES IN SCHEMA name [(cols)]` — the schema-scoped publication
/// object. The schema name is an unqualified identifier (PG accepts
/// `CURRENT_SCHEMA` as a special keyword form; pg-sql does not lex it as
/// a keyword so it flows through as a bare `Ident`).
///
/// Per gram.y `PublicationObjSpec: TABLES IN SCHEMA ColId`, no column
/// list is permitted — PG rejects `TABLES IN SCHEMA foo (a, b)`
/// syntactically. The corpus (`publication.sql`) exercises this PG-rejected
/// form to verify the error; pg-sql accepts it over-permissively so the
/// statement is modelled instead of surfacing as a
/// a file-level parse error. The round-tripped output is still
/// PG-rejected, so the differential oracle stays valid.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjTablesInSchema<'input> {
    #[tok(TABLES, IN, SCHEMA, this)]
    pub name: crate::tokens::ColId<'input>,
    pub columns: Option<PublicationColumnList<'input>>,
}

/// `ONLY`-prefixed continuation publication object (Postgres'
/// `extended_relation_expr`-with-ONLY branch). Used after a `TABLE` or
/// `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list` infers the
/// object kind from the previous prefixed item at semantic time.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjOnly<'input> {
    #[tok(ONLY, this)]
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// Bare-name continuation publication object — `qualified_name [*]
/// [(cols)] [WHERE (expr)]` with no leading keyword. Used after a
/// `TABLE` / `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list`
/// infers the object kind from the previous prefixed item at semantic
/// time.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjBare<'input> {
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// One entry in a publication object list — Postgres' `PublicationObjSpec`.
///
/// Variant ordering: keyword-prefixed forms first (`Table` reserved,
/// `TablesInSchema` soft, `Only` reserved), then the catch-all `Bare`
/// (starts with an ident) so the keyword-prefixed forms win on peek.
#[derive(recursa::Node, Debug, Clone)]
pub enum PublicationObjSpec<'input> {
    Table(PublicationObjTable<'input>),
    TablesInSchema(PublicationObjTablesInSchema<'input>),
    Only(PublicationObjOnly<'input>),
    Bare(PublicationObjBare<'input>),
}

/// `FOR ALL TABLES` — Postgres' `CREATE PUBLICATION ... FOR ALL TABLES`.
#[derive(recursa::Node, Debug, Clone)]
pub enum PublicationForAllTables {
    #[tok(FOR, ALL, TABLES)]
    Value,
}

/// `FOR pub_obj_list` — Postgres' `CREATE PUBLICATION ... FOR pub_obj_list`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FOR, this)]
pub struct PublicationForObjects<'input> {
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input>>,
}

/// The `FOR ...` clause on CREATE PUBLICATION.
///
/// Variant ordering: `AllTables` (`FOR ALL TABLES`, 3 tokens) before
/// `Objects` (`FOR pub_obj_list`, starts with `TABLE`/`TABLES`/ident)
/// — longest match wins on the `FOR ALL TABLES` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum PublicationForClause<'input> {
    AllTables(PublicationForAllTables),
    Objects(PublicationForObjects<'input>),
}

/// `WITH (def_list)` — Postgres' `opt_definition`. Reuses the shared
/// `DefList` and prefixes it with the `WITH` keyword.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithDefinition<'input> {
    #[tok(WITH, this)]
    pub list: DefList<'input>,
}

/// `CREATE PUBLICATION name [FOR ALL TABLES | FOR pub_obj_list]
/// [WITH (def_list)]` — Postgres' `CreatePublicationStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreatePublicationStmt<'input> {
    #[tok(CREATE, PUBLICATION, this)]
    pub name: crate::tokens::ColId<'input>,
    pub r#for: Option<PublicationForClause<'input>>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP PUBLICATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, PUBLICATION, this)]
pub struct DropPublicationStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET (def_list)` — Postgres' `definition` (parenthesised def_list) on
/// `ALTER PUBLICATION name SET ...` and `ALTER SUBSCRIPTION name
/// { SET | SKIP } ...`. Distinct from `WithDefinition` (which carries a
/// leading `WITH` keyword) and from `SetSchemaClause` (which carries a
/// `SCHEMA` keyword).
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, LPAREN, this, RPAREN)]
pub struct SetDefinitionClause<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<DefElem<'input>>,
}

/// `SET pub_obj_list` — Postgres' `ALTER PUBLICATION ... SET pub_obj_list`.
/// Distinct from `SetDefinitionClause` because the body is a publication
/// object list (TABLE / TABLES IN SCHEMA / bare name), not a def_list.
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, this)]
pub struct AlterPublicationSetObjects<'input> {
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input>>,
}

/// `ADD pub_obj_list` — Postgres' `ALTER PUBLICATION ... ADD pub_obj_list`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ADD, this)]
pub struct AlterPublicationAddObjects<'input> {
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input>>,
}

/// `DROP pub_obj_list` — Postgres' `ALTER PUBLICATION ... DROP pub_obj_list`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, this)]
pub struct AlterPublicationDropObjects<'input> {
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input>>,
}

/// One action on `ALTER PUBLICATION name action` — covers Postgres'
/// `AlterPublicationStmt` (`SET definition`, `{ ADD | DROP | SET }
/// pub_obj_list`) plus the `RENAME TO` / `OWNER TO` branches from
/// `RenameStmt` / `AlterOwnerStmt`.
///
/// Variant ordering: `Rename` and `Owner` have distinct first keywords
/// (`RENAME`, `OWNER`). The three `pub_obj_list` actions share `ADD` /
/// `DROP` / `SET` first tokens; `SET (def_list)` and `SET pub_obj_list`
/// both start with `SET` and disambiguate by the next token (`(` →
/// def_list, anything else → pub_obj_list). Lists `SetDef` before
/// `SetObjs` so the `SET (` longer-prefix peek wins.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterPublicationAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    AddObjs(AlterPublicationAddObjects<'input>),
    DropObjs(AlterPublicationDropObjects<'input>),
    SetDef(SetDefinitionClause<'input>),
    SetObjs(AlterPublicationSetObjects<'input>),
}

/// `ALTER PUBLICATION name action` — Postgres' `AlterPublicationStmt`
/// plus the publication branches of `RenameStmt` / `AlterOwnerStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPublicationStmt<'input> {
    #[tok(ALTER, PUBLICATION, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterPublicationAction<'input>,
}
