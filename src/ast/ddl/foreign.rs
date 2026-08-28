//! FOREIGN DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::table::{AlterTableCmds, CreateGenericOptions};
use crate::ast::ddl::view::RenameColumnClause;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `LIMIT TO (table[, ...]) | EXCEPT (table[, ...])` — Postgres'
/// `import_qualification`. Restricts the imported table set.
///
/// Variant ordering: variants begin with distinct leading keywords
/// (`LIMIT` / `EXCEPT`), so order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ImportQualification<'input> {
    LimitTo(ImportLimitTo<'input>),
    Except(ImportExcept<'input>),
}

/// `LIMIT TO (table[, ...])` — restrict the imported tables to the named
/// set. The table list is `relation_expr_list` in gram.y; corpus
/// statements use plain qualified names only, so we model the list as
/// `Seq1` of `QualifiedName` separated by commas.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ImportLimitTo<'input> {
    pub limit: LIMIT,
    pub to: TO,
    pub names: Surrounded<punct::LParen, Seq1<QualifiedName<'input>, punct::Comma>, punct::RParen>,
}

/// `EXCEPT (table[, ...])` — exclude the named tables from the import.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ImportExcept<'input> {
    pub except: EXCEPT,
    pub names: Surrounded<punct::LParen, Seq1<QualifiedName<'input>, punct::Comma>, punct::RParen>,
}

/// `IMPORT FOREIGN SCHEMA remote [LIMIT TO ... | EXCEPT ...] FROM SERVER
/// server INTO local [OPTIONS (...)]` — Postgres' `ImportForeignSchemaStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct ImportForeignSchemaStmt<'input> {
    pub import: IMPORT,
    pub foreign: FOREIGN,
    pub schema: SCHEMA,
    pub remote: crate::tokens::ColId<'input>,
    pub qualification: Option<ImportQualification<'input>>,
    pub from: FROM,
    pub server: SERVER,
    pub server_name: crate::tokens::ColId<'input>,
    pub into: INTO,
    pub local: crate::tokens::ColId<'input>,
    pub options: Option<crate::ast::ddl::table::CreateGenericOptions<'input>>,
}

/// `TYPE sconst` clause on CREATE SERVER — Postgres' `opt_type`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ServerTypeClause<'input> {
    pub r#type: TYPE,
    pub value: CopySconst<'input>,
}

/// `VERSION { sconst | NULL }` — Postgres' `foreign_server_version`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ServerVersionValue<'input> {
    Null(NULL),
    String(CopySconst<'input>),
}

/// `VERSION value` clause on CREATE/ALTER SERVER.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ServerVersionClause<'input> {
    pub version: VERSION,
    pub value: ServerVersionValue<'input>,
}

/// `FOREIGN DATA WRAPPER name` — the FDW reference on CREATE SERVER and
/// CREATE FOREIGN DATA WRAPPER's own header.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForeignDataWrapperRef<'input> {
    pub foreign: FOREIGN,
    pub data: DATA,
    pub wrapper: WRAPPER,
    pub name: crate::tokens::ColId<'input>,
}

/// `CREATE SERVER [IF NOT EXISTS] name [TYPE sconst]
/// [VERSION { sconst | NULL }] FOREIGN DATA WRAPPER fdw
/// [OPTIONS (...)]` — Postgres' `CreateForeignServerStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateServerStmt<'input> {
    pub create: CREATE,
    pub server: SERVER,
    pub if_not_exists: Option<IfNotExists>,
    pub name: crate::tokens::ColId<'input>,
    pub server_type: Option<ServerTypeClause<'input>>,
    pub version: Option<ServerVersionClause<'input>>,
    pub fdw: ForeignDataWrapperRef<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// `DROP SERVER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropServerStmt<'input> {
    pub drop: DROP,
    pub server: SERVER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterGenericOption<'input> {
    Set(AlterGenericOptionSet<'input>),
    Add(AlterGenericOptionAdd<'input>),
    Drop(AlterGenericOptionDrop<'input>),
    Plain(crate::ast::ddl::table::GenericOption<'input>),
}

/// `SET name 'value'` — the `SET`-prefixed variant of
/// `alter_generic_option_elem`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterGenericOptionSet<'input> {
    pub set: SET,
    pub option: crate::ast::ddl::table::GenericOption<'input>,
}

/// `ADD name 'value'` — the `ADD`-prefixed variant of
/// `alter_generic_option_elem`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterGenericOptionAdd<'input> {
    pub add: ADD,
    pub option: crate::ast::ddl::table::GenericOption<'input>,
}

/// `DROP name` — the `DROP`-prefixed variant of
/// `alter_generic_option_elem`. Unlike `SET` / `ADD` it takes only the
/// option name (a `ColLabel`), with no value.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterGenericOptionDrop<'input> {
    pub drop: DROP,
    pub name: literal::AliasName<'input>,
}

/// `OPTIONS (alter_generic_option_list)` — Postgres'
/// `alter_generic_options`. The `ALTER`-side counterpart of
/// [`CreateGenericOptions`]; differs only in that each element may
/// carry an `ADD` / `SET` / `DROP` prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterGenericOptions<'input> {
    pub options: OPTIONS,
    pub list:
        Surrounded<punct::LParen, Seq1<AlterGenericOption<'input>, punct::Comma>, punct::RParen>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterServerAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    Options(AlterGenericOptions<'input>),
    Version(AlterServerVersionAction<'input>),
}

/// `VERSION value [OPTIONS (...)]` — Postgres' `AlterForeignServerStmt`
/// VERSION branch, with optional trailing generic-options clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
/// [`crate::ast::FileItem::ParseError`] (the differential oracle stays
/// valid because both sides round-trip rejected).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterServerStmt<'input> {
    pub alter: ALTER,
    pub server: SERVER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
/// [`crate::ast::FileItem::ParseError`]; the differential oracle still
/// passes because the round-tripped output is also PG-rejected.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFdwBody<'input> {
    pub data: DATA,
    pub wrapper: WRAPPER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterForeignTableBody<'input> {
    pub table: TABLE,
    pub if_exists: Option<IfExists>,
    pub only: Option<ONLY>,
    pub name: QualifiedName<'input>,
    pub star: Option<punct::Star>,
    pub action: AlterForeignTableAction<'input>,
}

/// What follows `ALTER FOREIGN`: either `DATA WRAPPER ...`
/// (`AlterFdwStmt`) or `TABLE ...` (`AlterForeignTableStmt`).
/// Discriminated by the first post-`FOREIGN` token (`DATA` vs `TABLE`);
/// the two first-tokens are disjoint so peek order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterForeignBody<'input> {
    Fdw(AlterFdwBody<'input>),
    Table(AlterForeignTableBody<'input>),
}

/// `ALTER FOREIGN ...` umbrella statement covering
/// `ALTER FOREIGN DATA WRAPPER ...` (Postgres' `AlterFdwStmt`) and
/// `ALTER FOREIGN TABLE ...` (Postgres' `AlterForeignTableStmt`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterForeignStmt<'input> {
    pub alter: ALTER,
    pub foreign: FOREIGN,
    pub body: AlterForeignBody<'input>,
}

/// One repeatable FDW handler/validator option — Postgres' `fdw_option`.
///
/// Variant ordering: the two-token `NO HANDLER` / `NO VALIDATOR` forms
/// come before their single-token counterparts so longest-match-wins
/// picks the `NO`-prefixed spelling first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FdwOption<'input> {
    NoHandler((NO, HANDLER)),
    NoValidator((NO, VALIDATOR)),
    Handler(FdwHandlerOption<'input>),
    Validator(FdwValidatorOption<'input>),
}

/// `HANDLER handler_name` — Postgres' `fdw_option` HANDLER branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FdwHandlerOption<'input> {
    pub handler: HANDLER,
    pub name: QualifiedName<'input>,
}

/// `VALIDATOR handler_name` — Postgres' `fdw_option` VALIDATOR branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FdwValidatorOption<'input> {
    pub validator: VALIDATOR,
    pub name: QualifiedName<'input>,
}

/// `CREATE FOREIGN DATA WRAPPER name [HANDLER ... | NO HANDLER]
/// [VALIDATOR ... | NO VALIDATOR] [OPTIONS (...)]` — the body of
/// `CREATE FOREIGN DATA WRAPPER ...` after the `CREATE FOREIGN` head.
///
/// The fdw_options list is order-free and separator-free; we model it
/// with `Vec<FdwOption>` so it stops at the first non-option token
/// (OPTIONS or end-of-statement).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateFdwBody<'input> {
    pub data: DATA,
    pub wrapper: WRAPPER,
    pub name: crate::tokens::ColId<'input>,
    pub fdw_options: Vec<FdwOption<'input>>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// `SERVER name` reference on CREATE FOREIGN TABLE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForeignTableServerClause<'input> {
    pub server: SERVER,
    pub name: crate::tokens::ColId<'input>,
}

/// Body of `CREATE FOREIGN TABLE name (cols) [INHERITS (...)] SERVER name
/// [OPTIONS (...)]` — Postgres' columns form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForeignTableColumnsBody<'input> {
    pub columns: Surrounded<
        punct::LParen,
        recursa::seq::Seq0<crate::ast::ddl::table::ColumnOrConstraint<'input>, punct::Comma>,
        punct::RParen,
    >,
    pub inherits: Option<crate::ast::ddl::table::InheritsClause<'input>>,
    pub server: ForeignTableServerClause<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// Body of `CREATE FOREIGN TABLE name PARTITION OF parent [(opts)]
/// { FOR VALUES ... | DEFAULT } SERVER name [OPTIONS (...)]` — Postgres'
/// partition form. The bound is `for_values: Option` of `ForValuesClause`
/// OR `default: Option` of `DEFAULT` (exactly one of the two should be
/// `Some` for a syntactically valid statement).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForeignTablePartitionBody<'input> {
    pub partition: PARTITION,
    pub of: OF,
    pub parent: QualifiedName<'input>,
    pub column_options: Option<
        Surrounded<
            punct::LParen,
            recursa::seq::Seq0<crate::ast::ddl::table::PartitionColumnOption<'input>, punct::Comma>,
            punct::RParen,
        >,
    >,
    pub for_values: Option<crate::ast::ddl::table::ForValuesClause<'input>>,
    pub default: Option<DEFAULT>,
    pub server: ForeignTableServerClause<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// The body of `CREATE FOREIGN TABLE name ...` — either the columns
/// form `(cols) [INHERITS (...)] SERVER ...` or the partition form
/// `PARTITION OF parent [(opts)] FOR VALUES ... SERVER ...`.
///
/// Variant ordering: `Partition` (`PARTITION` keyword) is listed before
/// `Columns` (which starts with `(`); the two have disjoint first tokens.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreateForeignTableBody<'input> {
    Partition(ForeignTablePartitionBody<'input>),
    Columns(ForeignTableColumnsBody<'input>),
}

/// `TABLE [IF NOT EXISTS] name body` — the body of
/// `CREATE FOREIGN TABLE ...` after the `CREATE FOREIGN` head.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateForeignTableBodyStmt<'input> {
    pub table: TABLE,
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    pub body: CreateForeignTableBody<'input>,
}

/// What follows `CREATE FOREIGN`: either `DATA WRAPPER ...` (CreateFdwStmt)
/// or `TABLE ...` (CreateForeignTableStmt). Discriminated by the first
/// post-`FOREIGN` token (`DATA` vs `TABLE`); both first tokens are
/// disjoint so peek order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CreateForeignBody<'input> {
    Fdw(CreateFdwBody<'input>),
    Table(CreateForeignTableBodyStmt<'input>),
}

/// `CREATE FOREIGN ...` umbrella statement covering both
/// `CREATE FOREIGN DATA WRAPPER ...` and `CREATE FOREIGN TABLE ...`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateForeignStmt<'input> {
    pub create: CREATE,
    pub foreign: FOREIGN,
    pub body: CreateForeignBody<'input>,
}

/// The object kind after `DROP FOREIGN`: `DATA WRAPPER` or `TABLE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ForeignObjectKind {
    DataWrapper((DATA, WRAPPER)),
    Table(TABLE),
}

/// `DROP FOREIGN {DATA WRAPPER | TABLE} [IF EXISTS] name [, ...]
/// [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropForeignStmt<'input> {
    pub drop: DROP,
    pub foreign: FOREIGN,
    pub kind: ForeignObjectKind,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_foreign_data_wrapper() {
        let mut input = crate::tokens::test_input("DROP FOREIGN DATA WRAPPER fdw1");
        let stmt = DropForeignStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.kind, ForeignObjectKind::DataWrapper(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_foreign_table() {
        let mut input = crate::tokens::test_input("DROP FOREIGN TABLE IF EXISTS ft1, ft2");
        let stmt = DropForeignStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.kind, ForeignObjectKind::Table(_)));
        assert_eq!(stmt.names.len(), 2);
        assert!(input.is_empty());
    }

    /// Bare `ALTER FOREIGN DATA WRAPPER name` — PG itself rejects this
    /// (`AlterFdwStmt` requires at least one fdw_option or
    /// alter_generic_options), but the parser is over-permissive to avoid
    /// surfacing as a [`crate::ast::FileItem::ParseError`]; the differential
    /// oracle accepts the PG-rejected case because pg-sql's reformat is
    /// also PG-rejected.
    #[test]
    fn parse_alter_fdw_bare() {
        let mut input = crate::tokens::test_input("ALTER FOREIGN DATA WRAPPER foo");
        let _stmt = AlterForeignStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// Bare `ALTER SERVER name` — similar over-permissive acceptance for
    /// the bare form (gram.y `AlterForeignServerStmt` requires version,
    /// options, or both).
    #[test]
    fn parse_alter_server_bare() {
        let mut input = crate::tokens::test_input("ALTER SERVER s0");
        let _stmt = AlterServerStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn create_foreign_data_wrapper_bare_roundtrips() {
        let stmt: CreateForeignStmt = parse_stmt("CREATE FOREIGN DATA WRAPPER foo");
        if let CreateForeignBody::Fdw(b) = &stmt.body {
            assert_eq!(b.name.text(), "foo");
            assert!(b.fdw_options.is_empty());
            assert!(b.options.is_none());
        } else {
            panic!("expected Fdw body");
        }
        reparse_stable::<CreateForeignStmt>("CREATE FOREIGN DATA WRAPPER foo");
    }

    #[test]
    fn create_foreign_data_wrapper_handler_validator_options_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN DATA WRAPPER test_fdw HANDLER test_fdw_handler VALIDATOR postgresql_fdw_validator OPTIONS (testing '1', another '2')",
        );
    }

    #[test]
    fn create_foreign_data_wrapper_no_handler_no_validator_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN DATA WRAPPER foo NO HANDLER NO VALIDATOR",
        );
    }

    #[test]
    fn create_foreign_table_columns_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft2 (c1 integer NOT NULL, c2 text, c3 date) SERVER s0 OPTIONS (delimiter ',', quote '\"', \"be quoted\" 'value')",
        );
    }

    #[test]
    fn create_foreign_table_with_column_options_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft1 (c1 integer OPTIONS (\"param 1\" 'val1') NOT NULL, c2 text OPTIONS (param2 'val2') CHECK (c2 <> ''), c3 date) SERVER s0 OPTIONS (delimiter ',')",
        );
    }

    #[test]
    fn create_foreign_table_inherits_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft2 () INHERITS (fd_pt1) SERVER s0 OPTIONS (delimiter ',')",
        );
    }

    #[test]
    fn create_foreign_table_partition_of_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE ft_part1 PARTITION OF lt1 FOR VALUES FROM (0) TO (1000) SERVER s0",
        );
    }

    #[test]
    fn create_foreign_table_if_not_exists_roundtrips() {
        reparse_stable::<CreateForeignStmt>(
            "CREATE FOREIGN TABLE IF NOT EXISTS ft1 (a INT) SERVER s0",
        );
    }

    #[test]
    fn create_server_minimal_roundtrips() {
        let stmt: CreateServerStmt = parse_stmt("CREATE SERVER s1 FOREIGN DATA WRAPPER foo");
        assert_eq!(stmt.name.text(), "s1");
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.server_type.is_none());
        assert!(stmt.version.is_none());
        assert_eq!(stmt.fdw.name.text(), "foo");
        assert!(stmt.options.is_none());
        reparse_stable::<CreateServerStmt>("CREATE SERVER s1 FOREIGN DATA WRAPPER foo");
    }

    #[test]
    fn create_server_if_not_exists_roundtrips() {
        reparse_stable::<CreateServerStmt>(
            "CREATE SERVER IF NOT EXISTS s1 FOREIGN DATA WRAPPER foo",
        );
    }

    #[test]
    fn create_server_type_version_options_roundtrips() {
        reparse_stable::<CreateServerStmt>(
            "CREATE SERVER s7 TYPE 'oracle' VERSION '17.0' FOREIGN DATA WRAPPER foo OPTIONS (host 'a', dbname 'b')",
        );
    }

    #[test]
    fn create_server_version_null_roundtrips() {
        reparse_stable::<CreateServerStmt>("CREATE SERVER s VERSION NULL FOREIGN DATA WRAPPER foo");
    }
}
