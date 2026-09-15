//! Savepoint statements: SAVEPOINT, RELEASE.

recursa::ast_node! {
    /// SAVEPOINT name
    #[derive(Debug)]
    pub struct SavepointStmt {
        #[tok(SAVEPOINT, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// ```sql
    /// RELEASE [SAVEPOINT] name
    /// ```
    #[derive(Debug)]
    #[tok(RELEASE, this)]
    pub struct ReleaseStmt {
        #[presence(SAVEPOINT)]
        pub savepoint: bool,
        pub name: crate::tokens::ColId,
    }
}
