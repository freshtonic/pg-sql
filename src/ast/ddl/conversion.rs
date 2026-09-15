//! CONVERSION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `CREATE [DEFAULT] CONVERSION name FOR 'src_enc' TO 'dst_enc' FROM func`.
    #[derive(Debug)]
    pub struct CreateConversionStmt {
        #[tok(CREATE, this, CONVERSION)]
        #[presence(DEFAULT)]
        pub default: bool,
        pub name: QualifiedName,
        #[tok(FOR, this)]
        pub src_encoding: literal::StringLit,
        #[tok(TO, this)]
        pub dest_encoding: literal::StringLit,
        #[tok(FROM, this)]
        pub func_name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `DROP CONVERSION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, CONVERSION, this)]
    pub struct DropConversionStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One action on `ALTER CONVERSION any_name action` — Postgres'
    /// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches
    /// for conversions.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`RENAME`, `OWNER`, `SET`), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterConversionAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `ALTER CONVERSION any_name action` — Postgres' `RenameStmt` /
    /// `AlterOwnerStmt` / `AlterObjectSchemaStmt` branches for conversions.
    #[derive(Debug)]
    pub struct AlterConversionStmt {
        #[tok(ALTER, CONVERSION, this)]
        pub name: QualifiedName,
        pub action: AlterConversionAction,
    }
}
