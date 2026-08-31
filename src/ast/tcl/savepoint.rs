//! Savepoint statements: SAVEPOINT, RELEASE.

/// SAVEPOINT name
#[derive(recursa::Node, Debug, Clone)]
pub struct SavepointStmt<'input> {
    #[tok(SAVEPOINT, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// ```sql
/// RELEASE [SAVEPOINT] name
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct ReleaseStmt<'input> {
    #[tok(RELEASE, this)]
    #[presence(SAVEPOINT)]
    pub savepoint: bool,
    pub name: crate::tokens::ColId<'input>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/tcl/savepoint.tests.rs"
));
