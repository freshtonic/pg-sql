//! EXTENSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `SCHEMA name` option on `CREATE EXTENSION`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExtensionSchemaOption<'input> {
    #[tok(SCHEMA, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `VERSION { sconst | ident }` option on `CREATE EXTENSION`. Postgres'
/// `NonReservedWord_or_Sconst` allows either a quoted string or a bareword.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExtensionVersionValue<'input> {
    String(CopySconst<'input>),
    /// Any bareword (incl. soft keywords) — `NonReservedWord`.
    Word(literal::AliasName<'input>),
}

/// `VERSION value` option on `CREATE EXTENSION`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExtensionVersionOption<'input> {
    #[tok(VERSION, this)]
    pub value: ExtensionVersionValue<'input>,
}

/// A single CREATE EXTENSION option — Postgres' `create_extension_opt_item`.
/// Options are unordered and repeatable.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExtensionOption<'input> {
    Schema(ExtensionSchemaOption<'input>),
    Version(ExtensionVersionOption<'input>),
    #[tok(CASCADE)] Cascade,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateExtensionStmt<'input> {
    #[tok(CREATE, EXTENSION, this)]
    pub if_not_exists: Option<crate::ast::shared::flags::IfNotExists>,
    pub name: crate::tokens::ColId<'input>,
    #[tok(optional(WITH), this)]
    pub options: Vec<ExtensionOption<'input>>,
}

/// `DROP EXTENSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropExtensionStmt<'input> {
    #[tok(DROP, EXTENSION, this)]
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `TO version` (`NonReservedWord_or_Sconst`) — the version target on
/// `ALTER EXTENSION name UPDATE`. Postgres' `alter_extension_opt_item`.
///
/// pg-sql accepts `TO sconst` (a string literal); the bare-identifier
/// form is not exercised by the corpus.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionUpdateTo<'input> {
    #[tok(TO, this)]
    pub version: literal::StringLit<'input>,
}

/// `UPDATE [TO version]` — Postgres' `AlterExtensionStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionUpdate<'input> {
    #[tok(UPDATE, this)]
    pub to: Option<AlterExtensionUpdateTo<'input>>,
}

/// `MATERIALIZED VIEW` / `FOREIGN TABLE` / `TEXT SEARCH PARSER` etc. —
/// the multi-word entries of Postgres' `object_type_any_name` (any-name
/// objects whose type label is more than one keyword).
///
/// Variant ordering: longer multi-keyword variants first (e.g.
/// `MATERIALIZED VIEW` before `VIEW`).
#[derive(recursa::Node, Debug, Clone)]
pub enum ExtensionObjectTypeAnyName {
    #[tok(MATERIALIZED, VIEW)] MaterializedView,
    #[tok(FOREIGN, TABLE)] ForeignTable,
    #[tok(TEXT, SEARCH, PARSER)] TextSearchParser,
    #[tok(TEXT, SEARCH, DICTIONARY)] TextSearchDictionary,
    #[tok(TEXT, SEARCH, TEMPLATE)] TextSearchTemplate,
    #[tok(TEXT, SEARCH, CONFIGURATION)] TextSearchConfiguration,
    #[tok(TABLE)] Table,
    #[tok(SEQUENCE)] Sequence,
    #[tok(VIEW)] View,
    #[tok(INDEX)] Index,
    #[tok(COLLATION)] Collation,
    #[tok(CONVERSION)] Conversion,
    #[tok(STATISTICS)] Statistics,
}

/// `ACCESS METHOD` / `EVENT TRIGGER` / `FOREIGN DATA WRAPPER` / etc. —
/// Postgres' `drop_type_name` / `object_type_name` (`name`-taking
/// objects). Excludes the qualified-name `LANGUAGE` form (covered by
/// the dedicated `ALTER EXTENSION ... LANGUAGE` branch below).
///
/// Variant ordering: longest multi-keyword forms first.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExtensionObjectTypeName {
    #[tok(FOREIGN, DATA, WRAPPER)] ForeignDataWrapper,
    #[tok(ACCESS, METHOD)] AccessMethod,
    #[tok(EVENT, TRIGGER)] EventTrigger,
    #[tok(DATABASE)] Database,
    #[tok(ROLE)] Role,
    #[tok(SUBSCRIPTION)] Subscription,
    #[tok(TABLESPACE)] Tablespace,
    #[tok(EXTENSION)] Extension,
    #[tok(PUBLICATION)] Publication,
    #[tok(SCHEMA)] Schema,
    #[tok(SERVER)] Server,
}

/// `add_drop object_type_any_name any_name` — the dotted-name branch of
/// Postgres' `AlterExtensionContentsStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionAnyNameMember<'input> {
    pub object_type: ExtensionObjectTypeAnyName,
    pub name: QualifiedName<'input>,
}

/// `add_drop object_type_name name` — the simple-name branch of
/// Postgres' `AlterExtensionContentsStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionNameMember<'input> {
    pub object_type: ExtensionObjectTypeName,
    pub name: crate::tokens::ColId<'input>,
}

/// `add_drop [PROCEDURAL] LANGUAGE name` — Postgres'
/// `AlterExtensionContentsStmt` LANGUAGE branch (via the
/// `opt_procedural LANGUAGE` arm of `drop_type_name`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionLanguageMember<'input> {
    #[tok(optional(PROCEDURAL), LANGUAGE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// One `add_drop object` body — covers the simple-name and any-name
/// branches plus the `[PROCEDURAL] LANGUAGE` branch of Postgres'
/// `AlterExtensionContentsStmt`. The complex sub-grammar arms
/// (`AGGREGATE aggregate_with_argtypes`, `CAST '(' Typename AS Typename
/// ')'`, `FUNCTION function_with_argtypes`, `OPERATOR
/// operator_with_argtypes`, `OPERATOR CLASS any_name USING name`,
/// `OPERATOR FAMILY any_name USING name`, `PROCEDURE function_with_argtypes`,
/// `ROUTINE function_with_argtypes`, `TRANSFORM FOR Typename LANGUAGE
/// name`, `DOMAIN Typename`, `TYPE Typename`) are deferred — they reuse
/// sub-grammars (`aggregate_with_argtypes`, `function_with_argtypes`,
/// `operator_with_argtypes`) that aren't yet shared by this module, and
/// no corpus statement exercises them.
///
/// Variant ordering: `[PROCEDURAL] LANGUAGE` first so the optional
/// `PROCEDURAL` keyword is consumed before the simple-name branch tries
/// to match `LANGUAGE` as part of `ExtensionObjectTypeName`. The
/// any-name branch is then listed before the name branch so the
/// `TEXT SEARCH …` / `MATERIALIZED VIEW` / `FOREIGN TABLE` multi-keyword
/// forms win over their single-keyword cousins in
/// `ExtensionObjectTypeName`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterExtensionMember<'input> {
    Language(AlterExtensionLanguageMember<'input>),
    AnyName(AlterExtensionAnyNameMember<'input>),
    Name(AlterExtensionNameMember<'input>),
}

/// `ADD member` — Postgres' `AlterExtensionContentsStmt` ADD branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionAdd<'input> {
    #[tok(ADD, this)]
    pub member: AlterExtensionMember<'input>,
}

/// `DROP member` — Postgres' `AlterExtensionContentsStmt` DROP branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionDrop<'input> {
    #[tok(DROP, this)]
    pub member: AlterExtensionMember<'input>,
}

/// One action on `ALTER EXTENSION name action` — Postgres'
/// `AlterExtensionStmt` (UPDATE [TO version]),
/// `AlterExtensionContentsStmt` (ADD/DROP object), and the extension
/// branch of `AlterObjectSchemaStmt` (SET SCHEMA).
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`UPDATE`, `ADD`, `DROP`, `SET`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterExtensionAction<'input> {
    Update(AlterExtensionUpdate<'input>),
    Add(AlterExtensionAdd<'input>),
    Drop(AlterExtensionDrop<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER EXTENSION name action` — Postgres' `AlterExtensionStmt` and
/// `AlterExtensionContentsStmt` plus the extension branch of
/// `AlterObjectSchemaStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterExtensionStmt<'input> {
    #[tok(ALTER, EXTENSION, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterExtensionAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_extension_plain() {
        let lexed = crate::tokens::lex("CREATE EXTENSION hstore");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "hstore");
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_extension_if_not_exists_with_options() {
        let lexed = crate::tokens::lex("CREATE EXTENSION IF NOT EXISTS hstore WITH SCHEMA public VERSION '1.6' CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_update() {
        let lexed = crate::tokens::lex("ALTER EXTENSION my_ext UPDATE TO '1.1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_update_no_to() {
        let lexed = crate::tokens::lex("ALTER EXTENSION my_ext UPDATE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_set_schema() {
        let lexed = crate::tokens::lex("ALTER EXTENSION my_ext SET SCHEMA new_schema");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_extension_add_table() {
        let lexed = crate::tokens::lex("ALTER EXTENSION my_ext ADD TABLE my_table");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
