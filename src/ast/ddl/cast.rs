//! CAST DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `(source_type AS target_type)` — the type-pair signature shared by
/// `CREATE CAST` and `DROP CAST`. Distinct struct from `CastSignature`
/// further down (used by DROP CAST): the CREATE form uses `Typename`
/// (PG allows array/precision modifiers), so each type field is `CastType`,
/// not the bare `common::TypeName` used by DROP CAST today.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateCastSignature<'input> {
    pub source: CastType<'input>,
    #[tok(AS, this)]
    pub target: CastType<'input>,
}

/// `function_with_argtypes` in `CREATE CAST` — Postgres' `func_name func_args`
/// (the parenthesised form). The cast function is mandatory: bare-name
/// (`args_unspecified`) forms are not exercised by the corpus and are not
/// modelled here.
#[derive(recursa::Node, Debug, Clone)]
pub struct CastFunctionRef<'input> {
    pub name: QualifiedName<'input>,
    pub args: CastFunctionArgs<'input>,
}

/// Parenthesized argument list of a `CREATE CAST` function reference —
/// gram.y's `func_args`, which admits the empty `()` form.
///
/// The parentheses belong to the whole list: a field-level attachment would
/// bind to each element and declare `(int), (text)`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct CastFunctionArgs<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<crate::ast::ddl::function::FuncParam<'input>>,
);

/// `WITH FUNCTION function_with_argtypes` — the function-coercion branch of
/// `CREATE CAST`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CastWithFunction<'input> {
    #[tok(WITH, FUNCTION, this)]
    pub func: CastFunctionRef<'input>,
}

/// The coercion implementation chosen by `CREATE CAST`: a function, no
/// function (binary-compatible), or the type's I/O functions.
///
/// Variant ordering: `WithInout` (`WITH INOUT`) and `WithFunction`
/// (`WITH FUNCTION ...`) — both start with `WITH`. `WithInout` is two
/// keywords + nothing, `WithFunction` is `WITH FUNCTION ...`. PG
/// disambiguates on the second token (`INOUT` vs `FUNCTION`). Variant order
/// here is `WithFunction` then `WithInout` because `WithFunction` has the
/// longer specific match; the actual second-token disambiguation is handled
/// by the combined peek regex.
#[derive(recursa::Node, Debug, Clone)]
pub enum CastImpl<'input> {
    WithFunction(CastWithFunction<'input>),
    #[tok(WITH, INOUT)]
    WithInout,
    #[tok(WITHOUT, FUNCTION)]
    WithoutFunction,
}

/// `AS { IMPLICIT | ASSIGNMENT }` — the trailing `cast_context` keyword on
/// `CREATE CAST`. Absent ⇒ `EXPLICIT` (the default).
#[derive(recursa::Node, Debug, Clone)]
pub enum CastContextKind {
    #[tok(IMPLICIT)]
    Implicit,
    #[tok(ASSIGNMENT)]
    Assignment,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CastContext {
    #[tok(AS, this)]
    pub kind: CastContextKind,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateCastStmt<'input> {
    #[tok(CREATE, CAST, LPAREN, this, RPAREN)]
    pub signature: CreateCastSignature<'input>,
    pub r#impl: CastImpl<'input>,
    pub context: Option<CastContext>,
}

/// The `(source AS target)` type pair inside a `DROP CAST` statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct CastSignature<'input> {
    pub source: crate::ast::shared::names::TypeName<'input>,
    #[tok(AS, this)]
    pub target: crate::ast::shared::names::TypeName<'input>,
}

/// `DROP CAST [IF EXISTS] (source AS target) [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, CAST, this)]
pub struct DropCastStmt<'input> {
    pub if_exists: Option<IfExists>,
    #[tok(LPAREN, this, RPAREN)]
    pub signature: CastSignature<'input>,
    pub behavior: Option<DropBehavior>,
}
