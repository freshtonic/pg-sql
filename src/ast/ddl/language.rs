//! LANGUAGE DDL statements (CREATE/ALTER/DROP).
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

/// `INLINE name` — optional inline handler in `CREATE LANGUAGE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageInlineHandler<'input> {
    #[tok(INLINE, this)]
    pub name: QualifiedName<'input>,
}

/// `VALIDATOR name | NO VALIDATOR` — Postgres' `validator_clause`.
///
/// Variant ordering: the two-token `NO VALIDATOR` before the
/// `VALIDATOR name` so the longer match wins on a leading `NO`.
#[derive(recursa::Node, Debug, Clone)]
pub enum LanguageValidatorClause<'input> {
    #[tok(NO, VALIDATOR)] None,
    Some(LanguageValidator<'input>),
}

/// `VALIDATOR name` — the populated validator branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageValidator<'input> {
    #[tok(VALIDATOR, this)]
    pub name: QualifiedName<'input>,
}

/// `HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]` — the
/// populated CREATE LANGUAGE handler clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageHandlerClause<'input> {
    #[tok(HANDLER, this)]
    pub name: QualifiedName<'input>,
    pub inline: Option<LanguageInlineHandler<'input>>,
    pub validator: Option<LanguageValidatorClause<'input>>,
}

/// `CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE name
/// [HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]]` —
/// Postgres' `CreatePLangStmt`. The handler-less form is silently treated as
/// `CREATE EXTENSION` by PG; structurally it is still a CREATE LANGUAGE.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateLanguageStmt<'input> {
    #[tok(CREATE, this)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    #[tok(this, optional(PROCEDURAL), LANGUAGE)]
    #[presence(TRUSTED)]
    pub trusted: bool,
    pub name: crate::tokens::ColId<'input>,
    pub handler: Option<LanguageHandlerClause<'input>>,
}

/// `DROP [PROCEDURAL] LANGUAGE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropLanguageStmt<'input> {
    #[tok(DROP, optional(PROCEDURAL), LANGUAGE, this)]
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
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterLanguageAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER [PROCEDURAL] LANGUAGE name action` — Postgres' `RenameStmt`
/// and `AlterOwnerStmt` branches for procedural languages.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterLanguageStmt<'input> {
    #[tok(ALTER, optional(PROCEDURAL), LANGUAGE, this)]
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
        let lexed = crate::tokens::lex("CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_plain() {
        let lexed = crate::tokens::lex("CREATE LANGUAGE plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "plpgsql");
        assert!(stmt.handler.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_handler() {
        let lexed = crate::tokens::lex("CREATE LANGUAGE plpgsql HANDLER plpgsql_call_handler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap().into_ast();
        let h = stmt.handler.expect("handler present");
        assert_eq!(h.name.object(), "plpgsql_call_handler");
        assert!(h.inline.is_none());
        assert!(h.validator.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_or_replace_trusted_procedural_with_validator() {
        let lexed = crate::tokens::lex("CREATE OR REPLACE TRUSTED PROCEDURAL LANGUAGE plpgsql \
             HANDLER plpgsql_call_handler \
             INLINE plpgsql_inline_handler \
             VALIDATOR plpgsql_validator");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.or_replace.is_some());
        assert!(stmt.trusted.is_some());
        assert!(stmt.procedural.is_some());
        let h = stmt.handler.unwrap();
        assert!(h.inline.is_some());
        assert!(matches!(
            h.validator.unwrap(),
            LanguageValidatorClause::Some(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_language_no_validator() {
        let lexed = crate::tokens::lex("CREATE LANGUAGE plpgsql HANDLER h NO VALIDATOR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.handler.unwrap().validator.unwrap(),
            LanguageValidatorClause::None(_),
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_language_owner() {
        let lexed = crate::tokens::lex("ALTER LANGUAGE plpgsql OWNER TO foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_language() {
        let lexed = crate::tokens::lex("DROP LANGUAGE plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.procedural.is_none());
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_procedural_language() {
        let lexed = crate::tokens::lex("DROP PROCEDURAL LANGUAGE IF EXISTS plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.procedural.is_some());
        assert!(stmt.if_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_language_procedural_rename() {
        let lexed = crate::tokens::lex("ALTER PROCEDURAL LANGUAGE alt_lang1 RENAME TO alt_lang3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterLanguageStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
