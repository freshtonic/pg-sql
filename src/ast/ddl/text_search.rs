//! TEXT SEARCH DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `CREATE TEXT SEARCH { PARSER | DICTIONARY | TEMPLATE | CONFIGURATION }
    /// name (def_list)`.
    #[derive(Debug)]
    pub struct CreateTextSearchStmt {
        #[tok(CREATE, TEXT, SEARCH, this)]
        pub kind: TextSearchObjectKind,
        pub name: QualifiedName,
        pub definition: DefList,
    }
}

recursa::ast_node! {
    /// The object kind after `DROP TEXT SEARCH`.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `DROP TEXT SEARCH {CONFIGURATION | DICTIONARY | PARSER | TEMPLATE}
    /// [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropTextSearchStmt {
        #[tok(DROP, TEXT, SEARCH, this)]
        pub kind: TextSearchObjectKind,
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `FOR name_list` — token-type list on `ALTER TEXT SEARCH CONFIGURATION
    /// ... { ADD | ALTER | DROP } MAPPING FOR ...`. Tokens are plain `name`s
    /// (Postgres' `ColId`), not dotted `any_name`s.
    #[derive(Debug)]
    #[tok(FOR, this)]
    pub struct TextSearchTokenList {
        #[sep(COMMA)]
        pub tokens: one_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `WITH any_name_list` — dictionary list on `ALTER TEXT SEARCH
    /// CONFIGURATION ... { ADD | ALTER } MAPPING FOR ... WITH ...`.
    /// Dictionaries are dotted `any_name`s.
    #[derive(Debug)]
    pub struct TextSearchWithDicts {
        /// gram.y `any_with` (`WITH | WITH_LA`): a dictionary may be named
        /// `time` or `ordinality`.
        pub with: crate::ast::shared::flags::AnyWith,
        pub dicts: NameList,
    }
}

recursa::ast_node! {
    /// `ADD MAPPING FOR name_list WITH any_name_list` — Postgres'
    /// `ALTER_TSCONFIG_ADD_MAPPING` branch.
    ///
    /// `tokens` and `dicts` are inlined as flat fields (rather than wrapped in
    /// `TextSearchTokenList` / `TextSearchWithDicts`) so the LR production mirrors
    /// gram.y's `ADD MAPPING FOR name_list WITH any_name_list` sequence directly.
    #[derive(Debug)]
    #[tok(ADD, MAPPING, FOR, this)]
    pub struct TSConfigAddMapping {
        #[sep(COMMA)]
        pub tokens: one_or_many!(crate::tokens::ColId),
        /// gram.y `any_with` (`WITH | WITH_LA`).
        pub with: crate::ast::shared::flags::AnyWith,
        pub dicts: NameList,
    }
}

recursa::ast_node! {
    /// `REPLACE any_name WITH any_name` — the dictionary-replacement tail
    /// shared by the two `ALTER MAPPING REPLACE ...` forms.
    #[derive(Debug)]
    pub struct TSConfigReplaceClause {
        #[tok(REPLACE, this)]
        pub old_dict: QualifiedName,
        /// gram.y `any_with` (`WITH | WITH_LA`).
        pub with: crate::ast::shared::flags::AnyWith,
        pub new_dict: QualifiedName,
    }
}

recursa::ast_node! {
    /// `FOR name_list { WITH any_name_list | REPLACE any_name WITH any_name }`
    /// — the body of `ALTER MAPPING FOR ...`. `ALTER MAPPING FOR tokens WITH
    /// dicts` is gram.y's `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN`; `ALTER
    /// MAPPING FOR tokens REPLACE old WITH new` is
    /// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN`.
    ///
    /// Variant ordering: `With` (peek = `WITH`) and `Replace` (peek =
    /// `REPLACE`) are keyword-disjoint.
    #[derive(Debug)]
    pub enum TSConfigAlterMappingForTail {
        With(TextSearchWithDicts),
        Replace(TSConfigReplaceClause),
    }
}

recursa::ast_node! {
    /// `ALTER MAPPING FOR name_list { WITH any_name_list | REPLACE old WITH new }`
    /// — gram.y's `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN` and
    /// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN`.
    #[derive(Debug)]
    pub struct TSConfigAlterMappingFor {
        pub tokens: TextSearchTokenList,
        pub tail: TSConfigAlterMappingForTail,
    }
}

recursa::ast_node! {
    /// `ALTER MAPPING { FOR ... | REPLACE old WITH new }` — Postgres'
    /// `ALTER_TSCONFIG_ALTER_MAPPING_FOR_TOKEN` /
    /// `ALTER_TSCONFIG_REPLACE_DICT_FOR_TOKEN` /
    /// `ALTER_TSCONFIG_REPLACE_DICT` branches.
    ///
    /// Variant ordering: `ForTokens` (peek = `FOR`) and `Replace`
    /// (peek = `REPLACE`) are keyword-disjoint.
    #[derive(Debug)]
    pub enum TSConfigAlterMappingKind {
        ForTokens(TSConfigAlterMappingFor),
        Replace(TSConfigReplaceClause),
    }
}

recursa::ast_node! {
    /// `ALTER MAPPING ...` action on `ALTER TEXT SEARCH CONFIGURATION`.
    #[derive(Debug)]
    pub struct TSConfigAlterMapping {
        #[tok(ALTER, MAPPING, this)]
        pub kind: TSConfigAlterMappingKind,
    }
}

recursa::ast_node! {
    /// `DROP MAPPING [IF EXISTS] FOR name_list` — Postgres'
    /// `ALTER_TSCONFIG_DROP_MAPPING` branch (with optional `IF EXISTS`).
    #[derive(Debug)]
    #[tok(DROP, MAPPING, this)]
    pub struct TSConfigDropMapping {
        pub if_exists: Option<IfExists>,
        pub tokens: TextSearchTokenList,
    }
}

recursa::ast_node! {
    /// One action on `ALTER TEXT SEARCH CONFIGURATION name action` — covers
    /// Postgres' `AlterTSConfigurationStmt` (six mapping branches) plus the
    /// `RENAME TO` / `OWNER TO` / `SET SCHEMA` branches from `RenameStmt`,
    /// `AlterOwnerStmt`, and `AlterObjectSchemaStmt`.
    ///
    /// Variant ordering: each branch has a distinct leading keyword
    /// (`RENAME`, `OWNER`, `SET`, `ADD`, `ALTER`, `DROP`), so the variants
    /// are keyword-disjoint and order is for clarity.
    #[derive(Debug)]
    pub enum AlterTSConfigurationAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
        AddMapping(TSConfigAddMapping),
        AlterMapping(TSConfigAlterMapping),
        DropMapping(TSConfigDropMapping),
    }
}

recursa::ast_node! {
    /// `CONFIGURATION name action` — body of
    /// `ALTER TEXT SEARCH CONFIGURATION ...`.
    #[derive(Debug)]
    pub struct AlterTSConfigurationBody {
        #[tok(CONFIGURATION, this)]
        pub name: QualifiedName,
        pub action: AlterTSConfigurationAction,
    }
}

recursa::ast_node! {
    /// One action on `ALTER TEXT SEARCH DICTIONARY name action` — covers
    /// Postgres' `AlterTSDictionaryStmt` (`definition`) plus the
    /// `RENAME TO` / `OWNER TO` / `SET SCHEMA` branches.
    ///
    /// Variant ordering: keyword-distinct branches first (`Rename` on
    /// `RENAME`, `Owner` on `OWNER`, `SetSchema` on `SET`); the `Definition`
    /// branch starts with `(` (a `DefList`), so it cannot collide.
    #[derive(Debug)]
    pub enum AlterTSDictionaryAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
        Definition(DefList),
    }
}

recursa::ast_node! {
    /// `DICTIONARY name action` — body of
    /// `ALTER TEXT SEARCH DICTIONARY ...`.
    #[derive(Debug)]
    pub struct AlterTSDictionaryBody {
        #[tok(DICTIONARY, this)]
        pub name: QualifiedName,
        pub action: AlterTSDictionaryAction,
    }
}

recursa::ast_node! {
    /// One action on `ALTER TEXT SEARCH { PARSER | TEMPLATE } name action`
    /// — only the rename/set-schema branches from `RenameStmt` /
    /// `AlterObjectSchemaStmt`; gram.y has no parser/template `OWNER`
    /// action.
    ///
    /// Variant ordering: `Rename` (peek = `RENAME`) and `SetSchema`
    /// (peek = `SET`) are keyword-disjoint.
    #[derive(Debug)]
    pub enum AlterTSRenameSchemaAction {
        Rename(RenameTo),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `PARSER name action` — body of `ALTER TEXT SEARCH PARSER ...`.
    #[derive(Debug)]
    pub struct AlterTSParserBody {
        #[tok(PARSER, this)]
        pub name: QualifiedName,
        pub action: AlterTSRenameSchemaAction,
    }
}

recursa::ast_node! {
    /// `TEMPLATE name action` — body of `ALTER TEXT SEARCH TEMPLATE ...`.
    #[derive(Debug)]
    pub struct AlterTSTemplateBody {
        #[tok(TEMPLATE, this)]
        pub name: QualifiedName,
        pub action: AlterTSRenameSchemaAction,
    }
}

recursa::ast_node! {
    /// The `CONFIGURATION | DICTIONARY | PARSER | TEMPLATE` body of
    /// `ALTER TEXT SEARCH ...`. Each branch is gated by a distinct soft
    /// keyword token.
    #[derive(Debug)]
    pub enum AlterTextSearchBody {
        Configuration(AlterTSConfigurationBody),
        Dictionary(AlterTSDictionaryBody),
        Parser(AlterTSParserBody),
        Template(AlterTSTemplateBody),
    }
}

recursa::ast_node! {
    /// `ALTER TEXT SEARCH { CONFIGURATION | DICTIONARY | PARSER | TEMPLATE }
    /// name action` — Postgres' `AlterTSConfigurationStmt`,
    /// `AlterTSDictionaryStmt`, and the text-search branches of
    /// `RenameStmt` / `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
    #[derive(Debug)]
    pub struct AlterTextSearchStmt {
        #[tok(ALTER, TEXT, SEARCH, this)]
        pub body: AlterTextSearchBody,
    }
}
