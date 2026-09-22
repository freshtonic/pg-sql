//! WAIT FOR LSN statement, added in 19: gram.y `WaitStmt` (b73d13c:16635;
//! research PostgreSQL 19, "New statements", commits 447aae13b and
//! 49a181b5d). The whole module has a `since-pg19` gate.

use crate::ast::utility::explain::{Sconst, UtilityOptionList};

recursa::ast_node! {
    /// `WITH (option [, ...])` — gram.y `opt_wait_with_clause` (b73d13c:16646).
    #[derive(Debug)]
    pub struct WaitWithClause {
        #[tok(WITH, this)]
        pub options: UtilityOptionList,
    }
}

recursa::ast_node! {
    /// `WAIT FOR LSN 'lsn' [WITH (option [, ...])]` — gram.y `WaitStmt`.
    #[derive(Debug)]
    #[tok(WAIT, FOR, LSN, this)]
    pub struct WaitStmt {
        /// gram.y `Sconst`.
        pub lsn: Sconst,
        pub with: Option<WaitWithClause>,
    }
}
