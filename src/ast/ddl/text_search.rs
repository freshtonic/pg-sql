//! TEXT SEARCH DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `CREATE TEXT SEARCH { PARSER | DICTIONARY | TEMPLATE | CONFIGURATION }
/// name (def_list)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTextSearchStmt<'input> {
    #[tok(CREATE, TEXT, SEARCH, this)]
    pub kind: TextSearchObjectKind,
    pub name: QualifiedName<'input>,
    pub definition: DefList<'input>,
}

/// The object kind after `DROP TEXT SEARCH`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TextSearchObjectKind {
    #[tok(CONFIGURATION)]
    Configuration,
    #[tok(DICTIONARY)]
    Dictionary,
    #[tok(PARSER)]
    Parser,
    #[tok(TEMPLATE)]
    Template,
}

/// `DROP TEXT SEARCH {CONFIGURATION | DICTIONARY | PARSER | TEMPLATE}
/// [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropTextSearchStmt<'input> {
    #[tok(DROP, TEXT, SEARCH, this)]
    pub kind: TextSearchObjectKind,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `FOR name_list` — token-type list on `ALTER TEXT SEARCH CONFIGURATION
/// ... { ADD | ALTER | DROP } MAPPING FOR ...`. Tokens are plain `name`s
/// (Postgres' `ColId`), not dotted `any_name`s.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FOR, this)]
pub struct TextSearchTokenList<'input> {
    #[sep(COMMA)]
    pub tokens: recursa::Vec1<crate::tokens::ColId<'input>>,
}

/// `WITH any_name_list` — dictionary list on `ALTER TEXT SEARCH
/// CONFIGURATION ... { ADD | ALTER } MAPPING FOR ... WITH ...`.
/// Dictionaries are dotted `any_name`s.
#[derive(recursa::Node, Debug, Clone)]
pub struct TextSearchWithDicts<'input> {
    #[tok(WITH, this)]
    pub dicts: NameList<'input>,
}

/// `ADD MAPPING FOR name_list WITH any_name_list` — Postgres'
/// `ALTER_TSCONFIG_ADD_MAPPING` branch.
///
/// `tokens` and `dicts` are inlined as flat fields (rather than wrapped in
/// `TextSearchTokenList` / `TextSearchWithDicts`) so the build-time
/// first-set prefix dispatch sees a single-token chain
/// (`ADD MAPPING FOR …`). If we nested `TextSearchTokenList` here, codegen
/// would extend its first-set through both `FOR` and the trailing `WITH`,
/// assuming the two are adjacent — but they're separated by the token
/// list, so the prefix-driven parse path fails and the whole statement
/// surfaces as a file-level parse error.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ADD, MAPPING, FOR, this)]
pub struct TSConfigAddMapping<'input> {
    #[sep(COMMA)]
    pub tokens: recursa::Vec1<crate::tokens::ColId<'input>>,
    #[tok(WITH, this)]
    pub dicts: NameList<'input>,
}

/// `REPLACE any_name WITH any_name` — the dictionary-replacement tail
/// shared by the two `ALTER MAPPING REPLACE ...` forms.
#[derive(recursa::Node, Debug, Clone)]
pub struct TSConfigReplaceClause<'input> {
    #[tok(REPLACE, this)]
    pub old_dict: QualifiedName<'input>,
    #[tok(WITH, this)]
    pub new_dict: QualifiedName<'input>,
}

/// `FOR name_list { WITH any_name_list | REPLACE any_name WITH any_name }`
/// — the body of `ALTER MAPPING FOR ...`. `ALTER MAPPING FOR tokens WITH
/// dicts` is gram.y's `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN`; `ALTER
/// MAPPING FOR tokens REPLACE old WITH new` is
/// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN`.
///
/// Variant ordering: `With` (peek = `WITH`) and `Replace` (peek =
/// `REPLACE`) are keyword-disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum TSConfigAlterMappingForTail<'input> {
    With(TextSearchWithDicts<'input>),
    Replace(TSConfigReplaceClause<'input>),
}

/// `ALTER MAPPING FOR name_list { WITH any_name_list | REPLACE old WITH new }`
/// — gram.y's `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN` and
/// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TSConfigAlterMappingFor<'input> {
    pub tokens: TextSearchTokenList<'input>,
    pub tail: TSConfigAlterMappingForTail<'input>,
}

/// `ALTER MAPPING { FOR ... | REPLACE old WITH new }` — Postgres'
/// `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN` /
/// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN` /
/// `ALTER_TSCONFIG_REPLACE_DICT` branches.
///
/// Variant ordering: `ForTokens` (peek = `FOR`) and `Replace`
/// (peek = `REPLACE`) are keyword-disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum TSConfigAlterMappingKind<'input> {
    ForTokens(TSConfigAlterMappingFor<'input>),
    Replace(TSConfigReplaceClause<'input>),
}

/// `ALTER MAPPING ...` action on `ALTER TEXT SEARCH CONFIGURATION`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TSConfigAlterMapping<'input> {
    #[tok(ALTER, MAPPING, this)]
    pub kind: TSConfigAlterMappingKind<'input>,
}

/// `DROP MAPPING [IF EXISTS] FOR name_list` — Postgres'
/// `ALTER_TSCONFIG_DROP_MAPPING` branch (with optional `IF EXISTS`).
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, MAPPING, this)]
pub struct TSConfigDropMapping<'input> {
    pub if_exists: Option<IfExists>,
    pub tokens: TextSearchTokenList<'input>,
}

/// One action on `ALTER TEXT SEARCH CONFIGURATION name action` — covers
/// Postgres' `AlterTSConfigurationStmt` (six mapping branches) plus the
/// `RENAME TO` / `OWNER TO` / `SET SCHEMA` branches from `RenameStmt`,
/// `AlterOwnerStmt`, and `AlterObjectSchemaStmt`.
///
/// Variant ordering: each branch has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`, `ADD`, `ALTER`, `DROP`), so the variants
/// are keyword-disjoint and order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTSConfigurationAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    AddMapping(TSConfigAddMapping<'input>),
    AlterMapping(TSConfigAlterMapping<'input>),
    DropMapping(TSConfigDropMapping<'input>),
}

/// `CONFIGURATION name action` — body of
/// `ALTER TEXT SEARCH CONFIGURATION ...`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTSConfigurationBody<'input> {
    #[tok(CONFIGURATION, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterTSConfigurationAction<'input>,
}

/// One action on `ALTER TEXT SEARCH DICTIONARY name action` — covers
/// Postgres' `AlterTSDictionaryStmt` (`definition`) plus the
/// `RENAME TO` / `OWNER TO` / `SET SCHEMA` branches.
///
/// Variant ordering: keyword-distinct branches first (`Rename` on
/// `RENAME`, `Owner` on `OWNER`, `SetSchema` on `SET`); the `Definition`
/// branch starts with `(` (a `DefList`), so it cannot collide.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTSDictionaryAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    Definition(DefList<'input>),
}

/// `DICTIONARY name action` — body of
/// `ALTER TEXT SEARCH DICTIONARY ...`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTSDictionaryBody<'input> {
    #[tok(DICTIONARY, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterTSDictionaryAction<'input>,
}

/// One action on `ALTER TEXT SEARCH { PARSER | TEMPLATE } name action`
/// — only the rename/set-schema branches from `RenameStmt` /
/// `AlterObjectSchemaStmt`; gram.y has no parser/template `OWNER`
/// action.
///
/// Variant ordering: `Rename` (peek = `RENAME`) and `SetSchema`
/// (peek = `SET`) are keyword-disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTSRenameSchemaAction<'input> {
    Rename(RenameTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `PARSER name action` — body of `ALTER TEXT SEARCH PARSER ...`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTSParserBody<'input> {
    #[tok(PARSER, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterTSRenameSchemaAction<'input>,
}

/// `TEMPLATE name action` — body of `ALTER TEXT SEARCH TEMPLATE ...`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTSTemplateBody<'input> {
    #[tok(TEMPLATE, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterTSRenameSchemaAction<'input>,
}

/// The `CONFIGURATION | DICTIONARY | PARSER | TEMPLATE` body of
/// `ALTER TEXT SEARCH ...`. Each branch is gated by a distinct soft
/// keyword token.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTextSearchBody<'input> {
    Configuration(AlterTSConfigurationBody<'input>),
    Dictionary(AlterTSDictionaryBody<'input>),
    Parser(AlterTSParserBody<'input>),
    Template(AlterTSTemplateBody<'input>),
}

/// `ALTER TEXT SEARCH { CONFIGURATION | DICTIONARY | PARSER | TEMPLATE }
/// name action` — Postgres' `AlterTSConfigurationStmt`,
/// `AlterTSDictionaryStmt`, and the text-search branches of
/// `RenameStmt` / `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTextSearchStmt<'input> {
    #[tok(ALTER, TEXT, SEARCH, this)]
    pub body: AlterTextSearchBody<'input>,
}
