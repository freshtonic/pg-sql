//! DATABASE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// Name of a CREATE DATABASE option — Postgres' `createdb_opt_name`.
    ///
    /// `gram.y` allows a bare IDENT plus a handful of keywords (`CONNECTION
    /// LIMIT`, `ENCODING`, `LOCATION`, `OWNER`, `TABLESPACE`, `TEMPLATE`) that
    /// would otherwise be reserved against the option name. `CreateDbOptWord`
    /// admits exactly the IDENT branch and those single-word keywords; the
    /// IDENT-only names (`is_template`, `allow_connections`, `strategy`,
    /// `locale`, `locale_provider`, `oid`, `icu_locale`, `icu_rules`,
    /// `builtin_locale`, `collation_version`, `lc_collate`, `lc_ctype`) are
    /// not keywords in pg-sql. Admitting every keyword here made `WITH`, `SET`
    /// and `REFRESH` ambiguous between the statement's own syntax and an
    /// option name. `CONNECTION LIMIT` is two tokens and gets its own variant.
    ///
    /// Variant ordering: the two-token `CONNECTION LIMIT` form before the
    /// general word so the longer match wins.
    #[derive(Debug)]
    pub enum CreateDbOptName {
        #[tok(CONNECTION, LIMIT)]
        ConnectionLimit,
        Name(literal::CreateDbOptWord),
    }
}

recursa::ast_node! {
    /// The value of a CREATE DATABASE option — Postgres' `createdb_opt_item`.
    /// Three RHS forms: a number, `opt_boolean_or_string`, or `DEFAULT`.
    ///
    /// Variant ordering: `Default` (keyword) first, then the numbers (digits
    /// or `+`/`-`), then `Boolean` (`TRUE`/`FALSE`/`ON`), then `String`
    /// (quoted), then the catch-all `Word` (bareword incl. `off`,
    /// identifier-like values).
    #[derive(Debug)]
    pub enum CreateDbOptValue {
        #[tok(DEFAULT)]
        Default,
        /// `SignedIconst`, the only number of REL_14_24 gram.y 10203.
        Integer(SignedIconst),
        // Added in 15: the number is `NumericOnly` (REL_15_19 gram.y 11077),
        // so its `FCONST` arms parse, an integer above `int32` among them
        // (research, PostgreSQL 16, "Changes to existing statements",
        // `CREATE DATABASE`; back-patched into 15.1 as commit 2c6d43650d1).
        // A widening: the `SignedIconst` arm records nothing.
        #[config(since = pg15)]
        Float(SignedFconst),
        #[tok(TRUE)]
        True,
        #[tok(FALSE)]
        False,
        #[tok(ON)]
        On,
        String(CopySconst),
        /// `NonReservedWord` — bareword including soft keywords.
        Word(crate::tokens::NonReservedWord),
    }
}

recursa::ast_node! {
    /// A single CREATE DATABASE option — Postgres' `createdb_opt_item`. Options
    /// are unordered and repeatable, with an optional `=` between name and value.
    #[derive(Debug)]
    pub struct CreateDbOption {
        pub name: CreateDbOptName,
        #[tok(optional(EQ), this)]
        pub value: CreateDbOptValue,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CreateDatabaseStmt {
        #[tok(CREATE, DATABASE, this)]
        pub name: crate::tokens::ColId,
        // `this_each` states the element-wise attachment this site already
        // lowers to: the lead-in wraps every option, so
        // `WITH OWNER = x WITH TEMPLATE = y` is accepted where gram.y
        // `CREATE DATABASE name opt_with createdb_opt_list` (REL_17_11 gram.y
        // 11277) reads `WITH` once. RCA3152 asks only that the reading be
        // explicit; the container repair is #59 section B, which is open.
        #[tok(optional(WITH), this_each)]
        pub options: zero_or_many!(CreateDbOption),
    }
}

recursa::ast_node! {
    /// A single `DROP DATABASE` option. Postgres currently defines only `FORCE`,
    /// but the grammar is comma-separated and extensible.
    #[derive(Debug)]
    pub enum DropDatabaseOption {
        #[tok(FORCE)]
        Force,
    }
}

recursa::ast_node! {
    /// `[WITH] (option [, ...])` option list on `DROP DATABASE`.
    #[derive(Debug)]
    pub struct DropDatabaseOptions {
        /// gram.y `drop_option_list`: one or more options.
        ///
        /// `this_each` states the element-wise attachment this site already
        /// lowers to: the delimiters wrap every option, so `(FORCE), (FORCE)`
        /// is accepted where gram.y `DROP DATABASE ... opt_with '('
        /// drop_option_list ')'` (REL_17_11 gram.y 11429, `drop_option_list`
        /// 11440) reads them once. PostgreSQL defines one drop option, so no
        /// multi-element list can be written and the site is latent; #59
        /// section C records that and leaves the container repair open.
        #[tok(optional(WITH), LPAREN, this_each, RPAREN)]
        #[sep(COMMA)]
        pub options: one_or_many!(DropDatabaseOption),
    }
}

recursa::ast_node! {
    /// `DROP DATABASE [IF EXISTS] name [[WITH] (FORCE)]` — no `CASCADE`/`RESTRICT`.
    #[derive(Debug)]
    #[tok(DROP, DATABASE, this)]
    pub struct DropDatabaseStmt {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        pub options: Option<DropDatabaseOptions>,
    }
}

recursa::ast_node! {
    /// `SET TABLESPACE name` — Postgres' dedicated `ALTER DATABASE name
    /// SET TABLESPACE name` branch (also used by ALTER INDEX, ALTER MATVIEW,
    /// ALTER TABLE). The value is a tablespace name (an `Ident`).
    #[derive(Debug)]
    pub struct SetTablespaceClause {
        #[tok(SET, TABLESPACE, this)]
        pub name: crate::tokens::ColId,
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `AlterDatabaseRefreshCollStmt`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `REFRESH COLLATION VERSION` — Postgres'
    /// `AlterDatabaseRefreshCollStmt`. Three fixed keywords with no operands.
    #[derive(Debug)]
    pub enum RefreshCollVersion {
        #[tok(REFRESH, COLLATION, VERSION)]
        Value,
    }
}

recursa::ast_node! {
    /// `WITH createdb_opt_list`, the first arm of PostgreSQL's
    /// `AlterDatabaseStmt` (REL_17_11 gram.y 11349). `createdb_opt_list` can
    /// be empty, so `ALTER DATABASE d WITH` is a statement.
    ///
    /// `WITH` sits on the struct: on the list field it would attach to each
    /// option.
    #[derive(Debug)]
    #[tok(WITH, this)]
    pub struct AlterDatabaseWithOptions {
        pub options: zero_or_many!(CreateDbOption),
    }
}

recursa::ast_node! {
    /// The `createdb_opt_items` of `ALTER DATABASE name createdb_opt_list`,
    /// the second arm of PostgreSQL's `AlterDatabaseStmt`. The empty
    /// `createdb_opt_list` of that arm is an [`AlterDatabaseStmt`] with no
    /// action.
    #[derive(Debug, derive_more::Deref)]
    pub struct AlterDatabaseOptions(#[deref] pub one_or_many!(CreateDbOption));
}

recursa::ast_node! {
    /// One action on `ALTER DATABASE name action` — covers Postgres'
    /// `AlterDatabaseStmt`, `AlterDatabaseRefreshCollStmt`,
    /// `AlterDatabaseSetStmt`, `RenameStmt` and `AlterOwnerStmt` branches for
    /// databases.
    ///
    /// Variant ordering: `SetTablespace` is the dedicated
    /// `AlterDatabaseStmt: ALTER DATABASE name SET TABLESPACE name` branch and
    /// comes before the general `SetReset`, whose `generic_set` would take
    /// `TABLESPACE` as a `var_name`. `With` leads with `WITH`. `Options`
    /// comes last, so the `SET …`, `REFRESH …`, `OWNER TO …` and `RENAME TO
    /// …` branches win when they apply: `createdb_opt_name` names no `SET`,
    /// `REFRESH` or `RENAME`, and only the token after `OWNER` tells an
    /// `OWNER` option from `OWNER TO`.
    #[derive(Debug)]
    pub enum AlterDatabaseAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetTablespace(SetTablespaceClause),
        /// Added in 15: research, PostgreSQL 15, "Changes to existing statements".
        #[config(since = pg15)]
        RefreshCollVersion(RefreshCollVersion),
        /// gram.y `AlterDatabaseSetStmt: ALTER DATABASE name SetResetClause`.
        /// `RESET TABLESPACE` is its `RESET var_name` form.
        SetReset(crate::ast::ddl::role::SetResetClause),
        With(AlterDatabaseWithOptions),
        Options(AlterDatabaseOptions),
    }
}

recursa::ast_node! {
    /// `ALTER DATABASE name [action]` — Postgres' `AlterDatabaseStmt`,
    /// `AlterDatabaseRefreshCollStmt`, `AlterDatabaseSetStmt`, `RenameStmt`,
    /// and `AlterOwnerStmt` branches for databases. With no action, it is the
    /// `ALTER DATABASE name createdb_opt_list` arm with an empty list.
    #[derive(Debug)]
    pub struct AlterDatabaseStmt {
        #[tok(ALTER, DATABASE, this)]
        pub name: crate::tokens::ColId,
        pub action: Option<AlterDatabaseAction>,
    }
}
