//! TRANSFORM DDL statements (CREATE/ALTER/DROP).
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

/// `function_with_argtypes` reference inside a `CREATE TRANSFORM` element.
/// Always parenthesised in this position (`prsd_lextype(internal)`) — the
/// bare-name form is not exercised by the transform grammar.
#[derive(recursa::Node, Debug, Clone)]
pub struct TransformFunctionRef<'input> {
    pub name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:

        Vec<crate::ast::ddl::function::FuncParam<'input> >

    ,
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
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub elements:
         recursa::Vec1<TransformElement<'input> > ,
}

/// `DROP TRANSFORM [IF EXISTS] FOR Typename LANGUAGE name [CASCADE|RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropTransformStmt<'input> {
    #[tok(DROP, TRANSFORM, this)]
    pub if_exists: Option<IfExists>,
    #[tok(FOR, this)]
    pub type_name: crate::ast::shared::names::TypeName<'input>,
    #[tok(LANGUAGE, this)]
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
        let lexed = crate::tokens::lex("CREATE TRANSFORM FOR int LANGUAGE SQL (\
             FROM SQL WITH FUNCTION prsd_lextype(internal),\
             TO SQL WITH FUNCTION int4recv(internal))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTransformStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_or_replace_transform_to_only() {
        let lexed = crate::tokens::lex("CREATE OR REPLACE TRANSFORM FOR text LANGUAGE plpgsql \
             (TO SQL WITH FUNCTION textrecv(internal))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTransformStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_transform() {
        let lexed = crate::tokens::lex("DROP TRANSFORM IF EXISTS FOR int LANGUAGE SQL CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTransformStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
}
