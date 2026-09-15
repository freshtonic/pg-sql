//! Ownership-management statements: REASSIGN OWNED, DROP OWNED.
//!
//! Per §5 of the destination map these statements live under `utility/`
//! rather than `ddl/`, since their lifecycle (transfer/destroy by owning
//! role) is utility-shaped even though Postgres' `gram.y` files them as
//! object-management statements.

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::{RoleList, RoleSpec};

// --- REASSIGN ---

recursa::ast_node! {
    /// REASSIGN OWNED BY role_list TO role
    #[derive(Debug)]
    pub struct ReassignStmt {
        #[tok(REASSIGN, OWNED, BY, this)]
        pub roles: RoleList,
        #[tok(TO, this)]
        pub new_role: RoleSpec,
    }
}

// -----------------------------------------------------------------------
// DROP OWNED.
// -----------------------------------------------------------------------

recursa::ast_node! {
    /// `DROP OWNED BY role [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropOwnedStmt {
        #[tok(DROP, OWNED, BY, this)]
        pub roles: RoleList,
        pub behavior: Option<DropBehavior>,
    }
}
