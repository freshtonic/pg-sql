//! CAST DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `(source_type AS target_type)` — the type-pair signature shared by
    /// `CREATE CAST` and `DROP CAST`. Distinct struct from `CastSignature`
    /// further down (used by DROP CAST): the CREATE form uses `Typename`
    /// (PG allows array/precision modifiers), so each type field is `CastType`,
    /// not the bare `common::TypeName` used by DROP CAST today.
    #[derive(Debug)]
    pub struct CreateCastSignature {
        pub source: CastType,
        #[tok(AS, this)]
        pub target: CastType,
    }
}

recursa::ast_node! {
    /// `function_with_argtypes` in `CREATE CAST` — Postgres' `func_name func_args`
    /// (the parenthesised form). The cast function is mandatory: bare-name
    /// (`args_unspecified`) forms are not exercised by the corpus and are not
    /// modelled here.
    #[derive(Debug)]
    pub struct CastFunctionRef {
        pub name: QualifiedName,
        pub args: CastFunctionArgs,
    }
}

recursa::ast_node! {
    /// Parenthesized argument list of a `CREATE CAST` function reference —
    /// gram.y's `func_args`, which admits the empty `()` form.
    ///
    /// The parentheses belong to the whole list: a field-level attachment would
    /// bind to each element and declare `(int), (text)`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CastFunctionArgs(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(crate::ast::ddl::function::FuncParam),
    );
}

recursa::ast_node! {
    /// `WITH FUNCTION function_with_argtypes` — the function-coercion branch of
    /// `CREATE CAST`.
    #[derive(Debug)]
    pub struct CastWithFunction {
        #[tok(WITH, FUNCTION, this)]
        pub func: CastFunctionRef,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum CastImpl {
        WithFunction(CastWithFunction),
        #[tok(WITH, INOUT)]
        WithInout,
        #[tok(WITHOUT, FUNCTION)]
        WithoutFunction,
    }
}

recursa::ast_node! {
    /// `AS { IMPLICIT | ASSIGNMENT }` — the trailing `cast_context` keyword on
    /// `CREATE CAST`. Absent ⇒ `EXPLICIT` (the default).
    #[derive(Debug)]
    pub enum CastContextKind {
        #[tok(IMPLICIT)]
        Implicit,
        #[tok(ASSIGNMENT)]
        Assignment,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CastContext {
        #[tok(AS, this)]
        pub kind: CastContextKind,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CreateCastStmt {
        #[tok(CREATE, CAST, LPAREN, this, RPAREN)]
        pub signature: CreateCastSignature,
        pub r#impl: CastImpl,
        pub context: Option<CastContext>,
    }
}

recursa::ast_node! {
    /// The `(source AS target)` type pair inside a `DROP CAST` statement.
    #[derive(Debug)]
    pub struct CastSignature {
        pub source: crate::ast::shared::names::TypeName,
        #[tok(AS, this)]
        pub target: crate::ast::shared::names::TypeName,
    }
}

recursa::ast_node! {
    /// `DROP CAST [IF EXISTS] (source AS target) [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, CAST, this)]
    pub struct DropCastStmt {
        pub if_exists: Option<IfExists>,
        #[tok(LPAREN, this, RPAREN)]
        pub signature: CastSignature,
        pub behavior: Option<DropBehavior>,
    }
}
