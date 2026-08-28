//! COLLATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Body of `CREATE COLLATION` after the name: either a `def_list` of options
/// (`LOCALE`/`LC_COLLATE`/`PROVIDER`/...), or `FROM existing_collation_name`.
///
/// Variant ordering: `From` (keyword-led) before `Options` (paren-led) — they
/// begin with different tokens so peek disambiguation is unambiguous.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreateCollationBody<'input> {
    From(CollationFromClause<'input>),
    Options(DefList<'input>),
}

/// `FROM existing_collation_name` — copy an existing collation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CollationFromClause<'input> {
    pub from: FROM,
    pub name: QualifiedName<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateCollationStmt<'input> {
    pub create: CREATE,
    pub collation: COLLATION,
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    pub body: CreateCollationBody<'input>,
}

/// `DROP COLLATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropCollationStmt<'input> {
    pub drop: DROP,
    pub collation: COLLATION,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `REFRESH VERSION` — Postgres' `AlterCollationStmt` action.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CollationRefreshVersion {
    pub refresh: REFRESH,
    pub version: VERSION,
}

/// One action on `ALTER COLLATION any_name action` — Postgres'
/// `RenameStmt`, `AlterOwnerStmt`, `AlterObjectSchemaStmt`, and
/// `AlterCollationStmt` branches for collations.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`, `REFRESH`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterCollationAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    RefreshVersion(CollationRefreshVersion),
}

/// `ALTER COLLATION any_name action` — Postgres' `AlterCollationStmt`
/// (REFRESH VERSION) plus the collation branches of `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterCollationStmt<'input> {
    pub alter: ALTER,
    pub collation: COLLATION,
    pub name: QualifiedName<'input>,
    pub action: AlterCollationAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_collation_rename() {
        let mut input = crate::tokens::test_input("ALTER COLLATION test1 RENAME TO test11");
        let _stmt = AlterCollationStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_collation_refresh_version() {
        let mut input = crate::tokens::test_input("ALTER COLLATION en_us REFRESH VERSION");
        let _stmt = AlterCollationStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_collation_def_list() {
        let mut input = crate::tokens::test_input(
            "CREATE COLLATION mycoll (LC_COLLATE = \"POSIX\", LC_CTYPE = \"POSIX\")",
        );
        let stmt = CreateCollationStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "mycoll");
        assert!(matches!(stmt.body, CreateCollationBody::Options(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_collation_from() {
        let mut input = crate::tokens::test_input("CREATE COLLATION mycoll FROM \"C\"");
        let stmt = CreateCollationStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.body, CreateCollationBody::From(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_collation_if_not_exists() {
        let mut input =
            crate::tokens::test_input("CREATE COLLATION IF NOT EXISTS mycoll FROM \"C\"");
        let stmt = CreateCollationStmt::parse(&mut input).unwrap();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_empty());
    }
}
