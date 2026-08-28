//! REINDEX statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;
use crate::tokens::keyword::*;

/// `REINDEX … { INDEX | TABLE } [CONCURRENTLY] qualified_name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReindexRelation<'input> {
    pub kind: ReindexRelationKind,
    pub concurrently: Option<CONCURRENTLY>,
    pub name: QualifiedName<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ReindexRelationKind {
    Index(INDEX),
    Table(TABLE),
}

/// `REINDEX … SCHEMA [CONCURRENTLY] name` — Postgres' `reindex_target_relation`
/// branch for `SCHEMA`, which always requires a name.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReindexSchemaTarget<'input> {
    pub schema: SCHEMA,
    pub concurrently: Option<CONCURRENTLY>,
    pub name: crate::tokens::ColId<'input>,
}

/// `REINDEX … { SYSTEM | DATABASE } [CONCURRENTLY] [name]` — Postgres'
/// `reindex_target_all`, where the trailing name is optional
/// (`opt_single_name`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReindexAllTarget<'input> {
    pub kind: ReindexAllKind,
    pub concurrently: Option<CONCURRENTLY>,
    pub name: Option<crate::tokens::ColId<'input>>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ReindexAllKind {
    System(SYSTEM),
    Database(DATABASE),
}

/// The full target portion of a `REINDEX` statement.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`INDEX` / `TABLE` / `SCHEMA` / `SYSTEM` / `DATABASE`) so first-set
/// disambiguation is unambiguous.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct ReindexStmt<'input> {
    pub reindex: REINDEX,
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
        let mut input = crate::tokens::test_input("REINDEX (TABLESPACE ts) TABLE tbl");
        let _stmt = ReindexStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_reindex_verbose_index() {
        let mut input = crate::tokens::test_input("REINDEX (VERBOSE) INDEX i");
        let _stmt = ReindexStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
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
