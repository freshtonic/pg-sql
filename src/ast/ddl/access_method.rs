//! ACCESS METHOD DDL statements (CREATE/ALTER/DROP).
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

/// `INDEX | TABLE` — the access-method type keyword in `CREATE ACCESS METHOD`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AccessMethodType {
    Index(INDEX),
    Table(TABLE),
}

/// `CREATE ACCESS METHOD name TYPE { INDEX | TABLE } HANDLER handler_name` —
/// Postgres' `CreateAmStmt`. `handler_name` is a possibly-qualified function
/// name (`name [.name …]`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateAccessMethodStmt<'input> {
    pub create: CREATE,
    pub access: ACCESS,
    pub method: METHOD,
    pub name: crate::tokens::ColId<'input>,
    pub r#type: TYPE,
    pub am_type: AccessMethodType,
    pub handler: HANDLER,
    pub handler_name: QualifiedName<'input>,
}

/// `DROP ACCESS METHOD [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropAccessMethodStmt<'input> {
    pub drop: DROP,
    pub access: ACCESS,
    pub method: METHOD,
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
        let mut input =
            crate::tokens::test_input("CREATE ACCESS METHOD gist2 TYPE INDEX HANDLER gisthandler");
        let stmt = CreateAccessMethodStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "gist2");
        assert!(matches!(stmt.am_type, AccessMethodType::Index(_)));
        assert_eq!(stmt.handler_name.object(), "gisthandler");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_access_method_table() {
        let mut input = crate::tokens::test_input(
            "CREATE ACCESS METHOD heap2 TYPE TABLE HANDLER heap_tableam_handler",
        );
        let stmt = CreateAccessMethodStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "heap2");
        assert!(matches!(stmt.am_type, AccessMethodType::Table(_)));
        assert!(input.is_empty());
    }
}
