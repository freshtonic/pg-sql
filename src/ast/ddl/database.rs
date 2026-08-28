//! DATABASE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Name of a CREATE DATABASE option — Postgres' `createdb_opt_name`.
///
/// `gram.y` allows a bare IDENT plus a handful of keywords (`CONNECTION
/// LIMIT`, `ENCODING`, `LOCATION`, `OWNER`, `TABLESPACE`, `TEMPLATE`) that
/// would otherwise be reserved against the option name. `AliasName` admits
/// every bareword including soft keywords, so it covers the IDENT branch
/// and the bareword keyword cases. `CONNECTION LIMIT` is two tokens and
/// gets its own variant. The kwlist.h `OWNER`, `TABLESPACE`, etc., are
/// already either soft keywords or hard keywords in pg-sql — when soft,
/// `AliasName::Bare` reclassifies them.
///
/// Variant ordering: the two-token `CONNECTION LIMIT` form before the
/// general `AliasName` so the longer match wins.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreateDbOptName<'input> {
    ConnectionLimit((crate::tokens::soft_keyword::CONNECTION, LIMIT)),
    /// Any bareword — covers IDENT and all the keyword-spelled option names
    /// (`OWNER`, `TABLESPACE`, `TEMPLATE`, `ENCODING`, `LOCATION`, plus the
    /// IDENT-only names like `is_template`, `allow_connections`, `strategy`,
    /// `locale`, `locale_provider`, `oid`, `icu_locale`, `icu_rules`,
    /// `builtin_locale`, `collation_version`, `lc_collate`, `lc_ctype`).
    Name(literal::AliasName<'input>),
}

/// The value of a CREATE DATABASE option — Postgres' `createdb_opt_item`.
/// Three RHS forms: `NumericOnly`, `opt_boolean_or_string`, or `DEFAULT`.
///
/// Variant ordering: `Default` (keyword) first, then `Numeric` (digits or
/// `+`/`-`), then `Boolean` (`TRUE`/`FALSE`/`ON`), then `String` (quoted),
/// then the catch-all `Word` (bareword incl. `off`, identifier-like values).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreateDbOptValue<'input> {
    Default(DEFAULT),
    Numeric(crate::ast::shared::numbers::NumericOnly<'input>),
    True(TRUE),
    False(FALSE),
    On(ON),
    String(CopySconst<'input>),
    /// `NonReservedWord` — bareword including soft keywords.
    Word(literal::AliasName<'input>),
}

/// A single CREATE DATABASE option — Postgres' `createdb_opt_item`. Options
/// are unordered and repeatable, with an optional `=` between name and value.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateDbOption<'input> {
    pub name: CreateDbOptName<'input>,
    pub eq: Option<punct::Eq>,
    pub value: CreateDbOptValue<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateDatabaseStmt<'input> {
    pub create: CREATE,
    pub database: DATABASE,
    pub name: crate::tokens::ColId<'input>,
    pub with: Option<WITH>,
    pub options: Vec<CreateDbOption<'input>>,
}

/// A single `DROP DATABASE` option. Postgres currently defines only `FORCE`,
/// but the grammar is comma-separated and extensible.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DropDatabaseOption {
    Force(crate::tokens::soft_keyword::FORCE),
}

/// `[WITH] (option [, ...])` option list on `DROP DATABASE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropDatabaseOptions {
    pub with: Option<WITH>,
    pub options: Surrounded<punct::LParen, Seq0<DropDatabaseOption, punct::Comma>, punct::RParen>,
}

/// `DROP DATABASE [IF EXISTS] name [[WITH] (FORCE)]` — no `CASCADE`/`RESTRICT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropDatabaseStmt<'input> {
    pub drop: DROP,
    pub database: DATABASE,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub options: Option<DropDatabaseOptions>,
}

/// `SET TABLESPACE name` — Postgres' dedicated `ALTER DATABASE name
/// SET TABLESPACE name` branch (also used by ALTER INDEX, ALTER MATVIEW,
/// ALTER TABLE). The value is a tablespace name (an `Ident`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetTablespaceClause<'input> {
    pub set: SET,
    pub tablespace: TABLESPACE,
    pub name: crate::tokens::ColId<'input>,
}

/// `RESET TABLESPACE` — Postgres' `AlterDatabaseSetStmt` via
/// `SetResetClause` → `VariableResetStmt` → `RESET var_name` where
/// `var_name` is `TABLESPACE` (a `ColId` / `unreserved_keyword`). pg-sql
/// keeps `TABLESPACE` as a hard keyword, so we model this dedicated form
/// rather than allowing arbitrary `var_name` here. Only the `TABLESPACE`
/// case is exercised by the corpus.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ResetTablespaceClause {
    pub reset: RESET,
    pub tablespace: TABLESPACE,
}

/// `REFRESH COLLATION VERSION` — Postgres'
/// `AlterDatabaseRefreshCollStmt`. Three fixed keywords with no operands.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RefreshCollVersion {
    pub refresh: REFRESH,
    pub collation: COLLATION,
    pub version: VERSION,
}

/// One action on `ALTER DATABASE name action` — covers Postgres'
/// `AlterDatabaseStmt`, `AlterDatabaseRefreshCollStmt`, `RenameStmt` and
/// `AlterOwnerStmt` branches for databases, plus the corpus-exercised
/// `RESET TABLESPACE` form of `AlterDatabaseSetStmt`.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`RENAME`, `OWNER`, `SET`, `RESET`, `REFRESH`), so order is for clarity
/// only. The `[WITH] createdb_opt_list` branch isn't exercised by the
/// pg-sql differential corpus (which only uses a single bare option name
/// for ALTER DATABASE, e.g. `CONNECTION_LIMIT 123`), so we model the
/// single-option form as `WithOpt` — taking one `CreateDbOption` directly,
/// not a `[WITH] (list)`. When a corpus statement uses more than one
/// option or a leading `WITH`, extend this to a struct that wraps a
/// `Vec<CreateDbOption>` plus an optional `WITH` keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterDatabaseAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetTablespace(SetTablespaceClause<'input>),
    ResetTablespace(ResetTablespaceClause),
    RefreshCollVersion(RefreshCollVersion),
    /// A single `createdb_opt_item` (no leading `WITH`). Listed last so
    /// the more specific `SET …`, `RESET …`, `REFRESH …`, `OWNER TO …`,
    /// and `RENAME TO …` branches win when they apply — `CreateDbOption`
    /// starts with an `AliasName` (any bareword) and would otherwise
    /// swallow `OWNER`, `TEMPLATE`, etc.
    WithOpt(CreateDbOption<'input>),
}

/// `ALTER DATABASE name action` — Postgres' `AlterDatabaseStmt`,
/// `AlterDatabaseRefreshCollStmt`, `AlterDatabaseSetStmt`, `RenameStmt`,
/// and `AlterOwnerStmt` branches for databases.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterDatabaseStmt<'input> {
    pub alter: ALTER,
    pub database: DATABASE,
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterDatabaseAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_database_plain() {
        let mut input = crate::tokens::test_input("CREATE DATABASE mydb");
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "mydb");
        assert!(stmt.options.is_empty());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_database_with_options() {
        let mut input = crate::tokens::test_input(
            "CREATE DATABASE mydb ENCODING utf8 LC_COLLATE \"C\" LC_CTYPE \"C\" TEMPLATE template0",
        );
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 4);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_database_with_equals_and_connection_limit() {
        let mut input = crate::tokens::test_input(
            "CREATE DATABASE mydb WITH OWNER = alice CONNECTION LIMIT = 5 IS_TEMPLATE = TRUE",
        );
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap();
        assert!(stmt.with.is_some());
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_database_force() {
        let mut input = crate::tokens::test_input("DROP DATABASE IF EXISTS db1 WITH (FORCE)");
        let stmt = DropDatabaseStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.options.is_some());
        assert!(input.is_empty());
    }
}
