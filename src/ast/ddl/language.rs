//! LANGUAGE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `INLINE name` — optional inline handler in `CREATE LANGUAGE`.
    #[derive(Debug)]
    pub struct LanguageInlineHandler {
        #[tok(INLINE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `VALIDATOR name | NO VALIDATOR` — Postgres' `validator_clause`.
    ///
    /// Variant ordering: the two-token `NO VALIDATOR` before the
    /// `VALIDATOR name` so the longer match wins on a leading `NO`.
    #[derive(Debug)]
    pub enum LanguageValidatorClause {
        #[tok(NO, VALIDATOR)]
        None,
        Some(LanguageValidator),
    }
}

recursa::ast_node! {
    /// `VALIDATOR name` — the populated validator branch.
    #[derive(Debug)]
    pub struct LanguageValidator {
        #[tok(VALIDATOR, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]` — the
    /// populated CREATE LANGUAGE handler clause.
    #[derive(Debug)]
    pub struct LanguageHandlerClause {
        #[tok(HANDLER, this)]
        pub name: QualifiedName,
        pub inline: Option<LanguageInlineHandler>,
        pub validator: Option<LanguageValidatorClause>,
    }
}

recursa::ast_node! {
    /// `CREATE [OR REPLACE] [TRUSTED] [PROCEDURAL] LANGUAGE name
    /// [HANDLER name [INLINE name] [VALIDATOR name | NO VALIDATOR]]` —
    /// Postgres' `CreatePLangStmt`. The handler-less form is silently treated as
    /// `CREATE EXTENSION` by PG; structurally it is still a CREATE LANGUAGE.
    #[derive(Debug)]
    pub struct CreateLanguageStmt {
        #[tok(CREATE, this)]
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        #[tok(this, optional(PROCEDURAL), LANGUAGE)]
        #[presence(TRUSTED)]
        pub trusted: bool,
        pub name: crate::tokens::ColId,
        pub handler: Option<LanguageHandlerClause>,
    }
}

recursa::ast_node! {
    /// `DROP [PROCEDURAL] LANGUAGE [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, optional(PROCEDURAL), LANGUAGE, this)]
    pub struct DropLanguageStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One action on `ALTER [PROCEDURAL] LANGUAGE name action` — covers
    /// Postgres' `RenameStmt` and `AlterOwnerStmt` branches for
    /// procedural languages. Languages have no SET SCHEMA action.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`RENAME`, `OWNER`), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterLanguageAction {
        Rename(RenameTo),
        Owner(OwnerTo),
    }
}

recursa::ast_node! {
    /// `ALTER [PROCEDURAL] LANGUAGE name action` — Postgres' `RenameStmt`
    /// and `AlterOwnerStmt` branches for procedural languages.
    #[derive(Debug)]
    pub struct AlterLanguageStmt {
        #[tok(ALTER, optional(PROCEDURAL), LANGUAGE, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterLanguageAction,
    }
}
