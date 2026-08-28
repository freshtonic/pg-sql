//! LANGUAGE DDL statements (CREATE/ALTER/DROP).
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

/// `INLINE name` — optional inline handler in `CREATE LANGUAGE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LanguageInlineHandler<'input> {
    pub inline: INLINE,
    pub name: QualifiedName<'input>,
}

/// `VALIDATOR name | NO VALIDATOR` — Postgres' `validator_clause`.
///
/// Variant ordering: the two-token `NO VALIDATOR` before the
/// `VALIDATOR name` so the longer match wins on a leading `NO`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LanguageValidatorClause<'input> {
    None((NO, VALIDATOR)),
    Some(LanguageValidator<'input>),
}

/// `VALIDATOR name` — the populated validator branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LanguageValidator<'input> {
    pub validator: VALIDATOR,
    pub name: QualifiedName<'input>,
}

/// `HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]` — the
/// populated CREATE LANGUAGE handler clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LanguageHandlerClause<'input> {
    pub handler: HANDLER,
    pub name: QualifiedName<'input>,
    pub inline: Option<LanguageInlineHandler<'input>>,
    pub validator: Option<LanguageValidatorClause<'input>>,
}

/// `CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE name
/// [HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]]` —
/// Postgres' `CreatePLangStmt`. The handler-less form is silently treated as
/// `CREATE EXTENSION` by PG; structurally it is still a CREATE LANGUAGE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateLanguageStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub trusted: Option<TRUSTED>,
    pub procedural: Option<PROCEDURAL>,
    pub language: LANGUAGE,
    pub name: crate::tokens::ColId<'input>,
    pub handler: Option<LanguageHandlerClause<'input>>,
}

/// `DROP [PROCEDURAL] LANGUAGE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropLanguageStmt<'input> {
    pub drop: DROP,
    pub procedural: Option<PROCEDURAL>,
    pub language: LANGUAGE,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER [PROCEDURAL] LANGUAGE name action` — covers
/// Postgres' `RenameStmt` and `AlterOwnerStmt` branches for
/// procedural languages. Languages have no SET SCHEMA action.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterLanguageAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER [PROCEDURAL] LANGUAGE name action` — Postgres' `RenameStmt`
/// and `AlterOwnerStmt` branches for procedural languages.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterLanguageStmt<'input> {
    pub alter: ALTER,
    pub procedural: Option<PROCEDURAL>,
    pub language: LANGUAGE,
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterLanguageAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_language() {
        let mut input =
            crate::tokens::test_input("CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler");
        let _stmt = CreateLanguageStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_language_plain() {
        let mut input = crate::tokens::test_input("CREATE LANGUAGE plpgsql");
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "plpgsql");
        assert!(stmt.handler.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_language_handler() {
        let mut input =
            crate::tokens::test_input("CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler");
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap();
        let h = stmt.handler.expect("handler present");
        assert_eq!(h.name.object(), "plpgsql_call_handler");
        assert!(h.inline.is_none());
        assert!(h.validator.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_language_or_replace_trusted_procedural_with_validator() {
        let mut input = crate::tokens::test_input(
            "CREATE OR REPLACE TRUSTED PROCEDURAL LANGUAGE plpgsql \
             HANDLER plpgsql_call_handler \
             INLINE plpgsql_inline_handler \
             VALIDATOR plpgsql_validator",
        );
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap();
        assert!(stmt.or_replace.is_some());
        assert!(stmt.trusted.is_some());
        assert!(stmt.procedural.is_some());
        let h = stmt.handler.unwrap();
        assert!(h.inline.is_some());
        assert!(matches!(
            h.validator.unwrap(),
            LanguageValidatorClause::Some(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_language_no_validator() {
        let mut input = crate::tokens::test_input("CREATE LANGUAGE plpgsql HANDLER h NO VALIDATOR");
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.handler.unwrap().validator.unwrap(),
            LanguageValidatorClause::None(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_language_owner() {
        let mut input = crate::tokens::test_input("ALTER LANGUAGE plpgsql OWNER TO foo");
        let _stmt = AlterLanguageStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_language() {
        let mut input = crate::tokens::test_input("DROP LANGUAGE plpgsql");
        let stmt = DropLanguageStmt::parse(&mut input).unwrap();
        assert!(stmt.procedural.is_none());
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_procedural_language() {
        let mut input = crate::tokens::test_input("DROP PROCEDURAL LANGUAGE IF EXISTS plpgsql");
        let stmt = DropLanguageStmt::parse(&mut input).unwrap();
        assert!(stmt.procedural.is_some());
        assert!(stmt.if_exists.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_language_procedural_rename() {
        let mut input =
            crate::tokens::test_input("ALTER PROCEDURAL LANGUAGE alt_lang1 RENAME TO alt_lang3");
        let _stmt = AlterLanguageStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
