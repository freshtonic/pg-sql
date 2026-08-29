//! TYPE  DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

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
#[derive(recursa::Node, Debug, Clone)]
pub struct CompositeTypeColumn<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub type_name: CastType<'input>,
    pub collate: Option<CompositeTypeCollate<'input>>,
}

/// `COLLATE name` clause on a composite-type column.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompositeTypeCollate<'input> {
    #[tok(COLLATE, this)]
    pub name: QualifiedName<'input>,
}

/// `AS (col_list)` — composite-type definition body.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTypeComposite<'input> {
    #[tok(AS, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns:
         Vec<CompositeTypeColumn<'input> > ,
}

/// `AS ENUM ('label', ...)` — enum-type definition body. The label list may
/// be empty (Postgres allows `AS ENUM ()` to create a shell-only enum).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTypeEnum<'input> {
    #[tok(AS, ENUM, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub labels:
         Vec<literal::StringLit<'input> > ,
}

/// `AS RANGE (def_list)` — range-type definition body.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTypeRange<'input> {
    #[tok(AS, RANGE, this)]
    pub definition: DefList<'input>,
}

/// The body of a `CREATE TYPE name ‹body›` statement.
///
/// Variant ordering: multi-keyword forms (`AS ENUM`, `AS RANGE`) before
/// `Composite` (`AS` + paren list) so the longer match wins. `Base` is the
/// `(def_list)` form (no `AS`); it begins with `(` and so cannot collide
/// with the `AS …` variants.
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTypeStmt<'input> {
    #[tok(CREATE, TYPE, this)]
    pub name: QualifiedName<'input>,
    pub body: Option<CreateTypeBody<'input>>,
}

/// `DROP TYPE [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropTypeStmt<'input> {
    #[tok(DROP, TYPE, this)]
    pub if_exists: Option<IfExists>,
    pub types: TypeNameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `RENAME ATTRIBUTE old TO new [CASCADE | RESTRICT]` — Postgres'
/// `RenameStmt` branch for composite-type attribute renames.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeRenameAttribute<'input> {
    #[tok(RENAME, ATTRIBUTE, this)]
    pub old_name: crate::tokens::ColId<'input>,
    #[tok(TO, this)]
    pub new_name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `RENAME VALUE old_value TO new_value` — Postgres' `AlterEnumStmt`
/// branch for renaming enum values. Both literals are string literals.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeRenameValue<'input> {
    #[tok(RENAME, VALUE, this)]
    pub old_value: literal::StringLit<'input>,
    #[tok(TO, this)]
    pub new_value: literal::StringLit<'input>,
}

/// `BEFORE 'value'` or `AFTER 'value'` — neighbor anchor on
/// `ALTER TYPE name ADD VALUE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterEnumValuePosition<'input> {
    Before(AlterEnumValueBefore<'input>),
    After(AlterEnumValueAfter<'input>),
}

/// `BEFORE 'neighbor'` — neighbour anchor on
/// `ALTER TYPE name ADD VALUE ... BEFORE 'neighbor'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterEnumValueBefore<'input> {
    #[tok(BEFORE, this)]
    pub neighbor: literal::StringLit<'input>,
}

/// `AFTER 'neighbor'` — neighbour anchor on
/// `ALTER TYPE name ADD VALUE ... AFTER 'neighbor'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterEnumValueAfter<'input> {
    #[tok(AFTER, this)]
    pub neighbor: literal::StringLit<'input>,
}

/// `ADD VALUE [IF NOT EXISTS] 'val' [{BEFORE|AFTER} 'neighbour']` —
/// Postgres' `AlterEnumStmt` ADD VALUE branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeAddValue<'input> {
    #[tok(ADD, VALUE, this)]
    pub if_not_exists: Option<IfNotExists>,
    pub new_value: literal::StringLit<'input>,
    pub position: Option<AlterEnumValuePosition<'input>>,
}

/// `ADD ATTRIBUTE column_def [CASCADE | RESTRICT]` — one `alter_type_cmd`
/// in Postgres. `column_def` is modelled as the same `CompositeTypeColumn`
/// used by `CREATE TYPE name AS (...)` (Postgres' `TableFuncElement`):
/// `name typename [COLLATE qualified_name]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeAddAttribute<'input> {
    #[tok(ADD, ATTRIBUTE, this)]
    pub column: CompositeTypeColumn<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP ATTRIBUTE [IF EXISTS] name [CASCADE | RESTRICT]` — one
/// `alter_type_cmd` in Postgres.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeDropAttribute<'input> {
    #[tok(DROP, ATTRIBUTE, this)]
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `[SET DATA]` modifier preceding `TYPE` in
/// `ALTER ATTRIBUTE name [SET DATA] TYPE typename`. Postgres'
/// `opt_set_data`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetDataClause { #[tok(SET, DATA)] Value, }

/// `ALTER ATTRIBUTE name [SET DATA] TYPE typename [COLLATE qual] [CASCADE
/// | RESTRICT]` — one `alter_type_cmd` in Postgres. The typename uses the
/// same `CastType` enum as `CREATE TYPE name AS (col_list)` column types.
/// The optional `COLLATE` clause reuses [`CompositeTypeCollate`] (Postgres'
/// `opt_collate_clause` — `COLLATE any_name`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeAlterAttribute<'input> {
    #[tok(ALTER, ATTRIBUTE, this)]
    pub name: crate::tokens::ColId<'input>,
    pub set_data: Option<SetDataClause>,
    #[tok(TYPE, this)]
    pub type_name: CastType<'input>,
    pub collate: Option<CompositeTypeCollate<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// One `alter_type_cmd` in Postgres — an `ADD ATTRIBUTE`, `DROP ATTRIBUTE`,
/// or `ALTER ATTRIBUTE` action on `ALTER TYPE name action [, action ...]`.
///
/// Variant ordering: each variant has a distinct leading keyword (`ADD`,
/// `DROP`, `ALTER`) followed by `ATTRIBUTE`. Order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTypeCmd<'input> {
    AddAttribute(AlterTypeAddAttribute<'input>),
    DropAttribute(AlterTypeDropAttribute<'input>),
    AlterAttribute(AlterTypeAlterAttribute<'input>),
}

/// One or more comma-separated `alter_type_cmd`s — Postgres'
/// `alter_type_cmds` non-terminal.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeCmdList<'input> {
    #[sep(COMMA)]
    pub cmds: recursa::Vec1<AlterTypeCmd<'input> >,
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
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTypeStmt<'input> {
    #[tok(ALTER, TYPE, this)]
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
        let lexed = crate::tokens::lex("DROP TYPE IF EXISTS t1, t2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.types.types.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_rename_to() {
        let lexed = crate::tokens::lex("ALTER TYPE bogus RENAME TO bogon");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_set_schema() {
        let lexed = crate::tokens::lex("ALTER TYPE alter1.ctype SET SCHEMA alter2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_set_def() {
        let lexed = crate::tokens::lex("ALTER TYPE myvarchar SET (storage = extended)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_add_value() {
        let lexed = crate::tokens::lex("ALTER TYPE planets ADD VALUE 'uranus'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_add_value_if_not_exists_after() {
        let lexed = crate::tokens::lex("ALTER TYPE planets ADD VALUE IF NOT EXISTS 'pluto' AFTER 'neptune'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_rename_value() {
        let lexed = crate::tokens::lex("ALTER TYPE rainbow RENAME VALUE 'red' TO 'crimson'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_add_attribute() {
        let lexed = crate::tokens::lex("ALTER TYPE test_type ADD ATTRIBUTE b text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_drop_attribute_cascade() {
        let lexed = crate::tokens::lex("ALTER TYPE test_type2 DROP ATTRIBUTE b CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_alter_attribute_set_data_type() {
        let lexed = crate::tokens::lex("ALTER TYPE test_type ALTER ATTRIBUTE b SET DATA TYPE varchar");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_multi_cmd() {
        let lexed = crate::tokens::lex("ALTER TYPE test_type DROP ATTRIBUTE a, ADD ATTRIBUTE d boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_type_rename_attribute() {
        let lexed = crate::tokens::lex("ALTER TYPE test_type RENAME ATTRIBUTE a TO aa");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_shell() {
        let lexed = crate::tokens::lex("CREATE TYPE shell");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.body.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_composite() {
        let lexed = crate::tokens::lex("CREATE TYPE row1 AS (f1 text, f2 int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Composite(_))));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_enum() {
        let lexed = crate::tokens::lex("CREATE TYPE color AS ENUM ('red', 'green', 'blue')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Enum(_))));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_range() {
        let lexed = crate::tokens::lex("CREATE TYPE intrange AS RANGE (subtype = int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Range(_))));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_type_base() {
        let lexed = crate::tokens::lex("CREATE TYPE widget (internallength = 24, input = widget_in, output = widget_out)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTypeStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, Some(CreateTypeBody::Base(_))));
        assert!(input.is_eof());
    }
}
