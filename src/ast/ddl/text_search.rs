//! TEXT SEARCH DDL statements (CREATE/ALTER/DROP).
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

/// `CREATE TEXT SEARCH { PARSER | DICTIONARY | TEMPLATE | CONFIGURATION }
/// name (def_list)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateTextSearchStmt<'input> {
    pub create: CREATE,
    pub text: TEXT,
    pub search: SEARCH,
    pub kind: TextSearchObjectKind,
    pub name: QualifiedName<'input>,
    pub definition: DefList<'input>,
}

/// The object kind after `DROP TEXT SEARCH`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TextSearchObjectKind {
    Configuration(crate::tokens::soft_keyword::CONFIGURATION),
    Dictionary(crate::tokens::soft_keyword::DICTIONARY),
    Parser(crate::tokens::soft_keyword::PARSER),
    Template(crate::tokens::soft_keyword::TEMPLATE),
}

/// `DROP TEXT SEARCH {CONFIGURATION | DICTIONARY | PARSER | TEMPLATE}
/// [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropTextSearchStmt<'input> {
    pub drop: DROP,
    pub text: TEXT,
    pub search: SEARCH,
    pub kind: TextSearchObjectKind,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `FOR name_list` — token-type list on `ALTER TEXT SEARCH CONFIGURATION
/// ... { ADD | ALTER | DROP } MAPPING FOR ...`. Tokens are plain `name`s
/// (Postgres' `ColId`), not dotted `any_name`s.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TextSearchTokenList<'input> {
    pub for_: FOR,
    pub tokens: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
}

/// `WITH any_name_list` — dictionary list on `ALTER TEXT SEARCH
/// CONFIGURATION ... { ADD | ALTER } MAPPING FOR ... WITH ...`.
/// Dictionaries are dotted `any_name`s.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TextSearchWithDicts<'input> {
    pub with: WITH,
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
/// surfaces as a [`crate::ast::FileItem::ParseError`].
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TSConfigAddMapping<'input> {
    pub add: ADD,
    pub mapping: MAPPING,
    pub for_: FOR,
    pub tokens: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
    pub with: WITH,
    pub dicts: NameList<'input>,
}

/// `REPLACE any_name WITH any_name` — the dictionary-replacement tail
/// shared by the two `ALTER MAPPING REPLACE ...` forms.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TSConfigReplaceClause<'input> {
    pub replace: REPLACE,
    pub old_dict: QualifiedName<'input>,
    pub with: WITH,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TSConfigAlterMappingForTail<'input> {
    With(TextSearchWithDicts<'input>),
    Replace(TSConfigReplaceClause<'input>),
}

/// `ALTER MAPPING FOR name_list { WITH any_name_list | REPLACE old WITH new }`
/// — gram.y's `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN` and
/// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TSConfigAlterMappingKind<'input> {
    ForTokens(TSConfigAlterMappingFor<'input>),
    Replace(TSConfigReplaceClause<'input>),
}

/// `ALTER MAPPING ...` action on `ALTER TEXT SEARCH CONFIGURATION`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TSConfigAlterMapping<'input> {
    pub alter: ALTER,
    pub mapping: MAPPING,
    pub kind: TSConfigAlterMappingKind<'input>,
}

/// `DROP MAPPING [IF EXISTS] FOR name_list` — Postgres'
/// `ALTER_TSCONFIG_DROP_MAPPING` branch (with optional `IF EXISTS`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TSConfigDropMapping<'input> {
    pub drop: DROP,
    pub mapping: MAPPING,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTSConfigurationBody<'input> {
    pub configuration: crate::tokens::soft_keyword::CONFIGURATION,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterTSDictionaryAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    Definition(DefList<'input>),
}

/// `DICTIONARY name action` — body of
/// `ALTER TEXT SEARCH DICTIONARY ...`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTSDictionaryBody<'input> {
    pub dictionary: crate::tokens::soft_keyword::DICTIONARY,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterTSRenameSchemaAction<'input> {
    Rename(RenameTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `PARSER name action` — body of `ALTER TEXT SEARCH PARSER ...`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTSParserBody<'input> {
    pub parser: crate::tokens::soft_keyword::PARSER,
    pub name: QualifiedName<'input>,
    pub action: AlterTSRenameSchemaAction<'input>,
}

/// `TEMPLATE name action` — body of `ALTER TEXT SEARCH TEMPLATE ...`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTSTemplateBody<'input> {
    pub template: crate::tokens::soft_keyword::TEMPLATE,
    pub name: QualifiedName<'input>,
    pub action: AlterTSRenameSchemaAction<'input>,
}

/// The `CONFIGURATION | DICTIONARY | PARSER | TEMPLATE` body of
/// `ALTER TEXT SEARCH ...`. Each branch is gated by a distinct soft
/// keyword token.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterTextSearchStmt<'input> {
    pub alter: ALTER,
    pub text: TEXT,
    pub search: SEARCH,
    pub body: AlterTextSearchBody<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_text_search_configuration() {
        let mut input = crate::tokens::test_input("DROP TEXT SEARCH CONFIGURATION IF EXISTS tsc1");
        let stmt = DropTextSearchStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.kind, TextSearchObjectKind::Configuration(_)));
        assert!(stmt.if_exists.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_text_search_dictionary() {
        let mut input = crate::tokens::test_input(
            "CREATE TEXT SEARCH DICTIONARY alt_ts_dict1 (template=simple)",
        );
        let _stmt = CreateTextSearchStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_text_search_configuration_rename() {
        let mut input = crate::tokens::test_input(
            "ALTER TEXT SEARCH CONFIGURATION alt_ts_conf1 RENAME TO alt_ts_conf2",
        );
        let _stmt = AlterTextSearchStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_text_search_parser() {
        let mut input = crate::tokens::test_input("DROP TEXT SEARCH PARSER my_parser");
        let _stmt = DropTextSearchStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_text_search_dictionary_structured() {
        let mut input = crate::tokens::test_input(
            "CREATE TEXT SEARCH DICTIONARY ispell (Template=ispell, DictFile=ispell_sample)",
        );
        let stmt = CreateTextSearchStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.kind, TextSearchObjectKind::Dictionary(_)));
        assert_eq!(stmt.name.object(), "ispell");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_text_search_configuration() {
        let mut input = crate::tokens::test_input(
            "CREATE TEXT SEARCH CONFIGURATION ispell_tst (PARSER = default)",
        );
        let stmt = CreateTextSearchStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.kind, TextSearchObjectKind::Configuration(_)));
        assert!(input.is_empty());
    }
}
