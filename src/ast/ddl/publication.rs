//! PUBLICATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::ddl::role::{DefElem, DefList};
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Optional `(col, ...)` column-list on a publication table object —
/// Postgres' `opt_column_list`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationColumnList<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub cols:
         recursa::Vec1<crate::tokens::ColId<'input> > ,
}

/// `WHERE (a_expr)` row-filter on a publication table object —
/// Postgres' `OptWhereClause`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationWhereClause<'input> {
    #[tok(WHERE, LPAREN, this, RPAREN)]
    pub expr:  Box<Expr<'input>> ,
}

/// `TABLE [ONLY] name [*] [(cols)] [WHERE (expr)]` — the `TABLE`-prefixed
/// publication object.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjTable<'input> {
    #[tok(TABLE, this)]
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

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
/// [`crate::ast::FileItem::ParseError`]. The round-tripped output is still
/// PG-rejected, so the differential oracle stays valid.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjTablesInSchema<'input> {
    #[tok(TABLES, IN, SCHEMA, this)]
    pub name: crate::tokens::ColId<'input>,
    pub columns: Option<PublicationColumnList<'input>>,
}

/// `ONLY`-prefixed continuation publication object (Postgres'
/// `extended_relation_expr`-with-ONLY branch). Used after a `TABLE` or
/// `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list` infers the
/// object kind from the previous prefixed item at semantic time.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjOnly<'input> {
    #[tok(ONLY, this)]
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// Bare-name continuation publication object — `qualified_name [*]
/// [(cols)] [WHERE (expr)]` with no leading keyword. Used after a
/// `TABLE` / `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list`
/// infers the object kind from the previous prefixed item at semantic
/// time.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationObjBare<'input> {
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// One entry in a publication object list — Postgres' `PublicationObjSpec`.
///
/// Variant ordering: keyword-prefixed forms first (`Table` reserved,
/// `TablesInSchema` soft, `Only` reserved), then the catch-all `Bare`
/// (starts with an ident) so the keyword-prefixed forms win on peek.
#[derive(recursa::Node, Debug, Clone)]
pub enum PublicationObjSpec<'input> {
    Table(PublicationObjTable<'input>),
    TablesInSchema(PublicationObjTablesInSchema<'input>),
    Only(PublicationObjOnly<'input>),
    Bare(PublicationObjBare<'input>),
}

/// `FOR ALL TABLES` — Postgres' `CREATE PUBLICATION ... FOR ALL TABLES`.
#[derive(recursa::Node, Debug, Clone)]
pub enum PublicationForAllTables { #[tok(FOR, ALL, TABLES)] Value, }

/// `FOR pub_obj_list` — Postgres' `CREATE PUBLICATION ... FOR pub_obj_list`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PublicationForObjects<'input> {
    #[tok(FOR, this)]
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input> >,
}

/// The `FOR ...` clause on CREATE PUBLICATION.
///
/// Variant ordering: `AllTables` (`FOR ALL TABLES`, 3 tokens) before
/// `Objects` (`FOR pub_obj_list`, starts with `TABLE`/`TABLES`/ident)
/// — longest match wins on the `FOR ALL TABLES` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum PublicationForClause<'input> {
    AllTables(PublicationForAllTables),
    Objects(PublicationForObjects<'input>),
}

/// `WITH (def_list)` — Postgres' `opt_definition`. Reuses the shared
/// `DefList` and prefixes it with the `WITH` keyword.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithDefinition<'input> {
    #[tok(WITH, this)]
    pub list: DefList<'input>,
}

/// `CREATE PUBLICATION name [FOR ALL TABLES | FOR pub_obj_list]
/// [WITH (def_list)]` — Postgres' `CreatePublicationStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreatePublicationStmt<'input> {
    #[tok(CREATE, PUBLICATION, this)]
    pub name: crate::tokens::ColId<'input>,
    pub r#for: Option<PublicationForClause<'input>>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP PUBLICATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropPublicationStmt<'input> {
    #[tok(DROP, PUBLICATION, this)]
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET (def_list)` — Postgres' `definition` (parenthesised def_list) on
/// `ALTER PUBLICATION name SET ...` and `ALTER SUBSCRIPTION name
/// { SET | SKIP } ...`. Distinct from `WithDefinition` (which carries a
/// leading `WITH` keyword) and from `SetSchemaClause` (which carries a
/// `SCHEMA` keyword).
#[derive(recursa::Node, Debug, Clone)]
pub struct SetDefinitionClause<'input> {
    #[tok(SET, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub items:  recursa::Vec1<DefElem<'input> > ,
}

/// `SET pub_obj_list` — Postgres' `ALTER PUBLICATION ... SET pub_obj_list`.
/// Distinct from `SetDefinitionClause` because the body is a publication
/// object list (TABLE / TABLES IN SCHEMA / bare name), not a def_list.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPublicationSetObjects<'input> {
    #[tok(SET, this)]
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input> >,
}

/// `ADD pub_obj_list` — Postgres' `ALTER PUBLICATION ... ADD pub_obj_list`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPublicationAddObjects<'input> {
    #[tok(ADD, this)]
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input> >,
}

/// `DROP pub_obj_list` — Postgres' `ALTER PUBLICATION ... DROP pub_obj_list`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPublicationDropObjects<'input> {
    #[tok(DROP, this)]
    #[sep(COMMA)]
    pub objects: recursa::Vec1<PublicationObjSpec<'input> >,
}

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
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterPublicationAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    AddObjs(AlterPublicationAddObjects<'input>),
    DropObjs(AlterPublicationDropObjects<'input>),
    SetDef(SetDefinitionClause<'input>),
    SetObjs(AlterPublicationSetObjects<'input>),
}

/// `ALTER PUBLICATION name action` — Postgres' `AlterPublicationStmt`
/// plus the publication branches of `RenameStmt` / `AlterOwnerStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPublicationStmt<'input> {
    #[tok(ALTER, PUBLICATION, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterPublicationAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    /// `ALTER PUBLICATION ... ADD TABLES IN SCHEMA name (cols)` — PG rejects
    /// this syntactically (`TABLES IN SCHEMA ColId` has no opt_column_list),
    /// but pg-sql accepts it over-permissively so the publication.sql corpus
    /// statement parses into a structured AST.
    #[test]
    fn parse_alter_publication_add_tables_in_schema_with_columns() {
        let lexed = crate::tokens::lex("ALTER PUBLICATION testpub1_forschema ADD TABLES IN SCHEMA foo (a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterPublicationStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Sanity: `ALTER PUBLICATION ... ADD TABLES IN SCHEMA name` (bare,
    /// PG-accepted form) still parses.
    #[test]
    fn parse_alter_publication_add_tables_in_schema_bare() {
        let lexed = crate::tokens::lex("ALTER PUBLICATION p ADD TABLES IN SCHEMA foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterPublicationStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn create_publication_bare_roundtrips() {
        let stmt: CreatePublicationStmt = parse_stmt("CREATE PUBLICATION testpub_default");
        assert_eq!(stmt.name.text(), "testpub_default");
        assert!(stmt.r#for.is_none());
        assert!(stmt.with.is_none());
        reparse_stable::<CreatePublicationStmt>("CREATE PUBLICATION testpub_default");
    }

    #[test]
    fn create_publication_for_all_tables_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION testpub_foralltables FOR ALL TABLES WITH (publish = 'insert')",
        );
    }

    #[test]
    fn create_publication_for_table_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION testpub_fortable FOR TABLE testpub_tbl1",
        );
    }

    #[test]
    fn create_publication_for_table_only_where_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION p FOR TABLE testpub_rf_tbl1, ONLY testpub_rf_tbl3 WHERE (e < 999) WITH (publish = 'insert')",
        );
    }

    #[test]
    fn create_publication_for_tables_in_schema_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION testpub_forschema FOR TABLES IN SCHEMA pub_test",
        );
    }

    #[test]
    fn create_publication_mixed_tables_and_schema_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION p FOR TABLES IN SCHEMA pub_test, TABLE pub_test.testpub_nopk",
        );
    }

    #[test]
    fn create_publication_with_columns_and_where_roundtrips() {
        reparse_stable::<CreatePublicationStmt>(
            "CREATE PUBLICATION p FOR TABLE testpub_rf_tbl1 (c, d) WHERE (c <> 'test' AND d < 5)",
        );
    }
}
