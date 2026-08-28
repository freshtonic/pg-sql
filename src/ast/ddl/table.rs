/// CREATE TABLE statement AST.
use recursa::seq::{OptionalTrailing, Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::shared::expr::{Expr, TypeName};
use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::{literal, punct};

use crate::tokens::keyword::*;
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
use crate::tokens::soft_keyword::*;
// ---------------------------------------------------------------------------
/// `USING INDEX TABLESPACE name` — tablespace for the index backing a PRIMARY
/// KEY or UNIQUE column constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UsingIndexTablespace<'input> {
    pub using: USING,
    pub index: INDEX,
    pub tablespace: TABLESPACE,
    pub name: literal::Ident<'input>,
}

/// PRIMARY KEY column constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PrimaryKeyConstraint<'input> {
    pub primary: PRIMARY,
    pub key: KEY,
    pub index_tablespace: Option<UsingIndexTablespace<'input>>,
    /// Optional `[NOT] DEFERRABLE [INITIALLY {DEFERRED|IMMEDIATE}]` suffix.
    pub attrs: ConstraintAttrs,
}

/// UNIQUE column constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UniqueConstraint<'input> {
    pub unique: UNIQUE,
    /// Optional `NULLS [NOT] DISTINCT` qualifier (Postgres 15+).
    pub nulls: Option<NullsDistinctQualifier>,
    pub index_tablespace: Option<UsingIndexTablespace<'input>>,
    /// Optional `[NOT] DEFERRABLE [INITIALLY ...]` attributes.
    pub attrs: ConstraintAttrs,
}

/// `NULLS DISTINCT` or `NULLS NOT DISTINCT` for UNIQUE constraints.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NullsDistinctQualifier {
    pub nulls: NULLS,
    pub not: Option<NOT>,
    pub distinct: DISTINCT,
}

/// Referential action for `ON DELETE` / `ON UPDATE`.
///
/// Variant ordering: multi-word variants (`NO ACTION`, `SET NULL`, `SET DEFAULT`)
/// come before single-word ones to satisfy longest-match.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ReferentialAction<'input> {
    NoAction((NO, ACTION)),
    SetNull(SetNullKw<'input>),
    SetDefault(SetDefaultKw<'input>),
    Cascade(CASCADE),
    Restrict(RESTRICT),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetNullKw<'input> {
    pub set: SET,
    pub null: NULL,
    pub cols: Option<
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    >,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetDefaultKw<'input> {
    pub set: SET,
    pub default: DEFAULT,
    pub cols: Option<
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    >,
}

/// `ON DELETE action`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OnDeleteAction<'input> {
    pub on: ON,
    pub delete: DELETE,
    pub action: ReferentialAction<'input>,
}

/// `ON UPDATE action`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OnUpdateAction<'input> {
    pub on: ON,
    pub update: UPDATE,
    pub action: ReferentialAction<'input>,
}

/// Match type for a foreign key: `MATCH FULL | PARTIAL | SIMPLE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum MatchKind {
    Full(FULL),
    Partial(PARTIAL),
    Simple(SIMPLE),
}

/// `MATCH FULL | MATCH PARTIAL | MATCH SIMPLE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct MatchClause {
    pub r#match: MATCH,
    pub kind: MatchKind,
}

/// `DEFERRABLE | NOT DEFERRABLE`.
///
/// Variant ordering: `NotDeferrable` (two keywords) before `Deferrable`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DeferrableKind {
    NotDeferrable((NOT, DEFERRABLE)),
    Deferrable(DEFERRABLE),
}

/// `INITIALLY DEFERRED | INITIALLY IMMEDIATE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InitiallyClause {
    pub initially: INITIALLY,
    pub mode: InitiallyMode,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum InitiallyMode {
    Deferred(DEFERRED),
    Immediate(IMMEDIATE),
}

/// `ON DELETE ...` or `ON UPDATE ...` trailing action on a REFERENCES
/// constraint. Modeled as an enum so both orders of the two clauses
/// are accepted via a [`Vec`]`<`[`OnAction`]`>`.
///
/// Variant ordering: both start with `ON`; they diverge at the next keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OnAction<'input> {
    OnDelete(OnDeleteAction<'input>),
    OnUpdate(OnUpdateAction<'input>),
}

/// REFERENCES constraint:
/// `REFERENCES table [(col, ...)] [MATCH ...] [ON DELETE|UPDATE ...]* [DEFERRABLE | NOT DEFERRABLE] [INITIALLY ...]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReferencesConstraint<'input> {
    pub references: REFERENCES,
    pub table: crate::ast::shared::names::QualifiedName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
    pub match_clause: Option<MatchClause>,
    pub actions: Vec<OnAction<'input>>,
    pub deferrable: Option<DeferrableKind>,
    pub initially: Option<InitiallyClause>,
    pub not_valid: Option<(NOT, VALID)>,
}

/// `CHECK (expr) [NO INHERIT] [NOT VALID]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CheckConstraint<'input> {
    pub check: CHECK,
    pub expr: Surrounded<punct::LParen, crate::ast::shared::expr::Expr<'input>, punct::RParen>,
    pub no_inherit: Option<(NO, INHERIT)>,
    pub not_valid: Option<(NOT, VALID)>,
}

/// `GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY` modifier.
///
/// Variant ordering: both start with a distinct keyword after `GENERATED`
/// (`ALWAYS` vs `BY`), so order is cosmetic.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum GeneratedIdentityMode {
    Always(ALWAYS),
    ByDefault((BY, DEFAULT)),
}

/// GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY column constraint, with
/// optional `(sequence_option ...)` parenthesized list (e.g. `START WITH 44`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GeneratedIdentityConstraint<'input> {
    pub generated: GENERATED,
    pub mode: GeneratedIdentityMode,
    pub r#as: AS,
    pub identity: IDENTITY,
    pub seq_options:
        Option<Surrounded<punct::LParen, Vec<IdentitySeqOption<'input>>, punct::RParen>>,
}

/// One option inside an `IDENTITY ( ... )` sequence option list.
///
/// Variant ordering: longer multi-word forms first so longest-match-wins
/// picks them.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum IdentitySeqOption<'input> {
    StartWith(SeqOptStartWith<'input>),
    IncrementBy(SeqOptIncrementBy<'input>),
    MinValue(SeqOptMinValue<'input>),
    NoMinValue((NO, MINVALUE)),
    MaxValue(SeqOptMaxValue<'input>),
    NoMaxValue((NO, MAXVALUE)),
    Cache(SeqOptCache<'input>),
    Cycle(CYCLE),
    NoCycle((NO, CYCLE)),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOptStartWith<'input> {
    pub start: START,
    pub with: Option<WITH>,
    pub value: crate::ast::shared::expr::Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOptIncrementBy<'input> {
    pub increment: INCREMENT,
    pub by: Option<BY>,
    pub value: crate::ast::shared::expr::Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOptMinValue<'input> {
    pub minvalue: MINVALUE,
    pub value: crate::ast::shared::expr::Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOptMaxValue<'input> {
    pub maxvalue: MAXVALUE,
    pub value: crate::ast::shared::expr::Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SeqOptCache<'input> {
    pub cache: CACHE,
    pub value: crate::ast::shared::expr::Expr<'input>,
}

/// `GENERATED {ALWAYS | BY DEFAULT} AS (expr) STORED` column constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GeneratedStoredConstraint<'input> {
    pub generated: GENERATED,
    pub mode: GeneratedIdentityMode,
    pub r#as: AS,
    pub expr: Surrounded<punct::LParen, crate::ast::shared::expr::Expr<'input>, punct::RParen>,
    pub stored: STORED,
}

/// `COMPRESSION method` column clause. Sets the compression method
/// (e.g. `pglz`, `lz4`) for a toastable column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CompressionConstraint<'input> {
    pub compression: COMPRESSION,
    pub method: literal::Ident<'input>,
}

/// DEFAULT expr column constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefaultConstraint<'input> {
    pub default: DEFAULT,
    pub expr: crate::ast::shared::expr::Expr<'input>,
}

/// Column constraint kind (without the optional `CONSTRAINT name` prefix).
///
/// Variant ordering for longest-match-wins:
/// - GeneratedIdentity (`GENERATED`) first (unique keyword)
/// - PrimaryKey (`PRIMARY KEY`) before others (unique keyword)
/// - NotNull (`NOT NULL`) before others
/// - References, Unique, Default, Check all start with distinct keywords
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ColumnConstraintKind<'input> {
    GeneratedStored(GeneratedStoredConstraint<'input>),
    GeneratedIdentity(GeneratedIdentityConstraint<'input>),
    PrimaryKey(PrimaryKeyConstraint<'input>),
    NotNull((NOT, NULL)),
    /// Bare `NULL` — redundant (columns are nullable by default) but
    /// syntactically accepted.
    Null(NULL),
    Unique(UniqueConstraint<'input>),
    References(ReferencesConstraint<'input>),
    Default(DefaultConstraint<'input>),
    Check(CheckConstraint<'input>),
    Compression(CompressionConstraint<'input>),
    Storage(StorageConstraint),
}

/// Column STORAGE mode: `STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ColumnStorageMode {
    Plain(PLAIN),
    External(EXTERNAL),
    Extended(EXTENDED),
    Main(MAIN),
    Default(DEFAULT),
}

/// `STORAGE mode` column-level storage specifier (used inline in CREATE
/// TABLE column definitions).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct StorageConstraint {
    pub storage: STORAGE,
    pub mode: ColumnStorageMode,
}

/// Optional `CONSTRAINT name` prefix shared by column-level and
/// table-level constraints.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ConstraintNamePrefix<'input> {
    pub constraint: CONSTRAINT,
    pub name: literal::Ident<'input>,
}

/// A column constraint with its optional `CONSTRAINT name` prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ColumnConstraint<'input> {
    pub name: Option<ConstraintNamePrefix<'input>>,
    pub kind: ColumnConstraintKind<'input>,
}

/// `COLLATE "name"` clause used after a column's type.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CollateClause<'input> {
    pub collate: COLLATE,
    pub name: literal::Ident<'input>,
}

/// One entry in a column-level `OPTIONS (name 'value', ...)` clause —
/// Postgres' `generic_option_elem` (`generic_option_name generic_option_arg`).
///
/// The name is a `ColLabel` (any identifier-or-keyword), and the argument is
/// a single-quoted string constant (`Sconst`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GenericOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: crate::ast::utility::copy::CopySconst<'input>,
}

/// Postgres' `create_generic_options`: `OPTIONS (generic_option_list)`.
/// Used by CREATE FOREIGN DATA WRAPPER, CREATE SERVER, CREATE FOREIGN TABLE,
/// CREATE USER MAPPING, IMPORT FOREIGN SCHEMA, and column-level options on
/// foreign-table columns.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreateGenericOptions<'input> {
    pub options: OPTIONS,
    pub list: Surrounded<punct::LParen, Seq1<GenericOption<'input>, punct::Comma>, punct::RParen>,
}

/// A column definition: `name type [COLLATE "..."] [OPTIONS (...)] [constraints...]`.
///
/// The `column_options` slot models Postgres' `create_generic_options` on
/// `columnDef` — used in CREATE FOREIGN TABLE column lists.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ColumnDef<'input> {
    pub name: literal::Ident<'input>,
    pub type_name: crate::ast::shared::expr::CastType<'input>,
    pub collate: Option<CollateClause<'input>>,
    pub column_options: Option<CreateGenericOptions<'input>>,
    pub constraints: Seq0<ColumnConstraint<'input>, (), OptionalTrailing>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ConstraintAttrs {
    pub deferrable: Option<DeferrableKind>,
    pub initially: Option<InitiallyClause>,
}

/// `USING INDEX name` — gram.y `ExistingIndex`. The named index must
/// already exist on the table; used by `PRIMARY KEY USING INDEX name` and
/// `UNIQUE USING INDEX name` table constraint forms.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExistingIndex<'input> {
    pub using: USING,
    pub index: INDEX,
    pub name: literal::Ident<'input>,
}

/// Body of a table-level `PRIMARY KEY` / `UNIQUE` constraint — either the
/// `(cols) [INCLUDE (…)]` column-list form or the `USING INDEX name`
/// existing-index form (gram.y `ConstraintElem` `PRIMARY KEY (cols) …`
/// vs `PRIMARY KEY ExistingIndex …`, and the analogous `UNIQUE` pair).
///
/// Variant ordering: `UsingIndex` first because its first token (`USING`)
/// is disjoint from `(`; declaration order is then for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum IndexedConstraintBody<'input> {
    /// `USING INDEX name` — bind constraint to an existing index.
    UsingIndex(ExistingIndex<'input>),
    /// `(cols) [INCLUDE (…)]` — declare the constraint on columns.
    Columns(IndexedConstraintColumns<'input>),
}

/// `(cols) [INCLUDE (…)] [WITH (...)] [USING INDEX TABLESPACE name]` — the
/// column-list branch of a PK/UNIQUE constraint body. Per gram.y
/// `ConstraintElem`'s `UNIQUE … '(' columnList ')' opt_c_include
/// opt_definition OptConsTableSpace ConstraintAttributeSpec` rule.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IndexedConstraintColumns<'input> {
    pub columns:
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TablePrimaryKey<'input> {
    pub primary: PRIMARY,
    pub key: KEY,
    pub body: IndexedConstraintBody<'input>,
    pub attrs: ConstraintAttrs,
}

/// `INCLUDE (col, ...)` covering-index clause used on PRIMARY KEY / UNIQUE
/// table constraints and on CREATE INDEX.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IncludeColumns<'input> {
    pub include: INCLUDE,
    pub columns:
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
}

/// `UNIQUE {(cols) [INCLUDE (…)] | USING INDEX name}` — table-level
/// constraint. Per gram.y `ConstraintElem`:
/// `UNIQUE … '(' columnList ')' opt_c_include … ConstraintAttributeSpec`
/// or `UNIQUE ExistingIndex ConstraintAttributeSpec`. The `USING INDEX`
/// branch has no `NULLS [NOT] DISTINCT` qualifier (PG infers it from the
/// existing index definition).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableUnique<'input> {
    pub unique: UNIQUE,
    /// `NULLS [NOT] DISTINCT` qualifier — only meaningful for the
    /// `(cols)` branch but accepted before either body for parsing
    /// simplicity. If present alongside `USING INDEX`, PG rejects at
    /// semantic time; the diff oracle handles that case.
    pub nulls: Option<NullsDistinctQualifier>,
    pub body: IndexedConstraintBody<'input>,
    pub attrs: ConstraintAttrs,
}

/// `FOREIGN KEY (col, ...) REFERENCES table [(col, ...)] [MATCH ...] [ON ...] [DEFERRABLE ...] [INITIALLY ...]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableForeignKey<'input> {
    pub foreign: FOREIGN,
    pub key: KEY,
    pub columns:
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    pub references: ReferencesConstraint<'input>,
}

/// One entry in an EXCLUDE constraint's exclusion list: `index_elem WITH any_operator`.
///
/// Postgres' `ExclusionConstraintElem`. The operator may also appear wrapped
/// in `OPERATOR(...)` for the benefit of `ruleutils.c`; we accept both forms
/// via [`ExclusionOperator`].
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExclusionConstraintElem<'input> {
    pub elem: crate::ast::ddl::index::IndexElem<'input>,
    pub with: WITH,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExclusionOperator<'input> {
    Decorated(ExclusionOperatorDecorated<'input>),
    Plain(crate::ast::shared::names::QualifiedOperatorName<'input>),
}

/// `OPERATOR ( any_operator )` decorated form of an exclusion operator.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExclusionOperatorDecorated<'input> {
    pub operator: OPERATOR,
    pub name: Surrounded<
        punct::LParen,
        crate::ast::shared::names::QualifiedOperatorName<'input>,
        punct::RParen,
    >,
}

/// `WHERE (predicate)` clause on an EXCLUDE constraint — Postgres'
/// `OptWhereClause` in `gram.y`. The parens are mandatory (unlike the regular
/// `WHERE expr` form used by SELECT).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExclusionWhereClause<'input> {
    pub r#where: WHERE,
    pub expr: Surrounded<punct::LParen, crate::ast::shared::expr::Expr<'input>, punct::RParen>,
}

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableExclude<'input> {
    pub exclude: EXCLUDE,
    /// `access_method_clause` — `USING method` is optional (defaults to gist).
    pub using: Option<crate::ast::ddl::index::UsingMethod<'input>>,
    pub exclusions: Surrounded<
        punct::LParen,
        Seq1<ExclusionConstraintElem<'input>, punct::Comma>,
        punct::RParen,
    >,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TableConstraintKind<'input> {
    PrimaryKey(TablePrimaryKey<'input>),
    ForeignKey(TableForeignKey<'input>),
    Unique(TableUnique<'input>),
    Check(CheckConstraint<'input>),
    Exclude(TableExclude<'input>),
}

/// A table-level constraint with optional `CONSTRAINT name` prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableConstraint<'input> {
    pub name: Option<ConstraintNamePrefix<'input>>,
    pub kind: TableConstraintKind<'input>,
}

/// A single `INCLUDING` / `EXCLUDING` option on a `LIKE` source table clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LikeOptionKind {
    All(ALL),
    Defaults(DEFAULTS),
    Constraints(CONSTRAINTS),
    Indexes(INDEXES),
    Storage(STORAGE),
    Comments(COMMENTS),
    Statistics(STATISTICS),
    Generated(GENERATED),
    Identity(IDENTITY),
    Compression(COMPRESSION),
}

/// `INCLUDING what`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IncludingOption {
    pub including: INCLUDING,
    pub what: LikeOptionKind,
}

/// `EXCLUDING what`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExcludingOption {
    pub excluding: EXCLUDING,
    pub what: LikeOptionKind,
}

/// One option on a `LIKE table` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LikeOption {
    Including(IncludingOption),
    Excluding(ExcludingOption),
}

/// `LIKE source_table [INCLUDING/EXCLUDING option ...]` clause in a column
/// list body. Copies column definitions (and optionally other properties)
/// from an existing table.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LikeClause<'input> {
    pub like: LIKE,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ColumnOrConstraint<'input> {
    Like(LikeClause<'input>),
    Constraint(TableConstraint<'input>),
    Column(ColumnDef<'input>),
}

/// Optional TEMP or TEMPORARY keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TempKw {
    Temp(TEMP),
    Temporary(TEMPORARY),
}

/// INHERITS clause: `INHERITS (parent, ...)`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InheritsClause<'input> {
    pub inherits: INHERITS,
    pub parents:
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
}

/// `TABLESPACE name` clause on CREATE TABLE / CREATE INDEX, placing the
/// relation into a non-default tablespace.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TablespaceClause<'input> {
    pub tablespace: TABLESPACE,
    pub name: literal::Ident<'input>,
}

/// Legacy `WITH OIDS` / `WITHOUT OIDS` clause on CREATE TABLE. Kept for
/// backward-compat parsing of pre-12 dumps; Postgres now rejects it at
/// execution time but still accepts the syntax.
///
/// Variant ordering: `WithoutOids` (WITHOUT token) is disjoint from `WithOids`
/// (WITH OIDS) — distinct first tokens, listed in declaration order.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WithOidsClause {
    WithOids((WITH, OIDS)),
    WithoutOids((WITHOUT, OIDS)),
}

/// `USING access_method` clause on CREATE TABLE, selecting a non-default
/// table access method (e.g. `heap`, `heap2`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UsingAccessMethodClause<'input> {
    pub using: USING,
    pub method: literal::Ident<'input>,
}

/// Column-based table body: `(cols_and_constraints) [INHERITS (...)] [PARTITION BY ...]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ColumnsBody<'input> {
    pub columns:
        Surrounded<punct::LParen, Seq0<ColumnOrConstraint<'input>, punct::Comma>, punct::RParen>,
    pub inherits: Option<InheritsClause<'input>>,
    pub partition_by: Option<PartitionByClause<'input>>,
    pub using: Option<UsingAccessMethodClause<'input>>,
    pub with_oids: Option<WithOidsClause>,
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    pub on_commit: Option<OnCommitClause>,
    pub tablespace: Option<TablespaceClause<'input>>,
}

/// `ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP }` for temp tables.
///
/// Variant ordering: distinct first tokens (`PRESERVE` / `DELETE` / `DROP`),
/// so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OnCommitClause {
    pub on: ON,
    pub commit: COMMIT,
    pub action: OnCommitAction,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OnCommitAction {
    PreserveRows((PRESERVE, ROWS)),
    DeleteRows((DELETE, ROWS)),
    Drop(DROP),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
#[allow(clippy::large_enum_variant)]
pub enum PartitionColumnOption<'input> {
    Constraint(TableConstraint<'input>),
    Column(PartitionColumnOptionDef<'input>),
}

/// Per-partition column option: `name [WITH OPTIONS] [COLLATE "..."]
/// [constraints...]`. Overrides constraints/collation for a column
/// inherited from the partitioned parent table.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PartitionColumnOptionDef<'input> {
    pub name: literal::Ident<'input>,
    pub with_options: Option<(WITH, OPTIONS)>,
    pub collate: Option<CollateClause<'input>>,
    pub constraints: Seq0<ColumnConstraint<'input>, (), OptionalTrailing>,
}

/// Partition-of table body: `PARTITION OF parent [(col_options, ...)] FOR VALUES IN (...) [PARTITION BY ...]`
///
/// The optional `(col_options, ...)` list is a per-partition override of
/// column constraints (e.g. `b NOT NULL`, `b DEFAULT 1`, `CONSTRAINT c CHECK
/// (...)`), reusing the same `ColumnOrConstraint` grammar as a columns-based
/// table body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PartitionOfBody<'input> {
    pub partition: PARTITION,
    pub of: OF,
    pub parent: crate::ast::shared::names::QualifiedName<'input>,
    pub column_options: Option<
        Surrounded<punct::LParen, Seq0<PartitionColumnOption<'input>, punct::Comma>, punct::RParen>,
    >,
    pub for_values: Option<ForValuesClause<'input>>,
    pub default: Option<DEFAULT>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OfTypeBody<'input> {
    pub of: OF,
    pub type_name: crate::ast::shared::names::QualifiedName<'input>,
    pub column_options: Option<
        Surrounded<punct::LParen, Seq0<PartitionColumnOption<'input>, punct::Comma>, punct::RParen>,
    >,
}

/// AS-query table body: `AS SELECT ... [WITH [NO] DATA]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AsQueryBody<'input> {
    /// Optional `WITH (param = value, ...)` storage parameters before `AS`.
    pub with_storage: Option<crate::ast::ddl::index::WithStorage<'input>>,
    /// Optional `TABLESPACE name` before `AS`.
    pub tablespace: Option<TablespaceClause<'input>>,
    pub r#as: AS,
    pub query: Box<crate::ast::Statement<'input>>,
    pub with_data: Option<WithDataClause>,
}

/// `WITH DATA` or `WITH NO DATA` modifier on a CTAS query.
///
/// Variant ordering: `NoData` (`WITH NO DATA`, longer) before `Data`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WithDataClause {
    NoData((WITH, NO, DATA)),
    Data((WITH, DATA)),
}

/// `(col, col, ...) [ON COMMIT ...] AS query [WITH [NO] DATA]` — CTAS with column list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ColumnsAsQueryBody<'input> {
    pub columns:
        Surrounded<punct::LParen, Seq0<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    pub on_commit: Option<OnCommitClause>,
    pub r#as: AS,
    pub query: Box<crate::ast::Statement<'input>>,
    pub with_data: Option<WithDataClause>,
}

/// The body of a CREATE TABLE statement after `CREATE [TEMP] TABLE name`.
///
/// Variant ordering: AsQuery (`AS`) and PartitionOf (`PARTITION`) start with
/// keywords; Columns starts with `(`. Longest-match-wins disambiguates.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateTableStmt<'input> {
    pub create: CREATE,
    pub temp: Option<TempKw>,
    pub unlogged: Option<UNLOGGED>,
    pub table: TABLE,
    pub if_not_exists: Option<(IF, NOT, EXISTS)>,
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
    pub fn items(
        &self,
    ) -> Option<
        &Surrounded<punct::LParen, Seq0<ColumnOrConstraint<'input>, punct::Comma>, punct::RParen>,
    > {
        match &self.body {
            CreateTableBody::Columns(b) => Some(&b.columns),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PartitionKeyItem<'input> {
    pub expr: Expr<'input>,
    pub collate: Option<(COLLATE, literal::AliasName<'input>)>,
    pub opclass: Option<literal::AliasName<'input>>,
}

/// PARTITION BY LIST (col) clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PartitionByClause<'input> {
    pub partition: PARTITION,
    pub by: BY,
    pub strategy: literal::AliasName<'input>,
    /// Partition key items — may be plain column names or expressions like
    /// `((a+b)/2)`, optionally followed by a trailing opclass name.
    pub columns:
        Surrounded<punct::LParen, Seq0<PartitionKeyItem<'input>, punct::Comma>, punct::RParen>,
}

/// FOR VALUES IN (val, ...) clause — legacy name kept for backward compat
/// with partition.rs own tests; the general form lives in `ForValuesClause`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForValuesInClause<'input> {
    pub r#for: FOR,
    pub values_kw: VALUES,
    pub r#in: IN,
    pub values: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
}

/// `FROM (...) TO (...)` range partition spec.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FromToSpec<'input> {
    pub from: FROM,
    pub from_values: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
    pub to: TO,
    pub to_values: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
}

/// `IN (val, ...)` list partition spec.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InListSpec<'input> {
    pub r#in: IN,
    pub values: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
}

/// `MODULUS n` entry.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ModulusEntry<'input> {
    pub modulus: MODULUS,
    pub value: Expr<'input>,
}

/// `REMAINDER n` entry.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RemainderEntry<'input> {
    pub remainder: REMAINDER,
    pub value: Expr<'input>,
}

/// One item in `WITH (...)` for hash partitioning: MODULUS n or REMAINDER n.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum HashPartItem<'input> {
    Modulus(ModulusEntry<'input>),
    Remainder(RemainderEntry<'input>),
}

/// `WITH (MODULUS n, REMAINDER m)` hash partition spec.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithModulusSpec<'input> {
    pub with: WITH,
    pub items: Surrounded<punct::LParen, Seq0<HashPartItem<'input>, punct::Comma>, punct::RParen>,
}

/// Body after `FOR VALUES` in a PARTITION OF clause. Variant ordering:
/// `From` starts with `FROM`, `In` starts with `IN`, `With` starts with `WITH` —
/// all distinct keywords, so peek disambiguation is trivial.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ForValuesSpec<'input> {
    From(FromToSpec<'input>),
    In(InListSpec<'input>),
    With(WithModulusSpec<'input>),
}

/// Full `FOR VALUES ...` clause in a `PARTITION OF ...` body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForValuesClause<'input> {
    pub r#for: FOR,
    pub values: VALUES,
    pub spec: ForValuesSpec<'input>,
}

/// Column definition in partition table: `name type`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PartitionColumnDef<'input> {
    pub name: literal::Ident<'input>,
    pub type_name: TypeName<'input>,
}

/// CREATE TABLE with PARTITION BY: `CREATE TABLE name (cols) PARTITION BY strategy (cols)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreatePartitionedTableStmt<'input> {
    pub create: CREATE,
    pub table: TABLE,
    pub name: literal::Ident<'input>,
    pub columns:
        Surrounded<punct::LParen, Seq0<PartitionColumnDef<'input>, punct::Comma>, punct::RParen>,
    pub partition_by: PartitionByClause<'input>,
}

/// CREATE TABLE ... PARTITION OF parent FOR VALUES IN (...) [PARTITION BY ...].
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreatePartitionOfStmt<'input> {
    pub create: CREATE,
    pub table: TABLE,
    pub name: literal::Ident<'input>,
    pub partition: PARTITION,
    pub of: OF,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropTableStmt<'input> {
    pub drop: DROP,
    pub table: TABLE,
    pub if_exists: Option<(IF, EXISTS)>,
    pub names: Seq0<QualifiedName<'input>, punct::Comma>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_table_identity_seq_options() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY (START WITH 44))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_temp_table_on_commit() {
        for src in [
            "CREATE TEMP TABLE t (a int) ON COMMIT PRESERVE ROWS",
            "CREATE TEMP TABLE t (a int) ON COMMIT DELETE ROWS",
            "CREATE TEMP TABLE t (a int) ON COMMIT DROP",
        ] {
            let mut input = crate::tokens::test_input(src);
            let _stmt = CreateTableStmt::parse(&mut input).unwrap();
            assert!(input.is_empty(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_create_table_single_column() {
        let mut input = crate::tokens::test_input("CREATE TABLE BOOLTBL1 (f1 bool)");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "BOOLTBL1");
        assert_eq!(stmt.items().unwrap().len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_multiple_columns() {
        let mut input = crate::tokens::test_input("CREATE TABLE BOOLTBL3 (d text, b bool, o int)");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "BOOLTBL3");
        assert_eq!(stmt.items().unwrap().len(), 3);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_ctas_with_column_list() {
        // Regression: matview.sql uses `CREATE TABLE foo(a, b) AS VALUES(1, 10)`.
        let mut input = crate::tokens::test_input("CREATE TABLE mvtest_foo(a, b) AS VALUES(1, 10)");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.body,
            super::CreateTableBody::ColumnsAsQuery(_)
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_time_zone_types() {
        // Regression: brin.sql brintest table uses `time without time zone`,
        // `timestamp with time zone`, `bit varying(16)` as column types.
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a time without time zone, b timestamp with time zone, c time with time zone, d timestamp without time zone, e bit varying(16), f bit(10), g character)",
        );
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.items().unwrap().len(), 7);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_array_column_types() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int2[], b int4[][][], c varchar(5)[], d text[])",
        );
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.items().unwrap().len(), 4);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_boolean_type() {
        let mut input = crate::tokens::test_input("CREATE TABLE t (f1 boolean)");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.items().unwrap().len(), 1);
    }

    #[test]
    fn parse_create_temp_table() {
        let mut input = crate::tokens::test_input("CREATE TEMP TABLE foo (f1 int)");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(stmt.temp.is_some());
        assert_eq!(stmt.name.object(), "foo");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_partitioned_table() {
        let mut input = crate::tokens::test_input(
            "create table list_parted_tbl (a int,b int) partition by list (a)",
        );
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "list_parted_tbl");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_partition_of() {
        let mut input = crate::tokens::test_input(
            "create table list_parted_tbl1 partition of list_parted_tbl for values in (1) partition by list(b)",
        );
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "list_parted_tbl1");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_column_check_constraint() {
        let mut input = crate::tokens::test_input("CREATE TABLE t (a int CHECK (a > 0))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_column_references_full() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int REFERENCES other(id) MATCH FULL ON DELETE CASCADE ON UPDATE NO ACTION DEFERRABLE INITIALLY DEFERRED)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_column_named_constraint() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE t (a int CONSTRAINT pos CHECK (a > 0))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_column_default_constraint() {
        let mut input = crate::tokens::test_input("CREATE TABLE t (a int DEFAULT 0)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_primary_key() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE t (a int, b int, PRIMARY KEY (a, b))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_unique() {
        let mut input = crate::tokens::test_input("CREATE TABLE t (a int, UNIQUE (a))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_foreign_key() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES other(id) ON DELETE SET NULL)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_foreign_key_set_null_columns() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int, b int, FOREIGN KEY (a, b) REFERENCES p ON DELETE SET NULL (b))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_foreign_key_set_default_columns() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES p ON UPDATE SET DEFAULT (a))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_check() {
        let mut input = crate::tokens::test_input("CREATE TABLE t (a int, CHECK (a > 0))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_named_constraint() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int, b int, CONSTRAINT pk PRIMARY KEY (a, b) DEFERRABLE INITIALLY IMMEDIATE)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_check_no_inherit() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE t (a int, CHECK (a > 0) NO INHERIT)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_like_bare() {
        let mut input = crate::tokens::test_input("CREATE TABLE foo (LIKE bar)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_like_including_all() {
        let mut input = crate::tokens::test_input("CREATE TABLE foo (LIKE bar INCLUDING ALL)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_like_including_excluding() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE foo (LIKE bar INCLUDING DEFAULTS EXCLUDING CONSTRAINTS)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_like_mixed_with_columns() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE foo (a int, LIKE bar INCLUDING ALL, b text)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_check_no_inherit_not_valid() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (d date, CHECK (false) NO INHERIT NOT VALID)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_check_not_valid() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE t (a int, CHECK (a > 0) NOT VALID)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_with_storage_params() {
        let mut input = crate::tokens::test_input("CREATE TABLE t (a int) WITH (fillfactor = 70)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_temp_table_empty_columns() {
        let mut input = crate::tokens::test_input("CREATE TEMP TABLE nocols()");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.items().unwrap().len(), 0);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_unlogged_table() {
        let mut input = crate::tokens::test_input("CREATE UNLOGGED TABLE t (a int)");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(stmt.unlogged.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_unlogged_table_qualified() {
        let mut input = crate::tokens::test_input("CREATE UNLOGGED TABLE public.t (a int)");
        // This uses unqualified Ident only; restrict to the unqualified form.
        let _stmt = CreateTableStmt::parse(&mut input);
    }

    #[test]
    fn parse_column_with_collate() {
        let mut input = crate::tokens::test_input("CREATE TABLE foo (a text COLLATE \"C\")");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_partition_of_range_from_to() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE p1 PARTITION OF p FOR VALUES FROM (0) TO (10)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_partition_of_list_in() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE p2 PARTITION OF p FOR VALUES IN (1, 2, 3)");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_partition_of_hash_with_modulus() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE p3 PARTITION OF p FOR VALUES WITH (MODULUS 4, REMAINDER 0)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_partition_of_default() {
        let mut input = crate::tokens::test_input("CREATE TABLE p4 PARTITION OF p DEFAULT");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_primary_key_using_index_tablespace() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int PRIMARY KEY USING INDEX TABLESPACE pg_default) PARTITION BY LIST (a)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// Sanity check: ALTER TABLE ... ADD CONSTRAINT ... PRIMARY KEY (col)
    /// must parse (existing functionality).
    #[test]
    fn parse_alter_table_add_pk_cols_sanity() {
        let mut input = crate::tokens::test_input("ALTER TABLE t ADD PRIMARY KEY (a)");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// Table-level `PRIMARY KEY USING INDEX existing_idx` constraint form
    /// (gram.y `ConstraintElem: PRIMARY KEY ExistingIndex …`). Distinct
    /// from the `PRIMARY KEY (cols)` form modelled by `TablePrimaryKey`.
    #[test]
    fn parse_table_constraint_primary_key_using_index() {
        let mut input =
            crate::tokens::test_input("ALTER TABLE t ADD PRIMARY KEY USING INDEX my_idx");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// Table-level `UNIQUE USING INDEX existing_idx` form (gram.y
    /// `ConstraintElem: UNIQUE ExistingIndex …`).
    #[test]
    fn parse_table_constraint_unique_using_index() {
        let mut input = crate::tokens::test_input("ALTER TABLE t ADD UNIQUE USING INDEX my_idx");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// `ADD CONSTRAINT name PRIMARY KEY USING INDEX existing_idx` — the
    /// named-constraint form.
    #[test]
    fn parse_table_constraint_named_primary_key_using_index() {
        let mut input = crate::tokens::test_input(
            "ALTER TABLE t ADD CONSTRAINT my_pkey PRIMARY KEY USING INDEX my_idx",
        );
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_ctas_on_commit_delete_rows() {
        let mut input = crate::tokens::test_input(
            "CREATE TEMP TABLE temptest(col) ON COMMIT DELETE ROWS AS SELECT 1",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_ctas_on_commit_drop() {
        let mut input =
            crate::tokens::test_input("CREATE TEMP TABLE temptest(col) ON COMMIT DROP AS SELECT 1");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_partition_of_on_commit() {
        for src in [
            "CREATE TEMP TABLE t1 PARTITION OF p FOR VALUES IN (1) ON COMMIT DELETE ROWS",
            "CREATE TEMP TABLE t2 PARTITION OF p FOR VALUES IN (2) ON COMMIT DROP",
            "CREATE TEMP TABLE t3 PARTITION OF p FOR VALUES IN (1) ON COMMIT PRESERVE ROWS",
        ] {
            let mut input = crate::tokens::test_input(src);
            let _stmt = CreateTableStmt::parse(&mut input).unwrap();
            assert!(input.is_empty(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_create_table_of_type() {
        let mut input = crate::tokens::test_input("CREATE TABLE persons OF person_type");
        let stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(matches!(stmt.body, super::CreateTableBody::OfType(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_of_type_with_options() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE personsx OF person_type (myname WITH OPTIONS NOT NULL)",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_of_type_constraints() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE persons2 OF person_type (id WITH OPTIONS PRIMARY KEY, UNIQUE (name))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_of_type_default() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE persons3 OF person_type (PRIMARY KEY (id), name WITH OPTIONS DEFAULT '')",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_table_of_type_not_null_default() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE persons3 OF person_type (PRIMARY KEY (id), name NOT NULL DEFAULT '')",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// EXCLUDE table constraint, simplest form: `EXCLUDE (col WITH op)`.
    #[test]
    fn parse_table_exclude_bare() {
        let mut input =
            crate::tokens::test_input("CREATE TABLE deferred_excl (f1 int, EXCLUDE (f1 WITH =))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// EXCLUDE table constraint with explicit access method: `EXCLUDE USING gist (col WITH op)`.
    #[test]
    fn parse_table_exclude_using_gist() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int4range, EXCLUDE USING GIST (a WITH =))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// EXCLUDE constraint with multiple index elements: `EXCLUDE USING GIST (a WITH =, b WITH =)`.
    #[test]
    fn parse_table_exclude_multi_elements() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int4range, b int4range, EXCLUDE USING GIST (a WITH =, b WITH =))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// EXCLUDE constraint with a custom operator like `&&` or `-|-`.
    #[test]
    fn parse_table_exclude_custom_op() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (a int4range, EXCLUDE USING GIST (a WITH -|-))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// EXCLUDE constraint with `WHERE (predicate)` partial-index clause.
    #[test]
    fn parse_table_exclude_with_where() {
        let mut input = crate::tokens::test_input(
            "CREATE TABLE t (f4 int, EXCLUDE USING btree (f4 WITH =) WHERE (f4 IS NOT NULL))",
        );
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    // -------------------------------------------------------------------
    // Tests folded in from the former `ast/partition.rs`.
    // -------------------------------------------------------------------

    #[test]
    fn parse_partitioned_table_standalone() {
        use crate::ast::ddl::table::CreatePartitionedTableStmt;
        let mut input = crate::tokens::test_input(
            "create table list_parted_tbl (a int,b int) partition by list (a)",
        );
        let stmt = CreatePartitionedTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "list_parted_tbl");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_partition_of_standalone() {
        use crate::ast::ddl::table::CreatePartitionOfStmt;
        let mut input = crate::tokens::test_input(
            "create table list_parted_tbl1 partition of list_parted_tbl for values in (1) partition by list(b)",
        );
        let stmt = CreatePartitionOfStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "list_parted_tbl1");
        assert_eq!(stmt.parent.text(), "list_parted_tbl");
        assert!(stmt.partition_by.is_some());
        assert!(input.is_empty());
    }

    // -------------------------------------------------------------------
    // Tests folded in from the former `ast/drop_table.rs`.
    // -------------------------------------------------------------------

    #[test]
    fn parse_drop_table() {
        use crate::ast::ddl::table::DropTableStmt;
        let mut input = crate::tokens::test_input("DROP TABLE BOOLTBL1");
        let stmt = DropTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_table_lowercase() {
        use crate::ast::ddl::table::DropTableStmt;
        let mut input = crate::tokens::test_input("drop table my_table");
        let stmt = DropTableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.names.len(), 1);
    }

    #[test]
    fn parse_drop_table_if_exists() {
        use crate::ast::ddl::table::DropTableStmt;
        let mut input = crate::tokens::test_input("DROP TABLE IF EXISTS foo");
        let stmt = DropTableStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
    }

    #[test]
    fn parse_drop_table_multi_cascade() {
        use crate::ast::ddl::table::DropTableStmt;
        let mut input = crate::tokens::test_input("DROP TABLE IF EXISTS a, b, c CASCADE");
        let stmt = DropTableStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.names.len(), 3);
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_table_qualified() {
        use crate::ast::ddl::table::DropTableStmt;
        let mut input = crate::tokens::test_input("DROP TABLE schema1.foo RESTRICT");
        let stmt = DropTableStmt::parse(&mut input).unwrap();
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }
    /// Multi-element `alter_identity_column_option_list` (gram.y) — the
    /// `SET GENERATED …`, `SET seq_option`, and `RESTART …` clauses can
    /// chain in a single `ALTER COLUMN` action. identity.sql corpus uses
    /// this.
    #[test]
    fn parse_alter_table_set_generated_set_increment_restart() {
        let mut input = crate::tokens::test_input(
            "ALTER TABLE pitest2 ALTER COLUMN f3 SET GENERATED BY DEFAULT SET INCREMENT BY 2 RESTART",
        );
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_table_identity_single_set_generated_still_works() {
        let mut input =
            crate::tokens::test_input("ALTER TABLE t ALTER COLUMN c SET GENERATED ALWAYS");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_table_identity_set_seq_option_alone() {
        let mut input =
            crate::tokens::test_input("ALTER TABLE t ALTER COLUMN c SET INCREMENT BY 2");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_table_identity_restart_alone() {
        let mut input = crate::tokens::test_input("ALTER TABLE t ALTER COLUMN c RESTART");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// gram.y `reloption_elem` includes `ColLabel '=' def_arg`. PG accepts
    /// `RESET (name = value)` even though it ignores the value. Reloptions.sql
    /// has `ALTER TABLE reloptions_test RESET (fillfactor=12)` — must not
    /// surface as a [`crate::ast::FileItem::ParseError`].
    #[test]
    fn parse_alter_table_reset_reloptions_with_value() {
        let mut input =
            crate::tokens::test_input("ALTER TABLE reloptions_test RESET (fillfactor=12)");
        let _stmt = AlterTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// gram.y `Typename` accepts `expr_list` typmods, including negative
    /// integers like `numeric(3, -6)`. numeric.sql corpus needs this.
    #[test]
    fn parse_create_table_numeric_negative_typmod() {
        use crate::ast::ddl::table::CreateTableStmt;
        let mut input =
            crate::tokens::test_input("CREATE TABLE num_typemod_test (millions numeric(3, -6))");
        let _stmt = CreateTableStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterTableStmt<'input> {
    pub alter: ALTER,
    pub table: TABLE,
    pub body: AlterTableBody<'input>,
}

/// Body of `ALTER TABLE ...` — either the bulk-relocate `ALL IN TABLESPACE`
/// form or a per-relation form.
///
/// Variant ordering: `All` (starts with `ALL`) before `Single` (starts with
/// `IF` / `ONLY` / `qualified_name`, never `ALL`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTableSingle<'input> {
    pub if_exists: Option<IfExists>,
    pub only: Option<ONLY>,
    pub name: QualifiedName<'input>,
    pub star: Option<punct::Star>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTableRenameConstraint<'input> {
    pub rename: RENAME,
    pub constraint: CONSTRAINT,
    pub old_name: literal::Ident<'input>,
    pub to: TO,
    pub new_name: literal::Ident<'input>,
}

/// Comma-separated `alter_table_cmds` on ALTER TABLE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterTableCmds<'input> {
    pub cmds: Seq1<AlterTableCmd<'input>, punct::Comma>,
}

/// Postgres' `partition_cmd`: a single `ATTACH PARTITION` or `DETACH
/// PARTITION` action on a partitioned table.
///
/// Variant ordering: `Attach` (ATTACH) and `Detach` (DETACH) have disjoint
/// first tokens, so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PartitionCmd<'input> {
    Attach(AttachPartitionCmd<'input>),
    Detach(DetachPartitionCmd<'input>),
}

/// `ATTACH PARTITION qualified_name partition_bound_spec` — adds an existing
/// table as a partition of the target partitioned table.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AttachPartitionCmd<'input> {
    pub attach: ATTACH,
    pub partition: PARTITION,
    pub name: QualifiedName<'input>,
    pub bound: PartitionBoundSpec<'input>,
}

/// `DETACH PARTITION qualified_name [CONCURRENTLY | FINALIZE]` — removes a
/// partition from its parent.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DetachPartitionCmd<'input> {
    pub detach: crate::tokens::soft_keyword::DETACH,
    pub partition: PARTITION,
    pub name: QualifiedName<'input>,
    pub mode: Option<DetachPartitionMode>,
}

/// Trailing mode keyword on `DETACH PARTITION`: `CONCURRENTLY` (the default
/// nonblocking detach) or `FINALIZE` (completes a previously-CONCURRENTLY
/// detached partition).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DetachPartitionMode {
    Concurrently(CONCURRENTLY),
    Finalize(crate::tokens::soft_keyword::FINALIZE),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PartitionBoundSpec<'input> {
    Default(DEFAULT),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AddColumnIfNotExistsCmd<'input> {
    pub add: ADD,
    pub column: COLUMN,
    pub if_not_exists: IfNotExists,
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ADD IF NOT EXISTS columnDef`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AddIfNotExistsCmd<'input> {
    pub add: ADD,
    pub if_not_exists: IfNotExists,
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ADD COLUMN columnDef`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AddColumnCmd<'input> {
    pub add: ADD,
    pub column: COLUMN,
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ADD TableConstraint [NOT VALID]` — table-level constraint with optional
/// `NOT VALID` marker (Postgres routes this through `ConstraintAttributeSpec`
/// on the constraint).
///
/// The `NOT VALID` modifier is part of the constraint's attribute list in
/// gram.y; pg-sql models it as a trailing `Option` on this `AlterTableCmd`
/// variant for symmetry with the corpus' usage (it only ever sits at the end).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AddTableConstraintCmd<'input> {
    pub add: ADD,
    pub constraint: crate::ast::ddl::table::TableConstraint<'input>,
    pub not_valid: Option<NotValid>,
}

/// `NOT VALID` — the unverified-constraint marker.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NotValid {
    pub not: NOT,
    pub valid: VALID,
}

/// `ADD columnDef` (no `COLUMN` keyword, no `IF NOT EXISTS`).
///
/// Listed last in the ADD family because every column definition begins with
/// a bareword (the column name), which would otherwise greedily swallow
/// `COLUMN`, `IF`, `CONSTRAINT`, etc.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AddColumnBareCmd<'input> {
    pub add: ADD,
    pub column_def: crate::ast::ddl::table::ColumnDef<'input>,
}

/// `ALTER CONSTRAINT name [DEFERRABLE | NOT DEFERRABLE] [INITIALLY {DEFERRED
/// | IMMEDIATE}]` — Postgres' `AT_AlterConstraint` action.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterConstraintCmd<'input> {
    pub alter: ALTER,
    pub constraint: CONSTRAINT,
    pub name: literal::Ident<'input>,
    pub attrs: crate::ast::ddl::table::ConstraintAttrs,
}

/// `ALTER [COLUMN] colname …` — the big `ALTER COLUMN` cmd. The `colname`
/// can also be a numeric column index for `SET STATISTICS` (used on indexes,
/// not on tables — but accepted here for symmetry).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColumnCmd<'input> {
    pub alter: ALTER,
    pub column: Option<COLUMN>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterIdentityOption<'input> {
    SetGenerated(AlterColSetGenerated),
    SetSeqOption(AlterColSetSeqOption<'input>),
    Restart(AlterColRestart<'input>),
}

/// `alter_identity_column_option_list` — one or more
/// [`AlterIdentityOption`] items in sequence, no separator.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterIdentityOpts<'input> {
    pub items: Seq1<AlterIdentityOption<'input>, (), recursa::seq::OptionalTrailing>,
}

/// `SET EXPRESSION AS (expr)` — adjust a generated column's expression.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetExpression<'input> {
    pub set: SET,
    pub expression: crate::tokens::soft_keyword::EXPRESSION,
    pub r#as: AS,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `[SET DATA] TYPE Typename [COLLATE name] [USING expr]` — change a column's
/// type. The `SET DATA` is mandatory in this variant (the leading-`SET` form);
/// the bare `TYPE …` form is `AlterColTypeBare`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetDataType<'input> {
    pub set: SET,
    pub data: DATA,
    pub r#type: TYPE,
    pub type_name: CastType<'input>,
    pub collate: Option<crate::ast::ddl::table::CollateClause<'input>>,
    pub using: Option<AlterColUsing<'input>>,
}

/// `SET STATISTICS { SignedIconst | DEFAULT }` — adjust per-column statistics
/// target.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetStatistics<'input> {
    pub set: SET,
    pub statistics: STATISTICS,
    pub value: SetStatisticsValue<'input>,
}

/// `SET COMPRESSION { name | DEFAULT }` — change a column's compression
/// method.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetCompression<'input> {
    pub set: SET,
    pub compression: COMPRESSION,
    pub target: ColumnCompressionTarget<'input>,
}

/// `SET DEFAULT expr` — set a column's default expression.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetDefault<'input> {
    pub set: SET,
    pub default: DEFAULT,
    pub expr: Box<Expr<'input>>,
}

/// `DROP DEFAULT` — drop a column's default expression.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColDropDefault {
    pub drop: DROP,
    pub default: DEFAULT,
}

/// `USING expr` clause on `ALTER COLUMN … TYPE …`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColUsing<'input> {
    pub using: USING,
    pub expr: Box<Expr<'input>>,
}

/// `TYPE Typename [COLLATE name] [USING expr]` — change a column's type
/// without the leading `SET DATA`. Postgres accepts both spellings.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColTypeBare<'input> {
    pub r#type: TYPE,
    pub type_name: CastType<'input>,
    pub collate: Option<crate::ast::ddl::table::CollateClause<'input>>,
    pub using: Option<AlterColUsing<'input>>,
}

/// `SET GENERATED { ALWAYS | BY DEFAULT }` — change the identity column
/// generation mode.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetGenerated {
    pub set: SET,
    pub generated: GENERATED,
    pub mode: crate::ast::ddl::table::GeneratedIdentityMode,
}

/// `SET STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT }` — adjust a
/// column's TOAST storage strategy.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetStorage {
    pub set: SET,
    pub storage: STORAGE,
    pub mode: crate::ast::ddl::table::ColumnStorageMode,
}

/// `SET NOT NULL` — add a NOT NULL marker on the column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetNotNull {
    pub set: SET,
    pub not: NOT,
    pub null: NULL,
}

/// One `SET seqOpt` action on an identity column —
/// `SET { START WITH | INCREMENT BY | MINVALUE | MAXVALUE | CACHE | CYCLE |
/// NO MINVALUE | NO MAXVALUE | NO CYCLE }`.
///
/// Reuses `IdentitySeqOption` from `create_table.rs` so the full sequence-
/// option set is supported (and the formatter is shared).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColSetSeqOption<'input> {
    pub set: SET,
    pub option: crate::ast::ddl::table::IdentitySeqOption<'input>,
}

/// `DROP EXPRESSION [IF EXISTS]` — remove a generated column's expression,
/// turning it into a regular column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColDropExpression {
    pub drop: DROP,
    pub expression: crate::tokens::soft_keyword::EXPRESSION,
    pub if_exists: Option<IfExists>,
}

/// `DROP IDENTITY [IF EXISTS]` — remove an identity-column property.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColDropIdentity {
    pub drop: DROP,
    pub identity: IDENTITY,
    pub if_exists: Option<IfExists>,
}

/// `DROP NOT NULL` — remove a NOT NULL marker.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColDropNotNull {
    pub drop: DROP,
    pub not: NOT,
    pub null: NULL,
}

/// `ADD GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY [(seq_options)]` —
/// add an identity property to an existing column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColAddIdentity<'input> {
    pub add: ADD,
    pub identity: crate::ast::ddl::table::GeneratedIdentityConstraint<'input>,
}

/// `RESTART [WITH NumericOnly]` — restart an identity column's sequence.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterColRestart<'input> {
    pub restart: crate::tokens::soft_keyword::RESTART,
    pub value: Option<RestartWith<'input>>,
}

/// `[WITH] NumericOnly` — the value portion of `RESTART`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RestartWith<'input> {
    pub with: Option<WITH>,
    pub value: NumericOnly<'input>,
}

/// `DROP CONSTRAINT IF EXISTS name [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropConstraintIfExistsCmd<'input> {
    pub drop: DROP,
    pub constraint: CONSTRAINT,
    pub if_exists: IfExists,
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP CONSTRAINT name [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropConstraintCmd<'input> {
    pub drop: DROP,
    pub constraint: CONSTRAINT,
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP [COLUMN] IF EXISTS colname [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropColumnIfExistsCmd<'input> {
    pub drop: DROP,
    pub column: Option<COLUMN>,
    pub if_exists: IfExists,
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `DROP [COLUMN] colname [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropColumnCmd<'input> {
    pub drop: DROP,
    pub column: Option<COLUMN>,
    pub name: literal::Ident<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `ENABLE TRIGGER { name | ALL | USER }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableTriggerCmd<'input> {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub trigger: TRIGGER,
    pub target: TriggerOrRuleTarget<'input>,
}

/// `ENABLE ALWAYS TRIGGER name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableAlwaysTriggerCmd<'input> {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub always: ALWAYS,
    pub trigger: TRIGGER,
    pub name: literal::Ident<'input>,
}

/// `ENABLE REPLICA TRIGGER name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableReplicaTriggerCmd<'input> {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub replica: crate::tokens::soft_keyword::REPLICA,
    pub trigger: TRIGGER,
    pub name: literal::Ident<'input>,
}

/// `DISABLE TRIGGER { name | ALL | USER }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DisableTriggerCmd<'input> {
    pub disable: crate::tokens::soft_keyword::DISABLE,
    pub trigger: TRIGGER,
    pub target: TriggerOrRuleTarget<'input>,
}

/// `ENABLE RULE name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableRuleCmd<'input> {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub rule: RULE,
    pub name: literal::Ident<'input>,
}

/// `ENABLE ALWAYS RULE name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableAlwaysRuleCmd<'input> {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub always: ALWAYS,
    pub rule: RULE,
    pub name: literal::Ident<'input>,
}

/// `ENABLE REPLICA RULE name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableReplicaRuleCmd<'input> {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub replica: crate::tokens::soft_keyword::REPLICA,
    pub rule: RULE,
    pub name: literal::Ident<'input>,
}

/// `DISABLE RULE name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DisableRuleCmd<'input> {
    pub disable: crate::tokens::soft_keyword::DISABLE,
    pub rule: RULE,
    pub name: literal::Ident<'input>,
}

/// Trigger-action target on `ENABLE TRIGGER` / `DISABLE TRIGGER`:
/// `ALL` (every trigger), `USER` (every non-internal trigger), or a named
/// trigger.
///
/// Variant ordering: keyword variants (`All` / `User`) before `Name` (ident),
/// since `ALL` and `USER` are hard keywords that won't lex as `Ident`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TriggerOrRuleTarget<'input> {
    All(ALL),
    User(USER),
    Name(literal::Ident<'input>),
}

/// `ENABLE ROW LEVEL SECURITY`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EnableRowSecurityCmd {
    pub enable: crate::tokens::soft_keyword::ENABLE,
    pub row: ROW,
    pub level: LEVEL,
    pub security: SECURITY,
}

/// `DISABLE ROW LEVEL SECURITY`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DisableRowSecurityCmd {
    pub disable: crate::tokens::soft_keyword::DISABLE,
    pub row: ROW,
    pub level: LEVEL,
    pub security: SECURITY,
}

/// `FORCE ROW LEVEL SECURITY`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForceRowSecurityCmd {
    pub force: crate::tokens::soft_keyword::FORCE,
    pub row: ROW,
    pub level: LEVEL,
    pub security: SECURITY,
}

/// `NO FORCE ROW LEVEL SECURITY`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NoForceRowSecurityCmd {
    pub no: NO,
    pub force: crate::tokens::soft_keyword::FORCE,
    pub row: ROW,
    pub level: LEVEL,
    pub security: SECURITY,
}

/// `CLUSTER ON indexname`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ClusterOnCmd<'input> {
    pub cluster: CLUSTER,
    pub on: ON,
    pub name: literal::Ident<'input>,
}

/// `SET WITHOUT CLUSTER`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetWithoutClusterCmd {
    pub set: SET,
    pub without: WITHOUT,
    pub cluster: CLUSTER,
}

/// `SET WITHOUT OIDS`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetWithoutOidsCmd {
    pub set: SET,
    pub without: WITHOUT,
    pub oids: OIDS,
}

/// `SET LOGGED`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetLoggedCmd {
    pub set: SET,
    pub logged: crate::tokens::soft_keyword::LOGGED,
}

/// `SET UNLOGGED`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetUnloggedCmd {
    pub set: SET,
    pub unlogged: UNLOGGED,
}

/// `REPLICA IDENTITY { DEFAULT | NOTHING | FULL | USING INDEX name }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReplicaIdentityCmd<'input> {
    pub replica: crate::tokens::soft_keyword::REPLICA,
    pub identity: IDENTITY,
    pub kind: ReplicaIdentityKind<'input>,
}

/// One of `DEFAULT`, `NOTHING`, `FULL`, or `USING INDEX name`.
///
/// Variant ordering: keyword-only variants (single tokens, disjoint) first;
/// `UsingIndex` (`USING INDEX`) last — it has a unique `USING` prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ReplicaIdentityKind<'input> {
    Default(DEFAULT),
    Nothing(NOTHING),
    Full(FULL),
    UsingIndex(ReplicaIdentityUsingIndex<'input>),
}

/// `USING INDEX name` — the index-backed REPLICA IDENTITY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReplicaIdentityUsingIndex<'input> {
    pub using: USING,
    pub index: INDEX,
    pub name: literal::Ident<'input>,
}

/// `INHERIT parent`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InheritCmd<'input> {
    pub inherit: INHERIT,
    pub parent: QualifiedName<'input>,
}

/// `NO INHERIT parent`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NoInheritCmd<'input> {
    pub no: NO,
    pub inherit: INHERIT,
    pub parent: QualifiedName<'input>,
}

/// `OF type_name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OfCmd<'input> {
    pub of: OF,
    pub type_name: QualifiedName<'input>,
}

/// `NOT OF` — drop the typed-table relationship.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NotOfCmd {
    pub not: NOT,
    pub of: OF,
}

/// `VALIDATE CONSTRAINT name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ValidateConstraintCmd<'input> {
    pub validate: crate::tokens::soft_keyword::VALIDATE,
    pub constraint: CONSTRAINT,
    pub name: literal::Ident<'input>,
}
