//! EXTENSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExtensionSchemaOption<'input> {
    pub schema: SCHEMA,
    pub name: crate::tokens::ColId<'input>,
}

/// `VERSION { sconst | ident }` option on `CREATE EXTENSION`. Postgres'
/// `NonReservedWord_or_Sconst` allows either a quoted string or a bareword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExtensionVersionValue<'input> {
    String(CopySconst<'input>),
    /// Any bareword (incl. soft keywords) — `NonReservedWord`.
    Word(literal::AliasName<'input>),
}

/// `VERSION value` option on `CREATE EXTENSION`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExtensionVersionOption<'input> {
    pub version: VERSION,
    pub value: ExtensionVersionValue<'input>,
}

/// A single CREATE EXTENSION option — Postgres' `create_extension_opt_item`.
/// Options are unordered and repeatable.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExtensionOption<'input> {
    Schema(ExtensionSchemaOption<'input>),
    Version(ExtensionVersionOption<'input>),
    Cascade(CASCADE),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateExtensionStmt<'input> {
    pub create: CREATE,
    pub extension: EXTENSION,
    pub if_not_exists: Option<crate::ast::shared::flags::IfNotExists>,
    pub name: crate::tokens::ColId<'input>,
    pub with: Option<WITH>,
    pub options: Vec<ExtensionOption<'input>>,
}

/// `DROP EXTENSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropExtensionStmt<'input> {
    pub drop: DROP,
    pub extension: EXTENSION,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `TO version` (`NonReservedWord_or_Sconst`) — the version target on
/// `ALTER EXTENSION name UPDATE`. Postgres' `alter_extension_opt_item`.
///
/// pg-sql accepts `TO sconst` (a string literal); the bare-identifier
/// form is not exercised by the corpus.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionUpdateTo<'input> {
    pub to: TO,
    pub version: literal::StringLit<'input>,
}

/// `UPDATE [TO version]` — Postgres' `AlterExtensionStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionUpdate<'input> {
    pub update: UPDATE,
    pub to: Option<AlterExtensionUpdateTo<'input>>,
}

/// `MATERIALIZED VIEW` / `FOREIGN TABLE` / `TEXT SEARCH PARSER` etc. —
/// the multi-word entries of Postgres' `object_type_any_name` (any-name
/// objects whose type label is more than one keyword).
///
/// Variant ordering: longer multi-keyword variants first (e.g.
/// `MATERIALIZED VIEW` before `VIEW`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExtensionObjectTypeAnyName {
    MaterializedView((MATERIALIZED, VIEW)),
    ForeignTable((FOREIGN, TABLE)),
    TextSearchParser((TEXT, SEARCH, PARSER)),
    TextSearchDictionary((TEXT, SEARCH, DICTIONARY)),
    TextSearchTemplate((TEXT, SEARCH, TEMPLATE)),
    TextSearchConfiguration((TEXT, SEARCH, CONFIGURATION)),
    Table(TABLE),
    Sequence(SEQUENCE),
    View(VIEW),
    Index(INDEX),
    Collation(COLLATION),
    Conversion(CONVERSION),
    Statistics(STATISTICS),
}

/// `ACCESS METHOD` / `EVENT TRIGGER` / `FOREIGN DATA WRAPPER` / etc. —
/// Postgres' `drop_type_name` / `object_type_name` (`name`-taking
/// objects). Excludes the qualified-name `LANGUAGE` form (covered by
/// the dedicated `ALTER EXTENSION ... LANGUAGE` branch below).
///
/// Variant ordering: longest multi-keyword forms first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExtensionObjectTypeName {
    ForeignDataWrapper((FOREIGN, DATA, WRAPPER)),
    AccessMethod((ACCESS, METHOD)),
    EventTrigger((EVENT, TRIGGER)),
    Database(DATABASE),
    Role(ROLE),
    Subscription(SUBSCRIPTION),
    Tablespace(TABLESPACE),
    Extension(EXTENSION),
    Publication(PUBLICATION),
    Schema(SCHEMA),
    Server(SERVER),
}

/// `add_drop object_type_any_name any_name` — the dotted-name branch of
/// Postgres' `AlterExtensionContentsStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionAnyNameMember<'input> {
    pub object_type: ExtensionObjectTypeAnyName,
    pub name: QualifiedName<'input>,
}

/// `add_drop object_type_name name` — the simple-name branch of
/// Postgres' `AlterExtensionContentsStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionNameMember<'input> {
    pub object_type: ExtensionObjectTypeName,
    pub name: crate::tokens::ColId<'input>,
}

/// `add_drop [PROCEDURAL] LANGUAGE name` — Postgres'
/// `AlterExtensionContentsStmt` LANGUAGE branch (via the
/// `opt_procedural LANGUAGE` arm of `drop_type_name`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionLanguageMember<'input> {
    pub procedural: Option<PROCEDURAL>,
    pub language: LANGUAGE,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterExtensionMember<'input> {
    Language(AlterExtensionLanguageMember<'input>),
    AnyName(AlterExtensionAnyNameMember<'input>),
    Name(AlterExtensionNameMember<'input>),
}

/// `ADD member` — Postgres' `AlterExtensionContentsStmt` ADD branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionAdd<'input> {
    pub add: ADD,
    pub member: AlterExtensionMember<'input>,
}

/// `DROP member` — Postgres' `AlterExtensionContentsStmt` DROP branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterExtensionDrop<'input> {
    pub drop: DROP,
    pub member: AlterExtensionMember<'input>,
}

/// One action on `ALTER EXTENSION name action` — Postgres'
/// `AlterExtensionStmt` (UPDATE [TO version]),
/// `AlterExtensionContentsStmt` (ADD/DROP object), and the extension
/// branch of `AlterObjectSchemaStmt` (SET SCHEMA).
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`UPDATE`, `ADD`, `DROP`, `SET`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterExtensionAction<'input> {
    Update(AlterExtensionUpdate<'input>),
    Add(AlterExtensionAdd<'input>),
    Drop(AlterExtensionDrop<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER EXTENSION name action` — Postgres' `AlterExtensionStmt` and
/// `AlterExtensionContentsStmt` plus the extension branch of
/// `AlterObjectSchemaStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterExtensionStmt<'input> {
    pub alter: ALTER,
    pub extension: EXTENSION,
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
        let mut input = crate::tokens::test_input("CREATE EXTENSION hstore");
        let stmt = CreateExtensionStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "hstore");
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_extension_if_not_exists_with_options() {
        let mut input = crate::tokens::test_input(
            "CREATE EXTENSION IF NOT EXISTS hstore WITH SCHEMA public VERSION '1.6' CASCADE",
        );
        let stmt = CreateExtensionStmt::parse(&mut input).unwrap();
        assert!(stmt.if_not_exists.is_some());
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_extension_update() {
        let mut input = crate::tokens::test_input("ALTER EXTENSION my_ext UPDATE TO '1.1'");
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_extension_update_no_to() {
        let mut input = crate::tokens::test_input("ALTER EXTENSION my_ext UPDATE");
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_extension_set_schema() {
        let mut input = crate::tokens::test_input("ALTER EXTENSION my_ext SET SCHEMA new_schema");
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_extension_add_table() {
        let mut input = crate::tokens::test_input("ALTER EXTENSION my_ext ADD TABLE my_table");
        let _stmt = AlterExtensionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
