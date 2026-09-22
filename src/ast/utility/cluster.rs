//! CLUSTER statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;

// --- CLUSTER ---

recursa::ast_node! {
    /// `USING index_name` — Postgres' `cluster_index_specification`.
    #[derive(Debug)]
    pub struct ClusterUsingIndex {
        #[tok(USING, this)]
        pub index: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Modern target: `qualified_name [USING index]`.
    #[derive(Debug)]
    pub struct ClusterModernTarget {
        pub table: QualifiedName,
        pub using_index: Option<ClusterUsingIndex>,
    }
}

recursa::ast_node! {
    /// Pre-8.3 legacy target: `index_name ON qualified_name`.
    ///
    /// `ON` after the first identifier disambiguates this from the modern form
    /// (which would have `USING` there, or nothing).
    #[derive(Debug)]
    pub struct ClusterLegacyTarget {
        pub index: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
    }
}

recursa::ast_node! {
    /// `CLUSTER` target: either the pre-8.3 `index ON table` form or the
    /// modern `table [USING index]` form.
    ///
    /// Variant ordering: the legacy form is listed first because both variants
    /// start with an identifier; the legacy form's distinguishing `ON` is
    /// reached after two tokens, while the modern form can stop after one
    /// identifier (no `USING`). Declaration-order tiebreak prefers the legacy
    /// form when both could parse a prefix, but the modern form is selected
    /// once the parser sees no `ON` after the leading identifier.
    #[derive(Debug)]
    pub enum ClusterTarget {
        Legacy(ClusterLegacyTarget),
        Modern(ClusterModernTarget),
    }
}

// From 17, `CLUSTER (options)` takes no table: research, PostgreSQL 17,
// "Changes to existing statements" (REL_17_11 gram.y 11739).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// ```sql
    /// CLUSTER '(' option [, ...] ')' [qualified_name [USING index]]
    /// CLUSTER [VERBOSE]              [qualified_name [USING index]]
    /// CLUSTER [VERBOSE] index ON qualified_name              -- pre-8.3
    /// ```
    ///
    /// In the parenthesised form, `options` is `Some` and `verbose` is `None`
    /// (the option list expresses `VERBOSE` instead). In any legacy form,
    /// `options` is `None` and `verbose` may be `Some` or `None`.
    #[derive(Debug)]
    #[tok(CLUSTER, this)]
    pub struct ClusterStmt {
        pub options: Option<VacuumOptions>,
        #[presence(VERBOSE)]
        pub verbose: bool,
        pub target: Option<ClusterTarget>,
    }
}

// Before 17, `CLUSTER (options)` must name a table: research, PostgreSQL 17,
// "Changes to existing statements" (REL_16_15 gram.y 11570-11600). The
// option list and its table are one variant, so the list cannot occur alone.
#[cfg(not(feature = "since-pg17"))]
recursa::ast_node! {
    /// `CLUSTER '(' option [, ...] ')' qualified_name [USING index]`.
    #[derive(Debug)]
    pub struct ClusterWithOptions {
        #[tok(CLUSTER, this)]
        pub options: VacuumOptions,
        pub target: ClusterModernTarget,
    }
}

// Before 17: see `ClusterWithOptions`.
#[cfg(not(feature = "since-pg17"))]
recursa::ast_node! {
    /// `CLUSTER [VERBOSE] [qualified_name [USING index]]` and the pre-8.3
    /// `CLUSTER [VERBOSE] index ON qualified_name`.
    #[derive(Debug)]
    #[tok(CLUSTER, this)]
    pub struct ClusterWithoutOptions {
        #[presence(VERBOSE)]
        pub verbose: bool,
        pub target: Option<ClusterTarget>,
    }
}

// Before 17: see `ClusterWithOptions`.
#[cfg(not(feature = "since-pg17"))]
recursa::ast_node! {
    /// ```sql
    /// CLUSTER '(' option [, ...] ')' qualified_name [USING index]
    /// CLUSTER [VERBOSE]              [qualified_name [USING index]]
    /// CLUSTER [VERBOSE] index ON qualified_name              -- pre-8.3
    /// ```
    ///
    /// Variant ordering: `Options` continues with `(` after `CLUSTER`.
    #[derive(Debug)]
    pub enum ClusterStmt {
        Options(ClusterWithOptions),
        Plain(ClusterWithoutOptions),
    }
}
