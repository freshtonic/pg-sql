//! OPERATOR DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
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
    #[derive(Debug)]
    pub struct CreateOperatorStmt {
        #[tok(CREATE, OPERATOR, this)]
        pub name: QualifiedOperatorName,
        pub definition: DefList,
    }
}

/// One target of `DROP OPERATOR` — a qualified operator name plus its
/// `(left, right)` argument signature. Postgres' `operator_with_argtypes`.
pub type DropOperatorTarget<'input> = OperatorWithArgtypes<'input>;

recursa::ast_node! {
    /// `DROP OPERATOR [IF EXISTS] op(args) [, ...] [CASCADE | RESTRICT]` —
    /// Postgres' `RemoveOperStmt`.
    #[derive(Debug)]
    #[tok(DROP, OPERATOR, this)]
    pub struct DropOperatorStmt {
        pub if_exists: Option<IfExists>,
        #[sep(COMMA)]
        pub targets: one_or_many!(DropOperatorTarget),
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `FOR SEARCH` — the opclass-purpose marker for search support operators.
    #[derive(Debug)]
    pub enum OpclassPurposeSearch {
        #[tok(FOR, SEARCH)]
        Value,
    }
}

recursa::ast_node! {
    /// `FOR ORDER BY family_name` — the opclass-purpose marker on ordering
    /// operators, naming the operator family that owns the order semantics.
    #[derive(Debug)]
    pub struct OpclassPurposeOrderBy {
        #[tok(FOR, ORDER, BY, this)]
        pub family_name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `opclass_purpose` — the optional clause on `OPERATOR n any_op` items
    /// inside CREATE OPERATOR CLASS / ALTER OPERATOR FAMILY ADD.
    ///
    /// Variant ordering: each variant has a distinct two-token prefix (`FOR
    /// SEARCH` vs `FOR ORDER`), so order is for clarity.
    #[derive(Debug)]
    pub enum OpclassPurpose {
        OrderBy(OpclassPurposeOrderBy),
        Search(OpclassPurposeSearch),
    }
}

recursa::ast_node! {
    /// `OPERATOR Iconst any_operator [oper_argtypes] [opclass_purpose] [RECHECK]` —
    /// the operator-strategy entry in an opclass_item list.
    ///
    /// The argument-types signature is optional: gram.y's first alternative
    /// (`OPERATOR Iconst any_operator opclass_purpose opt_recheck`) accepts the
    /// no-argtypes spelling, while the second
    /// (`OPERATOR Iconst operator_with_argtypes opclass_purpose opt_recheck`)
    /// requires it. `RECHECK` is the legacy no-op modifier — still accepted by
    /// PG for old-dump portability and round-tripped here.
    #[derive(Debug)]
    pub struct OpclassItemOperator {
        #[tok(OPERATOR, this)]
        pub number: literal::IntegerLit,
        pub name: crate::ast::shared::names::QualifiedOperatorName,
        pub argtypes: Option<crate::ast::shared::names::OperatorArgtypes>,
        pub purpose: Option<OpclassPurpose>,
        #[presence(RECHECK)]
        pub recheck: bool,
    }
}

recursa::ast_node! {
    /// `'(' type_list ')' function_with_argtypes` — the class-args + function
    /// pair on `FUNCTION n (type_list) function_with_argtypes`. Used by the
    /// rarer four-arg form of FUNCTION opclass items.
    #[derive(Debug)]
    pub struct OpclassItemFunctionClassArgs {
        #[tok(LPAREN, this, RPAREN)]
        pub class_args: crate::ast::shared::names::TypeNameList,
        pub func: crate::ast::ddl::function::DropFunctionTarget,
    }
}

recursa::ast_node! {
    /// The body of a `FUNCTION n …` opclass item — either the plain
    /// `function_with_argtypes` form or the optional
    /// `(type_list) function_with_argtypes` form.
    ///
    /// Variant ordering: `WithClassArgs` (starts with `(`) before `Plain` (starts
    /// with an ident from `func_name`). Their first-token sets are disjoint.
    #[derive(Debug)]
    pub enum OpclassItemFunctionBody {
        WithClassArgs(OpclassItemFunctionClassArgs),
        Plain(crate::ast::ddl::function::DropFunctionTarget),
    }
}

recursa::ast_node! {
    /// `FUNCTION Iconst [(type_list)] function_with_argtypes` — the function
    /// support-procedure entry in an opclass_item list.
    #[derive(Debug)]
    pub struct OpclassItemFunction {
        #[tok(FUNCTION, this)]
        pub number: literal::IntegerLit,
        pub body: OpclassItemFunctionBody,
    }
}

recursa::ast_node! {
    /// `STORAGE Typename` — the storage-type entry in an opclass_item list.
    #[derive(Debug)]
    pub struct OpclassItemStorage {
        #[tok(STORAGE, this)]
        pub r#type: crate::ast::shared::names::TypeName,
    }
}

recursa::ast_node! {
    /// One entry in an `opclass_item_list`: an `OPERATOR`, `FUNCTION`, or
    /// `STORAGE` clause inside `CREATE OPERATOR CLASS ... AS ...` or
    /// `ALTER OPERATOR FAMILY ... ADD ...`.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`OPERATOR` / `FUNCTION` / `STORAGE`), so order is for clarity.
    #[derive(Debug)]
    pub enum OpclassItem {
        Operator(OpclassItemOperator),
        Function(OpclassItemFunction),
        Storage(OpclassItemStorage),
    }
}

recursa::ast_node! {
    /// `AS opclass_item [, ...]` body of `CREATE OPERATOR CLASS`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(AS, this)]
    pub struct CreateOpclassItemList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(OpclassItem),
    );
}

recursa::ast_node! {
    /// `OPERATOR Iconst '(' type_list ')'` — the operator-drop entry in an
    /// ALTER OPERATOR FAMILY ... DROP list. (`opclass_drop` in gram.y.)
    #[derive(Debug)]
    pub struct OpclassDropOperator {
        #[tok(OPERATOR, this)]
        pub number: literal::IntegerLit,
        #[tok(LPAREN, this, RPAREN)]
        pub argtypes: crate::ast::shared::names::TypeNameList,
    }
}

recursa::ast_node! {
    /// `FUNCTION Iconst '(' type_list ')'` — the function-drop entry in an
    /// ALTER OPERATOR FAMILY ... DROP list. (`opclass_drop` in gram.y.)
    #[derive(Debug)]
    pub struct OpclassDropFunction {
        #[tok(FUNCTION, this)]
        pub number: literal::IntegerLit,
        #[tok(LPAREN, this, RPAREN)]
        pub argtypes: crate::ast::shared::names::TypeNameList,
    }
}

recursa::ast_node! {
    /// One entry in an `opclass_drop_list`: either an `OPERATOR Iconst (types)` or
    /// a `FUNCTION Iconst (types)` clause. (`opclass_drop` in gram.y.)
    ///
    /// Variant ordering: each variant has a distinct leading keyword, so order
    /// is for clarity.
    #[derive(Debug)]
    pub enum OpclassDrop {
        Operator(OpclassDropOperator),
        Function(OpclassDropFunction),
    }
}

recursa::ast_node! {
    /// `FAMILY family_name` — the optional clause naming an enclosing operator
    /// family on `CREATE OPERATOR CLASS ... USING method [FAMILY family]`.
    #[derive(Debug)]
    pub struct CreateOpClassFamilyClause {
        #[tok(FAMILY, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `CREATE OPERATOR CLASS any_name [DEFAULT] FOR TYPE Typename USING access_method
    /// [FAMILY family_name] AS opclass_item [, ...]` — Postgres'
    /// `CreateOpClassStmt`.
    #[derive(Debug)]
    pub struct CreateOperatorClassStmt {
        #[tok(CREATE, OPERATOR, CLASS, this)]
        pub name: QualifiedName,
        #[tok(this, FOR, TYPE)]
        #[presence(DEFAULT)]
        pub default: bool,
        pub datatype: crate::ast::shared::names::TypeName,
        #[tok(USING, this)]
        pub access_method: crate::tokens::ColId,
        pub family: Option<CreateOpClassFamilyClause>,
        pub items: CreateOpclassItemList,
    }
}

recursa::ast_node! {
    /// `CREATE OPERATOR FAMILY any_name USING access_method` — Postgres'
    /// `CreateOpFamilyStmt`.
    #[derive(Debug)]
    pub struct CreateOperatorFamilyStmt {
        #[tok(CREATE, OPERATOR, FAMILY, this)]
        pub name: QualifiedName,
        #[tok(USING, this)]
        pub access_method: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `ADD opclass_item [, ...]` — the add arm of ALTER OPERATOR FAMILY.
    #[derive(Debug)]
    #[tok(ADD, this)]
    pub struct AlterOperatorFamilyAdd {
        #[sep(COMMA)]
        pub items: one_or_many!(OpclassItem),
    }
}

recursa::ast_node! {
    /// `DROP opclass_drop [, ...]` — the drop arm of ALTER OPERATOR FAMILY.
    #[derive(Debug)]
    #[tok(DROP, this)]
    pub struct AlterOperatorFamilyDrop {
        #[sep(COMMA)]
        pub items: one_or_many!(OpclassDrop),
    }
}

recursa::ast_node! {
    /// One action on `ALTER OPERATOR FAMILY name USING method action` — covers
    /// Postgres' `AlterOpFamilyStmt` ADD/DROP body plus the operator-family
    /// branches of `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt`.
    ///
    /// Variant ordering: each variant has a distinct leading keyword (`ADD`,
    /// `DROP`, `RENAME`, `OWNER`, `SET`), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterOperatorFamilyAction {
        Add(AlterOperatorFamilyAdd),
        Drop(AlterOperatorFamilyDrop),
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `ALTER OPERATOR FAMILY any_name USING access_method action` — Postgres'
    /// `AlterOpFamilyStmt` plus the operator-family branches of `RenameStmt` /
    /// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
    #[derive(Debug)]
    pub struct AlterOperatorFamilyStmt {
        #[tok(ALTER, OPERATOR, FAMILY, this)]
        pub name: QualifiedName,
        #[tok(USING, this)]
        pub access_method: crate::tokens::ColId,
        pub action: AlterOperatorFamilyAction,
    }
}

recursa::ast_node! {
    /// One action on `ALTER OPERATOR CLASS name USING method action` —
    /// covers the operator-class branches of `RenameStmt`, `AlterOwnerStmt`,
    /// and `AlterObjectSchemaStmt`. Unlike `AlterOperatorFamilyAction`,
    /// there is no ADD/DROP body: gram.y's `AlterOpFamilyStmt` is
    /// FAMILY-only; CLASS only carries RENAME / OWNER / SET SCHEMA arms.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`RENAME`, `OWNER`, `SET`), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterOperatorClassAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `ALTER OPERATOR CLASS any_name USING access_method action` —
    /// Postgres' operator-class branches of `RenameStmt` / `AlterOwnerStmt`
    /// / `AlterObjectSchemaStmt`. The ADD/DROP body lives only on the
    /// `AlterOpFamilyStmt` (FAMILY) production.
    #[derive(Debug)]
    pub struct AlterOperatorClassStmt {
        #[tok(ALTER, OPERATOR, CLASS, this)]
        pub name: QualifiedName,
        #[tok(USING, this)]
        pub access_method: crate::tokens::ColId,
        pub action: AlterOperatorClassAction,
    }
}

recursa::ast_node! {
    /// `DROP OPERATOR CLASS [IF EXISTS] any_name USING access_method
    /// [CASCADE | RESTRICT]` — Postgres' `DropOpClassStmt`.
    #[derive(Debug)]
    #[tok(DROP, OPERATOR, CLASS, this)]
    pub struct DropOperatorClassStmt {
        pub if_exists: Option<IfExists>,
        pub name: QualifiedName,
        #[tok(USING, this)]
        pub access_method: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `DROP OPERATOR FAMILY [IF EXISTS] any_name USING access_method
    /// [CASCADE | RESTRICT]` — Postgres' `DropOpFamilyStmt`.
    #[derive(Debug)]
    #[tok(DROP, OPERATOR, FAMILY, this)]
    pub struct DropOperatorFamilyStmt {
        pub if_exists: Option<IfExists>,
        pub name: QualifiedName,
        #[tok(USING, this)]
        pub access_method: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `SET (operator_def_list)` action on `ALTER OPERATOR` — Postgres'
    /// `AlterOperatorStmt` proper.
    ///
    /// The def-list inside the parens is the same `def_list` body shared with
    /// CREATE OPERATOR / CREATE AGGREGATE / etc. and is captured by [`DefList`].
    #[derive(Debug)]
    pub struct AlterOperatorSetOptions {
        #[tok(SET, this)]
        pub options: DefList,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterOperatorAction {
        SetSchema(SetSchemaClause),
        SetOptions(AlterOperatorSetOptions),
        Owner(OwnerTo),
    }
}

recursa::ast_node! {
    /// `ALTER OPERATOR op(args) { SET (...) | SET SCHEMA name | OWNER TO role }`.
    #[derive(Debug)]
    pub struct AlterOperatorStmt {
        #[tok(ALTER, OPERATOR, this)]
        pub target: OperatorWithArgtypes,
        pub action: AlterOperatorAction,
    }
}
