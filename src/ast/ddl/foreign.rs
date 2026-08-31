//! FOREIGN DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::table::{AlterTableCmds, CreateGenericOptions};
use crate::ast::ddl::view::RenameColumnClause;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::{literal, punct};

/// `LIMIT TO (table[, ...]) | EXCEPT (table[, ...])` — Postgres'
/// `import_qualification`. Restricts the imported table set.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`LIMIT` / `EXCEPT`), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum ImportQualification<'input> {
    LimitTo(ImportLimitTo<'input>),
    Except(ImportExcept<'input>),
}

/// `LIMIT TO (table[, ...])` — restrict the imported tables to the named
/// set. The table list is `relation_expr_list` in gram.y; corpus
/// statements use plain qualified names only, so we model the list as
/// `Seq1` of `QualifiedName` separated by commas.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LIMIT, TO, LPAREN, this, RPAREN)]
pub struct ImportLimitTo<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<QualifiedName<'input>>,
}

/// `EXCEPT (table[, ...])` — exclude the named tables from the import.
#[derive(recursa::Node, Debug, Clone)]
#[tok(EXCEPT, LPAREN, this, RPAREN)]
pub struct ImportExcept<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<QualifiedName<'input>>,
}

/// `IMPORT FOREIGN SCHEMA remote [LIMIT TO ... | EXCEPT ...] FROM SERVER
/// server INTO local [OPTIONS (...)]` — Postgres' `ImportForeignSchemaStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ImportForeignSchemaStmt<'input> {
    #[tok(IMPORT, FOREIGN, SCHEMA, this)]
    pub remote: crate::tokens::ColId<'input>,
    pub qualification: Option<ImportQualification<'input>>,
    #[tok(FROM, SERVER, this)]
    pub server_name: crate::tokens::ColId<'input>,
    #[tok(INTO, this)]
    pub local: crate::tokens::ColId<'input>,
    pub options: Option<crate::ast::ddl::table::CreateGenericOptions<'input>>,
}

/// `TYPE sconst` clause on CREATE SERVER — Postgres' `opt_type`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ServerTypeClause<'input> {
    #[tok(TYPE, this)]
    pub value: CopySconst<'input>,
}

/// `VERSION { sconst | NULL }` — Postgres' `foreign_server_version`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ServerVersionValue<'input> {
    #[tok(NULL)]
    Null,
    String(CopySconst<'input>),
}

/// `VERSION value` clause on CREATE/ALTER SERVER.
#[derive(recursa::Node, Debug, Clone)]
pub struct ServerVersionClause<'input> {
    #[tok(VERSION, this)]
    pub value: ServerVersionValue<'input>,
}

/// `FOREIGN DATA WRAPPER name` — the FDW reference on CREATE SERVER and
/// CREATE FOREIGN DATA WRAPPER's own header.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForeignDataWrapperRef<'input> {
    #[tok(FOREIGN, DATA, WRAPPER, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `CREATE SERVER [IF NOT EXISTS] name [TYPE sconst]
/// [VERSION { sconst | NULL }] FOREIGN DATA WRAPPER fdw
/// [OPTIONS (...)]` — Postgres' `CreateForeignServerStmt`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, SERVER, this)]
pub struct CreateServerStmt<'input> {
    pub if_not_exists: Option<IfNotExists>,
    pub name: crate::tokens::ColId<'input>,
    pub server_type: Option<ServerTypeClause<'input>>,
    pub version: Option<ServerVersionClause<'input>>,
    pub fdw: ForeignDataWrapperRef<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// `DROP SERVER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, SERVER, this)]
pub struct DropServerStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// One entry in an `alter_generic_options` (`OPTIONS (...)`) list on
/// ALTER FOREIGN DATA WRAPPER / ALTER SERVER / ALTER USER MAPPING — the
/// `ALTER`-side counterpart of [`GenericOption`].
///
/// Postgres' `alter_generic_option_elem` adds three action prefixes
/// (`ADD`, `SET`, `DROP`) to the plain `generic_option_elem`:
///
/// ```text
/// alter_generic_option_elem
///     : generic_option_elem            -- "name 'value'"
///     | SET   generic_option_elem      -- "SET name 'value'"
///     | ADD   generic_option_elem      -- "ADD name 'value'"
///     | DROP  generic_option_name      -- "DROP name"
/// ```
///
/// Variant ordering: the keyword-prefixed forms come first (`Set`,
/// `Add`, `Drop`) before the bare [`GenericOption`] (which starts with
/// a `ColLabel` identifier). The three prefixed variants have disjoint
/// first tokens, so order among them is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterGenericOption<'input> {
    Set(AlterGenericOptionSet<'input>),
    Add(AlterGenericOptionAdd<'input>),
    Drop(AlterGenericOptionDrop<'input>),
    Plain(crate::ast::ddl::table::GenericOption<'input>),
}

/// `SET name 'value'` — the `SET`-prefixed variant of
/// `alter_generic_option_elem`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterGenericOptionSet<'input> {
    #[tok(SET, this)]
    pub option: crate::ast::ddl::table::GenericOption<'input>,
}

/// `ADD name 'value'` — the `ADD`-prefixed variant of
/// `alter_generic_option_elem`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterGenericOptionAdd<'input> {
    #[tok(ADD, this)]
    pub option: crate::ast::ddl::table::GenericOption<'input>,
}

/// `DROP name` — the `DROP`-prefixed variant of
/// `alter_generic_option_elem`. Unlike `SET` / `ADD` it takes only the
/// option name (a `ColLabel`), with no value.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterGenericOptionDrop<'input> {
    #[tok(DROP, this)]
    pub name: literal::AliasName<'input>,
}

/// `OPTIONS (alter_generic_option_list)` — Postgres'
/// `alter_generic_options`. The `ALTER`-side counterpart of
/// [`CreateGenericOptions`]; differs only in that each element may
/// carry an `ADD` / `SET` / `DROP` prefix.
#[derive(recursa::Node, Debug, Clone)]
#[tok(OPTIONS, LPAREN, this, RPAREN)]
pub struct AlterGenericOptions<'input> {
    #[sep(COMMA)]
    pub list: recursa::Vec1<AlterGenericOption<'input>>,
}

/// One action on `ALTER SERVER name action` — covers Postgres'
/// `AlterForeignServerStmt` (VERSION-only, VERSION+OPTIONS, OPTIONS-only)
/// plus the `RENAME TO` / `OWNER TO` branches from `RenameStmt` /
/// `AlterOwnerStmt`.
///
/// Variant ordering: keyword-distinct branches first (`Rename`,
/// `Owner`, `Options`). The `Version` branch covers both `VERSION
/// sconst` (bare) and `VERSION sconst OPTIONS (...)` (with trailing
/// generic-options clause). Modelling both forms as one struct with
/// an `Option<AlterGenericOptions>` tail keeps recursa's first-set
/// prefix dispatch single-token (just `VERSION`), since two
/// variants sharing the `VERSION` prefix would otherwise need
/// multi-token first-set lookahead that doesn't account for the
/// version literal between `VERSION` and `OPTIONS`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterServerAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    Options(AlterGenericOptions<'input>),
    Version(AlterServerVersionAction<'input>),
}

/// `VERSION value [OPTIONS (...)]` — Postgres' `AlterForeignServerStmt`
/// VERSION branch, with optional trailing generic-options clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterServerVersionAction<'input> {
    pub version: ServerVersionClause<'input>,
    pub options: Option<AlterGenericOptions<'input>>,
}

/// `ALTER SERVER name action` — Postgres' `AlterForeignServerStmt`
/// plus the foreign-server branches of `RenameStmt` / `AlterOwnerStmt`.
///
/// `action` is `Option` for the same reason as [`AlterFdwBody`]: gram.y's
/// `AlterForeignServerStmt` requires at least one of `version` or
/// `options`, so the bare `ALTER SERVER name` is a syntax error in PG, but
/// the parser accepts it to avoid a
/// file-level parse error (the differential oracle stays
/// valid because both sides round-trip rejected).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterServerStmt<'input> {
    #[tok(ALTER, SERVER, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: Option<AlterServerAction<'input>>,
}

/// `fdw_option ...  [alter_generic_options]` — the
/// HANDLER/NO HANDLER/VALIDATOR/NO VALIDATOR action on
/// `ALTER FOREIGN DATA WRAPPER name ...`, optionally followed by a
/// trailing `OPTIONS (...)` clause. Matches Postgres'
/// `AlterFdwStmt: ALTER FOREIGN DATA WRAPPER name opt_fdw_options
/// alter_generic_options | ALTER FOREIGN DATA WRAPPER name fdw_options`
/// for the branches that begin with `HANDLER` / `NO` / `VALIDATOR`.
///
/// `head` is a single mandatory `FdwOption` (so this variant has a
/// concrete first-set: `HANDLER` | `NO` | `VALIDATOR`); `rest` collects
/// any further fdw_options; `generic` is the optional trailing
/// `alter_generic_options` (`OPTIONS (...)`).
///
/// The case where `alter_generic_options` is the *only* clause (no
/// leading fdw_options) is modelled by the sibling
/// [`AlterFdwAction::GenericOpts`] variant. Splitting these two avoids
/// a struct whose first field is an empty-allowed `Vec<FdwOption>` —
/// such a struct has an empty first-set and breaks enum peek dispatch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFdwOptsAction<'input> {
    pub head: FdwOption<'input>,
    pub rest: Vec<FdwOption<'input>>,
    pub generic: Option<AlterGenericOptions<'input>>,
}

/// One action on `ALTER FOREIGN DATA WRAPPER name action` — covers
/// Postgres' `AlterFdwStmt` plus the `RENAME TO` / `OWNER TO` branches
/// from `RenameStmt` / `AlterOwnerStmt`.
///
/// Variant ordering: each variant has disjoint first tokens.
/// - `Rename` — `RENAME`
/// - `Owner` — `OWNER`
/// - `FdwOpts` — `HANDLER` | `NO` | `VALIDATOR` (one or more
///   `fdw_option`s, optionally followed by `OPTIONS (...)`)
/// - `GenericOpts` — `OPTIONS` (`alter_generic_options` alone, no
///   leading fdw_options)
///
/// The `alter_generic_options` and `fdw_options` clauses are split into
/// two variants instead of one struct with an optional Vec, because a
/// struct whose first field is a possibly-empty `Vec<FdwOption>` has an
/// empty first-set and the enum's combined peek regex can't dispatch on
/// `OPTIONS`. Splitting gives each variant a concrete first-set.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterFdwAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    FdwOpts(AlterFdwOptsAction<'input>),
    GenericOpts(AlterGenericOptions<'input>),
}

/// Body of `ALTER FOREIGN DATA WRAPPER name action` — Postgres'
/// `AlterFdwStmt` family. The body starts at `DATA WRAPPER` (the
/// `ALTER FOREIGN` prefix lives on the outer [`AlterForeignStmt`]).
///
/// `gram.y` requires at least one of `fdw_options` or
/// `alter_generic_options` (or one of the RENAME/OWNER branches),
/// so `ALTER FOREIGN DATA WRAPPER foo;` is a syntax error in PG.
/// The `action` slot is `Option` so the bare form parses into the
/// structured AST rather than surfacing as a
/// file-level parse error; the differential oracle still
/// passes because the round-tripped output is also PG-rejected.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFdwBody<'input> {
    #[tok(DATA, WRAPPER, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: Option<AlterFdwAction<'input>>,
}

/// One action on `ALTER FOREIGN TABLE [IF EXISTS] name action` —
/// Postgres' `alter_table_cmds` for FOREIGN TABLE plus the foreign-table
/// branches of `RenameStmt` (`RENAME TO new`, `RENAME [COLUMN] old TO
/// new`) and `AlterObjectSchemaStmt` (`SET SCHEMA new`).
///
/// `gram.y` line 2284: `ALTER FOREIGN TABLE [IF EXISTS] relation_expr
/// alter_table_cmds`. The `alter_table_cmd` grammar is the superset
/// shared with ALTER TABLE — see [`AlterTableCmd`] for the full action
/// set, including `ADD/DROP/ALTER COLUMN`, `ADD/DROP CONSTRAINT`,
/// `OWNER TO`, `INHERIT`/`NO INHERIT`, `ENABLE/DISABLE TRIGGER`,
/// `OPTIONS (...)`, etc. Foreign-table-specific actions like the
/// column-level `OPTIONS (...)` (`AT_AlterColumnGenericOptions`,
/// gram.y line 2623) are already modelled inside [`AlterColumnAction`].
///
/// Variant ordering: longer/more-specific prefixes first within the
/// shared `RENAME …` family.
/// - `RenameColumn` (`RENAME [COLUMN] old TO new`) — `RENAME ident …` or
///   `RENAME COLUMN …`
/// - `Rename` (`RENAME TO new`) — `RENAME TO …`
/// - `SetSchema` (`SET SCHEMA …`) — disjoint from every `SET …` action
///   inside `alter_table_cmd` (which all use different 2nd tokens).
/// - `Cmds` last — the comma-separated `alter_table_cmds` catch-all.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterForeignTableAction<'input> {
    RenameColumn(RenameColumnClause<'input>),
    Rename(RenameTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    Cmds(AlterTableCmds<'input>),
}

/// Body of `ALTER FOREIGN TABLE [IF EXISTS] name action` — Postgres'
/// `AlterForeignTableStmt`. Reuses the per-relation [`AlterTableCmds`]
/// inside [`AlterForeignTableAction::Cmds`] so the full ALTER TABLE
/// action set applies to foreign tables. See [`AlterForeignTableAction`].
///
/// `relation_expr` in gram.y permits `name`, `ONLY name`, `ONLY (name)`,
/// and `name *`. The pg-sql corpus only exercises the bare and qualified
/// forms on ALTER FOREIGN TABLE, but we still accept `ONLY`/`*` for
/// grammar fidelity — the same shape used by `AlterTableSingle` for
/// regular tables.
#[derive(recursa::Node, Debug, Clone)]
#[tok(TABLE, this)]
pub struct AlterForeignTableBody<'input> {
    pub if_exists: Option<IfExists>,
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub action: AlterForeignTableAction<'input>,
}

/// What follows `ALTER FOREIGN`: either `DATA WRAPPER ...`
/// (`AlterFdwStmt`) or `TABLE ...` (`AlterForeignTableStmt`).
/// Discriminated by the first post-`FOREIGN` token (`DATA` vs `TABLE`);
/// the two first-tokens are disjoint so peek order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterForeignBody<'input> {
    Fdw(AlterFdwBody<'input>),
    Table(AlterForeignTableBody<'input>),
}

/// `ALTER FOREIGN ...` umbrella statement covering
/// `ALTER FOREIGN DATA WRAPPER ...` (Postgres' `AlterFdwStmt`) and
/// `ALTER FOREIGN TABLE ...` (Postgres' `AlterForeignTableStmt`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterForeignStmt<'input> {
    #[tok(ALTER, FOREIGN, this)]
    pub body: AlterForeignBody<'input>,
}

/// One repeatable FDW handler/validator option — Postgres' `fdw_option`.
///
/// Variant ordering: the two-token `NO HANDLER` / `NO VALIDATOR` forms
/// come before their single-token counterparts so longest-match-wins
/// picks the `NO`-prefixed spelling first.
#[derive(recursa::Node, Debug, Clone)]
pub enum FdwOption<'input> {
    #[tok(NO, HANDLER)]
    NoHandler,
    #[tok(NO, VALIDATOR)]
    NoValidator,
    Handler(FdwHandlerOption<'input>),
    Validator(FdwValidatorOption<'input>),
}

/// `HANDLER handler_name` — Postgres' `fdw_option` HANDLER branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct FdwHandlerOption<'input> {
    #[tok(HANDLER, this)]
    pub name: QualifiedName<'input>,
}

/// `VALIDATOR handler_name` — Postgres' `fdw_option` VALIDATOR branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct FdwValidatorOption<'input> {
    #[tok(VALIDATOR, this)]
    pub name: QualifiedName<'input>,
}

/// `CREATE FOREIGN DATA WRAPPER name [HANDLER ... | NO HANDLER]
/// [VALIDATOR ... | NO VALIDATOR] [OPTIONS (...)]` — the body of
/// `CREATE FOREIGN DATA WRAPPER ...` after the `CREATE FOREIGN` head.
///
/// The fdw_options list is order-free and separator-free; we model it
/// with `Vec<FdwOption>` so it stops at the first non-option token
/// (OPTIONS or end-of-statement).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateFdwBody<'input> {
    #[tok(DATA, WRAPPER, this)]
    pub name: crate::tokens::ColId<'input>,
    pub fdw_options: Vec<FdwOption<'input>>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// `SERVER name` reference on CREATE FOREIGN TABLE.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForeignTableServerClause<'input> {
    #[tok(SERVER, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// Body of `CREATE FOREIGN TABLE name (cols) [INHERITS (...)] SERVER name
/// [OPTIONS (...)]` — Postgres' columns form.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForeignTableColumnsBody<'input> {
    pub columns: ForeignTableColumnList<'input>,
    pub inherits: Option<crate::ast::ddl::table::InheritsClause<'input>>,
    pub server: ForeignTableServerClause<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// Parenthesized, comma-separated column and constraint list on a foreign
/// table. The legacy grammar used `Seq0`, so the empty `()` form remains
/// accepted (notably before an `INHERITS` clause).
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ForeignTableColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<crate::ast::ddl::table::ColumnOrConstraint<'input>>,
);

/// Body of `CREATE FOREIGN TABLE name PARTITION OF parent [(opts)]
/// { FOR VALUES ... | DEFAULT } SERVER name [OPTIONS (...)]` — Postgres'
/// partition form. The bound is `for_values: Option` of `ForValuesClause`
/// OR `default: Option` of `DEFAULT` (exactly one of the two should be
/// `Some` for a syntactically valid statement).
#[derive(recursa::Node, Debug, Clone)]
pub struct ForeignTablePartitionBody<'input> {
    #[tok(PARTITION, OF, this)]
    pub parent: QualifiedName<'input>,
    pub column_options: Option<ForeignTablePartitionColumnOptions<'input>>,
    pub for_values: Option<crate::ast::ddl::table::ForValuesClause<'input>>,
    #[presence(DEFAULT)]
    pub default: bool,
    pub server: ForeignTableServerClause<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// Optional parenthesized partition-column option list on a foreign table.
/// This is a zero-or-more list to preserve the legacy `Seq0` cardinality.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ForeignTablePartitionColumnOptions<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<crate::ast::ddl::table::PartitionColumnOption<'input>>,
);

/// The body of `CREATE FOREIGN TABLE name ...` — either the columns
/// form `(cols) [INHERITS (...)] SERVER ...` or the partition form
/// `PARTITION OF parent [(opts)] FOR VALUES ... SERVER ...`.
///
/// Variant ordering: `Partition` (`PARTITION` keyword) is listed before
/// `Columns` (which starts with `(`); the two have disjoint first tokens.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateForeignTableBody<'input> {
    Partition(ForeignTablePartitionBody<'input>),
    Columns(ForeignTableColumnsBody<'input>),
}

/// `TABLE [IF NOT EXISTS] name body` — the body of
/// `CREATE FOREIGN TABLE ...` after the `CREATE FOREIGN` head.
#[derive(recursa::Node, Debug, Clone)]
#[tok(TABLE, this)]
pub struct CreateForeignTableBodyStmt<'input> {
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    pub body: CreateForeignTableBody<'input>,
}

/// What follows `CREATE FOREIGN`: either `DATA WRAPPER ...` (CreateFdwStmt)
/// or `TABLE ...` (CreateForeignTableStmt). Discriminated by the first
/// post-`FOREIGN` token (`DATA` vs `TABLE`); both first tokens are
/// disjoint so peek order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateForeignBody<'input> {
    Fdw(CreateFdwBody<'input>),
    Table(CreateForeignTableBodyStmt<'input>),
}

/// `CREATE FOREIGN ...` umbrella statement covering both
/// `CREATE FOREIGN DATA WRAPPER ...` and `CREATE FOREIGN TABLE ...`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateForeignStmt<'input> {
    #[tok(CREATE, FOREIGN, this)]
    pub body: CreateForeignBody<'input>,
}

/// The object kind after `DROP FOREIGN`: `DATA WRAPPER` or `TABLE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ForeignObjectKind {
    #[tok(DATA, WRAPPER)]
    DataWrapper,
    #[tok(TABLE)]
    Table,
}

/// `DROP FOREIGN {DATA WRAPPER | TABLE} [IF EXISTS] name [, ...]
/// [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropForeignStmt<'input> {
    #[tok(DROP, FOREIGN, this)]
    pub kind: ForeignObjectKind,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/foreign.tests.rs"
));
