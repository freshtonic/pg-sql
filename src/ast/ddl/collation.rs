//! COLLATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// Body of `CREATE COLLATION` after the name: either a `def_list` of options
    /// (`LOCALE`/`LC_COLLATE`/`PROVIDER`/...), or `FROM existing_collation_name`.
    ///
    /// Variant ordering: `From` (keyword-led) before `Options` (paren-led) — they
    /// begin with different tokens so peek disambiguation is unambiguous.
    #[derive(Debug)]
    pub enum CreateCollationBody {
        From(CollationFromClause),
        Options(DefList),
    }
}

recursa::ast_node! {
    /// `FROM existing_collation_name` — copy an existing collation.
    #[derive(Debug)]
    pub struct CollationFromClause {
        #[tok(FROM, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(CREATE, COLLATION, this)]
    pub struct CreateCollationStmt {
        pub if_not_exists: Option<IfNotExists>,
        pub name: QualifiedName,
        pub body: CreateCollationBody,
    }
}

recursa::ast_node! {
    /// `DROP COLLATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, COLLATION, this)]
    pub struct DropCollationStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `REFRESH VERSION` — Postgres' `AlterCollationStmt` action.
    #[derive(Debug)]
    pub enum CollationRefreshVersion {
        #[tok(REFRESH, VERSION)]
        Value,
    }
}

recursa::ast_node! {
    /// One action on `ALTER COLLATION any_name action` — Postgres'
    /// `RenameStmt`, `AlterOwnerStmt`, `AlterObjectSchemaStmt`, and
    /// `AlterCollationStmt` branches for collations.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`RENAME`, `OWNER`, `SET`, `REFRESH`), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterCollationAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
        RefreshVersion(CollationRefreshVersion),
    }
}

recursa::ast_node! {
    /// `ALTER COLLATION any_name action` — Postgres' `AlterCollationStmt`
    /// (REFRESH VERSION) plus the collation branches of `RenameStmt` /
    /// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
    #[derive(Debug)]
    pub struct AlterCollationStmt {
        #[tok(ALTER, COLLATION, this)]
        pub name: QualifiedName,
        pub action: AlterCollationAction,
    }
}
