//! ACCESS METHOD DDL statements (CREATE/ALTER/DROP).
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

/// `INDEX | TABLE` — the access-method type keyword in `CREATE ACCESS METHOD`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AccessMethodType {
    #[tok(INDEX)] Index,
    #[tok(TABLE)] Table,
}

/// `CREATE ACCESS METHOD name TYPE { INDEX | TABLE } HANDLER handler_name` —
/// Postgres' `CreateAmStmt`. `handler_name` is a possibly-qualified function
/// name (`name [.name …]`).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAccessMethodStmt<'input> {
    #[tok(CREATE, ACCESS, METHOD, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(TYPE, this)]
    pub am_type: AccessMethodType,
    #[tok(HANDLER, this)]
    pub handler_name: QualifiedName<'input>,
}

/// `DROP ACCESS METHOD [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropAccessMethodStmt<'input> {
    #[tok(DROP, ACCESS, METHOD, this)]
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_access_method_index() {
        let lexed = crate::tokens::lex("CREATE ACCESS METHOD gist2 TYPE INDEX HANDLER gisthandler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAccessMethodStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "gist2");
        assert!(matches!(stmt.am_type, AccessMethodType::Index(_)));
        assert_eq!(stmt.handler_name.object(), "gisthandler");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_access_method_table() {
        let lexed = crate::tokens::lex("CREATE ACCESS METHOD heap2 TYPE TABLE HANDLER heap_tableam_handler");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAccessMethodStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "heap2");
        assert!(matches!(stmt.am_type, AccessMethodType::Table(_)));
        assert!(input.is_eof());
    }
}
