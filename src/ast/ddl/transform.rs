//! TRANSFORM DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `function_with_argtypes` reference inside a `CREATE TRANSFORM` element.
/// Always parenthesised in this position (`prsd_lextype(internal)`) — the
/// bare-name form is not exercised by the transform grammar.
#[derive(recursa::Node, Debug, Clone)]
pub struct TransformFunctionRef<'input> {
    pub name: QualifiedName<'input>,
    pub args: crate::ast::ddl::function::FunctionParameters<'input>,
}

/// One element of `CREATE TRANSFORM (..., ...)`. Per gram.y
/// `transform_element_list`: either `FROM SQL WITH FUNCTION fn` or
/// `TO SQL WITH FUNCTION fn`. Up to one of each is allowed, in either order
/// (modelled here as a `Seq1` of these elements separated by commas).
///
/// Variant ordering: disjoint first tokens (`FROM` vs `TO`), so order doesn't
/// matter for disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransformElement<'input> {
    From(TransformFromElement<'input>),
    To(TransformToElement<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct TransformFromElement<'input> {
    #[tok(FROM, SQL, WITH, FUNCTION, this)]
    pub func: TransformFunctionRef<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct TransformToElement<'input> {
    #[tok(TO, SQL, WITH, FUNCTION, this)]
    pub func: TransformFunctionRef<'input>,
}

/// Parenthesized `CREATE TRANSFORM` element list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct TransformElementList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<TransformElement<'input>>,
);

/// `CREATE [OR REPLACE] TRANSFORM FOR Typename LANGUAGE name (elements)`
/// (PG `CreateTransformStmt` in gram.y). The element list is one or two
/// `{FROM|TO} SQL WITH FUNCTION ...` entries; pg-sql models the list as
/// `Seq1` of `TransformElement` separated by `Comma`, and relies on PG to reject duplicates and
/// empty lists at semantic-analysis time.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTransformStmt<'input> {
    #[tok(CREATE, this, TRANSFORM, FOR)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub type_name: crate::ast::shared::names::TypeName<'input>,
    #[tok(LANGUAGE, this)]
    pub lang_name: crate::ast::ddl::function::LanguageName<'input>,
    pub elements: TransformElementList<'input>,
}

/// `DROP TRANSFORM [IF EXISTS] FOR Typename LANGUAGE name [CASCADE|RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, TRANSFORM, this)]
pub struct DropTransformStmt<'input> {
    pub if_exists: Option<IfExists>,
    #[tok(FOR, this)]
    pub type_name: crate::ast::shared::names::TypeName<'input>,
    #[tok(LANGUAGE, this)]
    pub lang_name: crate::ast::ddl::function::LanguageName<'input>,
    pub behavior: Option<DropBehavior>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/transform.tests.rs"
));
