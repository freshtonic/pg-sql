//! CAST DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateCastSignature<'input> {
    pub source: CastType<'input>,
    pub r#as: AS,
    pub target: CastType<'input>,
}

/// `function_with_argtypes` in `CREATE CAST` — Postgres' `func_name func_args`
/// (the parenthesised form). The cast function is mandatory: bare-name
/// (`args_unspecified`) forms are not exercised by the corpus and are not
/// modelled here.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastFunctionRef<'input> {
    pub name: QualifiedName<'input>,
    pub args: Surrounded<
        punct::LParen,
        Seq0<crate::ast::ddl::function::FuncParam<'input>, punct::Comma>,
        punct::RParen,
    >,
}

/// `WITH FUNCTION function_with_argtypes` — the function-coercion branch of
/// `CREATE CAST`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastWithFunction<'input> {
    pub with: WITH,
    pub function: FUNCTION,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CastImpl<'input> {
    WithFunction(CastWithFunction<'input>),
    WithInout((WITH, INOUT)),
    WithoutFunction((WITHOUT, FUNCTION)),
}

/// `AS { IMPLICIT | ASSIGNMENT }` — the trailing `cast_context` keyword on
/// `CREATE CAST`. Absent ⇒ `EXPLICIT` (the default).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CastContextKind {
    Implicit(IMPLICIT),
    Assignment(ASSIGNMENT),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastContext {
    pub r#as: AS,
    pub kind: CastContextKind,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateCastStmt<'input> {
    pub create: CREATE,
    pub cast: CAST,
    pub signature: Surrounded<punct::LParen, CreateCastSignature<'input>, punct::RParen>,
    pub r#impl: CastImpl<'input>,
    pub context: Option<CastContext>,
}

/// The `(source AS target)` type pair inside a `DROP CAST` statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastSignature<'input> {
    pub source: crate::ast::shared::names::TypeName<'input>,
    pub r#as: AS,
    pub target: crate::ast::shared::names::TypeName<'input>,
}

/// `DROP CAST [IF EXISTS] (source AS target) [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropCastStmt<'input> {
    pub drop: DROP,
    pub cast: CAST,
    pub if_exists: Option<IfExists>,
    pub signature: Surrounded<punct::LParen, CastSignature<'input>, punct::RParen>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_cast() {
        let mut input = crate::tokens::test_input("DROP CAST IF EXISTS (text AS text) RESTRICT");
        let stmt = DropCastStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_cast_without_function() {
        let mut input =
            crate::tokens::test_input("CREATE CAST (text AS casttesttype) WITHOUT FUNCTION");
        let stmt = CreateCastStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.r#impl, CastImpl::WithoutFunction(_)));
        assert!(stmt.context.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_cast_with_inout_implicit() {
        let mut input =
            crate::tokens::test_input("CREATE CAST (int4 AS casttesttype) WITH INOUT AS IMPLICIT");
        let stmt = CreateCastStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.r#impl, CastImpl::WithInout(_)));
        assert!(matches!(
            stmt.context.as_ref().unwrap().kind,
            CastContextKind::Implicit(_)
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_cast_with_function_assignment() {
        let mut input = crate::tokens::test_input(
            "CREATE CAST (int4 AS casttesttype) WITH FUNCTION int4_casttesttype(int4) AS ASSIGNMENT",
        );
        let stmt = CreateCastStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.r#impl, CastImpl::WithFunction(_)));
        assert!(matches!(
            stmt.context.as_ref().unwrap().kind,
            CastContextKind::Assignment(_)
        ));
        assert!(input.is_empty());
    }
}
