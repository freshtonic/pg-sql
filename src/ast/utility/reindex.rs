//! REINDEX statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;

/// `REINDEX … { INDEX | TABLE } [CONCURRENTLY] qualified_name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ReindexRelation<'input> {
    pub kind: ReindexRelationKind,
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub name: QualifiedName<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum ReindexRelationKind {
    #[tok(INDEX)]
    Index,
    #[tok(TABLE)]
    Table,
}

/// `REINDEX … SCHEMA [CONCURRENTLY] name` — Postgres' `reindex_target_relation`
/// branch for `SCHEMA`, which always requires a name.
#[derive(recursa::Node, Debug, Clone)]
pub struct ReindexSchemaTarget<'input> {
    #[tok(SCHEMA, this)]
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub name: crate::tokens::ColId<'input>,
}

/// `REINDEX … { SYSTEM | DATABASE } [CONCURRENTLY] [name]` — Postgres'
/// `reindex_target_all`, where the trailing name is optional
/// (`opt_single_name`).
#[derive(recursa::Node, Debug, Clone)]
pub struct ReindexAllTarget<'input> {
    pub kind: ReindexAllKind,
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub name: Option<crate::tokens::ColId<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum ReindexAllKind {
    #[tok(SYSTEM)]
    System,
    #[tok(DATABASE)]
    Database,
}

/// The full target portion of a `REINDEX` statement.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`INDEX` / `TABLE` / `SCHEMA` / `SYSTEM` / `DATABASE`) so first-set
/// disambiguation is unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum ReindexTarget<'input> {
    Relation(ReindexRelation<'input>),
    Schema(ReindexSchemaTarget<'input>),
    All(ReindexAllTarget<'input>),
}

/// ```sql
/// REINDEX [(option [, ...])]
///   { { INDEX | TABLE } [CONCURRENTLY] qualified_name
///   | SCHEMA            [CONCURRENTLY] name
///   | { SYSTEM | DATABASE } [CONCURRENTLY] [name] }
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(REINDEX, this)]
pub struct ReindexStmt<'input> {
    pub options: Option<VacuumOptions<'input>>,
    pub target: ReindexTarget<'input>,
}
