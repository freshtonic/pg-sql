//! DATABASE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

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
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateDbOptName<'input> {
    #[tok(CONNECTION, LIMIT)] ConnectionLimit,
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
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateDbOptValue<'input> {
    #[tok(DEFAULT)] Default,
    Numeric(crate::ast::shared::numbers::NumericOnly<'input>),
    #[tok(TRUE)] True,
    #[tok(FALSE)] False,
    #[tok(ON)] On,
    String(CopySconst<'input>),
    /// `NonReservedWord` — bareword including soft keywords.
    Word(literal::AliasName<'input>),
}

/// A single CREATE DATABASE option — Postgres' `createdb_opt_item`. Options
/// are unordered and repeatable, with an optional `=` between name and value.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateDbOption<'input> {
    pub name: CreateDbOptName<'input>,
    #[tok(optional(EQ), this)]
    pub value: CreateDbOptValue<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateDatabaseStmt<'input> {
    #[tok(CREATE, DATABASE, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(optional(WITH), this)]
    pub options: Vec<CreateDbOption<'input>>,
}

/// A single `DROP DATABASE` option. Postgres currently defines only `FORCE`,
/// but the grammar is comma-separated and extensible.
#[derive(recursa::Node, Debug, Clone)]
pub enum DropDatabaseOption {
    #[tok(FORCE)] Force,
}

/// `[WITH] (option [, ...])` option list on `DROP DATABASE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropDatabaseOptions {
    #[tok(optional(WITH), LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub options:  Vec<DropDatabaseOption > ,
}

/// `DROP DATABASE [IF EXISTS] name [[WITH] (FORCE)]` — no `CASCADE`/`RESTRICT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropDatabaseStmt<'input> {
    #[tok(DROP, DATABASE, this)]
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub options: Option<DropDatabaseOptions>,
}

/// `SET TABLESPACE name` — Postgres' dedicated `ALTER DATABASE name
/// SET TABLESPACE name` branch (also used by ALTER INDEX, ALTER MATVIEW,
/// ALTER TABLE). The value is a tablespace name (an `Ident`).
#[derive(recursa::Node, Debug, Clone)]
pub struct SetTablespaceClause<'input> {
    #[tok(SET, TABLESPACE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `RESET TABLESPACE` — Postgres' `AlterDatabaseSetStmt` via
/// `SetResetClause` → `VariableResetStmt` → `RESET var_name` where
/// `var_name` is `TABLESPACE` (a `ColId` / `unreserved_keyword`). pg-sql
/// keeps `TABLESPACE` as a hard keyword, so we model this dedicated form
/// rather than allowing arbitrary `var_name` here. Only the `TABLESPACE`
/// case is exercised by the corpus.
#[derive(recursa::Node, Debug, Clone)]
pub enum ResetTablespaceClause { #[tok(RESET, TABLESPACE)] Value, }

/// `REFRESH COLLATION VERSION` — Postgres'
/// `AlterDatabaseRefreshCollStmt`. Three fixed keywords with no operands.
#[derive(recursa::Node, Debug, Clone)]
pub enum RefreshCollVersion { #[tok(REFRESH, COLLATION, VERSION)] Value, }

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
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDatabaseStmt<'input> {
    #[tok(ALTER, DATABASE, this)]
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
        let lexed = crate::tokens::lex("CREATE DATABASE mydb");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "mydb");
        assert!(stmt.options.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_database_with_options() {
        let lexed = crate::tokens::lex("CREATE DATABASE mydb ENCODING utf8 LC_COLLATE \"C\" LC_CTYPE \"C\" TEMPLATE template0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.options.len(), 4);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_database_with_equals_and_connection_limit() {
        let lexed = crate::tokens::lex("CREATE DATABASE mydb WITH OWNER = alice CONNECTION LIMIT = 5 IS_TEMPLATE = TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateDatabaseStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with.is_some());
        assert_eq!(stmt.options.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_database_force() {
        let lexed = crate::tokens::lex("DROP DATABASE IF EXISTS db1 WITH (FORCE)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropDatabaseStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
    }
}
