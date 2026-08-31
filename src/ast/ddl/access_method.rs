//! ACCESS METHOD DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// `INDEX | TABLE` — the access-method type keyword in `CREATE ACCESS METHOD`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AccessMethodType {
    #[tok(INDEX)]
    Index,
    #[tok(TABLE)]
    Table,
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
#[tok(DROP, ACCESS, METHOD, this)]
pub struct DropAccessMethodStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/access_method.tests.rs"
));
