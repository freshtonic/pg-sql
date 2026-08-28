//! LARGE OBJECT DDL statements (CREATE/ALTER/DROP).
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

/// `ALTER LARGE OBJECT NumericOnly OWNER TO role_spec` — Postgres'
/// `AlterOwnerStmt` branch for large objects. The only modifiable
/// attribute is owner; large objects have no rename / set-schema /
/// other actions.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterLargeObjectStmt<'input> {
    pub alter: ALTER,
    pub large: LARGE,
    pub object: OBJECT,
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
        let mut input = crate::tokens::test_input("ALTER LARGE OBJECT 42 OWNER TO regress_lo_user");
        let _stmt = AlterLargeObjectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
