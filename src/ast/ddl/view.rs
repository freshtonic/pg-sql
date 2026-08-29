/// CREATE VIEW and DROP VIEW statement AST.
///
/// `CREATE [OR REPLACE] [TEMP|TEMPORARY] [RECURSIVE] VIEW name [(cols)] AS query`
/// `DROP VIEW [IF EXISTS] name`
use recursa::seq::Seq0;

use crate::ast::ddl::table::TempKw;
use crate::ast::dml::values::Subquery;
use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;
// ---------------------------------------------------------------------------
// Additional imports for the ALTER/DROP types appended to this file as part
// of the DDL physical-extraction migration. Glob imports keep cross-batch
// type references resolvable regardless of migration order; a polish pass
// will tighten these once the migration completes.
use crate::ast::ddl::index::{ResetReloptions, SetReloptions};
#[allow(unused_imports)]
use crate::ast::shared::expr::*;
#[allow(unused_imports)]
use crate::ast::shared::flags::*;
#[allow(unused_imports)]
use crate::ast::shared::names::*;
#[allow(unused_imports)]
use crate::ast::shared::numbers::*;
#[allow(unused_imports)]
use crate::tokens::soft_keyword::*;
// ---------------------------------------------------------------------------

/// CREATE VIEW statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateViewStmt<'input> {
    #[tok(CREATE, this)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub temp: Option<TempKw>,
    #[tok(this, VIEW)]
    #[presence(RECURSIVE)]
    pub recursive: bool,
    pub name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<
         Vec<literal::AliasName<'input> > ,
    >,
    /// Optional `USING access_method` (accepted by PG parser though rejected
    /// semantically for plain VIEW; tests include it).
    pub using: Option<ViewUsing<'input>>,
    /// Optional `WITH (option [= value], ...)` view options such as
    /// `security_invoker`, `security_barrier`, `check_option`.
    pub with_options: Option<crate::ast::ddl::index::WithStorage<'input>>,
    #[tok(AS, this)]
    pub query: Subquery<'input>,
    /// Optional `WITH [CASCADED|LOCAL] CHECK OPTION` trailer, used with
    /// updatable views to cascade predicate checks to underlying rows.
    pub check_option: Option<ViewCheckOption>,
}

/// `USING access_method` trailer on CREATE VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct ViewUsing<'input> {
    #[tok(USING, this)]
    pub method: literal::AliasName<'input>,
}

/// `WITH [CASCADED | LOCAL] CHECK OPTION` trailer on CREATE VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct ViewCheckOption {
    #[tok(WITH, this, CHECK, OPTION)]
    pub mode: Option<ViewCheckMode>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum ViewCheckMode {
    #[tok(CASCADED)] Cascaded,
    #[tok(LOCAL)] Local,
}

/// DROP VIEW statement:
///
/// ```sql
/// DROP VIEW [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct DropViewStmt<'input> {
    #[tok(DROP, VIEW, this)]
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    pub names: Vec<QualifiedName<'input> >,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_view() {
        let lexed = crate::tokens::lex("CREATE VIEW v AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "v");
        assert!(stmt.or_replace.is_none());
        assert!(stmt.temp.is_none());
        assert!(stmt.recursive.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_temp_view() {
        let lexed = crate::tokens::lex("CREATE TEMPORARY VIEW v AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.temp.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_recursive_view() {
        let lexed = crate::tokens::lex("CREATE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.recursive.is_some());
        assert!(stmt.columns.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_or_replace_recursive_view() {
        let lexed = crate::tokens::lex("CREATE OR REPLACE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 6");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.or_replace.is_some());
        assert!(stmt.recursive.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_view() {
        let lexed = crate::tokens::lex("DROP VIEW v");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropViewStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(stmt.if_exists.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_view_if_exists_multi_cascade() {
        let lexed = crate::tokens::lex("DROP VIEW IF EXISTS a, b CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.names.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
}

// =========================================================================
// ALTER/DROP VIEW — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `ALTER [COLUMN] name SET DEFAULT expr` — Postgres' alter_table_cmd
/// branch for setting a column default. Used by ALTER VIEW (the only
/// alter-table-cmd subset exercised by the corpus for views).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnSetDefault<'input> {
    #[tok(ALTER, optional(COLUMN), this)]
    pub name: literal::Ident<'input>,
    #[tok(SET, DEFAULT, this)]
    pub expr: Box<Expr<'input>>,
}

/// `ALTER [COLUMN] name DROP DEFAULT` — Postgres' alter_table_cmd branch
/// for dropping a column default. Used by ALTER VIEW (sister of
/// `AlterColumnSetDefault`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnDropDefault<'input> {
    #[tok(ALTER, optional(COLUMN), this, DROP, DEFAULT)]
    pub name: literal::Ident<'input>,
}

/// One `ALTER COLUMN …` cmd on ALTER VIEW. Both forms start with `ALTER
/// [COLUMN] name`; the disambiguation token after the column name is
/// `SET`/`DROP`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColumnViewCmd<'input> {
    SetDefault(AlterColumnSetDefault<'input>),
    DropDefault(AlterColumnDropDefault<'input>),
}

/// `RENAME [COLUMN] old TO new` — Postgres' RenameStmt branch for renaming
/// a view column. Used by ALTER VIEW / ALTER MATERIALIZED VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct RenameColumnClause<'input> {
    #[tok(RENAME, optional(COLUMN), this)]
    pub old_name: literal::Ident<'input>,
    #[tok(TO, this)]
    pub new_name: literal::Ident<'input>,
}

/// One action on `ALTER VIEW [IF EXISTS] name action` — Postgres'
/// `alter_table_cmds` (view subset: `OWNER TO`, `SET (...)`, `RESET (...)`,
/// `ALTER COLUMN … SET/DROP DEFAULT`), plus the view branches of
/// `RenameStmt` / `AlterObjectSchemaStmt`.
///
/// Variant ordering:
/// - `RenameColumn` (`RENAME [COLUMN] …`) before `Rename` (`RENAME TO …`)
///   — both start with `RENAME`; `RenameColumn` is the longer match.
/// - `SetSchema` (`SET SCHEMA`) and `SetReloptions` (`SET (`) — disjoint
///   second tokens.
/// - `AlterColumn` (`ALTER`) keyword-disjoint from `SET`/`RESET`/`RENAME`/
///   `OWNER`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterViewAction<'input> {
    SetSchema(SetSchemaClause<'input>),
    SetReloptions(SetReloptions<'input>),
    ResetReloptions(ResetReloptions<'input>),
    AlterColumn(AlterColumnViewCmd<'input>),
    RenameColumn(RenameColumnClause<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER VIEW [IF EXISTS] name action` — Postgres' `AlterTableStmt`
/// branches that begin with `ALTER VIEW …`, plus the view branches of
/// `RenameStmt` / `AlterObjectSchemaStmt` / `AlterOwnerStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterViewStmt<'input> {
    #[tok(ALTER, VIEW, this)]
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterViewAction<'input>,
}
