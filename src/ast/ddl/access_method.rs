//! ACCESS METHOD DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `INDEX | TABLE` — the access-method type keyword in `CREATE ACCESS METHOD`.
    #[derive(Debug)]
    pub enum AccessMethodType {
        #[tok(INDEX)]
        Index,
        #[tok(TABLE)]
        Table,
    }
}

recursa::ast_node! {
    /// `CREATE ACCESS METHOD name TYPE { INDEX | TABLE } HANDLER handler_name` —
    /// Postgres' `CreateAmStmt`. `handler_name` is a possibly-qualified function
    /// name (`name [.name …]`).
    #[derive(Debug)]
    pub struct CreateAccessMethodStmt {
        #[tok(CREATE, ACCESS, METHOD, this)]
        pub name: crate::tokens::ColId,
        #[tok(TYPE, this)]
        pub am_type: AccessMethodType,
        #[tok(HANDLER, this)]
        pub handler_name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `DROP ACCESS METHOD [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, ACCESS, METHOD, this)]
    pub struct DropAccessMethodStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}
