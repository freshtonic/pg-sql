//! CLUSTER statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;
use crate::tokens::keyword::*;

// --- CLUSTER ---

/// `USING index_name` — Postgres' `cluster_index_specification`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ClusterUsingIndex<'input> {
    #[tok(USING, this)]
    pub index: crate::tokens::ColId<'input>,
}

/// Modern target: `qualified_name [USING index]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ClusterModernTarget<'input> {
    pub table: QualifiedName<'input>,
    pub using_index: Option<ClusterUsingIndex<'input>>,
}

/// Pre-8.3 legacy target: `index_name ON qualified_name`.
///
/// `ON` after the first identifier disambiguates this from the modern form
/// (which would have `USING` there, or nothing).
#[derive(recursa::Node, Debug, Clone)]
pub struct ClusterLegacyTarget<'input> {
    pub index: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
}

/// `CLUSTER` target: either the pre-8.3 `index ON table` form or the
/// modern `table [USING index]` form.
///
/// Variant ordering: the legacy form is listed first because both variants
/// start with an identifier; the legacy form's distinguishing `ON` is
/// reached after two tokens, while the modern form can stop after one
/// identifier (no `USING`). Declaration-order tiebreak prefers the legacy
/// form when both could parse a prefix, but the modern form is selected
/// once the parser sees no `ON` after the leading identifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum ClusterTarget<'input> {
    Legacy(ClusterLegacyTarget<'input>),
    Modern(ClusterModernTarget<'input>),
}

/// ```sql
/// CLUSTER '(' option [, ...] ')' [qualified_name [USING index]]
/// CLUSTER [VERBOSE]              [qualified_name [USING index]]
/// CLUSTER [VERBOSE] index ON qualified_name              -- pre-8.3
/// ```
///
/// In the parenthesised form, `options` is `Some` and `verbose` is `None`
/// (the option list expresses `VERBOSE` instead). In any legacy form,
/// `options` is `None` and `verbose` may be `Some` or `None`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ClusterStmt<'input> {
    #[tok(CLUSTER, this)]
    pub options: Option<VacuumOptions<'input>>,
    #[presence(VERBOSE)]
    pub verbose: bool,
    pub target: Option<ClusterTarget<'input>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn cluster_bare_is_modelled() {
        let stmt: ClusterStmt = parse_stmt("CLUSTER");
        assert!(stmt.options.is_none());
        assert!(stmt.verbose.is_none());
        assert!(stmt.target.is_none());
        reparse_stable::<ClusterStmt>("CLUSTER");
    }

    #[test]
    fn cluster_table_only_roundtrips() {
        reparse_stable::<ClusterStmt>("CLUSTER clstr_2");
    }

    #[test]
    fn cluster_table_using_index_roundtrips() {
        reparse_stable::<ClusterStmt>("CLUSTER clstr_2 USING clstr_2_pkey");
    }

    #[test]
    fn cluster_verbose_roundtrips() {
        let stmt: ClusterStmt = parse_stmt("CLUSTER VERBOSE clstr_2");
        assert!(stmt.verbose.is_some());
        reparse_stable::<ClusterStmt>("CLUSTER VERBOSE clstr_2");
    }

    #[test]
    fn cluster_options_roundtrips() {
        let stmt: ClusterStmt = parse_stmt("CLUSTER (VERBOSE) clstr_2 USING clstr_2_pkey");
        assert!(stmt.options.is_some());
        reparse_stable::<ClusterStmt>("CLUSTER (VERBOSE) clstr_2 USING clstr_2_pkey");
    }

    #[test]
    fn cluster_legacy_index_on_table_roundtrips() {
        // pre-8.3 form: CLUSTER [VERBOSE] index ON table
        reparse_stable::<ClusterStmt>("CLUSTER clstr_2_pkey ON clstr_2");
    }
}
