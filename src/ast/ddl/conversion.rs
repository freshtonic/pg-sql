//! CONVERSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `CREATE [DEFAULT] CONVERSION name FOR 'src_enc' TO 'dst_enc' FROM func`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateConversionStmt<'input> {
    #[tok(CREATE, this, CONVERSION)]
    #[presence(DEFAULT)]
    pub default: bool,
    pub name: QualifiedName<'input>,
    #[tok(FOR, this)]
    pub src_encoding: literal::StringLit<'input>,
    #[tok(TO, this)]
    pub dest_encoding: literal::StringLit<'input>,
    #[tok(FROM, this)]
    pub func_name: QualifiedName<'input>,
}

/// `DROP CONVERSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropConversionStmt<'input> {
    #[tok(DROP, CONVERSION, this)]
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
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterConversionAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER CONVERSION any_name action` — Postgres' `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt` branches for conversions.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterConversionStmt<'input> {
    #[tok(ALTER, CONVERSION, this)]
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
        let lexed = crate::tokens::lex("ALTER CONVERSION alt_conv2 SET SCHEMA alt_nsp2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_default_conversion() {
        let lexed = crate::tokens::lex("CREATE DEFAULT CONVERSION mydef FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_conversion_without_default() {
        let lexed = crate::tokens::lex("CREATE CONVERSION myconv FOR 'UTF8' TO 'LATIN1' FROM utf8_to_iso8859_1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_conversion_plain() {
        let lexed = crate::tokens::lex("CREATE CONVERSION myconv FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.default.is_none());
        assert_eq!(stmt.name.object(), "myconv");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_default_conversion_structured() {
        let lexed = crate::tokens::lex("CREATE DEFAULT CONVERSION public.mydef FOR 'LATIN1' TO 'UTF8' FROM iso8859_1_to_utf8");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateConversionStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.default.is_some());
        assert!(input.is_eof());
    }
}
