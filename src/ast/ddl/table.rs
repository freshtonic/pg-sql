/// CREATE TABLE statement AST.
use crate::ast::shared::expr::{Expr, TypeName};
use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

// ---------------------------------------------------------------------------
// Additional imports for the ALTER/DROP types appended to this file as part
// of the DDL physical-extraction migration. Glob imports keep cross-batch
// type references resolvable regardless of migration order; a polish pass
// will tighten these once the migration completes.
use crate::ast::ddl::database::SetTablespaceClause;
use crate::ast::ddl::foreign::AlterGenericOptions;
use crate::ast::ddl::index::{AllInTablespaceBody, ColumnRef, ResetReloptions, SetReloptions};
use crate::ast::ddl::materialized_view::{ColumnCompressionTarget, SetAccessMethodClause};
use crate::ast::ddl::statistics::SetStatisticsValue;
use crate::ast::ddl::trigger::DependsOnExtension;
use crate::ast::ddl::view::RenameColumnClause;
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
/// `USING INDEX TABLESPACE name` — tablespace for the index backing a PRIMARY
/// KEY or UNIQUE column constraint.
#[derive(recursa::Node, Debug, Clone)]
pub struct UsingIndexTablespace<'input> {
    #[tok(USING, INDEX, TABLESPACE, this)]
    pub name: literal::Ident<'input>,
}

/// PRIMARY KEY column constraint.
#[derive(recursa::Node, Debug, Clone)]
#[tok(PRIMARY, KEY, this)]
pub struct PrimaryKeyConstraint<'input> {
    pub index_tablespace: Option<UsingIndexTablespace<'input>>,
    /// Optional `[NOT] DEFERRABLE [INITIALLY {DEFERRED|IMMEDIATE}]` suffix.
    pub attrs: ConstraintAttrs,
}

/// UNIQUE column constraint.
#[derive(recursa::Node, Debug, Clone)]
#[tok(UNIQUE, this)]
pub struct UniqueConstraint<'input> {
    /// Optional `NULLS [NOT] DISTINCT` qualifier (Postgres 15+).
    pub nulls: Option<NullsDistinctQualifier>,
    pub index_tablespace: Option<UsingIndexTablespace<'input>>,
    /// Optional `[NOT] DEFERRABLE [INITIALLY ...]` attributes.
    pub attrs: ConstraintAttrs,
}

/// `NULLS DISTINCT` or `NULLS NOT DISTINCT` for UNIQUE constraints.
#[derive(recursa::Node, Debug, Clone)]
pub struct NullsDistinctQualifier {
    #[tok(NULLS, this, DISTINCT)]
    #[presence(NOT)]
    pub not: bool,
}

/// Referential action for `ON DELETE` / `ON UPDATE`.
///
/// Variant ordering: multi-word variants (`NO ACTION`, `SET NULL`, `SET DEFAULT`)
/// come before single-word ones to satisfy longest-match.
#[derive(recursa::Node, Debug, Clone)]
pub enum ReferentialAction<'input> {
    #[tok(NO, ACTION)]
    NoAction,
    SetNull(SetNullKw<'input>),
    SetDefault(SetDefaultKw<'input>),
    #[tok(CASCADE)]
    Cascade,
    #[tok(RESTRICT)]
    Restrict,
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, NULL, this)]
pub struct SetNullKw<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub cols: Option<recursa::Vec1<crate::tokens::ColId<'input>>>,
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, DEFAULT, this)]
pub struct SetDefaultKw<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub cols: Option<recursa::Vec1<crate::tokens::ColId<'input>>>,
}

/// `ON DELETE action`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OnDeleteAction<'input> {
    #[tok(ON, DELETE, this)]
    pub action: ReferentialAction<'input>,
}

/// `ON UPDATE action`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OnUpdateAction<'input> {
    #[tok(ON, UPDATE, this)]
    pub action: ReferentialAction<'input>,
}

/// Match type for a foreign key: `MATCH FULL | PARTIAL | SIMPLE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum MatchKind {
    #[tok(FULL)]
    Full,
    #[tok(PARTIAL)]
    Partial,
    #[tok(SIMPLE)]
    Simple,
}

/// `MATCH FULL | MATCH PARTIAL | MATCH SIMPLE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct MatchClause {
    #[tok(MATCH, this)]
    pub kind: MatchKind,
}

/// `DEFERRABLE | NOT DEFERRABLE`.
///
/// Variant ordering: `NotDeferrable` (two keywords) before `Deferrable`.
#[derive(recursa::Node, Debug, Clone)]
pub enum DeferrableKind {
    #[tok(NOT, DEFERRABLE)]
    NotDeferrable,
    #[tok(DEFERRABLE)]
    Deferrable,
}

/// `INITIALLY DEFERRED | INITIALLY IMMEDIATE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct InitiallyClause {
    #[tok(INITIALLY, this)]
    pub mode: InitiallyMode,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum InitiallyMode {
    #[tok(DEFERRED)]
    Deferred,
    #[tok(IMMEDIATE)]
    Immediate,
}

/// `ON DELETE ...` or `ON UPDATE ...` trailing action on a REFERENCES
/// constraint. Modeled as an enum so both orders of the two clauses
/// are accepted via a [`Vec`]`<`[`OnAction`]`>`.
///
/// Variant ordering: both start with `ON`; they diverge at the next keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum OnAction<'input> {
    OnDelete(OnDeleteAction<'input>),
    OnUpdate(OnUpdateAction<'input>),
}

/// REFERENCES constraint:
/// `REFERENCES table [(col, ...)] [MATCH ...] [ON DELETE|UPDATE ...]* [DEFERRABLE | NOT DEFERRABLE] [INITIALLY ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct ReferencesConstraint<'input> {
    #[tok(REFERENCES, this)]
    pub table: crate::ast::shared::names::QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<recursa::Vec1<literal::AliasName<'input>>>,
    pub match_clause: Option<MatchClause>,
    pub actions: Vec<OnAction<'input>>,
    pub deferrable: Option<DeferrableKind>,
    pub initially: Option<InitiallyClause>,
    #[presence(NOT, VALID)]
    pub not_valid: bool,
}

/// `CHECK (expr) [NO INHERIT] [NOT VALID]`
#[derive(recursa::Node, Debug, Clone)]
pub struct CheckConstraint<'input> {
    #[tok(CHECK, LPAREN, this, RPAREN)]
    pub expr: crate::ast::shared::expr::Expr<'input>,
    #[presence(NO, INHERIT)]
    pub no_inherit: bool,
    #[presence(NOT, VALID)]
    pub not_valid: bool,
}

/// `GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY` modifier.
///
/// Variant ordering: both start with a distinct keyword after `GENERATED`
/// (`ALWAYS` vs `BY`), so order is cosmetic.
#[derive(recursa::Node, Debug, Clone)]
pub enum GeneratedIdentityMode {
    #[tok(ALWAYS)]
    Always,
    #[tok(BY, DEFAULT)]
    ByDefault,
}

/// GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY column constraint, with
/// optional `(sequence_option ...)` parenthesized list (e.g. `START WITH 44`).
#[derive(recursa::Node, Debug, Clone)]
pub struct GeneratedIdentityConstraint<'input> {
    #[tok(GENERATED, this)]
    pub mode: GeneratedIdentityMode,
    pub identity: AsIdentity,
    #[tok(LPAREN, this, RPAREN)]
    pub seq_options: Option<recursa::Vec1<IdentitySeqOption<'input>>>,
}

/// Required `AS IDENTITY` marker after the generation mode.
#[derive(recursa::Node, Debug, Clone)]
pub enum AsIdentity {
    #[tok(AS, IDENTITY)]
    Value,
}

/// One option inside an `IDENTITY ( ... )` sequence option list.
///
/// Variant ordering: longer multi-word forms first so longest-match-wins
/// picks them.
#[derive(recursa::Node, Debug, Clone)]
pub enum IdentitySeqOption<'input> {
    StartWith(SeqOptStartWith<'input>),
    IncrementBy(SeqOptIncrementBy<'input>),
    MinValue(SeqOptMinValue<'input>),
    #[tok(NO, MINVALUE)]
    NoMinValue,
    MaxValue(SeqOptMaxValue<'input>),
    #[tok(NO, MAXVALUE)]
    NoMaxValue,
    Cache(SeqOptCache<'input>),
    #[tok(CYCLE)]
    Cycle,
    #[tok(NO, CYCLE)]
    NoCycle,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOptStartWith<'input> {
    #[tok(START, optional(WITH), this)]
    pub value: crate::ast::shared::numbers::NumericOnly<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOptIncrementBy<'input> {
    #[tok(INCREMENT, optional(BY), this)]
    pub value: crate::ast::shared::numbers::NumericOnly<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOptMinValue<'input> {
    #[tok(MINVALUE, this)]
    pub value: crate::ast::shared::numbers::NumericOnly<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOptMaxValue<'input> {
    #[tok(MAXVALUE, this)]
    pub value: crate::ast::shared::numbers::NumericOnly<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SeqOptCache<'input> {
    #[tok(CACHE, this)]
    pub value: crate::ast::shared::numbers::NumericOnly<'input>,
}

/// `GENERATED {ALWAYS | BY DEFAULT} AS (expr) STORED` column constraint.
#[derive(recursa::Node, Debug, Clone)]
pub struct GeneratedStoredConstraint<'input> {
    #[tok(GENERATED, this)]
    pub mode: GeneratedIdentityMode,
    #[tok(AS, LPAREN, this, RPAREN, STORED)]
    pub expr: crate::ast::shared::expr::Expr<'input>,
}

/// `COMPRESSION method` column clause. Sets the compression method
/// (e.g. `pglz`, `lz4`) for a toastable column.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompressionConstraint<'input> {
    #[tok(COMPRESSION, this)]
    pub method: literal::Ident<'input>,
}

/// DEFAULT expr column constraint.
#[derive(recursa::Node, Debug, Clone)]
pub struct DefaultConstraint<'input> {
    #[tok(DEFAULT, this)]
    pub expr: crate::ast::shared::expr::Expr<'input>,
}

/// Column constraint kind (without the optional `CONSTRAINT name` prefix).
///
/// Variant ordering for longest-match-wins:
/// - GeneratedIdentity (`GENERATED`) first (unique keyword)
/// - PrimaryKey (`PRIMARY KEY`) before others (unique keyword)
/// - NotNull (`NOT NULL`) before others
/// - References, Unique, Default, Check all start with distinct keywords
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnConstraintKind<'input> {
    GeneratedStored(GeneratedStoredConstraint<'input>),
    GeneratedIdentity(GeneratedIdentityConstraint<'input>),
    PrimaryKey(PrimaryKeyConstraint<'input>),
    #[tok(NOT, NULL)]
    NotNull,
    #[tok(NULL)]
    /// Bare `NULL` — redundant (columns are nullable by default) but
    /// syntactically accepted.
    Null,
    Unique(UniqueConstraint<'input>),
    References(ReferencesConstraint<'input>),
    Default(DefaultConstraint<'input>),
    Check(CheckConstraint<'input>),
    Compression(CompressionConstraint<'input>),
    Storage(StorageConstraint),
}

/// Column STORAGE mode: `STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT }`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnStorageMode {
    #[tok(PLAIN)]
    Plain,
    #[tok(EXTERNAL)]
    External,
    #[tok(EXTENDED)]
    Extended,
    #[tok(MAIN)]
    Main,
    #[tok(DEFAULT)]
    Default,
}

/// `STORAGE mode` column-level storage specifier (used inline in CREATE
/// TABLE column definitions).
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageConstraint {
    #[tok(STORAGE, this)]
    pub mode: ColumnStorageMode,
}

/// Optional `CONSTRAINT name` prefix shared by column-level and
/// table-level constraints.
#[derive(recursa::Node, Debug, Clone)]
pub struct ConstraintNamePrefix<'input> {
    #[tok(CONSTRAINT, this)]
    pub name: literal::Ident<'input>,
}

/// A column constraint with its optional `CONSTRAINT name` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub struct ColumnConstraint<'input> {
    pub name: Option<ConstraintNamePrefix<'input>>,
    pub kind: ColumnConstraintKind<'input>,
}

/// `COLLATE "name"` clause used after a column's type.
#[derive(recursa::Node, Debug, Clone)]
pub struct CollateClause<'input> {
    #[tok(COLLATE, this)]
    pub name: literal::Ident<'input>,
}

/// One entry in a column-level `OPTIONS (name 'value', ...)` clause —
/// Postgres' `generic_option_elem` (`generic_option_name generic_option_arg`).
///
/// The name is a `ColLabel` (any identifier-or-keyword), and the argument is
/// a single-quoted string constant (`Sconst`).
#[derive(recursa::Node, Debug, Clone)]
pub struct GenericOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: crate::ast::utility::copy::CopySconst<'input>,
}

/// Postgres' `create_generic_options`: `OPTIONS (generic_option_list)`.
/// Used by CREATE FOREIGN DATA WRAPPER, CREATE SERVER, CREATE FOREIGN TABLE,
/// CREATE USER MAPPING, IMPORT FOREIGN SCHEMA, and column-level options on
/// foreign-table columns.
#[derive(recursa::Node, Debug, Clone)]
#[tok(OPTIONS, LPAREN, this, RPAREN)]
pub struct CreateGenericOptions<'input> {
    #[sep(COMMA)]
    pub list: recursa::Vec1<GenericOption<'input>>,
}

/// A column definition: `name type [COLLATE "..."] [OPTIONS (...)] [constraints...]`.
///
/// The `column_options` slot models Postgres' `create_generic_options` on
/// `columnDef` — used in CREATE FOREIGN TABLE column lists.
#[derive(recursa::Node, Debug, Clone)]
pub struct ColumnDef<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub type_name: crate::ast::shared::expr::CastType<'input>,
    pub collate: Option<CollateClause<'input>>,
    pub column_options: Option<CreateGenericOptions<'input>>,
    pub constraints: Vec<ColumnConstraint<'input>>,
}

impl<'input> ColumnDef<'input> {
    /// Returns true if any of this column's constraints is a PRIMARY KEY.
    pub fn primary_key(&self) -> bool {
        self.constraints
            .iter()
            .any(|c| matches!(c.kind, ColumnConstraintKind::PrimaryKey(_)))
    }
}

// --- Table-level constraints ---

/// Optional trailing deferrable/initially pair shared by PK/UNIQUE/FK.
#[derive(recursa::Node, Debug, Clone)]
pub struct ConstraintAttrs {
    pub deferrable: Option<DeferrableKind>,
    pub initially: Option<InitiallyClause>,
}

/// `USING INDEX name` — gram.y `ExistingIndex`. The named index must
/// already exist on the table; used by `PRIMARY KEY USING INDEX name` and
/// `UNIQUE USING INDEX name` table constraint forms.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExistingIndex<'input> {
    #[tok(USING, INDEX, this)]
    pub name: literal::Ident<'input>,
}

/// Body of a table-level `PRIMARY KEY` / `UNIQUE` constraint — either the
/// `(cols) [INCLUDE (…)]` column-list form or the `USING INDEX name`
/// existing-index form (gram.y `ConstraintElem` `PRIMARY KEY (cols) …`
/// vs `PRIMARY KEY ExistingIndex …`, and the analogous `UNIQUE` pair).
///
/// Variant ordering: `UsingIndex` first because its first token (`USING`)
/// is disjoint from `(`; declaration order is then for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum IndexedConstraintBody<'input> {
    /// `USING INDEX name` — bind constraint to an existing index.
    UsingIndex(ExistingIndex<'input>),
    /// `(cols) [INCLUDE (…)]` — declare the constraint on columns.
    Columns(IndexedConstraintColumns<'input>),
}

/// Parenthesized column list in a PRIMARY KEY or UNIQUE constraint.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct IndexedConstraintColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<crate::tokens::ColId<'input>>,
);

/// `(cols) [INCLUDE (…)] [WITH (...)] [USING INDEX TABLESPACE name]` — the
/// column-list branch of a PK/UNIQUE constraint body. Per gram.y
/// `ConstraintElem`'s `UNIQUE … '(' columnList ')' opt_c_include
/// opt_definition OptConsTableSpace ConstraintAttributeSpec` rule.
#[derive(recursa::Node, Debug, Clone)]
pub struct IndexedConstraintColumns<'input> {
    pub columns: IndexedConstraintColumnList<'input>,
    pub include: Option<IncludeColumns<'input>>,
    /// `WITH (storage_param = value, ...)` — gram.y's `opt_definition`.
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    /// `USING INDEX TABLESPACE name` — gram.y's `OptConsTableSpace`.
    pub index_tablespace: Option<UsingIndexTablespace<'input>>,
}

/// `PRIMARY KEY {(cols) [INCLUDE (…)] | USING INDEX name}` — table-level
/// constraint. Per gram.y `ConstraintElem`:
/// `PRIMARY KEY '(' columnList ')' opt_c_include … ConstraintAttributeSpec`
/// or `PRIMARY KEY ExistingIndex ConstraintAttributeSpec`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TablePrimaryKey<'input> {
    #[tok(PRIMARY, KEY, this)]
    pub body: IndexedConstraintBody<'input>,
    pub attrs: ConstraintAttrs,
}

/// `INCLUDE (col, ...)` covering-index clause used on PRIMARY KEY / UNIQUE
/// table constraints and on CREATE INDEX.
#[derive(recursa::Node, Debug, Clone)]
#[tok(INCLUDE, LPAREN, this, RPAREN)]
pub struct IncludeColumns<'input> {
    #[sep(COMMA)]
    pub columns: Vec<crate::tokens::ColId<'input>>,
}

/// `UNIQUE {(cols) [INCLUDE (…)] | USING INDEX name}` — table-level
/// constraint. Per gram.y `ConstraintElem`:
/// `UNIQUE … '(' columnList ')' opt_c_include … ConstraintAttributeSpec`
/// or `UNIQUE ExistingIndex ConstraintAttributeSpec`. The `USING INDEX`
/// branch has no `NULLS [NOT] DISTINCT` qualifier (PG infers it from the
/// existing index definition).
#[derive(recursa::Node, Debug, Clone)]
#[tok(UNIQUE, this)]
pub struct TableUnique<'input> {
    /// `NULLS [NOT] DISTINCT` qualifier — only meaningful for the
    /// `(cols)` branch but accepted before either body for parsing
    /// simplicity. If present alongside `USING INDEX`, PG rejects at
    /// semantic time; the diff oracle handles that case.
    pub nulls: Option<NullsDistinctQualifier>,
    pub body: IndexedConstraintBody<'input>,
    pub attrs: ConstraintAttrs,
}

/// Parenthesized local-column list in a table-level foreign-key constraint.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ForeignKeyColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<crate::tokens::ColId<'input>>,
);

/// `FOREIGN KEY (col, ...) REFERENCES table [(col, ...)] [MATCH ...] [ON ...] [DEFERRABLE ...] [INITIALLY ...]`
#[derive(recursa::Node, Debug, Clone)]
#[tok(FOREIGN, KEY, this)]
pub struct TableForeignKey<'input> {
    pub columns: ForeignKeyColumnList<'input>,
    pub references: ReferencesConstraint<'input>,
}

/// One entry in an EXCLUDE constraint's exclusion list: `index_elem WITH any_operator`.
///
/// Postgres' `ExclusionConstraintElem`. The operator may also appear wrapped
/// in `OPERATOR(...)` for the benefit of `ruleutils.c`; we accept both forms
/// via [`ExclusionOperator`].
#[derive(recursa::Node, Debug, Clone)]
pub struct ExclusionConstraintElem<'input> {
    pub elem: crate::ast::ddl::index::IndexElem<'input>,
    #[tok(WITH, this)]
    pub op: ExclusionOperator<'input>,
}

/// The operator slot of an exclusion constraint element.
///
/// Two forms per `gram.y::ExclusionConstraintElem`:
/// - `any_operator`                — bare operator name.
/// - `OPERATOR ( any_operator )`   — same operator decorated with `OPERATOR(...)`.
///
/// Variant ordering: `Decorated` starts with the `OPERATOR` keyword; `Plain`
/// starts with an operator-name token. Their first sets are disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExclusionOperator<'input> {
    Decorated(ExclusionOperatorDecorated<'input>),
    Plain(crate::ast::shared::names::QualifiedOperatorName<'input>),
}

/// `OPERATOR ( any_operator )` decorated form of an exclusion operator.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExclusionOperatorDecorated<'input> {
    #[tok(OPERATOR, LPAREN, this, RPAREN)]
    pub name: crate::ast::shared::names::QualifiedOperatorName<'input>,
}

/// `WHERE (predicate)` clause on an EXCLUDE constraint — Postgres'
/// `OptWhereClause` in `gram.y`. The parens are mandatory (unlike the regular
/// `WHERE expr` form used by SELECT).
#[derive(recursa::Node, Debug, Clone)]
pub struct ExclusionWhereClause<'input> {
    #[tok(WHERE, LPAREN, this, RPAREN)]
    pub expr: crate::ast::shared::expr::Expr<'input>,
}

/// Parenthesized, non-empty list of exclusion-constraint elements.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ExclusionConstraintList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<ExclusionConstraintElem<'input>>,
);

/// `EXCLUDE [USING method] (index_elem WITH op [, ...]) [INCLUDE (...)]
///         [WITH (storage_params)] [USING INDEX TABLESPACE name] [WHERE (expr)]
///         [DEFERRABLE/INITIALLY ...]` table-level constraint.
///
/// Per `gram.y::ConstraintElem`:
/// ```text
/// EXCLUDE access_method_clause '(' ExclusionConstraintList ')'
///     opt_c_include opt_definition OptConsTableSpace OptWhereClause
///     ConstraintAttributeSpec
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(EXCLUDE, this)]
pub struct TableExclude<'input> {
    /// `access_method_clause` — `USING method` is optional (defaults to gist).
    pub using: Option<crate::ast::ddl::index::UsingMethod<'input>>,
    pub exclusions: ExclusionConstraintList<'input>,
    /// `INCLUDE (col, ...)` covering-index clause.
    pub include: Option<IncludeColumns<'input>>,
    /// `WITH (param = value, ...)` storage parameters (`opt_definition`).
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    /// `USING INDEX TABLESPACE name` (`OptConsTableSpace`).
    pub index_tablespace: Option<UsingIndexTablespace<'input>>,
    /// `WHERE (expr)` partial-constraint predicate (parens mandatory).
    pub where_clause: Option<ExclusionWhereClause<'input>>,
    pub attrs: ConstraintAttrs,
}

/// A table-level constraint kind.
///
/// Variant ordering: `PRIMARY KEY` (PRIMARY), `FOREIGN KEY` (FOREIGN),
/// `UNIQUE`, `CHECK`, `EXCLUDE` — all start with distinct unique keywords
/// so order is not strictly required for disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum TableConstraintKind<'input> {
    PrimaryKey(TablePrimaryKey<'input>),
    ForeignKey(TableForeignKey<'input>),
    Unique(TableUnique<'input>),
    Check(CheckConstraint<'input>),
    Exclude(TableExclude<'input>),
}

/// A table-level constraint with optional `CONSTRAINT name` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableConstraint<'input> {
    pub name: Option<ConstraintNamePrefix<'input>>,
    pub kind: TableConstraintKind<'input>,
}

/// A single `INCLUDING` / `EXCLUDING` option on a `LIKE` source table clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum LikeOptionKind {
    #[tok(ALL)]
    All,
    #[tok(DEFAULTS)]
    Defaults,
    #[tok(CONSTRAINTS)]
    Constraints,
    #[tok(INDEXES)]
    Indexes,
    #[tok(STORAGE)]
    Storage,
    #[tok(COMMENTS)]
    Comments,
    #[tok(STATISTICS)]
    Statistics,
    #[tok(GENERATED)]
    Generated,
    #[tok(IDENTITY)]
    Identity,
    #[tok(COMPRESSION)]
    Compression,
}

/// `INCLUDING what`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IncludingOption {
    #[tok(INCLUDING, this)]
    pub what: LikeOptionKind,
}

/// `EXCLUDING what`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExcludingOption {
    #[tok(EXCLUDING, this)]
    pub what: LikeOptionKind,
}

/// One option on a `LIKE table` clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum LikeOption {
    Including(IncludingOption),
    Excluding(ExcludingOption),
}

/// `LIKE source_table [INCLUDING/EXCLUDING option ...]` clause in a column
/// list body. Copies column definitions (and optionally other properties)
/// from an existing table.
#[derive(recursa::Node, Debug, Clone)]
pub struct LikeClause<'input> {
    #[tok(LIKE, this)]
    pub source: crate::ast::shared::names::QualifiedName<'input>,
    pub options: Vec<LikeOption>,
}

/// One item in a CREATE TABLE column list: a `LIKE table` clause, a
/// table-level constraint, or a column definition.
///
/// Variant ordering: the `Like` variant starts with the `LIKE` keyword and
/// must come first (its leading token is otherwise an infix operator in
/// expressions, so it can't collide with `Column` which starts with an
/// ident). `Constraint` must come before `Column` because its leading
/// tokens (`CONSTRAINT`, `PRIMARY`, `UNIQUE`, `FOREIGN`, `CHECK`) are
/// keywords, while a `Column` starts with an identifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnOrConstraint<'input> {
    Like(LikeClause<'input>),
    Constraint(TableConstraint<'input>),
    Column(ColumnDef<'input>),
}

/// Optional TEMP or TEMPORARY keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum TempKw {
    #[tok(TEMP)]
    Temp,
    #[tok(TEMPORARY)]
    Temporary,
}

/// INHERITS clause: `INHERITS (parent, ...)`
#[derive(recursa::Node, Debug, Clone)]
#[tok(INHERITS, LPAREN, this, RPAREN)]
pub struct InheritsClause<'input> {
    #[sep(COMMA)]
    pub parents: Vec<crate::tokens::ColId<'input>>,
}

/// `TABLESPACE name` clause on CREATE TABLE / CREATE INDEX, placing the
/// relation into a non-default tablespace.
#[derive(recursa::Node, Debug, Clone)]
pub struct TablespaceClause<'input> {
    #[tok(TABLESPACE, this)]
    pub name: literal::Ident<'input>,
}

/// Legacy OIDS choice exposed by [`ColumnsBody::with_oids`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithOidsClause {
    WithOids,
    WithoutOids,
}

/// Parenthesized storage parameters following `WITH` on CREATE TABLE.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ColumnsStorageParams<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<crate::ast::ddl::index::StorageParam<'input>>,
);

/// Payload after the common CREATE TABLE `WITH` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnsWithValue<'input> {
    #[tok(OIDS)]
    Oids,
    Storage(ColumnsStorageParams<'input>),
}

/// CREATE TABLE's mutually exclusive `WITH OIDS`, `WITH (...)`, and
/// `WITHOUT OIDS` clauses.
///
/// Factoring `WITH` before choosing `OIDS` or `(` lets the parser use the
/// second token for disambiguation instead of committing one optional field
/// before it can try the other.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnsWithClause<'input> {
    With(#[tok(WITH, this)] ColumnsWithValue<'input>),
    #[tok(WITHOUT, OIDS)]
    WithoutOids,
}

/// `USING access_method` clause on CREATE TABLE, selecting a non-default
/// table access method (e.g. `heap`, `heap2`).
#[derive(recursa::Node, Debug, Clone)]
pub struct UsingAccessMethodClause<'input> {
    #[tok(USING, this)]
    pub method: literal::Ident<'input>,
}

/// Column-based table body: `(cols_and_constraints) [INHERITS (...)] [PARTITION BY ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct ColumnsBody<'input> {
    pub columns: TableElementList<'input>,
    pub inherits: Option<InheritsClause<'input>>,
    pub partition_by: Option<PartitionByClause<'input>>,
    pub using: Option<UsingAccessMethodClause<'input>>,
    pub with: Option<ColumnsWithClause<'input>>,
    pub on_commit: Option<OnCommitClause>,
    pub tablespace: Option<TablespaceClause<'input>>,
}

impl<'input> ColumnsBody<'input> {
    /// Legacy OIDS clause, when present.
    pub fn with_oids(&self) -> Option<WithOidsClause> {
        match self.with.as_ref() {
            Some(ColumnsWithClause::With(ColumnsWithValue::Oids)) => Some(WithOidsClause::WithOids),
            Some(ColumnsWithClause::WithoutOids) => Some(WithOidsClause::WithoutOids),
            Some(ColumnsWithClause::With(ColumnsWithValue::Storage(_))) | None => None,
        }
    }

    /// Storage parameters from `WITH (...)`, when present.
    pub fn with_storage(&self) -> Option<&[crate::ast::ddl::index::StorageParam<'input>]> {
        match self.with.as_ref() {
            Some(ColumnsWithClause::With(ColumnsWithValue::Storage(params))) => {
                Some(params.0.as_slice())
            }
            Some(ColumnsWithClause::With(ColumnsWithValue::Oids))
            | Some(ColumnsWithClause::WithoutOids)
            | None => None,
        }
    }
}

/// Parenthesized list of zero or more table elements.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct TableElementList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<ColumnOrConstraint<'input>>,
);

/// `ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP }` for temp tables.
///
/// Variant ordering: distinct first tokens (`PRESERVE` / `DELETE` / `DROP`),
/// so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub struct OnCommitClause {
    #[tok(ON, COMMIT, this)]
    pub action: OnCommitAction,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum OnCommitAction {
    #[tok(PRESERVE, ROWS)]
    PreserveRows,
    #[tok(DELETE, ROWS)]
    DeleteRows,
    #[tok(DROP)]
    Drop,
}

/// One entry inside a `PARTITION OF parent (...)` column-option list.
///
/// Unlike a full column definition, a partition column option omits the
/// column type — the type is inherited from the parent table. It is just
/// `name [WITH OPTIONS] [COLLATE "..."] [constraints...]`, or alternatively
/// a full table-level constraint (e.g. `CONSTRAINT c CHECK (...)`).
///
/// Variant ordering: `Constraint` (leading `CONSTRAINT` / `CHECK` /
/// `PRIMARY` / `UNIQUE` / `FOREIGN` keywords) comes before `Column` (a
/// bare identifier), so keyword-leading forms win.
#[derive(recursa::Node, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum PartitionColumnOption<'input> {
    Constraint(TableConstraint<'input>),
    Column(PartitionColumnOptionDef<'input>),
}

/// Per-partition column option: `name [WITH OPTIONS] [COLLATE "..."]
/// [constraints...]`. Overrides constraints/collation for a column
/// inherited from the partitioned parent table.
#[derive(recursa::Node, Debug, Clone)]
pub struct PartitionColumnOptionDef<'input> {
    pub name: literal::Ident<'input>,
    #[presence(WITH, OPTIONS)]
    pub with_options: bool,
    pub collate: Option<CollateClause<'input>>,
    pub constraints: Vec<ColumnConstraint<'input>>,
}

/// Optional parenthesized column-option list on typed and partition tables.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct PartitionColumnOptionList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<PartitionColumnOption<'input>>,
);

/// Partition-of table body: `PARTITION OF parent [(col_options, ...)] FOR VALUES IN (...) [PARTITION BY ...]`
///
/// The optional `(col_options, ...)` list is a per-partition override of
/// column constraints (e.g. `b NOT NULL`, `b DEFAULT 1`, `CONSTRAINT c CHECK
/// (...)`), reusing the same `ColumnOrConstraint` grammar as a columns-based
/// table body.
#[derive(recursa::Node, Debug, Clone)]
pub struct PartitionOfBody<'input> {
    #[tok(PARTITION, OF, this)]
    pub parent: crate::ast::shared::names::QualifiedName<'input>,
    pub column_options: Option<PartitionColumnOptionList<'input>>,
    pub for_values: Option<ForValuesClause<'input>>,
    #[presence(DEFAULT)]
    pub default: bool,
    pub partition_by: Option<PartitionByClause<'input>>,
    pub using: Option<UsingAccessMethodClause<'input>>,
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    pub on_commit: Option<OnCommitClause>,
    pub tablespace: Option<TablespaceClause<'input>>,
}

/// `OF type_name [(column_options)]` — typed-table body.
///
/// Creates a table whose columns are derived from a composite type.
/// Optional column options override constraints/defaults from the type.
#[derive(recursa::Node, Debug, Clone)]
pub struct OfTypeBody<'input> {
    #[tok(OF, this)]
    pub type_name: crate::ast::shared::names::QualifiedName<'input>,
    pub column_options: Option<PartitionColumnOptionList<'input>>,
}

/// AS-query table body: `AS SELECT ... [WITH [NO] DATA]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AsQueryBody<'input> {
    /// Optional `WITH (param = value, ...)` storage parameters before `AS`.
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    /// Optional `TABLESPACE name` before `AS`.
    pub tablespace: Option<TablespaceClause<'input>>,
    #[tok(AS, this)]
    pub query: Box<crate::ast::Statement<'input>>,
    pub with_data: Option<WithDataClause>,
}

/// `WITH DATA` or `WITH NO DATA` modifier on a CTAS query.
///
/// Variant ordering: `NoData` (`WITH NO DATA`, longer) before `Data`.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithDataClause {
    #[tok(WITH, NO, DATA)]
    NoData,
    #[tok(WITH, DATA)]
    Data,
}

/// `(col, col, ...) [ON COMMIT ...] AS query [WITH [NO] DATA]` — CTAS with column list.
#[derive(recursa::Node, Debug, Clone)]
pub struct ColumnsAsQueryBody<'input> {
    pub columns: CtasColumnList<'input>,
    pub on_commit: Option<OnCommitClause>,
    #[tok(AS, this)]
    pub query: Box<crate::ast::Statement<'input>>,
    pub with_data: Option<WithDataClause>,
}

/// Required, non-empty CTAS output-column list.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct CtasColumnList<'input> {
    #[sep(COMMA)]
    pub columns: recursa::Vec1<crate::tokens::ColId<'input>>,
}

/// The body of a CREATE TABLE statement after `CREATE [TEMP] TABLE name`.
///
/// Variant ordering: AsQuery (`AS`) and PartitionOf (`PARTITION`) start with
/// keywords; Columns starts with `(`. Longest-match-wins disambiguates.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateTableBody<'input> {
    AsQuery(AsQueryBody<'input>),
    PartitionOf(PartitionOfBody<'input>),
    /// `OF type_name [(column_options)]` — typed table.
    /// Distinct first token `OF`, no ambiguity with other variants.
    OfType(OfTypeBody<'input>),
    /// `(col, ...) AS query` — CTAS with explicit column list.
    /// Listed before `Columns` so the `( ... ) AS` form wins over the
    /// columns-only `( ... )` form via longer match.
    ColumnsAsQuery(ColumnsAsQueryBody<'input>),
    Columns(ColumnsBody<'input>),
}

/// ```sql
/// CREATE [TEMP] TABLE statement.
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, this)]
pub struct CreateTableStmt<'input> {
    pub temp: Option<TempKw>,
    #[tok(this, TABLE)]
    #[presence(UNLOGGED)]
    pub unlogged: bool,
    #[presence(IF, NOT, EXISTS)]
    pub if_not_exists: bool,
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    /// `USING am` between the table name and an `AS query` body, e.g.
    /// `CREATE TABLE t USING heap2 AS SELECT ...`. When the body starts
    /// with `(`, this clause is absent and `USING` appears after the
    /// column list inside `ColumnsBody`.
    pub using: Option<UsingAccessMethodClause<'input>>,
    pub body: CreateTableBody<'input>,
}

impl<'input> CreateTableStmt<'input> {
    /// Returns all items (columns + table-level constraints) of a
    /// columns-based CREATE TABLE.
    pub fn items(&self) -> Option<&[ColumnOrConstraint<'input>]> {
        match &self.body {
            CreateTableBody::Columns(b) => Some(b.columns.0.as_slice()),
            CreateTableBody::PartitionOf(_)
            | CreateTableBody::AsQuery(_)
            | CreateTableBody::ColumnsAsQuery(_)
            | CreateTableBody::OfType(_) => None,
        }
    }

    /// Returns only the column definitions (excluding table constraints).
    pub fn column_defs(&self) -> Option<Vec<&ColumnDef<'input>>> {
        self.items().map(|s| {
            s.iter()
                .filter_map(|item| match item {
                    ColumnOrConstraint::Column(c) => Some(c),
                    ColumnOrConstraint::Constraint(_) | ColumnOrConstraint::Like(_) => None,
                })
                .collect()
        })
    }
}

// -----------------------------------------------------------------------
// Partition table support — folded in from the former `ast/partition.rs`.
//
// `CREATE TABLE ... PARTITION BY LIST (col)`
// `CREATE TABLE ... PARTITION OF parent FOR VALUES IN (val, ...)`
// -----------------------------------------------------------------------

/// One partition key item: `{ column_name | ( expr ) } [COLLATE collation] [opclass_name]`.
///
/// The `opclass` operator class name is a trailing identifier (e.g.
/// `point_ops`, `int4_ops`) that binds the column/expression to a specific
/// operator class for the partition strategy.
#[derive(recursa::Node, Debug, Clone)]
pub struct PartitionKeyItem<'input> {
    pub expr: Expr<'input>,
    #[tok(COLLATE, this)]
    pub collate: Option<literal::AliasName<'input>>,
    pub opclass: Option<literal::AliasName<'input>>,
}

/// PARTITION BY LIST (col) clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct PartitionByClause<'input> {
    #[tok(PARTITION, BY, this)]
    pub strategy: literal::AliasName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    /// Partition key items — may be plain column names or expressions like
    /// `((a+b)/2)`, optionally followed by a trailing opclass name.
    pub columns: Vec<PartitionKeyItem<'input>>,
}

/// FOR VALUES IN (val, ...) clause — legacy name kept for backward compat
/// with partition.rs own tests; the general form lives in `ForValuesClause`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FOR, VALUES, IN, LPAREN, this, RPAREN)]
pub struct ForValuesInClause<'input> {
    #[sep(COMMA)]
    pub values: Vec<Expr<'input>>,
}

/// `FROM (...) TO (...)` range partition spec.
#[derive(recursa::Node, Debug, Clone)]
pub struct FromToSpec<'input> {
    #[tok(FROM, this)]
    pub from_values: PartitionValues<'input>,
    #[tok(TO, this)]
    pub to_values: PartitionValues<'input>,
}

/// Parenthesized value list in a partition bound.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct PartitionValues<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<Expr<'input>>,
);

/// `IN (val, ...)` list partition spec.
#[derive(recursa::Node, Debug, Clone)]
#[tok(IN, LPAREN, this, RPAREN)]
pub struct InListSpec<'input> {
    #[sep(COMMA)]
    pub values: Vec<Expr<'input>>,
}

/// `MODULUS n` entry.
#[derive(recursa::Node, Debug, Clone)]
pub struct ModulusEntry<'input> {
    #[tok(MODULUS, this)]
    pub value: Expr<'input>,
}

/// `REMAINDER n` entry.
#[derive(recursa::Node, Debug, Clone)]
pub struct RemainderEntry<'input> {
    #[tok(REMAINDER, this)]
    pub value: Expr<'input>,
}

/// One item in `WITH (...)` for hash partitioning: MODULUS n or REMAINDER n.
#[derive(recursa::Node, Debug, Clone)]
pub enum HashPartItem<'input> {
    Modulus(ModulusEntry<'input>),
    Remainder(RemainderEntry<'input>),
}

/// `WITH (MODULUS n, REMAINDER m)` hash partition spec.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WITH, LPAREN, this, RPAREN)]
pub struct WithModulusSpec<'input> {
    #[sep(COMMA)]
    pub items: Vec<HashPartItem<'input>>,
}

/// Body after `FOR VALUES` in a PARTITION OF clause. Variant ordering:
/// `From` starts with `FROM`, `In` starts with `IN`, `With` starts with `WITH` —
/// all distinct keywords, so peek disambiguation is trivial.
#[derive(recursa::Node, Debug, Clone)]
pub enum ForValuesSpec<'input> {
    From(FromToSpec<'input>),
    In(InListSpec<'input>),
    With(WithModulusSpec<'input>),
}

/// Full `FOR VALUES ...` clause in a `PARTITION OF ...` body.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForValuesClause<'input> {
    #[tok(FOR, VALUES, this)]
    pub spec: ForValuesSpec<'input>,
}

/// Column definition in partition table: `name type`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PartitionColumnDef<'input> {
    pub name: literal::Ident<'input>,
    pub type_name: TypeName<'input>,
}

/// Parenthesized column-definition list of a standalone partitioned table.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct PartitionColumnDefList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<PartitionColumnDef<'input>>,
);

/// CREATE TABLE with PARTITION BY: `CREATE TABLE name (cols) PARTITION BY strategy (cols)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreatePartitionedTableStmt<'input> {
    #[tok(CREATE, TABLE, this)]
    pub name: literal::Ident<'input>,
    pub columns: PartitionColumnDefList<'input>,
    pub partition_by: PartitionByClause<'input>,
}

/// CREATE TABLE ... PARTITION OF parent FOR VALUES IN (...) [PARTITION BY ...].
#[derive(recursa::Node, Debug, Clone)]
pub struct CreatePartitionOfStmt<'input> {
    #[tok(CREATE, TABLE, this)]
    pub name: literal::Ident<'input>,
    #[tok(PARTITION, OF, this)]
    pub parent: literal::Ident<'input>,
    pub for_values: ForValuesInClause<'input>,
    pub partition_by: Option<PartitionByClause<'input>>,
}

// -----------------------------------------------------------------------
// DROP TABLE — folded in from the former `ast/drop_table.rs`.
// -----------------------------------------------------------------------

/// ```sql
/// DROP TABLE [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct DropTableStmt<'input> {
    #[tok(DROP, TABLE, this)]
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    pub names: Vec<QualifiedName<'input>>,
    pub behavior: Option<DropBehavior>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/table.tests.rs"
));

// =========================================================================
// ALTER/DROP TABLE — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `ALTER TABLE ...` — Postgres' `AlterTableStmt` (table object kind), plus
/// the table-shaped branches of `RenameStmt` and `AlterObjectSchemaStmt`.
///
/// pg-sql's dispatcher commits at `ALTER TABLE`, so this one struct must cover
/// every shape that begins with those two keywords:
///
/// - `ALTER TABLE [IF EXISTS] [ONLY] name [*] alter_table_cmds`
/// - `ALTER TABLE [IF EXISTS] [ONLY] name [*] partition_cmd`
/// - `ALTER TABLE [IF EXISTS] name RENAME TO new`
/// - `ALTER TABLE [IF EXISTS] [ONLY] name [*] RENAME [COLUMN] old TO new`
/// - `ALTER TABLE [IF EXISTS] [ONLY] name [*] RENAME CONSTRAINT old TO new`
/// - `ALTER TABLE [IF EXISTS] name SET SCHEMA new`
/// - `ALTER TABLE ALL IN TABLESPACE name [OWNED BY roles] SET TABLESPACE new
///    [NOWAIT]`
///
/// The two top-level shapes — the bulk `ALL IN TABLESPACE …` form and the
/// per-relation form — are split into an enum body.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTableStmt<'input> {
    #[tok(ALTER, TABLE, this)]
    pub body: AlterTableBody<'input>,
}

/// Body of `ALTER TABLE ...` — either the bulk-relocate `ALL IN TABLESPACE`
/// form or a per-relation form.
///
/// Variant ordering: `All` (starts with `ALL`) before `Single` (starts with
/// `IF` / `ONLY` / `qualified_name`, never `ALL`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTableBody<'input> {
    All(AllInTablespaceBody<'input>),
    Single(AlterTableSingle<'input>),
}

/// Per-relation `ALTER TABLE` body: `[IF EXISTS] [ONLY] name [*] action`.
///
/// The relation reference is Postgres' `relation_expr`: a `qualified_name`
/// optionally prefixed by `ONLY` and/or suffixed by `*`. The `ONLY (name)`
/// parenthesised form is not exercised by any corpus statement, so it is
/// not modelled.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTableSingle<'input> {
    pub if_exists: Option<IfExists>,
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
    pub action: AlterTableSingleAction<'input>,
}

/// One action on a per-relation `ALTER TABLE` body — covers Postgres'
/// `alter_table_cmds`, `partition_cmd`, `RenameStmt` (table/column/constraint
/// rename), and `AlterObjectSchemaStmt` (SET SCHEMA) for tables.
///
/// Variant ordering:
/// - `RenameConstraint` (`RENAME CONSTRAINT …`) before `RenameColumn`
///   (`RENAME [COLUMN] …`) before `Rename` (`RENAME TO …`) — all start with
///   `RENAME`; `Rename` succeeds only when the second token is `TO`,
///   `RenameColumn` only when the second is `COLUMN` or an ident, and
///   `RenameConstraint` only when the second is `CONSTRAINT`.
/// - `SetSchema` (`SET SCHEMA name`) before `Cmds` — both can begin with
///   `SET`, but `SET SCHEMA` is not in `alter_table_cmd` so the parser must
///   try it first.
/// - `Partition` (`ATTACH PARTITION` / `DETACH PARTITION`) before `Cmds` —
///   `alter_table_cmd` does not begin with `ATTACH`/`DETACH`, but listing
///   the partition cmd first is clearer.
/// - `Cmds` last — the catch-all for the comma-separated `alter_table_cmds`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTableSingleAction<'input> {
    RenameConstraint(AlterTableRenameConstraint<'input>),
    RenameColumn(RenameColumnClause<'input>),
    Rename(RenameTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    Partition(PartitionCmd<'input>),
    Cmds(AlterTableCmds<'input>),
}

/// `RENAME CONSTRAINT old TO new` — Postgres' `RenameStmt` branch for table
/// constraints.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTableRenameConstraint<'input> {
    #[tok(RENAME, CONSTRAINT, this)]
    pub old_name: literal::Ident<'input>,
    #[tok(TO, this)]
    pub new_name: literal::Ident<'input>,
}

/// Comma-separated `alter_table_cmds` on ALTER TABLE.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTableCmds<'input> {
    #[sep(COMMA)]
    pub cmds: recursa::Vec1<AlterTableCmd<'input>>,
}

/// Postgres' `partition_cmd`: a single `ATTACH PARTITION` or `DETACH
/// PARTITION` action on a partitioned table.
///
/// Variant ordering: `Attach` (ATTACH) and `Detach` (DETACH) have disjoint
/// first tokens, so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum PartitionCmd<'input> {
    Attach(AttachPartitionCmd<'input>),
    Detach(DetachPartitionCmd<'input>),
}

/// `ATTACH PARTITION qualified_name partition_bound_spec` — adds an existing
/// table as a partition of the target partitioned table.
#[derive(recursa::Node, Debug, Clone)]
pub struct AttachPartitionCmd<'input> {
    #[tok(ATTACH, PARTITION, this)]
    pub name: QualifiedName<'input>,
    pub bound: PartitionBoundSpec<'input>,
}

/// `DETACH PARTITION qualified_name [CONCURRENTLY | FINALIZE]` — removes a
/// partition from its parent.
#[derive(recursa::Node, Debug, Clone)]
pub struct DetachPartitionCmd<'input> {
    #[tok(DETACH, PARTITION, this)]
    pub name: QualifiedName<'input>,
    pub mode: Option<DetachPartitionMode>,
}

/// Trailing mode keyword on `DETACH PARTITION`: `CONCURRENTLY` (the default
/// nonblocking detach) or `FINALIZE` (completes a previously-CONCURRENTLY
/// detached partition).
#[derive(recursa::Node, Debug, Clone)]
pub enum DetachPartitionMode {
    #[tok(CONCURRENTLY)]
    Concurrently,
    #[tok(FINALIZE)]
    Finalize,
}

/// Postgres' `PartitionBoundSpec` — the partition bound used by `ATTACH
/// PARTITION`. One of:
///
/// - `DEFAULT` (the catch-all partition)
/// - `FOR VALUES IN (val, ...)` (list)
/// - `FOR VALUES FROM (...) TO (...)` (range)
/// - `FOR VALUES WITH (MODULUS n, REMAINDER m)` (hash)
///
/// Variant ordering: `Default` (one keyword, distinct first token) before
/// `ForValues` (begins with `FOR`).
#[derive(recursa::Node, Debug, Clone)]
pub enum PartitionBoundSpec<'input> {
    #[tok(DEFAULT)]
    Default,
    ForValues(crate::ast::ddl::table::ForValuesClause<'input>),
}

/// A single `alter_table_cmd` — one comma-separated entry in `alter_table_cmds`.
///
/// Variant ordering:
/// - `ADD …` family: longer-prefix variants first. `AddColumnIfNotExists`
///   (4 keywords `ADD COLUMN IF NOT EXISTS`) before `AddIfNotExists`
///   (`ADD IF NOT EXISTS`) before `AddColumn` (`ADD COLUMN`) before
///   `AddConstraint` (`ADD …` table-constraint) before `AddColumnBare`
///   (`ADD coldef`).
/// - `ALTER …` family: `AlterColumnCmd` (matches `ALTER [COLUMN] colname …`).
/// - `ALTER CONSTRAINT name …` is a separate top-level variant, listed
///   before `AlterColumnCmd` so the `CONSTRAINT` keyword wins.
/// - `DROP …` family: `DropConstraintIfExists` (5 tokens), `DropConstraint`,
///   `DropColumnIfExists` (with optional COLUMN), `DropColumn`.
/// - `ENABLE`/`DISABLE`: multi-token `ENABLE REPLICA TRIGGER` / `ENABLE
///   ALWAYS TRIGGER` before `ENABLE TRIGGER`; same for RULE; `ENABLE ROW
///   LEVEL SECURITY` and `DISABLE ROW LEVEL SECURITY`.
/// - `SET WITHOUT CLUSTER` / `SET WITHOUT OIDS` / `SET LOGGED` /
///   `SET UNLOGGED` / `SET ACCESS METHOD` / `SET TABLESPACE` /
///   `SET (reloptions)` — all start with `SET` but each disambiguates on
///   the second token.
/// - `RESET (reloptions)` — disjoint from `SET …`.
/// - `CLUSTER ON name`, `INHERIT name`, `NO INHERIT name`, `OF type_name`,
///   `NOT OF`, `OWNER TO role`, `REPLICA IDENTITY …`, `FORCE ROW LEVEL
///   SECURITY`, `NO FORCE ROW LEVEL SECURITY`, `VALIDATE CONSTRAINT name`,
///   `DEPENDS ON EXTENSION name` / `NO DEPENDS ON EXTENSION name` — each
///   commits on a unique leading keyword (with `NO …` and `NOT …` carefully
///   placed against single-keyword variants).
/// - `GenericOptions` (FOREIGN-TABLE OPTIONS clause) last — `OPTIONS` is a
///   unique leading keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTableCmd<'input> {
    // ADD ... — longer prefixes first.
    AddColumnIfNotExists(AddColumnIfNotExistsCmd<'input>),
    AddIfNotExists(AddIfNotExistsCmd<'input>),
    AddColumn(AddColumnCmd<'input>),
    AddConstraint(AddTableConstraintCmd<'input>),
    AddColumnBare(AddColumnBareCmd<'input>),
    // ALTER CONSTRAINT ...
    AlterConstraint(AlterConstraintCmd<'input>),
    // ALTER [COLUMN] colname ...
    AlterColumn(AlterColumnCmd<'input>),
    // DROP ... — longer prefixes first.
    DropConstraintIfExists(DropConstraintIfExistsCmd<'input>),
    DropConstraint(DropConstraintCmd<'input>),
    DropColumnIfExists(DropColumnIfExistsCmd<'input>),
    DropColumn(DropColumnCmd<'input>),
    // ENABLE / DISABLE variants — longer prefixes first.
    EnableReplicaTrigger(EnableReplicaTriggerCmd<'input>),
    EnableAlwaysTrigger(EnableAlwaysTriggerCmd<'input>),
    EnableReplicaRule(EnableReplicaRuleCmd<'input>),
    EnableAlwaysRule(EnableAlwaysRuleCmd<'input>),
    EnableTrigger(EnableTriggerCmd<'input>),
    EnableRule(EnableRuleCmd<'input>),
    EnableRowSecurity(EnableRowSecurityCmd),
    DisableTrigger(DisableTriggerCmd<'input>),
    DisableRule(DisableRuleCmd<'input>),
    DisableRowSecurity(DisableRowSecurityCmd),
    // FORCE / NO FORCE ROW LEVEL SECURITY — NO FORCE listed first.
    NoForceRowSecurity(NoForceRowSecurityCmd),
    ForceRowSecurity(ForceRowSecurityCmd),
    // CLUSTER ON / SET WITHOUT CLUSTER.
    ClusterOn(ClusterOnCmd<'input>),
    // SET ... variants — longest prefixes first.
    SetWithoutCluster(SetWithoutClusterCmd),
    SetWithoutOids(SetWithoutOidsCmd),
    SetLogged(SetLoggedCmd),
    SetUnlogged(SetUnloggedCmd),
    SetAccessMethod(SetAccessMethodClause<'input>),
    SetTablespace(SetTablespaceClause<'input>),
    SetReloptions(SetReloptions<'input>),
    ResetReloptions(ResetReloptions<'input>),
    // REPLICA IDENTITY ...
    ReplicaIdentity(ReplicaIdentityCmd<'input>),
    // INHERIT / NO INHERIT — NO INHERIT listed first.
    NoInherit(NoInheritCmd<'input>),
    Inherit(InheritCmd<'input>),
    // OF / NOT OF.
    NotOf(NotOfCmd),
    Of(OfCmd<'input>),
    // OWNER TO role.
    Owner(OwnerTo<'input>),
    // VALIDATE CONSTRAINT name.
    ValidateConstraint(ValidateConstraintCmd<'input>),
    // [NO] DEPENDS ON EXTENSION name.
    DependsOnExtension(DependsOnExtension<'input>),
    // OPTIONS (...)  — foreign-table alter_generic_options. Listed last so
    // every keyword-led variant above wins first.
    GenericOptions(AlterGenericOptions<'input>),
}

/// `ADD COLUMN IF NOT EXISTS columnDef`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AddColumnIfNotExistsCmd<'input> {
    #[tok(ADD, COLUMN, this)]
    pub if_not_exists: IfNotExists,
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ADD IF NOT EXISTS columnDef`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AddIfNotExistsCmd<'input> {
    #[tok(ADD, this)]
    pub if_not_exists: IfNotExists,
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ADD COLUMN columnDef`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AddColumnCmd<'input> {
    #[tok(ADD, COLUMN, this)]
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ADD TableConstraint [NOT VALID]` — table-level constraint with optional
/// `NOT VALID` marker (Postgres routes this through `ConstraintAttributeSpec`
/// on the constraint).
///
/// The `NOT VALID` modifier is part of the constraint's attribute list in
/// gram.y; pg-sql models it as a trailing `Option` on this `AlterTableCmd`
/// variant for symmetry with the corpus' usage (it only ever sits at the end).
#[derive(recursa::Node, Debug, Clone)]
pub struct AddTableConstraintCmd<'input> {
    #[tok(ADD, this)]
    pub constraint: crate::ast::ddl::table::TableConstraint<'input>,
    pub not_valid: Option<NotValid>,
}

/// `NOT VALID` — the unverified-constraint marker.
#[derive(recursa::Node, Debug, Clone)]
pub enum NotValid {
    #[tok(NOT, VALID)]
    Value,
}

/// `ADD columnDef` (no `COLUMN` keyword, no `IF NOT EXISTS`).
///
/// Listed last in the ADD family because every column definition begins with
/// a bareword (the column name), which would otherwise greedily swallow
/// `COLUMN`, `IF`, `CONSTRAINT`, etc.
#[derive(recursa::Node, Debug, Clone)]
pub struct AddColumnBareCmd<'input> {
    #[tok(ADD, this)]
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ALTER CONSTRAINT name [DEFERRABLE | NOT DEFERRABLE] [INITIALLY {DEFERRED
/// | IMMEDIATE}]` — Postgres' `AT_AlterConstraint` action.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterConstraintCmd<'input> {
    #[tok(ALTER, CONSTRAINT, this)]
    pub name: literal::Ident<'input>,
    pub attrs: crate::ast::ddl::table::ConstraintAttrs,
}

/// `ALTER [COLUMN] colname …` — the big `ALTER COLUMN` cmd. The `colname`
/// can also be a numeric column index for `SET STATISTICS` (used on indexes,
/// not on tables — but accepted here for symmetry).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnCmd<'input> {
    #[tok(ALTER, optional(COLUMN), this)]
    pub col_ref: ColumnRef<'input>,
    pub action: AlterColumnAction<'input>,
}

/// One action on `ALTER [COLUMN] colname …` — the full per-column command
/// space.
///
/// Variant ordering: longer/more-specific prefixes first.
/// - `SET …` family: `SET EXPRESSION AS (expr)` (3 keywords before `(`),
///   `SET DATA TYPE Typename …` (SET DATA TYPE), `SET STATISTICS value`,
///   `SET COMPRESSION`, `SET STORAGE`, `SET DEFAULT expr` (followed by an
///   expression), `SET NOT NULL`, `SET (reloptions)`. Each disambiguates on
///   the second token after `SET`.
/// - `DROP …` family: `DROP EXPRESSION [IF EXISTS]`, `DROP IDENTITY [IF
///   EXISTS]`, `DROP NOT NULL`, `DROP DEFAULT`. Each disambiguates on the
///   second token after `DROP`.
/// - `ADD GENERATED … AS IDENTITY [(opts)]` — `ADD` is unique.
/// - `RESET (reloptions)` — `RESET` is unique.
/// - `TYPE Typename [COLLATE …] [USING expr]` — bare `TYPE` form (the
///   `SET DATA` is optional in gram.y).
/// - `IdentityOpts` — `alter_identity_column_option_list` (one or more of
///   `SET GENERATED {ALWAYS|BY DEFAULT}` / `SET seq_option` /
///   `RESTART [WITH n]`). Chained via the no-separator `Seq1` shape; covers
///   both the single-element form (e.g. `SET GENERATED ALWAYS` alone) and
///   the multi-element form (`SET GENERATED BY DEFAULT SET INCREMENT BY 2
///   RESTART`). Listed last in the SET/RESTART family so the longer-prefix
///   SET variants commit first; the bare `RESTART` keyword is unique and
///   only matched here.
/// - `GenericOptions` (`OPTIONS (...)`) — foreign-table column options.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColumnAction<'input> {
    // SET ... — longest prefixes first.
    SetExpressionAs(AlterColSetExpression<'input>),
    SetDataType(AlterColSetDataType<'input>),
    SetStatistics(AlterColSetStatistics<'input>),
    SetCompression(AlterColSetCompression<'input>),
    SetStorage(AlterColSetStorage),
    SetNotNull(AlterColSetNotNull),
    SetDefault(AlterColSetDefault<'input>),
    SetReloptions(SetReloptions<'input>),
    // DROP ... — longest prefixes first.
    DropExpression(AlterColDropExpression),
    DropIdentity(AlterColDropIdentity),
    DropNotNull(AlterColDropNotNull),
    DropDefault(AlterColDropDefault),
    // ADD GENERATED ... AS IDENTITY [(opts)]
    AddIdentity(AlterColAddIdentity<'input>),
    // RESET (reloptions)
    ResetReloptions(ResetReloptions<'input>),
    // TYPE Typename [COLLATE …] [USING expr] — without leading SET DATA.
    Type(AlterColTypeBare<'input>),
    // alter_identity_column_option_list — chained SET GENERATED / SET
    // seq_option / RESTART [WITH n] items (single- or multi-element).
    IdentityOpts(AlterIdentityOpts<'input>),
    // FOREIGN-TABLE column OPTIONS (...).
    GenericOptions(AlterGenericOptions<'input>),
}

/// One element of `alter_identity_column_option_list` (gram.y):
/// `SET GENERATED {ALWAYS|BY DEFAULT}` | `SET seq_option` | `RESTART [WITH n]`.
///
/// Variant ordering: `SetGenerated` (`SET GENERATED …`) before
/// `SetSeqOption` (`SET …seq_option`) so the more specific `SET GENERATED`
/// 2-token peek commits first; both share the leading `SET`. `Restart`
/// has a disjoint leading `RESTART` token.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterIdentityOption<'input> {
    SetGenerated(AlterColSetGenerated),
    SetSeqOption(AlterColSetSeqOption<'input>),
    Restart(AlterColRestart<'input>),
}

/// `alter_identity_column_option_list` — one or more
/// [`AlterIdentityOption`] items in sequence, no separator.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterIdentityOpts<'input> {
    pub items: recursa::Vec1<AlterIdentityOption<'input>>,
}

/// `SET EXPRESSION AS (expr)` — adjust a generated column's expression.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetExpression<'input> {
    #[tok(SET, EXPRESSION, AS, LPAREN, this, RPAREN)]
    pub expr: Box<Expr<'input>>,
}

/// `[SET DATA] TYPE Typename [COLLATE name] [USING expr]` — change a column's
/// type. The `SET DATA` is mandatory in this variant (the leading-`SET` form);
/// the bare `TYPE …` form is `AlterColTypeBare`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetDataType<'input> {
    #[tok(SET, DATA, TYPE, this)]
    pub type_name: CastType<'input>,
    pub collate: Option<crate::ast::ddl::table::CollateClause<'input>>,
    pub using: Option<AlterColUsing<'input>>,
}

/// `SET STATISTICS { SignedIconst | DEFAULT }` — adjust per-column statistics
/// target.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetStatistics<'input> {
    #[tok(SET, STATISTICS, this)]
    pub value: SetStatisticsValue<'input>,
}

/// `SET COMPRESSION { name | DEFAULT }` — change a column's compression
/// method.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetCompression<'input> {
    #[tok(SET, COMPRESSION, this)]
    pub target: ColumnCompressionTarget<'input>,
}

/// `SET DEFAULT expr` — set a column's default expression.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetDefault<'input> {
    #[tok(SET, DEFAULT, this)]
    pub expr: Box<Expr<'input>>,
}

/// `DROP DEFAULT` — drop a column's default expression.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColDropDefault {
    #[tok(DROP, DEFAULT)]
    Value,
}

/// `USING expr` clause on `ALTER COLUMN … TYPE …`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColUsing<'input> {
    #[tok(USING, this)]
    pub expr: Box<Expr<'input>>,
}

/// `TYPE Typename [COLLATE name] [USING expr]` — change a column's type
/// without the leading `SET DATA`. Postgres accepts both spellings.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColTypeBare<'input> {
    #[tok(TYPE, this)]
    pub type_name: CastType<'input>,
    pub collate: Option<crate::ast::ddl::table::CollateClause<'input>>,
    pub using: Option<AlterColUsing<'input>>,
}

/// `SET GENERATED { ALWAYS | BY DEFAULT }` — change the identity column
/// generation mode.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetGenerated {
    #[tok(SET, GENERATED, this)]
    pub mode: crate::ast::ddl::table::GeneratedIdentityMode,
}

/// `SET STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT }` — adjust a
/// column's TOAST storage strategy.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetStorage {
    #[tok(SET, STORAGE, this)]
    pub mode: crate::ast::ddl::table::ColumnStorageMode,
}

/// `SET NOT NULL` — add a NOT NULL marker on the column.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColSetNotNull {
    #[tok(SET, NOT, NULL)]
    Value,
}

/// One `SET seqOpt` action on an identity column —
/// `SET { START WITH | INCREMENT BY | MINVALUE | MAXVALUE | CACHE | CYCLE |
/// NO MINVALUE | NO MAXVALUE | NO CYCLE }`.
///
/// Reuses `IdentitySeqOption` from `create_table.rs` so the full sequence-
/// option set is supported (and the formatter is shared).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColSetSeqOption<'input> {
    #[tok(SET, this)]
    pub option: crate::ast::ddl::table::IdentitySeqOption<'input>,
}

/// `DROP EXPRESSION [IF EXISTS]` — remove a generated column's expression,
/// turning it into a regular column.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, EXPRESSION, this)]
pub struct AlterColDropExpression {
    pub if_exists: Option<IfExists>,
}

/// `DROP IDENTITY [IF EXISTS]` — remove an identity-column property.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, IDENTITY, this)]
pub struct AlterColDropIdentity {
    pub if_exists: Option<IfExists>,
}

/// `DROP NOT NULL` — remove a NOT NULL marker.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColDropNotNull {
    #[tok(DROP, NOT, NULL)]
    Value,
}

/// `ADD GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY [(seq_options)]` —
/// add an identity property to an existing column.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColAddIdentity<'input> {
    #[tok(ADD, this)]
    pub identity: crate::ast::ddl::table::GeneratedIdentityConstraint<'input>,
}

/// `RESTART [WITH NumericOnly]` — restart an identity column's sequence.
#[derive(recursa::Node, Debug, Clone)]
#[tok(RESTART, this)]
pub struct AlterColRestart<'input> {
    pub value: Option<RestartWith<'input>>,
}

/// `[WITH] NumericOnly` — the value portion of `RESTART`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RestartWith<'input> {
    #[tok(optional(WITH), this)]
    pub value: NumericOnly<'input>,
}

/// `DROP CONSTRAINT IF EXISTS name [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropConstraintIfExistsCmd<'input> {
    #[tok(DROP, CONSTRAINT, this)]
    pub if_exists: IfExists,
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP CONSTRAINT name [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropConstraintCmd<'input> {
    #[tok(DROP, CONSTRAINT, this)]
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP [COLUMN] IF EXISTS colname [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropColumnIfExistsCmd<'input> {
    #[tok(DROP, optional(COLUMN), this)]
    pub if_exists: IfExists,
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP [COLUMN] colname [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropColumnCmd<'input> {
    #[tok(DROP, optional(COLUMN), this)]
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `ENABLE TRIGGER { name | ALL | USER }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EnableTriggerCmd<'input> {
    #[tok(ENABLE, TRIGGER, this)]
    pub target: TriggerOrRuleTarget<'input>,
}

/// `ENABLE ALWAYS TRIGGER name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EnableAlwaysTriggerCmd<'input> {
    #[tok(ENABLE, ALWAYS, TRIGGER, this)]
    pub name: literal::Ident<'input>,
}

/// `ENABLE REPLICA TRIGGER name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EnableReplicaTriggerCmd<'input> {
    #[tok(ENABLE, REPLICA, TRIGGER, this)]
    pub name: literal::Ident<'input>,
}

/// `DISABLE TRIGGER { name | ALL | USER }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DisableTriggerCmd<'input> {
    #[tok(DISABLE, TRIGGER, this)]
    pub target: TriggerOrRuleTarget<'input>,
}

/// `ENABLE RULE name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EnableRuleCmd<'input> {
    #[tok(ENABLE, RULE, this)]
    pub name: literal::Ident<'input>,
}

/// `ENABLE ALWAYS RULE name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EnableAlwaysRuleCmd<'input> {
    #[tok(ENABLE, ALWAYS, RULE, this)]
    pub name: literal::Ident<'input>,
}

/// `ENABLE REPLICA RULE name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct EnableReplicaRuleCmd<'input> {
    #[tok(ENABLE, REPLICA, RULE, this)]
    pub name: literal::Ident<'input>,
}

/// `DISABLE RULE name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DisableRuleCmd<'input> {
    #[tok(DISABLE, RULE, this)]
    pub name: literal::Ident<'input>,
}

/// Trigger-action target on `ENABLE TRIGGER` / `DISABLE TRIGGER`:
/// `ALL` (every trigger), `USER` (every non-internal trigger), or a named
/// trigger.
///
/// Variant ordering: keyword variants (`All` / `User`) before `Name` (ident),
/// since `ALL` and `USER` are hard keywords that won't lex as `Ident`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TriggerOrRuleTarget<'input> {
    #[tok(ALL)]
    All,
    #[tok(USER)]
    User,
    Name(literal::Ident<'input>),
}

/// `ENABLE ROW LEVEL SECURITY`.
#[derive(recursa::Node, Debug, Clone)]
pub enum EnableRowSecurityCmd {
    #[tok(ENABLE, ROW, LEVEL, SECURITY)]
    Value,
}

/// `DISABLE ROW LEVEL SECURITY`.
#[derive(recursa::Node, Debug, Clone)]
pub enum DisableRowSecurityCmd {
    #[tok(DISABLE, ROW, LEVEL, SECURITY)]
    Value,
}

/// `FORCE ROW LEVEL SECURITY`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ForceRowSecurityCmd {
    #[tok(FORCE, ROW, LEVEL, SECURITY)]
    Value,
}

/// `NO FORCE ROW LEVEL SECURITY`.
#[derive(recursa::Node, Debug, Clone)]
pub enum NoForceRowSecurityCmd {
    #[tok(NO, FORCE, ROW, LEVEL, SECURITY)]
    Value,
}

/// `CLUSTER ON indexname`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ClusterOnCmd<'input> {
    #[tok(CLUSTER, ON, this)]
    pub name: literal::Ident<'input>,
}

/// `SET WITHOUT CLUSTER`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetWithoutClusterCmd {
    #[tok(SET, WITHOUT, CLUSTER)]
    Value,
}

/// `SET WITHOUT OIDS`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetWithoutOidsCmd {
    #[tok(SET, WITHOUT, OIDS)]
    Value,
}

/// `SET LOGGED`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetLoggedCmd {
    #[tok(SET, LOGGED)]
    Value,
}

/// `SET UNLOGGED`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetUnloggedCmd {
    #[tok(SET, UNLOGGED)]
    Value,
}

/// `REPLICA IDENTITY { DEFAULT | NOTHING | FULL | USING INDEX name }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ReplicaIdentityCmd<'input> {
    #[tok(REPLICA, IDENTITY, this)]
    pub kind: ReplicaIdentityKind<'input>,
}

/// One of `DEFAULT`, `NOTHING`, `FULL`, or `USING INDEX name`.
///
/// Variant ordering: keyword-only variants (single tokens, disjoint) first;
/// `UsingIndex` (`USING INDEX`) last — it has a unique `USING` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum ReplicaIdentityKind<'input> {
    #[tok(DEFAULT)]
    Default,
    #[tok(NOTHING)]
    Nothing,
    #[tok(FULL)]
    Full,
    UsingIndex(ReplicaIdentityUsingIndex<'input>),
}

/// `USING INDEX name` — the index-backed REPLICA IDENTITY.
#[derive(recursa::Node, Debug, Clone)]
pub struct ReplicaIdentityUsingIndex<'input> {
    #[tok(USING, INDEX, this)]
    pub name: literal::Ident<'input>,
}

/// `INHERIT parent`.
#[derive(recursa::Node, Debug, Clone)]
pub struct InheritCmd<'input> {
    #[tok(INHERIT, this)]
    pub parent: QualifiedName<'input>,
}

/// `NO INHERIT parent`.
#[derive(recursa::Node, Debug, Clone)]
pub struct NoInheritCmd<'input> {
    #[tok(NO, INHERIT, this)]
    pub parent: QualifiedName<'input>,
}

/// `OF type_name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OfCmd<'input> {
    #[tok(OF, this)]
    pub type_name: QualifiedName<'input>,
}

/// `NOT OF` — drop the typed-table relationship.
#[derive(recursa::Node, Debug, Clone)]
pub enum NotOfCmd {
    #[tok(NOT, OF)]
    Value,
}

/// `VALIDATE CONSTRAINT name`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ValidateConstraintCmd<'input> {
    #[tok(VALIDATE, CONSTRAINT, this)]
    pub name: literal::Ident<'input>,
}
