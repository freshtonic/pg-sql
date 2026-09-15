//! SCHEMA DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::sequence::CreateSequenceStmt;
use crate::ast::ddl::trigger::CreateTriggerStmt;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::grant::GrantStmt;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `AUTHORIZATION role_spec` clause on `CREATE SCHEMA`.
    #[derive(Debug)]
    pub struct SchemaAuthorization {
        #[tok(AUTHORIZATION, this)]
        pub role: RoleSpec,
    }
}

recursa::ast_node! {
    /// `name [AUTHORIZATION role]` — schema name with optional authorization.
    #[derive(Debug)]
    pub struct SchemaNameAndAuth {
        pub name: crate::tokens::ColId,
        pub authorization: Option<SchemaAuthorization>,
    }
}

recursa::ast_node! {
    /// The name/authorization clause of `CREATE SCHEMA` — Postgres allows either
    /// a schema name (with optional `AUTHORIZATION`) or just `AUTHORIZATION role`
    /// (which implicitly names the schema after the role).
    ///
    /// Variant ordering: the `Authorization` form must come first so a leading
    /// `AUTHORIZATION` keyword does not get consumed as the schema-name `Ident`
    /// (soft keywords are reclaimable as identifiers, and `AUTHORIZATION` is
    /// soft).
    #[derive(Debug)]
    pub enum SchemaNameClause {
        Authorization(SchemaAuthorization),
        Named(SchemaNameAndAuth),
    }
}

recursa::ast_node! {
    /// A single statement nested inside `CREATE SCHEMA … schema_element*`.
    ///
    /// Postgres' `OptSchemaEltList` accepts a small subset of statements:
    /// `CreateStmt | IndexStmt | CreateSeqStmt | CreateTrigStmt | GrantStmt |
    /// ViewStmt`. We type each one with the relevant statement struct rather
    /// than a general `Statement` to mirror the grammar precisely.
    ///
    /// Variant ordering: multi-keyword `GRANT` distinct from the `CREATE …`
    /// family; among the CREATE-led variants, `CreateView` (matches `CREATE [OR
    /// REPLACE] [TEMP|TEMPORARY] [RECURSIVE] VIEW …`) is the most specific.
    /// The other CREATE variants disambiguate on their `CREATE { TABLE | INDEX
    /// | SEQUENCE | TRIGGER }` second-token.
    #[derive(Debug)]
    pub enum SchemaElement {
        Grant(GrantStmt),
        CreateView(boxed!(crate::ast::ddl::view::CreateViewStmt)),
        CreateTable(boxed!(crate::ast::ddl::table::CreateTableStmt)),
        CreateIndex(boxed!(crate::ast::ddl::index::CreateIndexStmt)),
        CreateSequence(CreateSequenceStmt),
        CreateTrigger(boxed!(CreateTriggerStmt)),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(CREATE, SCHEMA, this)]
    pub struct CreateSchemaStmt {
        pub if_not_exists: Option<crate::ast::shared::flags::IfNotExists>,
        /// Schema name and/or `AUTHORIZATION` clause. Postgres requires at least
        /// one — the enum forces that structurally.
        pub head: SchemaNameClause,
        /// Nested `schema_element` statements — `OptSchemaEltList`. Each element
        /// is a top-level statement type; the surrounding semicolons live on the
        /// enclosing statement, not on the nested ones.
        pub elements: zero_or_many!(SchemaElement),
    }
}

recursa::ast_node! {
    /// `DROP SCHEMA [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, SCHEMA, this)]
    pub struct DropSchemaStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One action on `ALTER SCHEMA name action` — covers Postgres'
    /// `RenameStmt` and `AlterOwnerStmt` branches for schemas. Schemas
    /// have no SET SCHEMA action.
    ///
    /// Variant ordering: variants begin with distinct keywords (`RENAME`,
    /// `OWNER`), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterSchemaAction {
        Rename(RenameTo),
        Owner(OwnerTo),
    }
}

recursa::ast_node! {
    /// `ALTER SCHEMA name action` — Postgres' `RenameStmt` /
    /// `AlterOwnerStmt` branches for schemas.
    #[derive(Debug)]
    pub struct AlterSchemaStmt {
        #[tok(ALTER, SCHEMA, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterSchemaAction,
    }
}
