//! OPERATOR DDL statements (CREATE/ALTER/DROP).
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
/// as a [`crate::ast::FileItem::ParseError`] (also rejected by Postgres,
/// so the differential oracle stays valid).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateOperatorStmt<'input> {
    pub create: CREATE,
    pub operator: OPERATOR,
    pub name: QualifiedOperatorName<'input>,
    pub definition: DefList<'input>,
}

/// One target of `DROP OPERATOR` — a qualified operator name plus its
/// `(left, right)` argument signature. Postgres' `operator_with_argtypes`.
pub type DropOperatorTarget<'input> = OperatorWithArgtypes<'input>;

/// `DROP OPERATOR [IF EXISTS] op(args) [, ...] [CASCADE | RESTRICT]` —
/// Postgres' `RemoveOperStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropOperatorStmt<'input> {
    pub drop: DROP,
    pub operator: OPERATOR,
    pub if_exists: Option<IfExists>,
    pub targets: Seq1<DropOperatorTarget<'input>, punct::Comma>,
    pub behavior: Option<DropBehavior>,
}

/// `FOR SEARCH` — the opclass-purpose marker for search support operators.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassPurposeSearch {
    pub r#for: FOR,
    pub search: SEARCH,
}

/// `FOR ORDER BY family_name` — the opclass-purpose marker on ordering
/// operators, naming the operator family that owns the order semantics.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassPurposeOrderBy<'input> {
    pub r#for: FOR,
    pub order: ORDER,
    pub by: BY,
    pub family_name: QualifiedName<'input>,
}

/// `opclass_purpose` — the optional clause on `OPERATOR n any_op` items
/// inside CREATE OPERATOR CLASS / ALTER OPERATOR FAMILY ADD.
///
/// Variant ordering: each variant has a distinct two-token prefix (`FOR
/// SEARCH` vs `FOR ORDER`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassItemOperator<'input> {
    pub operator: OPERATOR,
    pub number: literal::IntegerLit<'input>,
    pub name: crate::ast::shared::names::QualifiedOperatorName<'input>,
    pub argtypes: Option<crate::ast::shared::names::OperatorArgtypes<'input>>,
    pub purpose: Option<OpclassPurpose<'input>>,
    pub recheck: Option<RECHECK>,
}

/// `'(' type_list ')' function_with_argtypes` — the class-args + function
/// pair on `FUNCTION n (type_list) function_with_argtypes`. Used by the
/// rarer four-arg form of FUNCTION opclass items.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassItemFunctionClassArgs<'input> {
    pub class_args:
        Surrounded<punct::LParen, crate::ast::shared::names::TypeNameList<'input>, punct::RParen>,
    pub func: crate::ast::ddl::function::DropFunctionTarget<'input>,
}

/// The body of a `FUNCTION n …` opclass item — either the plain
/// `function_with_argtypes` form or the optional
/// `(type_list) function_with_argtypes` form.
///
/// Variant ordering: `WithClassArgs` (starts with `(`) before `Plain` (starts
/// with an ident from `func_name`). Their first-token sets are disjoint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OpclassItemFunctionBody<'input> {
    WithClassArgs(OpclassItemFunctionClassArgs<'input>),
    Plain(crate::ast::ddl::function::DropFunctionTarget<'input>),
}

/// `FUNCTION Iconst [(type_list)] function_with_argtypes` — the function
/// support-procedure entry in an opclass_item list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassItemFunction<'input> {
    pub function: FUNCTION,
    pub number: literal::IntegerLit<'input>,
    pub body: OpclassItemFunctionBody<'input>,
}

/// `STORAGE Typename` — the storage-type entry in an opclass_item list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassItemStorage<'input> {
    pub storage: STORAGE,
    pub r#type: crate::ast::shared::names::TypeName<'input>,
}

/// One entry in an `opclass_item_list`: an `OPERATOR`, `FUNCTION`, or
/// `STORAGE` clause inside `CREATE OPERATOR CLASS ... AS ...` or
/// `ALTER OPERATOR FAMILY ... ADD ...`.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`OPERATOR` / `FUNCTION` / `STORAGE`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OpclassItem<'input> {
    Operator(OpclassItemOperator<'input>),
    Function(OpclassItemFunction<'input>),
    Storage(OpclassItemStorage<'input>),
}

/// `OPERATOR Iconst '(' type_list ')'` — the operator-drop entry in an
/// ALTER OPERATOR FAMILY ... DROP list. (`opclass_drop` in gram.y.)
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassDropOperator<'input> {
    pub operator: OPERATOR,
    pub number: literal::IntegerLit<'input>,
    pub argtypes:
        Surrounded<punct::LParen, crate::ast::shared::names::TypeNameList<'input>, punct::RParen>,
}

/// `FUNCTION Iconst '(' type_list ')'` — the function-drop entry in an
/// ALTER OPERATOR FAMILY ... DROP list. (`opclass_drop` in gram.y.)
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OpclassDropFunction<'input> {
    pub function: FUNCTION,
    pub number: literal::IntegerLit<'input>,
    pub argtypes:
        Surrounded<punct::LParen, crate::ast::shared::names::TypeNameList<'input>, punct::RParen>,
}

/// One entry in an `opclass_drop_list`: either an `OPERATOR Iconst (types)` or
/// a `FUNCTION Iconst (types)` clause. (`opclass_drop` in gram.y.)
///
/// Variant ordering: each variant has a distinct leading keyword, so order
/// is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OpclassDrop<'input> {
    Operator(OpclassDropOperator<'input>),
    Function(OpclassDropFunction<'input>),
}

/// `FAMILY family_name` — the optional clause naming an enclosing operator
/// family on `CREATE OPERATOR CLASS ... USING method [FAMILY family]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateOpClassFamilyClause<'input> {
    pub family: FAMILY,
    pub name: QualifiedName<'input>,
}

/// `CREATE OPERATOR CLASS any_name [DEFAULT] FOR TYPE Typename USING access_method
/// [FAMILY family_name] AS opclass_item [, ...]` — Postgres'
/// `CreateOpClassStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateOperatorClassStmt<'input> {
    pub create: CREATE,
    pub operator: OPERATOR,
    pub class: CLASS,
    pub name: QualifiedName<'input>,
    pub default: Option<DEFAULT>,
    pub r#for: FOR,
    pub r#type: TYPE,
    pub datatype: crate::ast::shared::names::TypeName<'input>,
    pub using: USING,
    pub access_method: crate::tokens::ColId<'input>,
    pub family: Option<CreateOpClassFamilyClause<'input>>,
    pub r#as: AS,
    pub items: Seq1<OpclassItem<'input>, punct::Comma>,
}

/// `CREATE OPERATOR FAMILY any_name USING access_method` — Postgres'
/// `CreateOpFamilyStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateOperatorFamilyStmt<'input> {
    pub create: CREATE,
    pub operator: OPERATOR,
    pub family: FAMILY,
    pub name: QualifiedName<'input>,
    pub using: USING,
    pub access_method: crate::tokens::ColId<'input>,
}

/// `ADD opclass_item [, ...]` — the add arm of ALTER OPERATOR FAMILY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterOperatorFamilyAdd<'input> {
    pub add: ADD,
    pub items: Seq1<OpclassItem<'input>, punct::Comma>,
}

/// `DROP opclass_drop [, ...]` — the drop arm of ALTER OPERATOR FAMILY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterOperatorFamilyDrop<'input> {
    pub drop: DROP,
    pub items: Seq1<OpclassDrop<'input>, punct::Comma>,
}

/// One action on `ALTER OPERATOR FAMILY name USING method action` — covers
/// Postgres' `AlterOpFamilyStmt` ADD/DROP body plus the operator-family
/// branches of `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt`.
///
/// Variant ordering: each variant has a distinct leading keyword (`ADD`,
/// `DROP`, `RENAME`, `OWNER`, `SET`), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterOperatorFamilyStmt<'input> {
    pub alter: ALTER,
    pub operator: OPERATOR,
    pub family: FAMILY,
    pub name: QualifiedName<'input>,
    pub using: USING,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterOperatorClassAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER OPERATOR CLASS any_name USING access_method action` —
/// Postgres' operator-class branches of `RenameStmt` / `AlterOwnerStmt`
/// / `AlterObjectSchemaStmt`. The ADD/DROP body lives only on the
/// `AlterOpFamilyStmt` (FAMILY) production.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterOperatorClassStmt<'input> {
    pub alter: ALTER,
    pub operator: OPERATOR,
    pub class: CLASS,
    pub name: QualifiedName<'input>,
    pub using: USING,
    pub access_method: crate::tokens::ColId<'input>,
    pub action: AlterOperatorClassAction<'input>,
}

/// `DROP OPERATOR CLASS [IF EXISTS] any_name USING access_method
/// [CASCADE | RESTRICT]` — Postgres' `DropOpClassStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropOperatorClassStmt<'input> {
    pub drop: DROP,
    pub operator: OPERATOR,
    pub class: CLASS,
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub using: USING,
    pub access_method: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP OPERATOR FAMILY [IF EXISTS] any_name USING access_method
/// [CASCADE | RESTRICT]` — Postgres' `DropOpFamilyStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropOperatorFamilyStmt<'input> {
    pub drop: DROP,
    pub operator: OPERATOR,
    pub family: FAMILY,
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub using: USING,
    pub access_method: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET (operator_def_list)` action on `ALTER OPERATOR` — Postgres'
/// `AlterOperatorStmt` proper.
///
/// The def-list inside the parens is the same `def_list` body shared with
/// CREATE OPERATOR / CREATE AGGREGATE / etc. and is captured by [`DefList`].
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterOperatorSetOptions<'input> {
    pub set: SET,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterOperatorAction<'input> {
    SetSchema(SetSchemaClause<'input>),
    SetOptions(AlterOperatorSetOptions<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER OPERATOR op(args) { SET (...) | SET SCHEMA name | OWNER TO role }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterOperatorStmt<'input> {
    pub alter: ALTER,
    pub operator: OPERATOR,
    pub target: OperatorWithArgtypes<'input>,
    pub action: AlterOperatorAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_operator_custom_op() {
        let mut input = crate::tokens::test_input(
            "CREATE OPERATOR @-@ ( leftarg = int4, rightarg = int4, procedure = int4mi )",
        );
        let _stmt = CreateOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_operator_custom_op() {
        let mut input = crate::tokens::test_input("DROP OPERATOR ===(bigint, bigint)");
        let _stmt = DropOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_custom_op() {
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR @+@(int4, int4) OWNER TO regress_alter_generic_user2",
        );
        let _stmt = AlterOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_operator_commutator_custom_op() {
        // From `create_operator.sql`: `COMMUTATOR = ===` on a CREATE OPERATOR
        // body. The RHS of the `=` is itself a custom operator.
        let mut input = crate::tokens::test_input(
            "CREATE OPERATOR === (\
                 LEFTARG = boolean,\
                 RIGHTARG = boolean,\
                 PROCEDURE = fn_op2,\
                 COMMUTATOR = ===,\
                 NEGATOR = !==\
             )",
        );
        let _stmt = CreateOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_set_commutator_custom_op() {
        // From `alter_operator.sql`: `SET (COMMUTATOR = ====)` — the RHS is
        // a 4-char custom op (`====`) which lexes as `CustomOp`.
        let mut input =
            crate::tokens::test_input("ALTER OPERATOR === (boolean, real) SET (COMMUTATOR = ====)");
        let _stmt = AlterOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_set_negator_at_eq() {
        // `SET (COMMUTATOR = @=)` — `@=` is a 2-char custom op starting with `@`.
        let mut input =
            crate::tokens::test_input("ALTER OPERATOR === (boolean, real) SET (COMMUTATOR = @=)");
        let _stmt = AlterOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_operator_single_char() {
        let mut input = crate::tokens::test_input(
            "CREATE OPERATOR = (procedure = int8alias1eq, leftarg = int8alias1, rightarg = int8alias1)",
        );
        let _stmt = CreateOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_operator_single_char() {
        let mut input = crate::tokens::test_input("DROP OPERATOR <|(bigint, bigint)");
        let _stmt = DropOperatorStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_operator_family_basic() {
        let mut input = crate::tokens::test_input("CREATE OPERATOR FAMILY my_family USING hash");
        let stmt = CreateOperatorFamilyStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "my_family");
        assert_eq!(stmt.access_method.text(), "hash");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_operator_class_storage_only() {
        let mut input = crate::tokens::test_input(
            "CREATE OPERATOR CLASS alt_opc1 FOR TYPE uuid USING hash AS STORAGE uuid",
        );
        let stmt = CreateOperatorClassStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "alt_opc1");
        assert!(stmt.default.is_none());
        assert!(stmt.family.is_none());
        assert_eq!(stmt.items.iter().count(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_operator_class_default_with_family() {
        let mut input = crate::tokens::test_input(
            "CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int4 USING btree \
             FAMILY my_fam AS OPERATOR 1 < , OPERATOR 3 = , FUNCTION 1 my_cmp(int4, int4)",
        );
        let stmt = CreateOperatorClassStmt::parse(&mut input).unwrap();
        assert!(stmt.default.is_some());
        assert!(stmt.family.is_some());
        assert_eq!(stmt.items.iter().count(), 3);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_family_add() {
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR FAMILY alt_opf17 USING btree ADD \
             OPERATOR 1 < (int4, int4), FUNCTION 1 btint4cmp(int4, int4)",
        );
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_family_drop() {
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR FAMILY alt_opf11 USING gist DROP OPERATOR 1 (int4, int4)",
        );
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_family_rename() {
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR FAMILY alt_opf1 USING hash RENAME TO alt_opf3",
        );
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_family_owner() {
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR FAMILY alt_opf1 USING hash OWNER TO regress_alter_generic_user1",
        );
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_family_set_schema() {
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR FAMILY alt_opf2 USING hash SET SCHEMA alt_nsp2",
        );
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_operator_family_add_order_by() {
        // `OPERATOR n any_op FOR ORDER BY family_name` — the ordering-operator
        // arm of opclass_purpose.
        let mut input = crate::tokens::test_input(
            "ALTER OPERATOR FAMILY alt_opf10 USING btree ADD \
             OPERATOR 1 < (int4, int4) FOR ORDER BY some_family",
        );
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_operator_class_basic() {
        let mut input = crate::tokens::test_input("DROP OPERATOR CLASS my_ops USING btree CASCADE");
        let stmt = DropOperatorClassStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "my_ops");
        assert_eq!(stmt.access_method.text(), "btree");
        assert!(stmt.if_exists.is_none());
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_operator_family_if_exists() {
        let mut input = crate::tokens::test_input(
            "DROP OPERATOR FAMILY IF EXISTS my_family USING hash RESTRICT",
        );
        let stmt = DropOperatorFamilyStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_operator_class_recheck_modifier() {
        // Legacy `RECHECK` modifier on opclass operator items — PG accepts it
        // for old-dump portability (no-op since 8.4) and we round-trip it.
        let mut input = crate::tokens::test_input(
            "CREATE OPERATOR CLASS legacy_ops FOR TYPE int4 USING gist AS \
             OPERATOR 1 < RECHECK, STORAGE int4",
        );
        let stmt = CreateOperatorClassStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.items.iter().count(), 2);
        assert!(input.is_empty());
    }
}
