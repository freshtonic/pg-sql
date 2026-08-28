//! CONVERSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `CREATE [DEFAULT] CONVERSION name FOR 'src_enc' TO 'dst_enc' FROM func`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateConversionStmt<'input> {
    pub create: CREATE,
    pub default: Option<DEFAULT>,
    pub conversion: CONVERSION,
    pub name: QualifiedName<'input>,
    pub r#for: FOR,
    pub src_encoding: literal::StringLit<'input>,
    pub to: TO,
    pub dest_encoding: literal::StringLit<'input>,
    pub from: FROM,
    pub func_name: QualifiedName<'input>,
}

/// `DROP CONVERSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropConversionStmt<'input> {
    pub drop: DROP,
    pub conversion: CONVERSION,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER CONVERSION any_name action` — Postgres'
/// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches
/// for conversions.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterConversionAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER CONVERSION any_name action` — Postgres' `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt` branches for conversions.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterConversionStmt<'input> {
    pub alter: ALTER,
    pub conversion: CONVERSION,
    pub name: QualifiedName<'input>,
    pub action: AlterConversionAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_conversion_set_schema() {
        let mut input = crate::tokens::test_input("ALTER CONVERSION alt_conv2 SET SCHEMA alt_nsp2");
        let _stmt = AlterConversionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_default_conversion() {
        let mut input = crate::tokens::test_input(
            "CREATE DEFAULT CONVERSION mydef FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8",
        );
        let _stmt = CreateConversionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_conversion_without_default() {
        let mut input = crate::tokens::test_input(
            "CREATE CONVERSION myconv FOR 'UTF8' TO 'LATIN1' FROM utf8_to_iso8859_1",
        );
        let _stmt = CreateConversionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_conversion_plain() {
        let mut input = crate::tokens::test_input(
            "CREATE CONVERSION myconv FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8",
        );
        let stmt = CreateConversionStmt::parse(&mut input).unwrap();
        assert!(stmt.default.is_none());
        assert_eq!(stmt.name.object(), "myconv");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_default_conversion_structured() {
        let mut input = crate::tokens::test_input(
            "CREATE DEFAULT CONVERSION public.mydef FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8",
        );
        let stmt = CreateConversionStmt::parse(&mut input).unwrap();
        assert!(stmt.default.is_some());
        assert!(input.is_empty());
    }
}
