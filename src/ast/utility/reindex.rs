//! REINDEX statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;
use crate::tokens::keyword::*;

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
    #[tok(INDEX)] Index,
    #[tok(TABLE)] Table,
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
    #[tok(SYSTEM)] System,
    #[tok(DATABASE)] Database,
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
pub struct ReindexStmt<'input> {
    #[tok(REINDEX, this)]
    pub options: Option<VacuumOptions<'input>>,
    pub target: ReindexTarget<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_reindex_tablespace_table() {
        let lexed = crate::tokens::lex("REINDEX (TABLESPACE ts) TABLE tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ReindexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reindex_verbose_index() {
        let lexed = crate::tokens::lex("REINDEX (VERBOSE) INDEX i");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ReindexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn reindex_table_is_modelled() {
        let stmt: ReindexStmt = parse_stmt("REINDEX TABLE concur_heap");
        assert!(stmt.options.is_none());
        reparse_stable::<ReindexStmt>("REINDEX TABLE concur_heap");
    }

    #[test]
    fn reindex_index_concurrently_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX INDEX CONCURRENTLY brin_insert_optimization_idx");
    }

    #[test]
    fn reindex_schema_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX SCHEMA concur_reindex_schema");
    }

    #[test]
    fn reindex_schema_concurrently_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX SCHEMA CONCURRENTLY pg_catalog");
    }

    #[test]
    fn reindex_database_with_name_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX DATABASE not_current_database");
    }

    #[test]
    fn reindex_system_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX (CONCURRENTLY) SYSTEM");
    }

    #[test]
    fn reindex_system_concurrently_with_name_roundtrips() {
        // Unparenthesized CONCURRENTLY between kind and name. PG rejects this
        // at runtime ("not allowed for SYSTEM") but the grammar accepts it.
        reparse_stable::<ReindexStmt>("REINDEX SYSTEM CONCURRENTLY postgres");
    }

    #[test]
    fn reindex_options_table_roundtrips() {
        let stmt: ReindexStmt = parse_stmt("REINDEX (TABLESPACE ts) TABLE tbl");
        assert!(stmt.options.is_some());
        reparse_stable::<ReindexStmt>("REINDEX (TABLESPACE ts) TABLE tbl");
    }

    #[test]
    fn reindex_qualified_name_roundtrips() {
        reparse_stable::<ReindexStmt>("REINDEX INDEX CONCURRENTLY pg_toast.pg_toast_1260_index");
    }
}
