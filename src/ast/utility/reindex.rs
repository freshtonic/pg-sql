//! REINDEX statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;

recursa::ast_node! {
    /// `REINDEX … { INDEX | TABLE } [CONCURRENTLY] qualified_name`.
    #[derive(Debug)]
    pub struct ReindexRelation {
        pub kind: ReindexRelationKind,
        #[presence(CONCURRENTLY)]
        pub concurrently: bool,
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum ReindexRelationKind {
        #[tok(INDEX)]
        Index,
        #[tok(TABLE)]
        Table,
    }
}

recursa::ast_node! {
    /// `REINDEX … SCHEMA [CONCURRENTLY] name` — Postgres' `reindex_target_relation`
    /// branch for `SCHEMA`, which always requires a name.
    #[derive(Debug)]
    pub struct ReindexSchemaTarget {
        #[tok(SCHEMA, this)]
        #[presence(CONCURRENTLY)]
        pub concurrently: bool,
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `REINDEX … { SYSTEM | DATABASE } [CONCURRENTLY] [name]` — Postgres'
    /// `reindex_target_all`, where the trailing name is optional
    /// (`opt_single_name`) from 16.
    #[derive(Debug)]
    pub struct ReindexAllTarget {
        pub kind: ReindexAllKind,
        #[presence(CONCURRENTLY)]
        pub concurrently: bool,
        // The name is optional from 16: research, PostgreSQL 16, "Changes to
        // existing statements" (commits 2cbc3c17a, 0a5f06b84). REL_15_19 gram.y
        // has `REINDEX reindex_target_multitable opt_concurrently name`.
        //
        // A shape rule, so the requirement belongs to the absence of the name
        // (ADR 0010). REL_15_19 raises no ereport for it: the rule has no
        // alternative without the name, so the parser gives a plain syntax
        // error and the gate carries no message.
        #[config(
            since = pg16,
            on = absent,
            cite = "gram.y REL_15_19:8988 `ReindexStmt: REINDEX reindex_target_multitable \
                    opt_concurrently name`; optional from 16, commits 2cbc3c17a, 0a5f06b84"
        )]
        pub name: Option<crate::tokens::ColId>,
        #[config(before = pg16)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum ReindexAllKind {
        #[tok(SYSTEM)]
        System,
        #[tok(DATABASE)]
        Database,
    }
}

recursa::ast_node! {
    /// The full target portion of a `REINDEX` statement.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`INDEX` / `TABLE` / `SCHEMA` / `SYSTEM` / `DATABASE`) so first-set
    /// disambiguation is unambiguous.
    #[derive(Debug)]
    pub enum ReindexTarget {
        Relation(ReindexRelation),
        Schema(ReindexSchemaTarget),
        All(ReindexAllTarget),
    }
}

recursa::ast_node! {
    /// ```sql
    /// REINDEX [(option [, ...])]
    ///   { { INDEX | TABLE } [CONCURRENTLY] qualified_name
    ///   | SCHEMA            [CONCURRENTLY] name
    ///   | { SYSTEM | DATABASE } [CONCURRENTLY] [name] }
    /// ```
    #[derive(Debug)]
    #[tok(REINDEX, this)]
    pub struct ReindexStmt {
        pub options: Option<VacuumOptions>,
        pub target: ReindexTarget,
    }
}
