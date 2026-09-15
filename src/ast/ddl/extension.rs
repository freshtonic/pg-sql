//! EXTENSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `SCHEMA name` option on `CREATE EXTENSION`.
    #[derive(Debug)]
    pub struct ExtensionSchemaOption {
        #[tok(SCHEMA, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `VERSION { sconst | ident }` option on `CREATE EXTENSION`. Postgres'
    /// `NonReservedWord_or_Sconst` allows either a quoted string or a bareword.
    #[derive(Debug)]
    pub enum ExtensionVersionValue {
        String(CopySconst),
        /// Any bareword (incl. soft keywords) — `NonReservedWord`.
        Word(literal::AliasName),
    }
}

recursa::ast_node! {
    /// `VERSION value` option on `CREATE EXTENSION`.
    #[derive(Debug)]
    pub struct ExtensionVersionOption {
        #[tok(VERSION, this)]
        pub value: ExtensionVersionValue,
    }
}

recursa::ast_node! {
    /// A single CREATE EXTENSION option — Postgres' `create_extension_opt_item`.
    /// Options are unordered and repeatable.
    #[derive(Debug)]
    pub enum ExtensionOption {
        Schema(ExtensionSchemaOption),
        Version(ExtensionVersionOption),
        #[tok(CASCADE)]
        Cascade,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(CREATE, EXTENSION, this)]
    pub struct CreateExtensionStmt {
        pub if_not_exists: Option<crate::ast::shared::flags::IfNotExists>,
        pub name: crate::tokens::ColId,
        #[tok(optional(WITH), this)]
        pub options: zero_or_many!(ExtensionOption),
    }
}

recursa::ast_node! {
    /// `DROP EXTENSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, EXTENSION, this)]
    pub struct DropExtensionStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `TO version` (`NonReservedWord_or_Sconst`) — the version target on
    /// `ALTER EXTENSION name UPDATE`. Postgres' `alter_extension_opt_item`.
    ///
    /// pg-sql accepts `TO sconst` (a string literal); the bare-identifier
    /// form is not exercised by the corpus.
    #[derive(Debug)]
    pub struct AlterExtensionUpdateTo {
        #[tok(TO, this)]
        pub version: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `UPDATE [TO version]` — Postgres' `AlterExtensionStmt`.
    #[derive(Debug)]
    #[tok(UPDATE, this)]
    pub struct AlterExtensionUpdate {
        pub to: Option<AlterExtensionUpdateTo>,
    }
}

recursa::ast_node! {
    /// `MATERIALIZED VIEW` / `FOREIGN TABLE` / `TEXT SEARCH PARSER` etc. —
    /// the multi-word entries of Postgres' `object_type_any_name` (any-name
    /// objects whose type label is more than one keyword).
    ///
    /// Variant ordering: longer multi-keyword variants first (e.g.
    /// `MATERIALIZED VIEW` before `VIEW`).
    #[derive(Debug)]
    pub enum ExtensionObjectTypeAnyName {
        #[tok(MATERIALIZED, VIEW)]
        MaterializedView,
        #[tok(FOREIGN, TABLE)]
        ForeignTable,
        #[tok(TEXT, SEARCH, PARSER)]
        TextSearchParser,
        #[tok(TEXT, SEARCH, DICTIONARY)]
        TextSearchDictionary,
        #[tok(TEXT, SEARCH, TEMPLATE)]
        TextSearchTemplate,
        #[tok(TEXT, SEARCH, CONFIGURATION)]
        TextSearchConfiguration,
        #[tok(TABLE)]
        Table,
        #[tok(SEQUENCE)]
        Sequence,
        #[tok(VIEW)]
        View,
        #[tok(INDEX)]
        Index,
        #[tok(COLLATION)]
        Collation,
        #[tok(CONVERSION)]
        Conversion,
        #[tok(STATISTICS)]
        Statistics,
    }
}

recursa::ast_node! {
    /// `ACCESS METHOD` / `EVENT TRIGGER` / `FOREIGN DATA WRAPPER` / etc. —
    /// Postgres' `drop_type_name` / `object_type_name` (`name`-taking
    /// objects). Excludes the qualified-name `LANGUAGE` form (covered by
    /// the dedicated `ALTER EXTENSION ... LANGUAGE` branch below).
    ///
    /// Variant ordering: longest multi-keyword forms first.
    #[derive(Debug)]
    pub enum ExtensionObjectTypeName {
        #[tok(FOREIGN, DATA, WRAPPER)]
        ForeignDataWrapper,
        #[tok(ACCESS, METHOD)]
        AccessMethod,
        #[tok(EVENT, TRIGGER)]
        EventTrigger,
        #[tok(DATABASE)]
        Database,
        #[tok(ROLE)]
        Role,
        #[tok(SUBSCRIPTION)]
        Subscription,
        #[tok(TABLESPACE)]
        Tablespace,
        #[tok(EXTENSION)]
        Extension,
        #[tok(PUBLICATION)]
        Publication,
        #[tok(SCHEMA)]
        Schema,
        #[tok(SERVER)]
        Server,
    }
}

recursa::ast_node! {
    /// `add_drop object_type_any_name any_name` — the dotted-name branch of
    /// Postgres' `AlterExtensionContentsStmt`.
    #[derive(Debug)]
    pub struct AlterExtensionAnyNameMember {
        pub object_type: ExtensionObjectTypeAnyName,
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `add_drop object_type_name name` — the simple-name branch of
    /// Postgres' `AlterExtensionContentsStmt`.
    #[derive(Debug)]
    pub struct AlterExtensionNameMember {
        pub object_type: ExtensionObjectTypeName,
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `add_drop [PROCEDURAL] LANGUAGE name` — Postgres'
    /// `AlterExtensionContentsStmt` LANGUAGE branch (via the
    /// `opt_procedural LANGUAGE` arm of `drop_type_name`).
    #[derive(Debug)]
    pub struct AlterExtensionLanguageMember {
        #[tok(optional(PROCEDURAL), LANGUAGE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterExtensionMember {
        Language(AlterExtensionLanguageMember),
        AnyName(AlterExtensionAnyNameMember),
        Name(AlterExtensionNameMember),
    }
}

recursa::ast_node! {
    /// `ADD member` — Postgres' `AlterExtensionContentsStmt` ADD branch.
    #[derive(Debug)]
    pub struct AlterExtensionAdd {
        #[tok(ADD, this)]
        pub member: AlterExtensionMember,
    }
}

recursa::ast_node! {
    /// `DROP member` — Postgres' `AlterExtensionContentsStmt` DROP branch.
    #[derive(Debug)]
    pub struct AlterExtensionDrop {
        #[tok(DROP, this)]
        pub member: AlterExtensionMember,
    }
}

recursa::ast_node! {
    /// One action on `ALTER EXTENSION name action` — Postgres'
    /// `AlterExtensionStmt` (UPDATE [TO version]),
    /// `AlterExtensionContentsStmt` (ADD/DROP object), and the extension
    /// branch of `AlterObjectSchemaStmt` (SET SCHEMA).
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`UPDATE`, `ADD`, `DROP`, `SET`), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterExtensionAction {
        Update(AlterExtensionUpdate),
        Add(AlterExtensionAdd),
        Drop(AlterExtensionDrop),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `ALTER EXTENSION name action` — Postgres' `AlterExtensionStmt` and
    /// `AlterExtensionContentsStmt` plus the extension branch of
    /// `AlterObjectSchemaStmt`.
    #[derive(Debug)]
    pub struct AlterExtensionStmt {
        #[tok(ALTER, EXTENSION, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterExtensionAction,
    }
}
