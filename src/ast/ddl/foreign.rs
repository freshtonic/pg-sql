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

recursa::ast_node! {
    /// `LIMIT TO (table[, ...]) | EXCEPT (table[, ...])` — Postgres'
    /// `import_qualification`. Restricts the imported table set.
    ///
    /// Variant ordering: variants begin with distinct leading keywords
    /// (`LIMIT` / `EXCEPT`), so order is for clarity only.
    #[derive(Debug)]
    pub enum ImportQualification {
        LimitTo(ImportLimitTo),
        Except(ImportExcept),
    }
}

recursa::ast_node! {
    /// `LIMIT TO (table[, ...])` — restrict the imported tables to the named
    /// set. The table list is `relation_expr_list` in gram.y; corpus
    /// statements use plain qualified names only, so we model the list as
    /// `Seq1` of `QualifiedName` separated by commas.
    #[derive(Debug)]
    #[tok(LIMIT, TO, LPAREN, this, RPAREN)]
    pub struct ImportLimitTo {
        #[sep(COMMA)]
        pub names: one_or_many!(QualifiedName),
    }
}

recursa::ast_node! {
    /// `EXCEPT (table[, ...])` — exclude the named tables from the import.
    #[derive(Debug)]
    #[tok(EXCEPT, LPAREN, this, RPAREN)]
    pub struct ImportExcept {
        #[sep(COMMA)]
        pub names: one_or_many!(QualifiedName),
    }
}

recursa::ast_node! {
    /// `IMPORT FOREIGN SCHEMA remote [LIMIT TO ... | EXCEPT ...] FROM SERVER
    /// server INTO local [OPTIONS (...)]` — Postgres' `ImportForeignSchemaStmt`.
    #[derive(Debug)]
    pub struct ImportForeignSchemaStmt {
        #[tok(IMPORT, FOREIGN, SCHEMA, this)]
        pub remote: crate::tokens::ColId,
        pub qualification: Option<ImportQualification>,
        #[tok(FROM, SERVER, this)]
        pub server_name: crate::tokens::ColId,
        #[tok(INTO, this)]
        pub local: crate::tokens::ColId,
        pub options: Option<crate::ast::ddl::table::CreateGenericOptions>,
    }
}

recursa::ast_node! {
    /// `TYPE sconst` clause on CREATE SERVER — Postgres' `opt_type`.
    #[derive(Debug)]
    pub struct ServerTypeClause {
        #[tok(TYPE, this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `VERSION { sconst | NULL }` — Postgres' `foreign_server_version`.
    #[derive(Debug)]
    pub enum ServerVersionValue {
        #[tok(NULL)]
        Null,
        String(CopySconst),
    }
}

recursa::ast_node! {
    /// `VERSION value` clause on CREATE/ALTER SERVER.
    #[derive(Debug)]
    pub struct ServerVersionClause {
        #[tok(VERSION, this)]
        pub value: ServerVersionValue,
    }
}

recursa::ast_node! {
    /// `FOREIGN DATA WRAPPER name` — the FDW reference on CREATE SERVER and
    /// CREATE FOREIGN DATA WRAPPER's own header.
    #[derive(Debug)]
    pub struct ForeignDataWrapperRef {
        #[tok(FOREIGN, DATA, WRAPPER, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `CREATE SERVER [IF NOT EXISTS] name [TYPE sconst]
    /// [VERSION { sconst | NULL }] FOREIGN DATA WRAPPER fdw
    /// [OPTIONS (...)]` — Postgres' `CreateForeignServerStmt`.
    #[derive(Debug)]
    #[tok(CREATE, SERVER, this)]
    pub struct CreateServerStmt {
        pub if_not_exists: Option<IfNotExists>,
        pub name: crate::tokens::ColId,
        pub server_type: Option<ServerTypeClause>,
        pub version: Option<ServerVersionClause>,
        pub fdw: ForeignDataWrapperRef,
        pub options: Option<CreateGenericOptions>,
    }
}

recursa::ast_node! {
    /// `DROP SERVER [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, SERVER, this)]
    pub struct DropServerStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One entry in an `alter_generic_options` (`OPTIONS (...)`) list on
    /// ALTER FOREIGN DATA WRAPPER / ALTER SERVER / ALTER USER MAPPING — the
    /// `ALTER`-side counterpart of [`GenericOption`](crate::ast::ddl::table::GenericOption).
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
    /// `Add`, `Drop`) before the bare
    /// [`GenericOption`](crate::ast::ddl::table::GenericOption) (which starts with
    /// a `ColLabel` identifier). The three prefixed variants have disjoint
    /// first tokens, so order among them is for clarity only.
    #[derive(Debug)]
    pub enum AlterGenericOption {
        Set(AlterGenericOptionSet),
        Add(AlterGenericOptionAdd),
        Drop(AlterGenericOptionDrop),
        Plain(crate::ast::ddl::table::GenericOption),
    }
}

recursa::ast_node! {
    /// `SET name 'value'` — the `SET`-prefixed variant of
    /// `alter_generic_option_elem`.
    #[derive(Debug)]
    pub struct AlterGenericOptionSet {
        #[tok(SET, this)]
        pub option: crate::ast::ddl::table::GenericOption,
    }
}

recursa::ast_node! {
    /// `ADD name 'value'` — the `ADD`-prefixed variant of
    /// `alter_generic_option_elem`.
    #[derive(Debug)]
    pub struct AlterGenericOptionAdd {
        #[tok(ADD, this)]
        pub option: crate::ast::ddl::table::GenericOption,
    }
}

recursa::ast_node! {
    /// `DROP name` — the `DROP`-prefixed variant of
    /// `alter_generic_option_elem`. Unlike `SET` / `ADD` it takes only the
    /// option name (a `ColLabel`), with no value.
    #[derive(Debug)]
    pub struct AlterGenericOptionDrop {
        #[tok(DROP, this)]
        pub name: literal::AliasName,
    }
}

recursa::ast_node! {
    /// `OPTIONS (alter_generic_option_list)` — Postgres'
    /// `alter_generic_options`. The `ALTER`-side counterpart of
    /// [`CreateGenericOptions`]; differs only in that each element may
    /// carry an `ADD` / `SET` / `DROP` prefix.
    #[derive(Debug)]
    #[tok(OPTIONS, LPAREN, this, RPAREN)]
    pub struct AlterGenericOptions {
        #[sep(COMMA)]
        pub list: one_or_many!(AlterGenericOption),
    }
}

recursa::ast_node! {
    /// One action on `ALTER SERVER name action` — covers Postgres'
    /// `AlterForeignServerStmt` (VERSION-only, VERSION+OPTIONS, OPTIONS-only)
    /// plus the `RENAME TO` / `OWNER TO` branches from `RenameStmt` /
    /// `AlterOwnerStmt`.
    ///
    /// The branches begin with distinct keywords. The `Version` branch covers
    /// both `VERSION
    /// sconst` (bare) and `VERSION sconst OPTIONS (...)` (with trailing
    /// generic-options clause). Modelling both forms as one struct with
    /// an `Option<AlterGenericOptions>` tail keeps the shared `VERSION sconst`
    /// prefix in one production and leaves the LR parser to decide whether the
    /// `OPTIONS` tail is present.
    #[derive(Debug)]
    pub enum AlterServerAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        Options(AlterGenericOptions),
        Version(AlterServerVersionAction),
    }
}

recursa::ast_node! {
    /// `VERSION value [OPTIONS (...)]` — Postgres' `AlterForeignServerStmt`
    /// VERSION branch, with optional trailing generic-options clause.
    #[derive(Debug)]
    pub struct AlterServerVersionAction {
        pub version: ServerVersionClause,
        pub options: Option<AlterGenericOptions>,
    }
}

recursa::ast_node! {
    /// `ALTER SERVER name action` — Postgres' `AlterForeignServerStmt`
    /// plus the foreign-server branches of `RenameStmt` / `AlterOwnerStmt`.
    ///
    /// `action` is `Option` for the same reason as [`AlterFdwBody`]: gram.y's
    /// `AlterForeignServerStmt` requires at least one of `version` or
    /// `options`, so the bare `ALTER SERVER name` is a syntax error in PG, but
    /// the parser accepts it to avoid a
    /// file-level parse error (the differential oracle stays
    /// valid because both sides round-trip rejected).
    #[derive(Debug)]
    pub struct AlterServerStmt {
        #[tok(ALTER, SERVER, this)]
        pub name: crate::tokens::ColId,
        pub action: Option<AlterServerAction>,
    }
}

recursa::ast_node! {
    /// `fdw_option ...  [alter_generic_options]` — the
    /// HANDLER/NO HANDLER/VALIDATOR/NO VALIDATOR action on
    /// `ALTER FOREIGN DATA WRAPPER name ...`, optionally followed by a
    /// trailing `OPTIONS (...)` clause. Matches Postgres'
    /// `AlterFdwStmt: ALTER FOREIGN DATA WRAPPER name opt_fdw_options
    /// alter_generic_options | ALTER FOREIGN DATA WRAPPER name fdw_options`
    /// for the branches that begin with `HANDLER` / `NO` / `VALIDATOR`.
    ///
    /// `head` is a single mandatory `FdwOption` (so this variant begins with
    /// `HANDLER` | `NO` | `VALIDATOR`); `rest` collects
    /// any further fdw_options; `generic` is the optional trailing
    /// `alter_generic_options` (`OPTIONS (...)`).
    ///
    /// The case where `alter_generic_options` is the *only* clause (no
    /// leading fdw_options) is modelled by the sibling
    /// [`AlterFdwAction::GenericOpts`] variant. Splitting these two gives the LR
    /// grammar one non-nullable option-led production and one `OPTIONS` production.
    #[derive(Debug)]
    pub struct AlterFdwOptsAction {
        pub head: FdwOption,
        pub rest: zero_or_many!(FdwOption),
        pub generic: Option<AlterGenericOptions>,
    }
}

recursa::ast_node! {
    /// One action on `ALTER FOREIGN DATA WRAPPER name action` — covers
    /// Postgres' `AlterFdwStmt` plus the `RENAME TO` / `OWNER TO` branches
    /// from `RenameStmt` / `AlterOwnerStmt`.
    ///
    /// Each variant has disjoint first tokens.
    /// - `Rename` — `RENAME`
    /// - `Owner` — `OWNER`
    /// - `FdwOpts` — `HANDLER` | `NO` | `VALIDATOR` (one or more
    ///   `fdw_option`s, optionally followed by `OPTIONS (...)`)
    /// - `GenericOpts` — `OPTIONS` (`alter_generic_options` alone, no
    ///   leading fdw_options)
    ///
    /// The `alter_generic_options` and `fdw_options` clauses are split into two
    /// variants instead of one struct with a nullable leading `recursa::ArenaVec<'input, FdwOption>`,
    /// so the LR grammar has an explicit `OPTIONS`-only production.
    #[derive(Debug)]
    pub enum AlterFdwAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        FdwOpts(AlterFdwOptsAction),
        GenericOpts(AlterGenericOptions),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub struct AlterFdwBody {
        #[tok(DATA, WRAPPER, this)]
        pub name: crate::tokens::ColId,
        pub action: Option<AlterFdwAction>,
    }
}

recursa::ast_node! {
    /// One action on `ALTER FOREIGN TABLE [IF EXISTS] name action` —
    /// Postgres' `alter_table_cmds` for FOREIGN TABLE plus the foreign-table
    /// branches of `RenameStmt` (`RENAME TO new`, `RENAME [COLUMN] old TO
    /// new`) and `AlterObjectSchemaStmt` (`SET SCHEMA new`).
    ///
    /// `gram.y` line 2284: `ALTER FOREIGN TABLE [IF EXISTS] relation_expr
    /// alter_table_cmds`. The `alter_table_cmd` grammar is the superset
    /// shared with ALTER TABLE — see
    /// [`AlterTableCmd`](crate::ast::ddl::table::AlterTableCmd) for the full action
    /// set, including `ADD/DROP/ALTER COLUMN`, `ADD/DROP CONSTRAINT`,
    /// `OWNER TO`, `INHERIT`/`NO INHERIT`, `ENABLE/DISABLE TRIGGER`,
    /// `OPTIONS (...)`, etc. Foreign-table-specific actions like the
    /// column-level `OPTIONS (...)` (`AT_AlterColumnGenericOptions`,
    /// gram.y line 2623) are already modelled inside
    /// [`AlterColumnAction`](crate::ast::ddl::table::AlterColumnAction).
    ///
    /// Variant ordering: longer/more-specific prefixes first within the
    /// shared `RENAME …` family.
    /// - `RenameColumn` (`RENAME [COLUMN] old TO new`) — `RENAME ident …` or
    ///   `RENAME COLUMN …`
    /// - `Rename` (`RENAME TO new`) — `RENAME TO …`
    /// - `SetSchema` (`SET SCHEMA …`) — disjoint from every `SET …` action
    ///   inside `alter_table_cmd` (which all use different 2nd tokens).
    /// - `Cmds` last — the comma-separated `alter_table_cmds` catch-all.
    #[derive(Debug)]
    pub enum AlterForeignTableAction {
        RenameColumn(RenameColumnClause),
        Rename(RenameTo),
        SetSchema(SetSchemaClause),
        Cmds(AlterTableCmds),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    #[tok(TABLE, this)]
    pub struct AlterForeignTableBody {
        pub if_exists: Option<IfExists>,
        #[presence(ONLY)]
        pub only: bool,
        pub name: QualifiedName,
        #[presence(STAR)]
        pub star: bool,
        pub action: AlterForeignTableAction,
    }
}

recursa::ast_node! {
    /// What follows `ALTER FOREIGN`: either `DATA WRAPPER ...`
    /// (`AlterFdwStmt`) or `TABLE ...` (`AlterForeignTableStmt`).
    /// Discriminated by the first post-`FOREIGN` token (`DATA` vs `TABLE`);
    /// the two first-tokens are disjoint so peek order is for clarity only.
    #[derive(Debug)]
    pub enum AlterForeignBody {
        Fdw(AlterFdwBody),
        Table(AlterForeignTableBody),
    }
}

recursa::ast_node! {
    /// `ALTER FOREIGN ...` umbrella statement covering
    /// `ALTER FOREIGN DATA WRAPPER ...` (Postgres' `AlterFdwStmt`) and
    /// `ALTER FOREIGN TABLE ...` (Postgres' `AlterForeignTableStmt`).
    #[derive(Debug)]
    pub struct AlterForeignStmt {
        #[tok(ALTER, FOREIGN, this)]
        pub body: AlterForeignBody,
    }
}

recursa::ast_node! {
    /// One repeatable FDW handler/validator option — Postgres' `fdw_option`.
    ///
    /// Variant ordering: the two-token `NO HANDLER` / `NO VALIDATOR` forms
    /// come before their single-token counterparts so longest-match-wins
    /// picks the `NO`-prefixed spelling first.
    #[derive(Debug)]
    pub enum FdwOption {
        #[tok(NO, HANDLER)]
        NoHandler,
        #[tok(NO, VALIDATOR)]
        NoValidator,
        Handler(FdwHandlerOption),
        Validator(FdwValidatorOption),
    }
}

recursa::ast_node! {
    /// `HANDLER handler_name` — Postgres' `fdw_option` HANDLER branch.
    #[derive(Debug)]
    pub struct FdwHandlerOption {
        #[tok(HANDLER, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `VALIDATOR handler_name` — Postgres' `fdw_option` VALIDATOR branch.
    #[derive(Debug)]
    pub struct FdwValidatorOption {
        #[tok(VALIDATOR, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `CREATE FOREIGN DATA WRAPPER name [HANDLER ... | NO HANDLER]
    /// [VALIDATOR ... | NO VALIDATOR] [OPTIONS (...)]` — the body of
    /// `CREATE FOREIGN DATA WRAPPER ...` after the `CREATE FOREIGN` head.
    ///
    /// The fdw_options list is order-free and separator-free; we model it
    /// with `recursa::ArenaVec<'input, FdwOption>` so it stops at the first non-option token
    /// (OPTIONS or end-of-statement).
    #[derive(Debug)]
    pub struct CreateFdwBody {
        #[tok(DATA, WRAPPER, this)]
        pub name: crate::tokens::ColId,
        pub fdw_options: zero_or_many!(FdwOption),
        pub options: Option<CreateGenericOptions>,
    }
}

recursa::ast_node! {
    /// `SERVER name` reference on CREATE FOREIGN TABLE.
    #[derive(Debug)]
    pub struct ForeignTableServerClause {
        #[tok(SERVER, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Body of `CREATE FOREIGN TABLE name (cols) [INHERITS (...)] SERVER name
    /// [OPTIONS (...)]` — Postgres' columns form.
    #[derive(Debug)]
    pub struct ForeignTableColumnsBody {
        pub columns: ForeignTableColumnList,
        pub inherits: Option<crate::ast::ddl::table::InheritsClause>,
        pub server: ForeignTableServerClause,
        pub options: Option<CreateGenericOptions>,
    }
}

recursa::ast_node! {
    /// Parenthesized, comma-separated column and constraint list on a foreign
    /// table. The legacy grammar used `Seq0`, so the empty `()` form remains
    /// accepted (notably before an `INHERITS` clause).
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ForeignTableColumnList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(crate::ast::ddl::table::ColumnOrConstraint),
    );
}

recursa::ast_node! {
    /// Body of `CREATE FOREIGN TABLE name PARTITION OF parent [(opts)]
    /// { FOR VALUES ... | DEFAULT } SERVER name [OPTIONS (...)]` — Postgres'
    /// partition form. The bound is `for_values: Option` of `ForValuesClause`
    /// OR `default: Option` of `DEFAULT` (exactly one of the two should be
    /// `Some` for a syntactically valid statement).
    #[derive(Debug)]
    pub struct ForeignTablePartitionBody {
        #[tok(PARTITION, OF, this)]
        pub parent: QualifiedName,
        pub column_options: Option<ForeignTablePartitionColumnOptions>,
        pub for_values: Option<crate::ast::ddl::table::ForValuesClause>,
        #[presence(DEFAULT)]
        pub default: bool,
        pub server: ForeignTableServerClause,
        pub options: Option<CreateGenericOptions>,
    }
}

recursa::ast_node! {
    /// Optional parenthesized partition-column option list on a foreign table.
    /// This is a zero-or-more list to preserve the legacy `Seq0` cardinality.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ForeignTablePartitionColumnOptions(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(crate::ast::ddl::table::PartitionColumnOption),
    );
}

recursa::ast_node! {
    /// The body of `CREATE FOREIGN TABLE name ...` — either the columns
    /// form `(cols) [INHERITS (...)] SERVER ...` or the partition form
    /// `PARTITION OF parent [(opts)] FOR VALUES ... SERVER ...`.
    ///
    /// Variant ordering: `Partition` (`PARTITION` keyword) is listed before
    /// `Columns` (which starts with `(`); the two have disjoint first tokens.
    #[derive(Debug)]
    pub enum CreateForeignTableBody {
        Partition(ForeignTablePartitionBody),
        Columns(ForeignTableColumnsBody),
    }
}

recursa::ast_node! {
    /// `TABLE [IF NOT EXISTS] name body` — the body of
    /// `CREATE FOREIGN TABLE ...` after the `CREATE FOREIGN` head.
    #[derive(Debug)]
    #[tok(TABLE, this)]
    pub struct CreateForeignTableBodyStmt {
        pub if_not_exists: Option<IfNotExists>,
        pub name: QualifiedName,
        pub body: CreateForeignTableBody,
    }
}

recursa::ast_node! {
    /// What follows `CREATE FOREIGN`: either `DATA WRAPPER ...` (CreateFdwStmt)
    /// or `TABLE ...` (CreateForeignTableStmt). Discriminated by the first
    /// post-`FOREIGN` token (`DATA` vs `TABLE`); both first tokens are
    /// disjoint so peek order is for clarity only.
    #[derive(Debug)]
    pub enum CreateForeignBody {
        Fdw(CreateFdwBody),
        Table(boxed!(CreateForeignTableBodyStmt)),
    }
}

recursa::ast_node! {
    /// `CREATE FOREIGN ...` umbrella statement covering both
    /// `CREATE FOREIGN DATA WRAPPER ...` and `CREATE FOREIGN TABLE ...`.
    #[derive(Debug)]
    pub struct CreateForeignStmt {
        #[tok(CREATE, FOREIGN, this)]
        pub body: CreateForeignBody,
    }
}

recursa::ast_node! {
    /// The object kind after `DROP FOREIGN`: `DATA WRAPPER` or `TABLE`.
    #[derive(Debug)]
    pub enum ForeignObjectKind {
        #[tok(DATA, WRAPPER)]
        DataWrapper,
        #[tok(TABLE)]
        Table,
    }
}

recursa::ast_node! {
    /// `DROP FOREIGN {DATA WRAPPER | TABLE} [IF EXISTS] name [, ...]
    /// [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropForeignStmt {
        #[tok(DROP, FOREIGN, this)]
        pub kind: ForeignObjectKind,
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}
