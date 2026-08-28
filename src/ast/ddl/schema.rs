//! SCHEMA DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::sequence::CreateSequenceStmt;
use crate::ast::ddl::trigger::CreateTriggerStmt;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::grant::GrantStmt;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `AUTHORIZATION role_spec` clause on `CREATE SCHEMA`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SchemaAuthorization<'input> {
    pub authorization: AUTHORIZATION,
    pub role: RoleSpec<'input>,
}

/// `name [AUTHORIZATION role]` — schema name with optional authorization.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SchemaNameAndAuth<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub authorization: Option<SchemaAuthorization<'input>>,
}

/// The name/authorization clause of `CREATE SCHEMA` — Postgres allows either
/// a schema name (with optional `AUTHORIZATION`) or just `AUTHORIZATION role`
/// (which implicitly names the schema after the role).
///
/// Variant ordering: the `Authorization` form must come first so a leading
/// `AUTHORIZATION` keyword does not get consumed as the schema-name `Ident`
/// (soft keywords are reclaimable as identifiers, and `AUTHORIZATION` is
/// soft).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SchemaNameClause<'input> {
    Authorization(SchemaAuthorization<'input>),
    Named(SchemaNameAndAuth<'input>),
}

/// A single statement nested inside `CREATE SCHEMA … schema_element*`.
///
/// Postgres' `OptSchemaEltList` accepts a small subset of statements:
/// `CreateStmt | IndexStmt | CreateSeqStmt | CreateTrigStmt | GrantStmt |
/// ViewStmt`. We type each one with the relevant statement struct rather
/// than a general `Statement` to mirror the grammar precisely.
///
/// Variant ordering: multi-keyword `GRANT` distinct from the `CREATE …`
/// family; among the CREATE-led variants, `CreateView` (matches `CREATE [OR
/// REPLACE] [TEMP|TEMPORARY] [RECURSIVE] VIEW …`) is the most specific.
/// The other CREATE variants disambiguate on their `CREATE { TABLE | INDEX
/// | SEQUENCE | TRIGGER }` second-token.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SchemaElement<'input> {
    Grant(GrantStmt<'input>),
    CreateView(Box<crate::ast::ddl::view::CreateViewStmt<'input>>),
    CreateTable(Box<crate::ast::ddl::table::CreateTableStmt<'input>>),
    CreateIndex(Box<crate::ast::ddl::index::CreateIndexStmt<'input>>),
    CreateSequence(CreateSequenceStmt<'input>),
    CreateTrigger(Box<CreateTriggerStmt<'input>>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateSchemaStmt<'input> {
    pub create: CREATE,
    pub schema: SCHEMA,
    pub if_not_exists: Option<crate::ast::shared::flags::IfNotExists>,
    /// Schema name and/or `AUTHORIZATION` clause. Postgres requires at least
    /// one — the enum forces that structurally.
    pub head: SchemaNameClause<'input>,
    /// Nested `schema_element` statements — `OptSchemaEltList`. Each element
    /// is a top-level statement type; the surrounding semicolons live on the
    /// enclosing statement, not on the nested ones.
    pub elements: Vec<SchemaElement<'input>>,
}

/// `DROP SCHEMA [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropSchemaStmt<'input> {
    pub drop: DROP,
    pub schema: SCHEMA,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER SCHEMA name action` — covers Postgres'
/// `RenameStmt` and `AlterOwnerStmt` branches for schemas. Schemas
/// have no SET SCHEMA action.
///
/// Variant ordering: variants begin with distinct keywords (`RENAME`,
/// `OWNER`), so order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterSchemaAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER SCHEMA name action` — Postgres' `RenameStmt` /
/// `AlterOwnerStmt` branches for schemas.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterSchemaStmt<'input> {
    pub alter: ALTER,
    pub schema: SCHEMA,
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterSchemaAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_schema_named() {
        let mut input = crate::tokens::test_input("CREATE SCHEMA s1");
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.head, SchemaNameClause::Named(_)));
        assert!(stmt.elements.is_empty());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_schema_authorization_only() {
        let mut input = crate::tokens::test_input("CREATE SCHEMA AUTHORIZATION alice");
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.head, SchemaNameClause::Authorization(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_schema_if_not_exists() {
        let mut input = crate::tokens::test_input("CREATE SCHEMA IF NOT EXISTS s1");
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_schema_with_named_and_auth() {
        let mut input = crate::tokens::test_input("CREATE SCHEMA s1 AUTHORIZATION alice");
        let stmt = CreateSchemaStmt::parse(&mut input).unwrap();
        match &stmt.head {
            SchemaNameClause::Named(n) => {
                assert_eq!(n.name.text(), "s1");
                assert!(n.authorization.is_some());
            }
            _ => panic!("expected Named"),
        }
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_schema_cascade() {
        let mut input = crate::tokens::test_input("DROP SCHEMA IF EXISTS s1, s2 CASCADE");
        let stmt = DropSchemaStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.names.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_schema_rename() {
        let mut input = crate::tokens::test_input(
            "ALTER SCHEMA test_ns_schema_1 RENAME TO test_ns_schema_renamed",
        );
        let _stmt = AlterSchemaStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_schema_owner() {
        let mut input =
            crate::tokens::test_input("ALTER SCHEMA testns OWNER TO regress_schemauser2");
        let _stmt = AlterSchemaStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
