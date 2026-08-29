//! CAST DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

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
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:

        Vec<crate::ast::ddl::function::FuncParam<'input> >

    ,
}

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
    #[tok(WITH, INOUT)] WithInout,
    #[tok(WITHOUT, FUNCTION)] WithoutFunction,
}

/// `AS { IMPLICIT | ASSIGNMENT }` — the trailing `cast_context` keyword on
/// `CREATE CAST`. Absent ⇒ `EXPLICIT` (the default).
#[derive(recursa::Node, Debug, Clone)]
pub enum CastContextKind {
    #[tok(IMPLICIT)] Implicit,
    #[tok(ASSIGNMENT)] Assignment,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CastContext {
    #[tok(AS, this)]
    pub kind: CastContextKind,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateCastStmt<'input> {
    #[tok(CREATE, CAST, LPAREN, this, RPAREN)]
    pub signature:  CreateCastSignature<'input> ,
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
pub struct DropCastStmt<'input> {
    #[tok(DROP, CAST, this)]
    pub if_exists: Option<IfExists>,
    #[tok(LPAREN, this, RPAREN)]
    pub signature:  CastSignature<'input> ,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_cast() {
        let lexed = crate::tokens::lex("DROP CAST IF EXISTS (text AS text) RESTRICT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_cast_without_function() {
        let lexed = crate::tokens::lex("CREATE CAST (text AS casttesttype) WITHOUT FUNCTION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.r#impl, CastImpl::WithoutFunction(_)));
        assert!(stmt.context.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_cast_with_inout_implicit() {
        let lexed = crate::tokens::lex("CREATE CAST (int4 AS casttesttype) WITH INOUT AS IMPLICIT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.r#impl, CastImpl::WithInout(_)));
        assert!(matches!(
            stmt.context.as_ref().unwrap().kind,
            CastContextKind::Implicit(_)
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_cast_with_function_assignment() {
        let lexed = crate::tokens::lex("CREATE CAST (int4 AS casttesttype) WITH FUNCTION int4_casttesttype(int4) AS ASSIGNMENT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCastStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.r#impl, CastImpl::WithFunction(_)));
        assert!(matches!(
            stmt.context.as_ref().unwrap().kind,
            CastContextKind::Assignment(_)
        ));
        assert!(input.is_eof());
    }
}
