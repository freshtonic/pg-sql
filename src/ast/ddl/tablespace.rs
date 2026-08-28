/// CREATE TABLESPACE / DROP TABLESPACE statement AST.
use recursa::seq::Seq0;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::index::{StorageParam, WithStorage};
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `OWNER role` optional clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OwnerClause<'input> {
    pub owner: OWNER,
    pub role: crate::tokens::NonReservedWord<'input>,
}

/// `LOCATION 'path'` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LocationClause<'input> {
    pub location: LOCATION,
    pub path: literal::StringLit<'input>,
}

/// `CREATE TABLESPACE name [OWNER role] LOCATION 'path' [WITH (params)]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateTablespaceStmt<'input> {
    pub create: CREATE,
    pub tablespace: TABLESPACE,
    pub name: crate::tokens::ColId<'input>,
    pub owner: Option<OwnerClause<'input>>,
    pub location: LocationClause<'input>,
    pub with_options: Option<WithStorage<'input>>,
}

/// `RENAME TO new_name` action on ALTER TABLESPACE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTablespaceRename<'input> {
    pub rename: RENAME,
    pub to: TO,
    pub new_name: crate::tokens::ColId<'input>,
}

/// `OWNER TO new_owner` action on ALTER TABLESPACE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTablespaceOwner<'input> {
    pub owner: OWNER,
    pub to: TO,
    pub new_owner: crate::tokens::NonReservedWord<'input>,
}

/// `SET (param = value, ...)` action on ALTER TABLESPACE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTablespaceSetAction<'input> {
    pub set: SET,
    pub params: Surrounded<punct::LParen, Seq0<StorageParam<'input>, punct::Comma>, punct::RParen>,
}

/// `RESET (param [= value] [, ...])` action on ALTER TABLESPACE.
///
/// Postgres accepts the same `reloptions` payload here as for `SET`, even
/// though the `= value` half is ignored: `gram.y`'s `AlterTblSpcStmt` uses
/// the `reloptions` rule for both branches.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTablespaceResetAction<'input> {
    pub reset: RESET,
    pub params: Surrounded<punct::LParen, Seq0<StorageParam<'input>, punct::Comma>, punct::RParen>,
}

/// One of the supported ALTER TABLESPACE actions.
///
/// Variant ordering: all variants start with distinct keywords (SET, RESET,
/// RENAME, OWNER), so order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterTablespaceAction<'input> {
    Set(AlterTablespaceSetAction<'input>),
    Reset(AlterTablespaceResetAction<'input>),
    Rename(AlterTablespaceRename<'input>),
    Owner(AlterTablespaceOwner<'input>),
}

/// `ALTER TABLESPACE name { RENAME TO new_name | OWNER TO new_owner
///                         | SET (params) | RESET (params) }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterTablespaceStmt<'input> {
    pub alter: ALTER,
    pub tablespace: TABLESPACE,
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterTablespaceAction<'input>,
}

/// `DROP TABLESPACE [IF EXISTS] name`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropTablespaceStmt<'input> {
    pub drop: DROP,
    pub tablespace: TABLESPACE,
    pub if_exists: Option<(IF, EXISTS)>,
    pub name: crate::tokens::ColId<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_tablespace_basic() {
        let mut input = crate::tokens::test_input("CREATE TABLESPACE ts1 LOCATION '/tmp'");
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_tablespace_with_options() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLESPACE ts1 LOCATION '' WITH (random_page_cost = 3.0)",
        );
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_tablespace_owner() {
        let mut input =
            crate::tokens::test_input("CREATE TABLESPACE ts1 OWNER foo LOCATION '/tmp'");
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_tablespace() {
        let mut input = crate::tokens::test_input("DROP TABLESPACE ts1");
        let _stmt = DropTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_tablespace_if_exists() {
        let mut input = crate::tokens::test_input("DROP TABLESPACE IF EXISTS ts1");
        let _stmt = DropTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_tablespace_set() {
        let mut input =
            crate::tokens::test_input("ALTER TABLESPACE ts SET (random_page_cost = 1.0)");
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_tablespace_reset() {
        let mut input = crate::tokens::test_input(
            "ALTER TABLESPACE ts RESET (random_page_cost, effective_io_concurrency)",
        );
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_tablespace_rename() {
        let mut input = crate::tokens::test_input("ALTER TABLESPACE ts RENAME TO ts2");
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_tablespace_owner() {
        let mut input = crate::tokens::test_input("ALTER TABLESPACE ts OWNER TO foo");
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
