/// CREATE VIEW and DROP VIEW statement AST.
///
/// `CREATE [OR REPLACE] [TEMP|TEMPORARY] [RECURSIVE] VIEW name [(cols)] AS query`
/// `DROP VIEW [IF EXISTS] name`
use recursa::seq::Seq0;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateViewStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub temp: Option<TempKw>,
    pub recursive: Option<RECURSIVE>,
    pub view: VIEW,
    pub name: QualifiedName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
    /// Optional `USING access_method` (accepted by PG parser though rejected
    /// semantically for plain VIEW; tests include it).
    pub using: Option<ViewUsing<'input>>,
    /// Optional `WITH (option [= value], ...)` view options such as
    /// `security_invoker`, `security_barrier`, `check_option`.
    pub with_options: Option<crate::ast::ddl::index::WithStorage<'input>>,
    pub r#as: AS,
    pub query: Subquery<'input>,
    /// Optional `WITH [CASCADED|LOCAL] CHECK OPTION` trailer, used with
    /// updatable views to cascade predicate checks to underlying rows.
    pub check_option: Option<ViewCheckOption>,
}

/// `USING access_method` trailer on CREATE VIEW.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ViewUsing<'input> {
    pub using: USING,
    pub method: literal::AliasName<'input>,
}

/// `WITH [CASCADED | LOCAL] CHECK OPTION` trailer on CREATE VIEW.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ViewCheckOption {
    pub with: WITH,
    pub mode: Option<ViewCheckMode>,
    pub check: CHECK,
    pub option: OPTION,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ViewCheckMode {
    Cascaded(CASCADED),
    Local(LOCAL),
}

/// DROP VIEW statement:
///
/// ```sql
/// DROP VIEW [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropViewStmt<'input> {
    pub drop: DROP,
    pub view: VIEW,
    pub if_exists: Option<(IF, EXISTS)>,
    pub names: Seq0<QualifiedName<'input>, punct::Comma>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_view() {
        let mut input = crate::tokens::test_input("CREATE VIEW v AS SELECT 1");
        let stmt = CreateViewStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "v");
        assert!(stmt.or_replace.is_none());
        assert!(stmt.temp.is_none());
        assert!(stmt.recursive.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_temp_view() {
        let mut input = crate::tokens::test_input("CREATE TEMPORARY VIEW v AS SELECT 1");
        let stmt = CreateViewStmt::parse(&mut input).unwrap();
        assert!(stmt.temp.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_recursive_view() {
        let mut input = crate::tokens::test_input(
            "CREATE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 5",
        );
        let stmt = CreateViewStmt::parse(&mut input).unwrap();
        assert!(stmt.recursive.is_some());
        assert!(stmt.columns.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_or_replace_recursive_view() {
        let mut input = crate::tokens::test_input(
            "CREATE OR REPLACE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 6",
        );
        let stmt = CreateViewStmt::parse(&mut input).unwrap();
        assert!(stmt.or_replace.is_some());
        assert!(stmt.recursive.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_view() {
        let mut input = crate::tokens::test_input("DROP VIEW v");
        let stmt = DropViewStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.names.len(), 1);
        assert!(stmt.if_exists.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_view_if_exists_multi_cascade() {
        let mut input = crate::tokens::test_input("DROP VIEW IF EXISTS a, b CASCADE");
        let stmt = DropViewStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.names.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }
}

// =========================================================================
// ALTER/DROP VIEW — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `ALTER [COLUMN] name SET DEFAULT expr` — Postgres' alter_table_cmd
/// branch for setting a column default. Used by ALTER VIEW (the only
/// alter-table-cmd subset exercised by the corpus for views).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColumnSetDefault<'input> {
    pub alter: ALTER,
    pub column: Option<COLUMN>,
    pub name: literal::Ident<'input>,
    pub set: SET,
    pub default: DEFAULT,
    pub expr: Box<Expr<'input>>,
}

/// `ALTER [COLUMN] name DROP DEFAULT` — Postgres' alter_table_cmd branch
/// for dropping a column default. Used by ALTER VIEW (sister of
/// `AlterColumnSetDefault`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColumnDropDefault<'input> {
    pub alter: ALTER,
    pub column: Option<COLUMN>,
    pub name: literal::Ident<'input>,
    pub drop: DROP,
    pub default: DEFAULT,
}

/// One `ALTER COLUMN …` cmd on ALTER VIEW. Both forms start with `ALTER
/// [COLUMN] name`; the disambiguation token after the column name is
/// `SET`/`DROP`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterColumnViewCmd<'input> {
    SetDefault(AlterColumnSetDefault<'input>),
    DropDefault(AlterColumnDropDefault<'input>),
}

/// `RENAME [COLUMN] old TO new` — Postgres' RenameStmt branch for renaming
/// a view column. Used by ALTER VIEW / ALTER MATERIALIZED VIEW.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RenameColumnClause<'input> {
    pub rename: RENAME,
    pub column: Option<COLUMN>,
    pub old_name: literal::Ident<'input>,
    pub to: TO,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterViewStmt<'input> {
    pub alter: ALTER,
    pub view: VIEW,
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterViewAction<'input>,
}
