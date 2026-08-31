//! CLUSTER statement. The shared `VacuumOption`/`VacuumOptions` types live
//! in `utility/vacuum.rs`.

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::vacuum::VacuumOptions;

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
#[tok(CLUSTER, this)]
pub struct ClusterStmt<'input> {
    pub options: Option<VacuumOptions<'input>>,
    #[presence(VERBOSE)]
    pub verbose: bool,
    pub target: Option<ClusterTarget<'input>>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/utility/cluster.tests.rs"
));
