//! Ownership-management statements: REASSIGN OWNED, DROP OWNED.
//!
//! Per §5 of the destination map these statements live under `utility/`
//! rather than `ddl/`, since their lifecycle (transfer/destroy by owning
//! role) is utility-shaped even though Postgres' `gram.y` files them as
//! object-management statements.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::{RoleList, RoleSpec};
use crate::tokens::keyword::*;

// --- REASSIGN ---

/// REASSIGN OWNED BY role_list TO role
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct ReassignStmt<'input> {
    pub reassign: REASSIGN,
    pub owned: OWNED,
    pub by: BY,
    pub roles: RoleList<'input>,
    pub to: TO,
    pub new_role: RoleSpec<'input>,
}

// -----------------------------------------------------------------------
// DROP OWNED.
// -----------------------------------------------------------------------

/// `DROP OWNED BY role [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropOwnedStmt<'input> {
    pub drop: DROP,
    pub owned: OWNED,
    pub by: BY,
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
        let mut input = crate::tokens::test_input("DROP OWNED BY r1, r2 CASCADE");
        let stmt = DropOwnedStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.roles.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
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
