//! PUBLICATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::{DefElem, DefList};
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// Optional `(col, ...)` column-list on a publication table object —
    /// Postgres' `opt_column_list`.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct PublicationColumnList {
        #[sep(COMMA)]
        pub cols: one_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `WHERE (a_expr)` row-filter on a publication table object —
    /// Postgres' `OptWhereClause`.
    #[derive(Debug)]
    pub struct PublicationWhereClause {
        #[tok(WHERE, LPAREN, this, RPAREN)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `TABLE [ONLY] name [*] [(cols)] [WHERE (expr)]` — the `TABLE`-prefixed
    /// publication object.
    #[derive(Debug)]
    pub struct PublicationObjTable {
        /// gram.y `PublicationObjSpec: TABLE relation_expr ...`.
        #[tok(TABLE, this)]
        pub relation: crate::ast::shared::names::RelationExpr,
        pub columns: Option<PublicationColumnList>,
        pub r#where: Option<PublicationWhereClause>,
    }
}

recursa::ast_node! {
    /// `TABLES IN SCHEMA name [(cols)]` — the schema-scoped publication
    /// object. The schema name is an unqualified identifier (PG accepts
    /// `CURRENT_SCHEMA` as a special keyword form; pg-sql does not lex it as
    /// a keyword so it flows through as a bare `Ident`).
    ///
    /// Per gram.y `PublicationObjSpec: TABLES IN SCHEMA ColId`, no column
    /// list is permitted — PG rejects `TABLES IN SCHEMA foo (a, b)`
    /// syntactically. The corpus (`publication.sql`) exercises this PG-rejected
    /// form to verify the error; pg-sql accepts it over-permissively so the
    /// statement is modelled instead of surfacing as a
    /// a file-level parse error. The round-tripped output is still
    /// PG-rejected, so the differential oracle stays valid.
    #[derive(Debug)]
    pub struct PublicationObjTablesInSchema {
        #[tok(TABLES, IN, SCHEMA, this)]
        pub name: crate::tokens::ColId,
        pub columns: Option<PublicationColumnList>,
    }
}

recursa::ast_node! {
    /// `ONLY`-prefixed continuation publication object (Postgres'
    /// `extended_relation_expr`-with-ONLY branch). Used after a `TABLE` or
    /// `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list` infers the
    /// object kind from the previous prefixed item at semantic time.
    #[derive(Debug)]
    pub struct PublicationObjOnly {
        /// `ONLY name` or `ONLY ( name )`; neither takes a `*`.
        pub relation: crate::ast::shared::names::OnlyRelation,
        pub columns: Option<PublicationColumnList>,
        pub r#where: Option<PublicationWhereClause>,
    }
}

recursa::ast_node! {
    /// Bare-name continuation publication object — `qualified_name [*]
    /// [(cols)] [WHERE (expr)]` with no leading keyword. Used after a
    /// `TABLE` / `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list`
    /// infers the object kind from the previous prefixed item at semantic
    /// time.
    #[derive(Debug)]
    pub struct PublicationObjBare {
        pub name: QualifiedName,
        #[presence(STAR)]
        pub star: bool,
        pub columns: Option<PublicationColumnList>,
        pub r#where: Option<PublicationWhereClause>,
    }
}

recursa::ast_node! {
    /// One entry in a publication object list — Postgres' `PublicationObjSpec`.
    ///
    /// Variant ordering: keyword-prefixed forms first (`Table` reserved,
    /// `TablesInSchema` soft, `Only` reserved), then the catch-all `Bare`
    /// (starts with an ident) so the keyword-prefixed forms win on peek.
    #[derive(Debug)]
    pub enum PublicationObjSpec {
        Table(PublicationObjTable),
        TablesInSchema(PublicationObjTablesInSchema),
        Only(PublicationObjOnly),
        Bare(PublicationObjBare),
    }
}

recursa::ast_node! {
    /// `FOR ALL TABLES` — Postgres' `CREATE PUBLICATION ... FOR ALL TABLES`.
    #[derive(Debug)]
    pub enum PublicationForAllTables {
        #[tok(FOR, ALL, TABLES)]
        Value,
    }
}

recursa::ast_node! {
    /// `FOR pub_obj_list` — Postgres' `CREATE PUBLICATION ... FOR pub_obj_list`.
    #[derive(Debug)]
    #[tok(FOR, this)]
    pub struct PublicationForObjects {
        #[sep(COMMA)]
        pub objects: one_or_many!(PublicationObjSpec),
    }
}

recursa::ast_node! {
    /// The `FOR ...` clause on CREATE PUBLICATION.
    ///
    /// Variant ordering: `AllTables` (`FOR ALL TABLES`, 3 tokens) before
    /// `Objects` (`FOR pub_obj_list`, starts with `TABLE`/`TABLES`/ident)
    /// — longest match wins on the `FOR ALL TABLES` prefix.
    #[derive(Debug)]
    pub enum PublicationForClause {
        AllTables(PublicationForAllTables),
        Objects(PublicationForObjects),
    }
}

recursa::ast_node! {
    /// `WITH (def_list)` — Postgres' `opt_definition`. Reuses the shared
    /// `DefList` and prefixes it with the `WITH` keyword.
    #[derive(Debug)]
    pub struct WithDefinition {
        #[tok(WITH, this)]
        pub list: DefList,
    }
}

recursa::ast_node! {
    /// `CREATE PUBLICATION name [FOR ALL TABLES | FOR pub_obj_list]
    /// [WITH (def_list)]` — Postgres' `CreatePublicationStmt`.
    #[derive(Debug)]
    pub struct CreatePublicationStmt {
        #[tok(CREATE, PUBLICATION, this)]
        pub name: crate::tokens::ColId,
        pub r#for: Option<PublicationForClause>,
        pub with: Option<WithDefinition>,
    }
}

recursa::ast_node! {
    /// `DROP PUBLICATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, PUBLICATION, this)]
    pub struct DropPublicationStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `SET (def_list)` — Postgres' `definition` (parenthesised def_list) on
    /// `ALTER PUBLICATION name SET ...` and `ALTER SUBSCRIPTION name
    /// { SET | SKIP } ...`. Distinct from `WithDefinition` (which carries a
    /// leading `WITH` keyword) and from `SetSchemaClause` (which carries a
    /// `SCHEMA` keyword).
    #[derive(Debug)]
    #[tok(SET, LPAREN, this, RPAREN)]
    pub struct SetDefinitionClause {
        #[sep(COMMA)]
        pub items: one_or_many!(DefElem),
    }
}

recursa::ast_node! {
    /// `SET pub_obj_list` — Postgres' `ALTER PUBLICATION ... SET pub_obj_list`.
    /// Distinct from `SetDefinitionClause` because the body is a publication
    /// object list (TABLE / TABLES IN SCHEMA / bare name), not a def_list.
    #[derive(Debug)]
    #[tok(SET, this)]
    pub struct AlterPublicationSetObjects {
        #[sep(COMMA)]
        pub objects: one_or_many!(PublicationObjSpec),
    }
}

recursa::ast_node! {
    /// `ADD pub_obj_list` — Postgres' `ALTER PUBLICATION ... ADD pub_obj_list`.
    #[derive(Debug)]
    #[tok(ADD, this)]
    pub struct AlterPublicationAddObjects {
        #[sep(COMMA)]
        pub objects: one_or_many!(PublicationObjSpec),
    }
}

recursa::ast_node! {
    /// `DROP pub_obj_list` — Postgres' `ALTER PUBLICATION ... DROP pub_obj_list`.
    #[derive(Debug)]
    #[tok(DROP, this)]
    pub struct AlterPublicationDropObjects {
        #[sep(COMMA)]
        pub objects: one_or_many!(PublicationObjSpec),
    }
}

recursa::ast_node! {
    /// One action on `ALTER PUBLICATION name action` — covers Postgres'
    /// `AlterPublicationStmt` (`SET definition`, `{ ADD | DROP | SET }
    /// pub_obj_list`) plus the `RENAME TO` / `OWNER TO` branches from
    /// `RenameStmt` / `AlterOwnerStmt`.
    ///
    /// Variant ordering: `Rename` and `Owner` have distinct first keywords
    /// (`RENAME`, `OWNER`). The three `pub_obj_list` actions share `ADD` /
    /// `DROP` / `SET` first tokens; `SET (def_list)` and `SET pub_obj_list`
    /// both start with `SET` and disambiguate by the next token (`(` →
    /// def_list, anything else → pub_obj_list). Lists `SetDef` before
    /// `SetObjs` so the `SET (` longer-prefix peek wins.
    #[derive(Debug)]
    pub enum AlterPublicationAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        AddObjs(AlterPublicationAddObjects),
        DropObjs(AlterPublicationDropObjects),
        SetDef(SetDefinitionClause),
        SetObjs(AlterPublicationSetObjects),
    }
}

recursa::ast_node! {
    /// `ALTER PUBLICATION name action` — Postgres' `AlterPublicationStmt`
    /// plus the publication branches of `RenameStmt` / `AlterOwnerStmt`.
    #[derive(Debug)]
    pub struct AlterPublicationStmt {
        #[tok(ALTER, PUBLICATION, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterPublicationAction,
    }
}
