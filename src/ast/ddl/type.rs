//! TYPE  DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::publication::SetDefinitionClause;
use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// A single column in `CREATE TYPE name AS (col_list)` — Postgres'
/// `TableFuncElement`: `ColId Typename [COLLATE name]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CompositeTypeColumn<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub type_name: CastType<'input>,
    pub collate: Option<CompositeTypeCollate<'input>>,
}

/// `COLLATE name` clause on a composite-type column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CompositeTypeCollate<'input> {
    pub collate: COLLATE,
    pub name: QualifiedName<'input>,
}

/// `AS (col_list)` — composite-type definition body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateTypeComposite<'input> {
    pub r#as: AS,
    pub columns:
        Surrounded<punct::LParen, Seq0<CompositeTypeColumn<'input>, punct::Comma>, punct::RParen>,
}

/// `AS ENUM ('label', ...)` — enum-type definition body. The label list may
/// be empty (Postgres allows `AS ENUM ()` to create a shell-only enum).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateTypeEnum<'input> {
    pub r#as: AS,
    pub r#enum: ENUM,
    pub labels:
        Surrounded<punct::LParen, Seq0<literal::StringLit<'input>, punct::Comma>, punct::RParen>,
}

/// `AS RANGE (def_list)` — range-type definition body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateTypeRange<'input> {
    pub r#as: AS,
    pub range: RANGE,
    pub definition: DefList<'input>,
}

/// The body of a `CREATE TYPE name ‹body›` statement.
///
/// Variant ordering: multi-keyword forms (`AS ENUM`, `AS RANGE`) before
/// `Composite` (`AS` + paren list) so the longer match wins. `Base` is the
/// `(def_list)` form (no `AS`); it begins with `(` and so cannot collide
/// with the `AS …` variants.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreateTypeBody<'input> {
    Enum(CreateTypeEnum<'input>),
    Range(CreateTypeRange<'input>),
    Composite(CreateTypeComposite<'input>),
    Base(DefList<'input>),
}

/// `CREATE TYPE name [body]`.
///
/// - `CREATE TYPE name` — shell type
/// - `CREATE TYPE name AS (col_list)` — composite
/// - `CREATE TYPE name AS ENUM (labels)` — enum
/// - `CREATE TYPE name AS RANGE (def_list)` — range
/// - `CREATE TYPE name (def_list)` — base type
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateTypeStmt<'input> {
    pub create: CREATE,
    pub r#type: TYPE,
    pub name: QualifiedName<'input>,
    pub body: Option<CreateTypeBody<'input>>,
}

/// `DROP TYPE [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropTypeStmt<'input> {
    pub drop: DROP,
    pub r#type: TYPE,
    pub if_exists: Option<IfExists>,
    pub types: TypeNameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `RENAME ATTRIBUTE old TO new [CASCADE | RESTRICT]` — Postgres'
/// `RenameStmt` branch for composite-type attribute renames.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeRenameAttribute<'input> {
    pub rename: RENAME,
    pub attribute: ATTRIBUTE,
    pub old_name: crate::tokens::ColId<'input>,
    pub to: TO,
    pub new_name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `RENAME VALUE old_value TO new_value` — Postgres' `AlterEnumStmt`
/// branch for renaming enum values. Both literals are string literals.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeRenameValue<'input> {
    pub rename: RENAME,
    pub value: VALUE,
    pub old_value: literal::StringLit<'input>,
    pub to: TO,
    pub new_value: literal::StringLit<'input>,
}

/// `BEFORE 'value'` or `AFTER 'value'` — neighbor anchor on
/// `ALTER TYPE name ADD VALUE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterEnumValuePosition<'input> {
    Before(AlterEnumValueBefore<'input>),
    After(AlterEnumValueAfter<'input>),
}

/// `BEFORE 'neighbor'` — neighbour anchor on
/// `ALTER TYPE name ADD VALUE ... BEFORE 'neighbor'`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterEnumValueBefore<'input> {
    pub before: BEFORE,
    pub neighbor: literal::StringLit<'input>,
}

/// `AFTER 'neighbor'` — neighbour anchor on
/// `ALTER TYPE name ADD VALUE ... AFTER 'neighbor'`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterEnumValueAfter<'input> {
    pub after: AFTER,
    pub neighbor: literal::StringLit<'input>,
}

/// `ADD VALUE [IF NOT EXISTS] 'val' [{BEFORE|AFTER} 'neighbour']` —
/// Postgres' `AlterEnumStmt` ADD VALUE branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeAddValue<'input> {
    pub add: ADD,
    pub value: VALUE,
    pub if_not_exists: Option<IfNotExists>,
    pub new_value: literal::StringLit<'input>,
    pub position: Option<AlterEnumValuePosition<'input>>,
}

/// `ADD ATTRIBUTE column_def [CASCADE | RESTRICT]` — one `alter_type_cmd`
/// in Postgres. `column_def` is modelled as the same `CompositeTypeColumn`
/// used by `CREATE TYPE name AS (...)` (Postgres' `TableFuncElement`):
/// `name typename [COLLATE qualified_name]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeAddAttribute<'input> {
    pub add: ADD,
    pub attribute: ATTRIBUTE,
    pub column: CompositeTypeColumn<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP ATTRIBUTE [IF EXISTS] name [CASCADE | RESTRICT]` — one
/// `alter_type_cmd` in Postgres.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeDropAttribute<'input> {
    pub drop: DROP,
    pub attribute: ATTRIBUTE,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `[SET DATA]` modifier preceding `TYPE` in
/// `ALTER ATTRIBUTE name [SET DATA] TYPE typename`. Postgres'
/// `opt_set_data`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetDataClause {
    pub set: SET,
    pub data: DATA,
}

/// `ALTER ATTRIBUTE name [SET DATA] TYPE typename [COLLATE qual] [CASCADE
/// | RESTRICT]` — one `alter_type_cmd` in Postgres. The typename uses the
/// same `CastType` enum as `CREATE TYPE name AS (col_list)` column types.
/// The optional `COLLATE` clause reuses [`CompositeTypeCollate`] (Postgres'
/// `opt_collate_clause` — `COLLATE any_name`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeAlterAttribute<'input> {
    pub alter: ALTER,
    pub attribute: ATTRIBUTE,
    pub name: crate::tokens::ColId<'input>,
    pub set_data: Option<SetDataClause>,
    pub r#type: TYPE,
    pub type_name: CastType<'input>,
    pub collate: Option<CompositeTypeCollate<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// One `alter_type_cmd` in Postgres — an `ADD ATTRIBUTE`, `DROP ATTRIBUTE`,
/// or `ALTER ATTRIBUTE` action on `ALTER TYPE name action [, action ...]`.
///
/// Variant ordering: each variant has a distinct leading keyword (`ADD`,
/// `DROP`, `ALTER`) followed by `ATTRIBUTE`. Order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterTypeCmd<'input> {
    AddAttribute(AlterTypeAddAttribute<'input>),
    DropAttribute(AlterTypeDropAttribute<'input>),
    AlterAttribute(AlterTypeAlterAttribute<'input>),
}

/// One or more comma-separated `alter_type_cmd`s — Postgres'
/// `alter_type_cmds` non-terminal.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTypeCmdList<'input> {
    pub cmds: Seq1<AlterTypeCmd<'input>, punct::Comma>,
}

/// One action on `ALTER TYPE any_name action` — covers Postgres'
/// `RenameStmt` (RENAME TO, RENAME ATTRIBUTE), `AlterOwnerStmt`
/// (OWNER TO), `AlterObjectSchemaStmt` (SET SCHEMA), `AlterTypeStmt`
/// (SET (...)), `AlterEnumStmt` (ADD VALUE, RENAME VALUE), and
/// `alter_type_cmds` (ADD/DROP/ALTER ATTRIBUTE, comma-separated).
///
/// Variant ordering: variants with two-keyword prefixes go before
/// single-keyword variants that share the same first token.
/// `RENAME ATTRIBUTE` / `RENAME VALUE` (two tokens) before `RENAME TO`
/// (also two tokens — distinct second). `SET SCHEMA` / `SET (` (the
/// def-list form starts with `SET LPAREN`) — distinct second tokens.
/// `ADD VALUE` (two tokens) before `Cmds` (which can start with `ADD
/// ATTRIBUTE`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterTypeAction<'input> {
    RenameAttribute(AlterTypeRenameAttribute<'input>),
    RenameValue(AlterTypeRenameValue<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    SetDef(SetDefinitionClause<'input>),
    AddValue(AlterTypeAddValue<'input>),
    Cmds(AlterTypeCmdList<'input>),
}

/// `ALTER TYPE any_name action` — Postgres' `AlterTypeStmt` /
/// `AlterEnumStmt` / `RenameStmt` / `AlterOwnerStmt` /
/// `AlterObjectSchemaStmt` branches for types, plus the composite-type
/// `alter_type_cmds` set (ADD/DROP/ALTER ATTRIBUTE, comma-separated).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterTypeStmt<'input> {
    pub alter: ALTER,
    pub r#type: TYPE,
    pub name: QualifiedName<'input>,
    pub action: AlterTypeAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_type_list() {
        let mut input = crate::tokens::test_input("DROP TYPE IF EXISTS t1, t2");
        let stmt = DropTypeStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.types.types.len(), 2);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_rename_to() {
        let mut input = crate::tokens::test_input("ALTER TYPE bogus RENAME TO bogon");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_set_schema() {
        let mut input = crate::tokens::test_input("ALTER TYPE alter1.ctype SET SCHEMA alter2");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_set_def() {
        let mut input = crate::tokens::test_input("ALTER TYPE myvarchar SET (storage = extended)");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_add_value() {
        let mut input = crate::tokens::test_input("ALTER TYPE planets ADD VALUE 'uranus'");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_add_value_if_not_exists_after() {
        let mut input = crate::tokens::test_input(
            "ALTER TYPE planets ADD VALUE IF NOT EXISTS 'pluto' AFTER 'neptune'",
        );
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_rename_value() {
        let mut input =
            crate::tokens::test_input("ALTER TYPE rainbow RENAME VALUE 'red' TO 'crimson'");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_add_attribute() {
        let mut input = crate::tokens::test_input("ALTER TYPE test_type ADD ATTRIBUTE b text");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_drop_attribute_cascade() {
        let mut input = crate::tokens::test_input("ALTER TYPE test_type2 DROP ATTRIBUTE b CASCADE");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_alter_attribute_set_data_type() {
        let mut input = crate::tokens::test_input(
            "ALTER TYPE test_type ALTER ATTRIBUTE b SET DATA TYPE varchar",
        );
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_multi_cmd() {
        let mut input = crate::tokens::test_input(
            "ALTER TYPE test_type DROP ATTRIBUTE a, ADD ATTRIBUTE d boolean",
        );
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_type_rename_attribute() {
        let mut input = crate::tokens::test_input("ALTER TYPE test_type RENAME ATTRIBUTE a TO aa");
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_type_shell() {
        let mut input = crate::tokens::test_input("CREATE TYPE shell");
        let stmt = CreateTypeStmt::parse(&mut input).unwrap();
        assert!(stmt.body.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_type_composite() {
        let mut input = crate::tokens::test_input("CREATE TYPE row1 AS (f1 text, f2 int)");
        let stmt = CreateTypeStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Composite(_))));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_type_enum() {
        let mut input =
            crate::tokens::test_input("CREATE TYPE color AS ENUM ('red', 'green', 'blue')");
        let stmt = CreateTypeStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Enum(_))));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_type_range() {
        let mut input = crate::tokens::test_input("CREATE TYPE intrange AS RANGE (subtype = int)");
        let stmt = CreateTypeStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Range(_))));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_type_base() {
        let mut input = crate::tokens::test_input(
            "CREATE TYPE widget (internallength = 24, input = widget_in, output = widget_out)",
        );
        let stmt = CreateTypeStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Base(_))));
        assert!(input.is_empty());
    }
}
