//! OPERATOR DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `CREATE OPERATOR any_operator (def_list)` — Postgres' DefineStmt for
/// OBJECT_OPERATOR.
///
/// The `definition` body is shared with CREATE AGGREGATE / CREATE TYPE / etc.
/// and is captured by the generic [`DefList`]. The corpus exercises bare
/// `(name = value, ...)` lists with keys like `LEFTARG`, `RIGHTARG`,
/// `PROCEDURE`, `FUNCTION`, `COMMUTATOR`, `NEGATOR`, `RESTRICT`, `JOIN`,
/// `HASHES`, `MERGES`, `SORT1`, `SORT2`, `LTCMP`, `GTCMP` — `DefElem`
/// already accepts the bare-name (no `= value`) form for `HASHES` /
/// `MERGES` / etc.
///
/// Postgres does NOT accept `CREATE OR REPLACE OPERATOR` (the `DefineStmt`
/// production for OPERATOR has no `opt_or_replace`), nor `CREATE TEMP
/// OPERATOR`. The earlier raw-tailed stub tolerated both for uniformity;
/// the modelled form rejects them, and any input that uses them surfaces
/// as a file-level parse error (also rejected by Postgres,
/// so the differential oracle stays valid).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateOperatorStmt<'input> {
    #[tok(CREATE, OPERATOR, this)]
    pub name: QualifiedOperatorName<'input>,
    pub definition: DefList<'input>,
}

/// One target of `DROP OPERATOR` — a qualified operator name plus its
/// `(left, right)` argument signature. Postgres' `operator_with_argtypes`.
pub type DropOperatorTarget<'input> = OperatorWithArgtypes<'input>;

/// `DROP OPERATOR [IF EXISTS] op(args) [, ...] [CASCADE | RESTRICT]` —
/// Postgres' `RemoveOperStmt`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, OPERATOR, this)]
pub struct DropOperatorStmt<'input> {
    pub if_exists: Option<IfExists>,
    #[sep(COMMA)]
    pub targets: recursa::Vec1<DropOperatorTarget<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// `FOR SEARCH` — the opclass-purpose marker for search support operators.
#[derive(recursa::Node, Debug, Clone)]
pub enum OpclassPurposeSearch {
    #[tok(FOR, SEARCH)]
    Value,
}

/// `FOR ORDER BY family_name` — the opclass-purpose marker on ordering
/// operators, naming the operator family that owns the order semantics.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassPurposeOrderBy<'input> {
    #[tok(FOR, ORDER, BY, this)]
    pub family_name: QualifiedName<'input>,
}

/// `opclass_purpose` — the optional clause on `OPERATOR n any_op` items
/// inside CREATE OPERATOR CLASS / ALTER OPERATOR FAMILY ADD.
///
/// Variant ordering: each variant has a distinct two-token prefix (`FOR
/// SEARCH` vs `FOR ORDER`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum OpclassPurpose<'input> {
    OrderBy(OpclassPurposeOrderBy<'input>),
    Search(OpclassPurposeSearch),
}

/// `OPERATOR Iconst any_operator [oper_argtypes] [opclass_purpose] [RECHECK]` —
/// the operator-strategy entry in an opclass_item list.
///
/// The argument-types signature is optional: gram.y's first alternative
/// (`OPERATOR Iconst any_operator opclass_purpose opt_recheck`) accepts the
/// no-argtypes spelling, while the second
/// (`OPERATOR Iconst operator_with_argtypes opclass_purpose opt_recheck`)
/// requires it. `RECHECK` is the legacy no-op modifier — still accepted by
/// PG for old-dump portability and round-tripped here.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassItemOperator<'input> {
    #[tok(OPERATOR, this)]
    pub number: literal::IntegerLit<'input>,
    pub name: crate::ast::shared::names::QualifiedOperatorName<'input>,
    pub argtypes: Option<crate::ast::shared::names::OperatorArgtypes<'input>>,
    pub purpose: Option<OpclassPurpose<'input>>,
    #[presence(RECHECK)]
    pub recheck: bool,
}

/// `'(' type_list ')' function_with_argtypes` — the class-args + function
/// pair on `FUNCTION n (type_list) function_with_argtypes`. Used by the
/// rarer four-arg form of FUNCTION opclass items.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassItemFunctionClassArgs<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub class_args: crate::ast::shared::names::TypeNameList<'input>,
    pub func: crate::ast::ddl::function::DropFunctionTarget<'input>,
}

/// The body of a `FUNCTION n …` opclass item — either the plain
/// `function_with_argtypes` form or the optional
/// `(type_list) function_with_argtypes` form.
///
/// Variant ordering: `WithClassArgs` (starts with `(`) before `Plain` (starts
/// with an ident from `func_name`). Their first-token sets are disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum OpclassItemFunctionBody<'input> {
    WithClassArgs(OpclassItemFunctionClassArgs<'input>),
    Plain(crate::ast::ddl::function::DropFunctionTarget<'input>),
}

/// `FUNCTION Iconst [(type_list)] function_with_argtypes` — the function
/// support-procedure entry in an opclass_item list.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassItemFunction<'input> {
    #[tok(FUNCTION, this)]
    pub number: literal::IntegerLit<'input>,
    pub body: OpclassItemFunctionBody<'input>,
}

/// `STORAGE Typename` — the storage-type entry in an opclass_item list.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassItemStorage<'input> {
    #[tok(STORAGE, this)]
    pub r#type: crate::ast::shared::names::TypeName<'input>,
}

/// One entry in an `opclass_item_list`: an `OPERATOR`, `FUNCTION`, or
/// `STORAGE` clause inside `CREATE OPERATOR CLASS ... AS ...` or
/// `ALTER OPERATOR FAMILY ... ADD ...`.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`OPERATOR` / `FUNCTION` / `STORAGE`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum OpclassItem<'input> {
    Operator(OpclassItemOperator<'input>),
    Function(OpclassItemFunction<'input>),
    Storage(OpclassItemStorage<'input>),
}

/// `AS opclass_item [, ...]` body of `CREATE OPERATOR CLASS`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(AS, this)]
pub struct CreateOpclassItemList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<OpclassItem<'input>>,
);

/// `OPERATOR Iconst '(' type_list ')'` — the operator-drop entry in an
/// ALTER OPERATOR FAMILY ... DROP list. (`opclass_drop` in gram.y.)
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassDropOperator<'input> {
    #[tok(OPERATOR, this)]
    pub number: literal::IntegerLit<'input>,
    #[tok(LPAREN, this, RPAREN)]
    pub argtypes: crate::ast::shared::names::TypeNameList<'input>,
}

/// `FUNCTION Iconst '(' type_list ')'` — the function-drop entry in an
/// ALTER OPERATOR FAMILY ... DROP list. (`opclass_drop` in gram.y.)
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassDropFunction<'input> {
    #[tok(FUNCTION, this)]
    pub number: literal::IntegerLit<'input>,
    #[tok(LPAREN, this, RPAREN)]
    pub argtypes: crate::ast::shared::names::TypeNameList<'input>,
}

/// One entry in an `opclass_drop_list`: either an `OPERATOR Iconst (types)` or
/// a `FUNCTION Iconst (types)` clause. (`opclass_drop` in gram.y.)
///
/// Variant ordering: each variant has a distinct leading keyword, so order
/// is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum OpclassDrop<'input> {
    Operator(OpclassDropOperator<'input>),
    Function(OpclassDropFunction<'input>),
}

/// `FAMILY family_name` — the optional clause naming an enclosing operator
/// family on `CREATE OPERATOR CLASS ... USING method [FAMILY family]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateOpClassFamilyClause<'input> {
    #[tok(FAMILY, this)]
    pub name: QualifiedName<'input>,
}

/// `CREATE OPERATOR CLASS any_name [DEFAULT] FOR TYPE Typename USING access_method
/// [FAMILY family_name] AS opclass_item [, ...]` — Postgres'
/// `CreateOpClassStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateOperatorClassStmt<'input> {
    #[tok(CREATE, OPERATOR, CLASS, this)]
    pub name: QualifiedName<'input>,
    #[tok(this, FOR, TYPE)]
    #[presence(DEFAULT)]
    pub default: bool,
    pub datatype: crate::ast::shared::names::TypeName<'input>,
    #[tok(USING, this)]
    pub access_method: crate::tokens::ColId<'input>,
    pub family: Option<CreateOpClassFamilyClause<'input>>,
    pub items: CreateOpclassItemList<'input>,
}

/// `CREATE OPERATOR FAMILY any_name USING access_method` — Postgres'
/// `CreateOpFamilyStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateOperatorFamilyStmt<'input> {
    #[tok(CREATE, OPERATOR, FAMILY, this)]
    pub name: QualifiedName<'input>,
    #[tok(USING, this)]
    pub access_method: crate::tokens::ColId<'input>,
}

/// `ADD opclass_item [, ...]` — the add arm of ALTER OPERATOR FAMILY.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ADD, this)]
pub struct AlterOperatorFamilyAdd<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<OpclassItem<'input>>,
}

/// `DROP opclass_drop [, ...]` — the drop arm of ALTER OPERATOR FAMILY.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, this)]
pub struct AlterOperatorFamilyDrop<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<OpclassDrop<'input>>,
}

/// One action on `ALTER OPERATOR FAMILY name USING method action` — covers
/// Postgres' `AlterOpFamilyStmt` ADD/DROP body plus the operator-family
/// branches of `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt`.
///
/// Variant ordering: each variant has a distinct leading keyword (`ADD`,
/// `DROP`, `RENAME`, `OWNER`, `SET`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterOperatorFamilyAction<'input> {
    Add(AlterOperatorFamilyAdd<'input>),
    Drop(AlterOperatorFamilyDrop<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER OPERATOR FAMILY any_name USING access_method action` — Postgres'
/// `AlterOpFamilyStmt` plus the operator-family branches of `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterOperatorFamilyStmt<'input> {
    #[tok(ALTER, OPERATOR, FAMILY, this)]
    pub name: QualifiedName<'input>,
    #[tok(USING, this)]
    pub access_method: crate::tokens::ColId<'input>,
    pub action: AlterOperatorFamilyAction<'input>,
}

/// One action on `ALTER OPERATOR CLASS name USING method action` —
/// covers the operator-class branches of `RenameStmt`, `AlterOwnerStmt`,
/// and `AlterObjectSchemaStmt`. Unlike `AlterOperatorFamilyAction`,
/// there is no ADD/DROP body: gram.y's `AlterOpFamilyStmt` is
/// FAMILY-only; CLASS only carries RENAME / OWNER / SET SCHEMA arms.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterOperatorClassAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER OPERATOR CLASS any_name USING access_method action` —
/// Postgres' operator-class branches of `RenameStmt` / `AlterOwnerStmt`
/// / `AlterObjectSchemaStmt`. The ADD/DROP body lives only on the
/// `AlterOpFamilyStmt` (FAMILY) production.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterOperatorClassStmt<'input> {
    #[tok(ALTER, OPERATOR, CLASS, this)]
    pub name: QualifiedName<'input>,
    #[tok(USING, this)]
    pub access_method: crate::tokens::ColId<'input>,
    pub action: AlterOperatorClassAction<'input>,
}

/// `DROP OPERATOR CLASS [IF EXISTS] any_name USING access_method
/// [CASCADE | RESTRICT]` — Postgres' `DropOpClassStmt`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, OPERATOR, CLASS, this)]
pub struct DropOperatorClassStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    #[tok(USING, this)]
    pub access_method: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP OPERATOR FAMILY [IF EXISTS] any_name USING access_method
/// [CASCADE | RESTRICT]` — Postgres' `DropOpFamilyStmt`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, OPERATOR, FAMILY, this)]
pub struct DropOperatorFamilyStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    #[tok(USING, this)]
    pub access_method: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET (operator_def_list)` action on `ALTER OPERATOR` — Postgres'
/// `AlterOperatorStmt` proper.
///
/// The def-list inside the parens is the same `def_list` body shared with
/// CREATE OPERATOR / CREATE AGGREGATE / etc. and is captured by [`DefList`].
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterOperatorSetOptions<'input> {
    #[tok(SET, this)]
    pub options: DefList<'input>,
}

/// One action on `ALTER OPERATOR operator_with_argtypes action` — Postgres'
/// routes these through three productions:
///
/// * `AlterOperatorStmt`: `SET (operator_def_list)`
/// * `AlterObjectSchemaStmt`: `SET SCHEMA name`
/// * `AlterOwnerStmt`: `OWNER TO RoleSpec`
///
/// Variant ordering: `SetSchema` (multi-token `SET SCHEMA`) must precede
/// `SetOptions` (single-token `SET` then a `(`), since both begin with
/// `SET` and longest-match-wins picks the more specific path first.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterOperatorAction<'input> {
    SetSchema(SetSchemaClause<'input>),
    SetOptions(AlterOperatorSetOptions<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER OPERATOR op(args) { SET (...) | SET SCHEMA name | OWNER TO role }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterOperatorStmt<'input> {
    #[tok(ALTER, OPERATOR, this)]
    pub target: OperatorWithArgtypes<'input>,
    pub action: AlterOperatorAction<'input>,
}
