//! TRANSFORM DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `function_with_argtypes` reference inside a `CREATE TRANSFORM` element.
    /// Always parenthesised in this position (`prsd_lextype(internal)`) — the
    /// bare-name form is not exercised by the transform grammar.
    #[derive(Debug)]
    pub struct TransformFunctionRef {
        pub name: QualifiedName,
        pub args: crate::ast::ddl::function::FunctionParameters,
    }
}

recursa::ast_node! {
    /// One element of `CREATE TRANSFORM (..., ...)`. Per gram.y
    /// `transform_element_list`: either `FROM SQL WITH FUNCTION fn` or
    /// `TO SQL WITH FUNCTION fn`. Up to one of each is allowed, in either order
    /// (modelled here as a `Seq1` of these elements separated by commas).
    ///
    /// Variant ordering: disjoint first tokens (`FROM` vs `TO`), so order doesn't
    /// matter for disambiguation.
    #[derive(Debug)]
    pub enum TransformElement {
        From(TransformFromElement),
        To(TransformToElement),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct TransformFromElement {
        #[tok(FROM, SQL, WITH, FUNCTION, this)]
        pub func: TransformFunctionRef,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct TransformToElement {
        #[tok(TO, SQL, WITH, FUNCTION, this)]
        pub func: TransformFunctionRef,
    }
}

recursa::ast_node! {
    /// Parenthesized `CREATE TRANSFORM` element list.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct TransformElementList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(TransformElement),
    );
}

recursa::ast_node! {
    /// `CREATE [OR REPLACE] TRANSFORM FOR Typename LANGUAGE name (elements)`
    /// (PG `CreateTransformStmt` in gram.y). The element list is one or two
    /// `{FROM|TO} SQL WITH FUNCTION ...` entries; pg-sql models the list as
    /// `Seq1` of `TransformElement` separated by `Comma`, and relies on PG to reject duplicates and
    /// empty lists at semantic-analysis time.
    #[derive(Debug)]
    pub struct CreateTransformStmt {
        #[tok(CREATE, this, TRANSFORM, FOR)]
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        pub type_name: crate::ast::shared::names::TypeName,
        #[tok(LANGUAGE, this)]
        pub lang_name: crate::ast::ddl::function::LanguageName,
        pub elements: TransformElementList,
    }
}

recursa::ast_node! {
    /// `DROP TRANSFORM [IF EXISTS] FOR Typename LANGUAGE name [CASCADE|RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, TRANSFORM, this)]
    pub struct DropTransformStmt {
        pub if_exists: Option<IfExists>,
        #[tok(FOR, this)]
        pub type_name: crate::ast::shared::names::TypeName,
        #[tok(LANGUAGE, this)]
        pub lang_name: crate::ast::ddl::function::LanguageName,
        pub behavior: Option<DropBehavior>,
    }
}
