//! LARGE OBJECT DDL statements (CREATE/ALTER/DROP).
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

/// `ALTER LARGE OBJECT NumericOnly OWNER TO role_spec` — Postgres'
/// `AlterOwnerStmt` branch for large objects. The only modifiable
/// attribute is owner; large objects have no rename / set-schema /
/// other actions.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterLargeObjectStmt<'input> {
    #[tok(ALTER, LARGE, OBJECT, this)]
    pub oid: NumericOnly<'input>,
    pub owner_to: OwnerTo<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_large_object() {
        let lexed = crate::tokens::lex("ALTER LARGE OBJECT 42 OWNER TO regress_lo_user");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterLargeObjectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
