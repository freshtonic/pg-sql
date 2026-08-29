//! Ownership-management statements: REASSIGN OWNED, DROP OWNED.
//!
//! Per §5 of the destination map these statements live under `utility/`
//! rather than `ddl/`, since their lifecycle (transfer/destroy by owning
//! role) is utility-shaped even though Postgres' `gram.y` files them as
//! object-management statements.

use recursa_diagram::railroad;

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::{RoleList, RoleSpec};
use crate::tokens::keyword::*;

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

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_owned_by() {
        let lexed = crate::tokens::lex("DROP OWNED BY r1, r2 CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropOwnedStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.roles.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn reassign_owned_is_modelled() {
        let stmt: ReassignStmt = parse_stmt("REASSIGN OWNED BY a TO b");
        assert_eq!(stmt.roles.len(), 1);
        assert_eq!(
            roundtrip::<ReassignStmt>("REASSIGN OWNED BY a TO b"),
            "REASSIGN OWNED BY a TO b"
        );
    }

    #[test]
    fn reassign_owned_multiple_roles_roundtrips() {
        let stmt: ReassignStmt = parse_stmt("REASSIGN OWNED BY a, b TO c");
        assert_eq!(stmt.roles.len(), 2);
        assert_eq!(
            roundtrip::<ReassignStmt>("REASSIGN OWNED BY a, b TO c"),
            "REASSIGN OWNED BY a, b TO c"
        );
    }
}
