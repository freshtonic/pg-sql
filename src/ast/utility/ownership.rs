//! Ownership-management statements: REASSIGN OWNED, DROP OWNED.
//!
//! Per §5 of the destination map these statements live under `utility/`
//! rather than `ddl/`, since their lifecycle (transfer/destroy by owning
//! role) is utility-shaped even though Postgres' `gram.y` files them as
//! object-management statements.

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::{RoleList, RoleSpec};

// --- REASSIGN ---

/// REASSIGN OWNED BY role_list TO role
#[derive(recursa::Node, Debug, Clone)]
pub struct ReassignStmt<'input> {
    #[tok(REASSIGN, OWNED, BY, this)]
    pub roles: RoleList<'input>,
    #[tok(TO, this)]
    pub new_role: RoleSpec<'input>,
}

// -----------------------------------------------------------------------
// DROP OWNED.
// -----------------------------------------------------------------------

/// `DROP OWNED BY role [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropOwnedStmt<'input> {
    #[tok(DROP, OWNED, BY, this)]
    pub roles: RoleList<'input>,
    pub behavior: Option<DropBehavior>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/utility/ownership.tests.rs"
));
