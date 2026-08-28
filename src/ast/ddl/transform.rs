//! TRANSFORM DDL statements (CREATE/ALTER/DROP).
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

/// `function_with_argtypes` reference inside a `CREATE TRANSFORM` element.
/// Always parenthesised in this position (`prsd_lextype(internal)`) — the
/// bare-name form is not exercised by the transform grammar.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TransformFunctionRef<'input> {
    pub name: QualifiedName<'input>,
    pub args: Surrounded<
        punct::LParen,
        Seq0<crate::ast::ddl::function::FuncParam<'input>, punct::Comma>,
        punct::RParen,
    >,
}

/// One element of `CREATE TRANSFORM (..., ...)`. Per gram.y
/// `transform_element_list`: either `FROM SQL WITH FUNCTION fn` or
/// `TO SQL WITH FUNCTION fn`. Up to one of each is allowed, in either order
/// (modelled here as a `Seq1` of these elements separated by commas).
///
/// Variant ordering: disjoint first tokens (`FROM` vs `TO`), so order doesn't
/// matter for disambiguation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TransformElement<'input> {
    From(TransformFromElement<'input>),
    To(TransformToElement<'input>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TransformFromElement<'input> {
    pub from: FROM,
    pub sql: SQL,
    pub with: WITH,
    pub function: FUNCTION,
    pub func: TransformFunctionRef<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TransformToElement<'input> {
    pub to: TO,
    pub sql: SQL,
    pub with: WITH,
    pub function: FUNCTION,
    pub func: TransformFunctionRef<'input>,
}

/// `CREATE [OR REPLACE] TRANSFORM FOR Typename LANGUAGE name (elements)`
/// (PG `CreateTransformStmt` in gram.y). The element list is one or two
/// `{FROM|TO} SQL WITH FUNCTION ...` entries; pg-sql models the list as
/// `Seq1` of `TransformElement` separated by `Comma`, and relies on PG to reject duplicates and
/// empty lists at semantic-analysis time.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateTransformStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub transform: TRANSFORM,
    pub r#for: FOR,
    pub type_name: crate::ast::shared::names::TypeName<'input>,
    pub language: LANGUAGE,
    pub lang_name: crate::ast::ddl::function::LanguageName<'input>,
    pub elements:
        Surrounded<punct::LParen, Seq1<TransformElement<'input>, punct::Comma>, punct::RParen>,
}

/// `DROP TRANSFORM [IF EXISTS] FOR Typename LANGUAGE name [CASCADE|RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropTransformStmt<'input> {
    pub drop: DROP,
    pub transform: TRANSFORM,
    pub if_exists: Option<IfExists>,
    pub r#for: FOR,
    pub type_name: crate::ast::shared::names::TypeName<'input>,
    pub language: LANGUAGE,
    pub lang_name: crate::ast::ddl::function::LanguageName<'input>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    /// `CREATE TRANSFORM FOR Typename LANGUAGE name ( from_fn, to_fn )` —
    /// from the object_address.sql corpus, the only `CREATE TRANSFORM` in
    /// the regression suite. Exercises both `FROM SQL WITH FUNCTION ...`
    /// and `TO SQL WITH FUNCTION ...` element forms.
    #[test]
    fn parse_create_transform() {
        let mut input = crate::tokens::test_input(
            "CREATE TRANSFORM FOR int LANGUAGE SQL (\
             FROM SQL WITH FUNCTION prsd_lextype(internal),\
             TO SQL WITH FUNCTION int4recv(internal))",
        );
        let _stmt = CreateTransformStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_or_replace_transform_to_only() {
        let mut input = crate::tokens::test_input(
            "CREATE OR REPLACE TRANSFORM FOR text LANGUAGE plpgsql \
             (TO SQL WITH FUNCTION textrecv(internal))",
        );
        let _stmt = CreateTransformStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_transform() {
        let mut input =
            crate::tokens::test_input("DROP TRANSFORM IF EXISTS FOR int LANGUAGE SQL CASCADE");
        let stmt = DropTransformStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }
}
