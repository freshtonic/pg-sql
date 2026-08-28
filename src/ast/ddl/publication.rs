//! PUBLICATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationColumnList<'input> {
    pub cols:
        Surrounded<punct::LParen, Seq1<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
}

/// `WHERE (a_expr)` row-filter on a publication table object —
/// Postgres' `OptWhereClause`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationWhereClause<'input> {
    pub r#where: WHERE,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `TABLE [ONLY] name [*] [(cols)] [WHERE (expr)]` — the `TABLE`-prefixed
/// publication object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationObjTable<'input> {
    pub table: TABLE,
    pub only: Option<ONLY>,
    pub name: QualifiedName<'input>,
    pub star: Option<punct::Star>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationObjTablesInSchema<'input> {
    pub tables: crate::tokens::soft_keyword::TABLES,
    pub r#in: IN,
    pub schema: SCHEMA,
    pub name: crate::tokens::ColId<'input>,
    pub columns: Option<PublicationColumnList<'input>>,
}

/// `ONLY`-prefixed continuation publication object (Postgres'
/// `extended_relation_expr`-with-ONLY branch). Used after a `TABLE` or
/// `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list` infers the
/// object kind from the previous prefixed item at semantic time.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationObjOnly<'input> {
    pub only: ONLY,
    pub name: QualifiedName<'input>,
    pub star: Option<punct::Star>,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// Bare-name continuation publication object — `qualified_name [*]
/// [(cols)] [WHERE (expr)]` with no leading keyword. Used after a
/// `TABLE` / `TABLES IN SCHEMA` prefix; PG's `preprocess_pubobj_list`
/// infers the object kind from the previous prefixed item at semantic
/// time.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationObjBare<'input> {
    pub name: QualifiedName<'input>,
    pub star: Option<punct::Star>,
    pub columns: Option<PublicationColumnList<'input>>,
    pub r#where: Option<PublicationWhereClause<'input>>,
}

/// One entry in a publication object list — Postgres' `PublicationObjSpec`.
///
/// Variant ordering: keyword-prefixed forms first (`Table` reserved,
/// `TablesInSchema` soft, `Only` reserved), then the catch-all `Bare`
/// (starts with an ident) so the keyword-prefixed forms win on peek.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PublicationObjSpec<'input> {
    Table(PublicationObjTable<'input>),
    TablesInSchema(PublicationObjTablesInSchema<'input>),
    Only(PublicationObjOnly<'input>),
    Bare(PublicationObjBare<'input>),
}

/// `FOR ALL TABLES` — Postgres' `CREATE PUBLICATION ... FOR ALL TABLES`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationForAllTables {
    pub r#for: FOR,
    pub all: ALL,
    pub tables: crate::tokens::soft_keyword::TABLES,
}

/// `FOR pub_obj_list` — Postgres' `CREATE PUBLICATION ... FOR pub_obj_list`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PublicationForObjects<'input> {
    pub r#for: FOR,
    pub objects: Seq1<PublicationObjSpec<'input>, punct::Comma>,
}

/// The `FOR ...` clause on CREATE PUBLICATION.
///
/// Variant ordering: `AllTables` (`FOR ALL TABLES`, 3 tokens) before
/// `Objects` (`FOR pub_obj_list`, starts with `TABLE`/`TABLES`/ident)
/// — longest match wins on the `FOR ALL TABLES` prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PublicationForClause<'input> {
    AllTables(PublicationForAllTables),
    Objects(PublicationForObjects<'input>),
}

/// `WITH (def_list)` — Postgres' `opt_definition`. Reuses the shared
/// `DefList` and prefixes it with the `WITH` keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithDefinition<'input> {
    pub with: WITH,
    pub list: DefList<'input>,
}

/// `CREATE PUBLICATION name [FOR ALL TABLES | FOR pub_obj_list]
/// [WITH (def_list)]` — Postgres' `CreatePublicationStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreatePublicationStmt<'input> {
    pub create: CREATE,
    pub publication: PUBLICATION,
    pub name: crate::tokens::ColId<'input>,
    pub r#for: Option<PublicationForClause<'input>>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP PUBLICATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropPublicationStmt<'input> {
    pub drop: DROP,
    pub publication: PUBLICATION,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET (def_list)` — Postgres' `definition` (parenthesised def_list) on
/// `ALTER PUBLICATION name SET ...` and `ALTER SUBSCRIPTION name
/// { SET | SKIP } ...`. Distinct from `WithDefinition` (which carries a
/// leading `WITH` keyword) and from `SetSchemaClause` (which carries a
/// `SCHEMA` keyword).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetDefinitionClause<'input> {
    pub set: SET,
    pub items: Surrounded<punct::LParen, Seq1<DefElem<'input>, punct::Comma>, punct::RParen>,
}

/// `SET pub_obj_list` — Postgres' `ALTER PUBLICATION ... SET pub_obj_list`.
/// Distinct from `SetDefinitionClause` because the body is a publication
/// object list (TABLE / TABLES IN SCHEMA / bare name), not a def_list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterPublicationSetObjects<'input> {
    pub set: SET,
    pub objects: Seq1<PublicationObjSpec<'input>, punct::Comma>,
}

/// `ADD pub_obj_list` — Postgres' `ALTER PUBLICATION ... ADD pub_obj_list`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterPublicationAddObjects<'input> {
    pub add: ADD,
    pub objects: Seq1<PublicationObjSpec<'input>, punct::Comma>,
}

/// `DROP pub_obj_list` — Postgres' `ALTER PUBLICATION ... DROP pub_obj_list`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterPublicationDropObjects<'input> {
    pub drop: DROP,
    pub objects: Seq1<PublicationObjSpec<'input>, punct::Comma>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterPublicationStmt<'input> {
    pub alter: ALTER,
    pub publication: PUBLICATION,
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
        let mut input = crate::tokens::test_input(
            "ALTER PUBLICATION testpub1_forschema ADD TABLES IN SCHEMA foo (a, b)",
        );
        let _stmt = AlterPublicationStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// Sanity: `ALTER PUBLICATION ... ADD TABLES IN SCHEMA name` (bare,
    /// PG-accepted form) still parses.
    #[test]
    fn parse_alter_publication_add_tables_in_schema_bare() {
        let mut input = crate::tokens::test_input("ALTER PUBLICATION p ADD TABLES IN SCHEMA foo");
        let _stmt = AlterPublicationStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
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
