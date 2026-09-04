/// CREATE INDEX / DROP INDEX statement AST.
pub use crate::ast::shared::flags::{DropBehavior, IfExists, IfNotExists};

use crate::ast::dml::select::{NullsOrder, SortDir, WhereClause};
use crate::ast::session::set_reset::SetValue;
use crate::ast::shared::expr::{Expr, FuncCall, JsonFuncExpr};
use crate::tokens::literal;

// ---------------------------------------------------------------------------
// Additional imports for the ALTER/DROP types appended to this file as part
// of the DDL physical-extraction migration. Glob imports keep cross-batch
// type references resolvable regardless of migration order; a polish pass
// will tighten these once the migration completes.
use crate::ast::ddl::database::SetTablespaceClause;
use crate::ast::ddl::statistics::SetStatisticsValue;
use crate::ast::ddl::trigger::DependsOnExtension;
#[allow(unused_imports)]
use crate::ast::shared::expr::*;
#[allow(unused_imports)]
use crate::ast::shared::flags::*;
#[allow(unused_imports)]
use crate::ast::shared::names::*;
#[allow(unused_imports)]
use crate::ast::shared::numbers::*;
#[allow(unused_imports)]
// ---------------------------------------------------------------------------
/// Index access method: `USING method_name`.
///
/// The method name can be an identifier or one of the built-in method
/// keywords (`btree`, `gin`, ...). We accept `literal::AliasName` so both
/// identifiers and keywords are allowed in this position.
#[derive(recursa::Node, Debug, Clone)]
pub struct UsingMethod<'input> {
    #[tok(USING, this)]
    pub method: literal::AliasName<'input>,
}

/// A single opclass option: `name = value`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassOption<'input> {
    pub name: literal::AliasName<'input>,
    #[tok(EQ, this)]
    pub value: Expr<'input>,
}

/// Parenthesized opclass option list: `(name = value, ...)`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct OpclassOptions<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<OpclassOption<'input>>,
);

/// Opclass name plus optional options: `int4_ops [(opt = val, ...)]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassSpec<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub options: Option<OpclassOptions<'input>>,
}

/// A storage parameter entry: `name [= value]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParam<'input> {
    pub name: StorageParamName<'input>,
    pub value: Option<StorageParamValue<'input>>,
}

/// Storage parameter name: either a bare word or `namespace.word`.
///
/// Parsing the first word unconditionally keeps the optional suffix's first
/// token (`.`) disjoint from the first word. In particular, a bare name does
/// not speculatively commit to the qualified form and then require a dot.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParamName<'input> {
    pub head: literal::AliasName<'input>,
    pub qualified_tail: Option<StorageParamQualifiedTail<'input>>,
}

/// `.name` suffix on a qualified storage parameter name.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParamQualifiedTail<'input> {
    #[tok(DOT, this)]
    pub name: literal::AliasName<'input>,
}

impl<'input> StorageParamName<'input> {
    /// Namespace of a qualified name, or `None` for a bare name.
    pub fn namespace(&self) -> Option<&literal::AliasName<'input>> {
        self.qualified_tail.as_ref().map(|_| &self.head)
    }

    /// Unqualified name component.
    pub fn name(&self) -> &literal::AliasName<'input> {
        self.qualified_tail
            .as_ref()
            .map_or(&self.head, |tail| &tail.name)
    }
}

/// `= value` suffix for a storage parameter.
///
/// The value is a permissive SetValue (keywords like `off`, `on`, string/numeric
/// literals, identifiers) rather than a full `Expr` — storage param values are
/// simple literals and `Expr::ColumnRef` rejects keywords like `off`.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParamValue<'input> {
    #[tok(EQ, this)]
    pub value: SetValue<'input>,
}

/// `WITH (name = value, ...)` storage parameters clause.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WITH, LPAREN, this, RPAREN)]
pub struct WithStorage<'input> {
    #[sep(COMMA)]
    pub params: Vec<StorageParam<'input>>,
}

/// `INCLUDE (col, ...)` covering-index clause.
#[derive(recursa::Node, Debug, Clone)]
#[tok(INCLUDE, LPAREN, this, RPAREN)]
pub struct IncludeClause<'input> {
    #[sep(COMMA)]
    pub columns: Vec<crate::tokens::ColId<'input>>,
}

/// Index column target: a parenthesized expression, a bare SQL/JSON
/// function expression, a bare function call (e.g., `lower(fruit)`), or a
/// plain column identifier. Postgres allows any `func_expr_windowless` as a
/// bare index element — that includes the SQL/JSON functions.
///
/// Variant ordering:
/// - `Expr` (`(`) starts with a different token than the others.
/// - `Json` before `Func`: a JSON function keyword is soft and `Func`
///   would otherwise reclaim it as an ordinary function name.
/// - `Func` (`ident(`) must come before `Col` (`ident`) so longest-match
///   prefers the function call form.
#[derive(recursa::Node, Debug, Clone)]
pub enum IndexTarget<'input> {
    Expr(#[tok(LPAREN, this, RPAREN)] Box<Expr<'input>>),
    Json(Box<JsonFuncExpr<'input>>),
    Func(Box<FuncCall<'input>>),
    Col(crate::tokens::ColId<'input>),
}

/// `COLLATE "name"` on an index element.
#[derive(recursa::Node, Debug, Clone)]
pub struct IndexCollate<'input> {
    #[tok(COLLATE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// An index element:
/// `column_or_expr [COLLATE "name"] [opclass [(options)]] [ASC|DESC] [NULLS FIRST|LAST]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IndexElem<'input> {
    pub target: IndexTarget<'input>,
    pub collate: Option<IndexCollate<'input>>,
    pub opclass: Option<OpclassSpec<'input>>,
    pub dir: Option<SortDir>,
    pub nulls: Option<NullsOrder>,
}

/// Parenthesized, comma-separated index-element list.
///
/// The legacy grammar represented this as `Seq0`, so this wrapper retains a
/// zero-or-more [`Vec`] while applying the delimiters to the whole list rather
/// than to each element.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct IndexElementList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<IndexElem<'input>>,
);

/// CREATE INDEX statement.
///
/// ```sql
/// CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] [name]
///        ON table [USING method] (index_elem, ...)
///        [INCLUDE (col, ...)]
///        [WITH (storage_param = value, ...)]
///        [WHERE predicate]
/// ```
///
/// The index name is optional (Postgres allows it to be omitted).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateIndexStmt<'input> {
    #[tok(CREATE, this, INDEX)]
    #[presence(UNIQUE)]
    pub unique: bool,
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub if_not_exists: Option<IfNotExists>,
    pub name: Option<crate::tokens::ColId<'input>>,
    #[tok(ON, this)]
    #[presence(ONLY)]
    /// Optional `ONLY` modifier — restricts the index to the named table
    /// without descending into inheritance children (partitioned tables).
    pub only: bool,
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub using: Option<Box<UsingMethod<'input>>>,
    pub columns: IndexElementList<'input>,
    pub include: Option<Box<IncludeClause<'input>>>,
    pub nulls_distinct: Option<NullsDistinctClause>,
    pub with_storage: Option<Box<WithStorage<'input>>>,
    pub tablespace: Option<crate::ast::ddl::table::TablespaceClause<'input>>,
    pub where_clause: Option<Box<WhereClause<'input>>>,
}

/// `NULLS [NOT] DISTINCT` modifier on a unique index.
///
/// Variant ordering: `NotDistinct` (`NULLS NOT DISTINCT`, longer) before
/// `Distinct` (`NULLS DISTINCT`, shorter).
#[derive(recursa::Node, Debug, Clone)]
pub enum NullsDistinctClause {
    #[tok(NULLS, NOT, DISTINCT)]
    NotDistinct,
    #[tok(NULLS, DISTINCT)]
    Distinct,
}

/// DROP INDEX statement:
///
/// ```sql
/// DROP INDEX [CONCURRENTLY] [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct DropIndexStmt<'input> {
    /// Greedy: a leading CONCURRENTLY starts this element instead of ending `DropIndexStmt` (bison shift preference).
    #[greedy(CONCURRENTLY)]
    #[tok(DROP, INDEX, this)]
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub if_exists: Option<IfExists>,
    /// Greedy: a leading CASCADE, RESTRICT starts this element instead of ending `DropIndexStmt` (bison shift preference).
    #[greedy(CASCADE, RESTRICT)]
    #[sep(COMMA)]
    pub names: Vec<crate::ast::shared::names::QualifiedName<'input>>,
    pub behavior: Option<DropBehavior>,
}

// =========================================================================
// ALTER/DROP INDEX — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `SET (storage_param = value, ...)` action shared by ALTER INDEX /
/// ALTER VIEW / ALTER MATERIALIZED VIEW / ALTER TABLE — modifies storage
/// parameters. Differs from `WithStorage` (`WITH (...)` on CREATE) only in
/// the leading keyword.
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, LPAREN, this, RPAREN)]
pub struct SetReloptions<'input> {
    #[sep(COMMA)]
    pub params: recursa::Vec1<crate::ast::ddl::index::StorageParam<'input>>,
}

/// `RESET (param_name [= value], ...)` action shared by ALTER INDEX /
/// ALTER VIEW / ALTER MATERIALIZED VIEW / ALTER TABLE — removes storage
/// parameters. Postgres' gram.y `reloption_elem` allows
/// `ColLabel [. ColLabel] [= def_arg]`, so the syntax accepts `name = value`
/// in RESET too (PG ignores the value semantically). Modeled via the same
/// `StorageParam` type used by `WITH (...)`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(RESET, LPAREN, this, RPAREN)]
pub struct ResetReloptions<'input> {
    #[sep(COMMA)]
    pub params: recursa::Vec1<crate::ast::ddl::index::StorageParam<'input>>,
}

/// `ATTACH PARTITION qualified_name` — Postgres' `index_partition_cmd` (the
/// single ALTER INDEX form that takes a partition operation).
#[derive(recursa::Node, Debug, Clone)]
pub struct AttachPartitionClause<'input> {
    #[tok(ATTACH, PARTITION, this)]
    pub name: QualifiedName<'input>,
}

/// A column reference inside `ALTER INDEX … ALTER COLUMN col_ref …`:
/// either an integer column position (`SignedIconst`) or a column name
/// (`Ident`). Postgres' gram.y has two productions; we union them as one
/// enum so the surrounding action struct can be derived.
///
/// Variant ordering: `Number` first (lex token kind disjoint from
/// `Ident`), then `Name`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnRef<'input> {
    Number(SignedIconst<'input>),
    Name(crate::tokens::ColId<'input>),
}

/// `SET STATISTICS …` tail of an ALTER COLUMN command.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnStatisticsAction<'input> {
    #[tok(SET, STATISTICS, this)]
    pub value: SetStatisticsValue<'input>,
}

/// Action following the shared `ALTER [COLUMN] col_ref` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColumnIndexAction<'input> {
    Statistics(AlterColumnStatisticsAction<'input>),
    Reloptions(SetReloptions<'input>),
}

/// One `ALTER COLUMN …` cmd on ALTER INDEX. The two forms (`SET
/// STATISTICS` and `SET (params)`) both start with `ALTER … SET`; the
/// disambiguation token is `STATISTICS` vs `(`.
///
/// The common prefix is represented once so dispatch only needs to inspect
/// the keyword or parenthesis immediately following `SET`, rather than look
/// through a potentially multi-token signed column reference twice.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnIndexCmd<'input> {
    #[tok(ALTER, optional(COLUMN), this)]
    pub col_ref: ColumnRef<'input>,
    pub action: AlterColumnIndexAction<'input>,
}

/// One action on a single-target `ALTER INDEX [IF EXISTS] name action` —
/// the corpus-exercised subset of `alter_table_cmds` plus `RenameStmt` and
/// `AlterObjectDependsStmt` and `index_partition_cmd`.
///
/// Variant ordering:
/// - `SetTablespace` (`SET TABLESPACE`) and `SetReloptions` (`SET (`) and
///   `ResetReloptions` (`RESET …`) — second tokens are disjoint, so order
///   is for clarity.
/// - `AlterColumn` starts with the `ALTER` token, distinct from all
///   `SET`/`RESET`/`ATTACH`/`RENAME`/`NO`/`DEPENDS` first tokens.
/// - `Depends` allows a bare `DEPENDS …` (without `NO`), and `NoDepends`
///   is reached via the `Depends` arm since both share the
///   `DependsOnExtension` type (with `NO` as an `Option`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterIndexAction<'input> {
    SetTablespace(SetTablespaceClause<'input>),
    SetReloptions(SetReloptions<'input>),
    ResetReloptions(ResetReloptions<'input>),
    Attach(AttachPartitionClause<'input>),
    AlterColumn(AlterColumnIndexCmd<'input>),
    Depends(DependsOnExtension<'input>),
    Rename(RenameTo<'input>),
}

/// `ALL IN TABLESPACE name [OWNED BY role_list] SET TABLESPACE new
/// [NOWAIT]` — Postgres' bulk-relocate action on ALTER INDEX (and ALTER
/// MATERIALIZED VIEW). Moves every index in the named tablespace to a new
/// tablespace, optionally filtered by owner role(s).
#[derive(recursa::Node, Debug, Clone)]
pub struct AllInTablespaceBody<'input> {
    #[tok(ALL, IN, TABLESPACE, this)]
    pub source: crate::tokens::ColId<'input>,
    pub owned_by: Option<OwnedByRoles<'input>>,
    pub set_tablespace: SetTablespaceClause<'input>,
    #[presence(NOWAIT)]
    pub nowait: bool,
}

/// `OWNED BY role_list` — owner filter on the bulk `ALL IN TABLESPACE`
/// action.
#[derive(recursa::Node, Debug, Clone)]
pub struct OwnedByRoles<'input> {
    #[tok(OWNED, BY, this)]
    pub roles: RoleList<'input>,
}

/// `ALTER INDEX [IF EXISTS] name action` plus the bulk `ALTER INDEX ALL IN
/// TABLESPACE …` form. The two top-level shapes share the leading `ALTER
/// INDEX` keywords, so they sit on either side of a single enum to
/// preserve dispatcher commitment.
///
/// Variant ordering: `All` (starts with `ALL`) before `Single`
/// (starts with `[IF EXISTS] qualified_name` — never `ALL`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterIndexBody<'input> {
    All(AllInTablespaceBody<'input>),
    Single(AlterIndexSingle<'input>),
}

/// `[IF EXISTS] name action` — the per-index branch of ALTER INDEX.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterIndexSingle<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterIndexAction<'input>,
}

/// `ALTER INDEX [IF EXISTS] name action`
/// `ALTER INDEX ALL IN TABLESPACE name [OWNED BY role_list] SET TABLESPACE
///   new [NOWAIT]` — the two top-level shapes of Postgres' `AlterTableStmt`
/// branches that begin with `ALTER INDEX …`, plus the index branches of
/// `RenameStmt` / `AlterObjectDependsStmt` (single form).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterIndexStmt<'input> {
    #[tok(ALTER, INDEX, this)]
    pub body: AlterIndexBody<'input>,
}
