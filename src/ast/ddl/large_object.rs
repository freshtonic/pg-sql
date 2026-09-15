//! LARGE OBJECT DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `ALTER LARGE OBJECT NumericOnly OWNER TO role_spec` — Postgres'
    /// `AlterOwnerStmt` branch for large objects. The only modifiable
    /// attribute is owner; large objects have no rename / set-schema /
    /// other actions.
    #[derive(Debug)]
    pub struct AlterLargeObjectStmt {
        #[tok(ALTER, LARGE, OBJECT, this)]
        pub oid: NumericOnly,
        pub owner_to: OwnerTo,
    }
}
