/// CREATE VIEW and DROP VIEW statement AST.
///
/// `CREATE [OR REPLACE] [TEMP|TEMPORARY] [RECURSIVE] VIEW name [(cols)] AS query`
/// `DROP VIEW [IF EXISTS] name`
use crate::ast::ddl::table::TempKw;
use crate::ast::dml::values::Subquery;
use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;
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
// ---------------------------------------------------------------------------

/// CREATE VIEW statement.
/// gram.y `ViewStmt: CREATE OptTemp VIEW ... | CREATE OR REPLACE OptTemp
/// VIEW ...`. The optional keywords are plain presences so they inline into
/// this rule, as gram.y spells them; a presence with a fixed token attached
/// to it lowers to its own nonterminal, which has to be reduced before the
/// `TEMP` that `CREATE TEMP TABLE` shifts.
#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, this)]
pub struct CreateViewStmt<'input> {
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub temp: Option<TempKw>,
    #[presence(RECURSIVE)]
    pub recursive: bool,
    #[tok(VIEW, this)]
    pub name: QualifiedName<'input>,
    pub columns: Option<CreateViewColumnList<'input>>,
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

/// Optional parenthesized, nonempty CREATE VIEW output-column list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct CreateViewColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<literal::AliasName<'input>>,
);

/// `USING access_method` trailer on CREATE VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct ViewUsing<'input> {
    #[tok(USING, this)]
    pub method: literal::AliasName<'input>,
}

/// `WITH [CASCADED | LOCAL] CHECK OPTION` trailer on CREATE VIEW.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WITH, this, CHECK, OPTION)]
pub struct ViewCheckOption {
    pub mode: Option<ViewCheckMode>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum ViewCheckMode {
    #[tok(CASCADED)]
    Cascaded,
    #[tok(LOCAL)]
    Local,
}

/// DROP VIEW statement:
///
/// ```sql
/// DROP VIEW [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, VIEW, this)]
pub struct DropViewStmt<'input> {
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    /// gram.y `any_name_list`: one or more names.
    pub names: recursa::Vec1<QualifiedName<'input>>,
    pub behavior: Option<DropBehavior>,
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
#[tok(ALTER, VIEW, this)]
pub struct AlterViewStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterViewAction<'input>,
}
