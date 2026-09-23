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
use crate::ast::ddl::materialized_view::ColumnCompressionTarget;
// Added in 15: research, PostgreSQL 15, "Changes to existing statements".
#[cfg(feature = "since-pg15")]
use crate::ast::ddl::materialized_view::SetAccessMethodClause;
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
recursa::ast_node! {
    #[allow(unused_imports)]
    // ---------------------------------------------------------------------------
    /// `USING INDEX TABLESPACE name` — tablespace for the index backing a PRIMARY
    /// KEY or UNIQUE column constraint.
    #[derive(Debug)]
    pub struct UsingIndexTablespace {
        #[tok(USING, INDEX, TABLESPACE, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// PRIMARY KEY column constraint — gram.y `ColConstraintElem: PRIMARY KEY
    /// opt_definition OptConsTableSpace`.
    #[derive(Debug)]
    #[tok(PRIMARY, KEY, this)]
    pub struct PrimaryKeyConstraint {
        /// `WITH (storage_param = value, ...)` — gram.y `opt_definition`.
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        pub index_tablespace: Option<UsingIndexTablespace>,
    }
}

recursa::ast_node! {
    /// UNIQUE column constraint — gram.y `ColConstraintElem: UNIQUE
    /// opt_unique_null_treatment opt_definition OptConsTableSpace`.
    #[derive(Debug)]
    #[tok(UNIQUE, this)]
    pub struct UniqueConstraint {
        /// Optional `NULLS [NOT] DISTINCT` qualifier. Added in 15: research, PostgreSQL 15, "Changes to existing statements"
        /// (REL_15_19 gram.y `opt_unique_null_treatment`).
        #[config(since = pg15)]
        pub nulls: Option<NullsDistinctQualifier>,
        /// `WITH (storage_param = value, ...)` — gram.y `opt_definition`.
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        pub index_tablespace: Option<UsingIndexTablespace>,
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `opt_unique_null_treatment`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `NULLS DISTINCT` or `NULLS NOT DISTINCT` for UNIQUE constraints.
    #[derive(Debug)]
    pub struct NullsDistinctQualifier {
        #[tok(NULLS, this, DISTINCT)]
        #[presence(NOT)]
        pub not: bool,
    }
}

recursa::ast_node! {
    /// Referential action for `ON DELETE` / `ON UPDATE`.
    ///
    /// Variant ordering: multi-word variants (`NO ACTION`, `SET NULL`, `SET DEFAULT`)
    /// come before single-word ones to satisfy longest-match.
    #[derive(Debug)]
    pub enum ReferentialAction {
        #[tok(NO, ACTION)]
        NoAction,
        SetNull(SetNullKw),
        SetDefault(SetDefaultKw),
        #[tok(CASCADE)]
        Cascade,
        #[tok(RESTRICT)]
        Restrict,
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `key_action`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// Parenthesized column list on `ON DELETE SET NULL` / `SET DEFAULT`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ReferentialActionColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
    );
}

recursa::ast_node! {
    /// `SET NULL [(columns)]`. The column list is added in 15: research, PostgreSQL 15, "Changes to existing statements"
    /// (REL_15_19 gram.y `key_action: SET NULL_P opt_column_list`).
    #[derive(Debug)]
    #[config_attr(since = pg15, tok(SET, NULL, this))]
    #[config_attr(before = pg15, tok(SET, NULL))]
    pub struct SetNullKw {
        #[config(since = pg15)]
        pub cols: Option<ReferentialActionColumnList>,
    }
}

recursa::ast_node! {
    /// `SET DEFAULT [(columns)]`. The column list is added in 15: research, PostgreSQL 15, "Changes to existing statements"
    /// (REL_15_19 gram.y `key_action: SET DEFAULT opt_column_list`).
    #[derive(Debug)]
    #[config_attr(since = pg15, tok(SET, DEFAULT, this))]
    #[config_attr(before = pg15, tok(SET, DEFAULT))]
    pub struct SetDefaultKw {
        #[config(since = pg15)]
        pub cols: Option<ReferentialActionColumnList>,
    }
}

recursa::ast_node! {
    /// `ON DELETE action`.
    #[derive(Debug)]
    pub struct OnDeleteAction {
        #[tok(ON, DELETE, this)]
        pub action: ReferentialAction,
    }
}

recursa::ast_node! {
    /// `ON UPDATE action`.
    #[derive(Debug)]
    pub struct OnUpdateAction {
        #[tok(ON, UPDATE, this)]
        pub action: ReferentialAction,
    }
}

recursa::ast_node! {
    /// Match type for a foreign key: `MATCH FULL | PARTIAL | SIMPLE`.
    #[derive(Debug)]
    pub enum MatchKind {
        #[tok(FULL)]
        Full,
        #[tok(PARTIAL)]
        Partial,
        #[tok(SIMPLE)]
        Simple,
    }
}

recursa::ast_node! {
    /// `MATCH FULL | MATCH PARTIAL | MATCH SIMPLE`.
    #[derive(Debug)]
    pub struct MatchClause {
        #[tok(MATCH, this)]
        pub kind: MatchKind,
    }
}

recursa::ast_node! {
    /// gram.y `ConstraintAttr`: the deferrability entries of a column's
    /// constraint list (`ColConstraint: ConstraintAttr`).
    #[derive(Debug)]
    pub enum ColumnConstraintAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(DEFERRABLE)]
        Deferrable,
        #[tok(INITIALLY, DEFERRED)]
        InitiallyDeferred,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
        /// Added in 18: gram.y `ConstraintAttr: ENFORCED | NOT ENFORCED`
        /// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
        /// item 4; REL_18_6 gram.y `ConstraintAttr`).
        #[config(since = pg18)]
        #[tok(ENFORCED)]
        Enforced,
        /// Added in 18: see [`Self::Enforced`].
        #[config(since = pg18)]
        #[tok(NOT, ENFORCED)]
        NotEnforced,
    }
}

recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y accepts on a
    /// `UNIQUE`, `PRIMARY KEY` or `EXCLUDE` table constraint. `processCASbits`
    /// gets `deferrable` and `initdeferred` pointers and nothing else, so
    /// `NOT VALID`, `NO INHERIT` and (from 18) `[NOT] ENFORCED` are
    /// raw-parser errors (REL_17_11 gram.y 4157, 4173, 4190, 4206, 4226;
    /// REL_18_6 gram.y 4244, 4260, 4278, 4294, 4314).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum IndexConstraintAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
        #[tok(INITIALLY, DEFERRED)]
        InitiallyDeferred,
        #[tok(DEFERRABLE)]
        Deferrable,
    }
}

recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y accepts on a
    /// table-level `CHECK`. `processCASbits` gets no `deferrable` and no
    /// `initdeferred` pointer, so `DEFERRABLE` and `INITIALLY DEFERRED` are
    /// raw-parser errors; `NOT VALID` and `NO INHERIT` are passed, and from 18
    /// `is_enforced` as well (REL_17_11 gram.y 4138-4140; REL_18_6 gram.y
    /// 4211-4213).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum TableCheckAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(NOT, VALID)]
        NotValid,
        #[tok(NO, INHERIT)]
        NoInherit,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
        /// Added in 18: gram.y `ConstraintAttributeElem: NOT ENFORCED |
        /// ENFORCED`, which `ConstraintElem`'s CHECK arm passes an
        /// `is_enforced` pointer for (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 4; REL_18_6 gram.y 4211-4213).
        #[config(since = pg18)]
        #[tok(NOT, ENFORCED)]
        NotEnforced,
        /// Added in 18: see [`Self::NotEnforced`].
        #[config(since = pg18)]
        #[tok(ENFORCED)]
        Enforced,
    }
}

recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y accepts on a
    /// `FOREIGN KEY` table constraint: the deferrability entries, `NOT VALID`,
    /// and from 18 `[NOT] ENFORCED`. `no_inherit` is the one pointer that
    /// `processCASbits` does not get (REL_17_11 gram.y 4245-4248; REL_18_6
    /// gram.y 4343-4346).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum ForeignKeyConstraintAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(NOT, VALID)]
        NotValid,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
        #[tok(INITIALLY, DEFERRED)]
        InitiallyDeferred,
        #[tok(DEFERRABLE)]
        Deferrable,
        /// Added in 18: see [`TableCheckAttr::NotEnforced`].
        #[config(since = pg18)]
        #[tok(NOT, ENFORCED)]
        NotEnforced,
        /// Added in 18: see [`TableCheckAttr::NotEnforced`].
        #[config(since = pg18)]
        #[tok(ENFORCED)]
        Enforced,
    }
}

recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y accepts on
    /// `ALTER TABLE ... ALTER CONSTRAINT name`: the deferrability entries in
    /// every version, and from 18 also `NO INHERIT` and `[NOT] ENFORCED`. The
    /// 18 action rejects `NOT VALID` itself, before `processCASbits`
    /// (REL_17_11 gram.y 2652-2654; REL_18_6 gram.y 2671-2685).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum AlterConstraintAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
        #[tok(INITIALLY, DEFERRED)]
        InitiallyDeferred,
        #[tok(DEFERRABLE)]
        Deferrable,
        /// Added in 18: `ALTER CONSTRAINT name NO INHERIT`
        /// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
        /// item 5; REL_18_6 gram.y `alter_table_cmd` 2682-2683).
        #[config(since = pg18)]
        #[tok(NO, INHERIT)]
        NoInherit,
        /// Added in 18: see [`TableCheckAttr::NotEnforced`] and research item 5.
        #[config(since = pg18)]
        #[tok(NOT, ENFORCED)]
        NotEnforced,
        /// Added in 18: see [`Self::NotEnforced`].
        #[config(since = pg18)]
        #[tok(ENFORCED)]
        Enforced,
    }
}

// Added in 18: a table-level `NOT NULL` constraint
// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18, item 6).
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// The `ConstraintAttributeSpec` entries that gram.y accepts on a
    /// table-level `NOT NULL`. `processCASbits` gets `not_valid` and
    /// `no_inherit` only, so the deferrable entries other than `NOT
    /// DEFERRABLE` and `INITIALLY IMMEDIATE`, and `[NOT] ENFORCED`, are
    /// raw-parser errors (REL_18_6 gram.y 4224-4226).
    ///
    /// Variant ordering: multi-keyword forms first.
    #[derive(Debug)]
    pub enum TableNotNullAttr {
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(NOT, VALID)]
        NotValid,
        #[tok(NO, INHERIT)]
        NoInherit,
        #[tok(INITIALLY, IMMEDIATE)]
        InitiallyImmediate,
    }
}

recursa::ast_node! {
    /// `ON DELETE ...` or `ON UPDATE ...` trailing action on a REFERENCES
    /// constraint. Modeled as an enum so both orders of the two clauses
    /// are accepted via a [`Vec`]`<`[`OnAction`]`>`.
    ///
    /// Variant ordering: both start with `ON`; they diverge at the next keyword.
    #[derive(Debug)]
    pub enum OnAction {
        OnDelete(OnDeleteAction),
        OnUpdate(OnUpdateAction),
    }
}

recursa::ast_node! {
    /// Parenthesized referenced-column list on a `REFERENCES` clause.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ReferencedColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(literal::AliasName),
    );
}

recursa::ast_node! {
    /// REFERENCES constraint:
    /// `REFERENCES table [(col, ...)] [MATCH ...] [ON DELETE|UPDATE ...]* [DEFERRABLE | NOT DEFERRABLE] [INITIALLY ...]`
    #[derive(Debug)]
    pub struct ReferencesConstraint {
        #[tok(REFERENCES, this)]
        pub table: crate::ast::shared::names::QualifiedName,
        pub columns: Option<ReferencedColumnList>,
        pub match_clause: Option<MatchClause>,
        pub actions: zero_or_many!(OnAction),
    }
}

recursa::ast_node! {
    /// `CHECK (expr) [NO INHERIT] [NOT VALID]`
    #[derive(Debug)]
    pub struct CheckConstraint {
        #[tok(CHECK, LPAREN, this, RPAREN)]
        pub expr: crate::ast::shared::expr::Expr,
        #[presence(NO, INHERIT)]
        pub no_inherit: bool,
    }
}

recursa::ast_node! {
    /// Table-level `CHECK (expr)` — gram.y `ConstraintElem: CHECK '(' a_expr ')'
    /// ConstraintAttributeSpec`, where the spec also carries `NO INHERIT` and
    /// `NOT VALID`.
    #[derive(Debug)]
    pub struct TableCheck {
        #[tok(CHECK, LPAREN, this, RPAREN)]
        pub expr: crate::ast::shared::expr::Expr,
        pub attrs: zero_or_many!(TableCheckAttr),
    }
}

recursa::ast_node! {
    /// `GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY` modifier.
    ///
    /// Variant ordering: both start with a distinct keyword after `GENERATED`
    /// (`ALWAYS` vs `BY`), so order is cosmetic.
    #[derive(Debug)]
    pub enum GeneratedIdentityMode {
        #[tok(ALWAYS)]
        Always,
        #[tok(BY, DEFAULT)]
        ByDefault,
    }
}

recursa::ast_node! {
    /// GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY column constraint, with
    /// optional `(sequence_option ...)` parenthesized list (e.g. `START WITH 44`).
    #[derive(Debug)]
    pub struct GeneratedIdentityConstraint {
        #[tok(GENERATED, this)]
        pub mode: GeneratedIdentityMode,
        pub identity: AsIdentity,
        pub seq_options: Option<IdentitySeqOptionList>,
    }
}

recursa::ast_node! {
    /// Parenthesized sequence-option list on `GENERATED ... AS IDENTITY (...)`.
    ///
    /// `SeqOptList` is space-separated, so the parentheses have to surround the
    /// whole list; a field-level attachment would demand a fresh pair per option
    /// (`(START WITH 44) (INCREMENT BY 2)`).
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct IdentitySeqOptionList(#[deref] pub one_or_many!(IdentitySeqOption));
}

recursa::ast_node! {
    /// Required `AS IDENTITY` marker after the generation mode.
    #[derive(Debug)]
    pub enum AsIdentity {
        #[tok(AS, IDENTITY)]
        Value,
    }
}

recursa::ast_node! {
    /// One option inside an `IDENTITY ( ... )` sequence option list, and the
    /// option of `alter_identity_column_option: SET SeqOptElem`.
    ///
    /// Variant ordering: longer multi-word forms first so longest-match-wins
    /// picks them.
    #[derive(Debug)]
    pub enum IdentitySeqOption {
        StartWith(SeqOptStartWith),
        IncrementBy(SeqOptIncrementBy),
        MinValue(SeqOptMinValue),
        #[tok(NO, MINVALUE)]
        NoMinValue,
        MaxValue(SeqOptMaxValue),
        #[tok(NO, MAXVALUE)]
        NoMaxValue,
        Cache(SeqOptCache),
        #[tok(CYCLE)]
        Cycle,
        #[tok(NO, CYCLE)]
        NoCycle,
        /// Added in 15: REL_15_19 gram.y `SeqOptElem: ... | LOGGED` (4741).
        /// The research lists it under PostgreSQL 17, "Changes to existing
        /// statements" (commit f7567f9e53d, back-patched to 15 and 16).
        /// REL_14_24 gram.y `SeqOptElem` has no `LOGGED` or `UNLOGGED`.
        #[config(since = pg15)]
        #[tok(LOGGED)]
        Logged,
        /// Added in 15: REL_15_19 gram.y `SeqOptElem: ... | UNLOGGED` (4781).
        /// Same commit and research entry as [`IdentitySeqOption::Logged`].
        /// `SET UNLOGGED` on a column is `SET SeqOptElem`, which is separate
        /// from the table-level `alter_table_cmd: SET UNLOGGED`.
        #[config(since = pg15)]
        #[tok(UNLOGGED)]
        Unlogged,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SeqOptStartWith {
        #[tok(START, optional(WITH), this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SeqOptIncrementBy {
        #[tok(INCREMENT, optional(BY), this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SeqOptMinValue {
        #[tok(MINVALUE, this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SeqOptMaxValue {
        #[tok(MAXVALUE, this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SeqOptCache {
        #[tok(CACHE, this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    /// `GENERATED {ALWAYS | BY DEFAULT} AS IDENTITY [(seq options)]` or
    /// `GENERATED ALWAYS AS (expr) [STORED | VIRTUAL]` — gram.y
    /// `ColConstraintElem`'s two `GENERATED generated_when AS` forms.
    ///
    /// The expression form takes `ALWAYS` only: its action stops on any other
    /// `generated_when` ("for a generated column, GENERATED ALWAYS must be
    /// specified"), so `GENERATED BY DEFAULT AS (1) STORED` is a raw-parser
    /// error in every target version (REL_14_24 gram.y `ColConstraintElem`;
    /// b73d13c gram.y `ColConstraintElem`). `ALWAYS` is therefore a token of
    /// the rule, not a reduced `generated_when`: the parser shifts `GENERATED
    /// ALWAYS AS` and parts at `IDENTITY` or `(`.
    ///
    /// Variant ordering: `BY` and `ALWAYS` part the two after `GENERATED`.
    #[derive(Debug)]
    pub enum GeneratedConstraint {
        Always(GeneratedAlwaysConstraint),
        ByDefault(GeneratedByDefaultConstraint),
    }
}

recursa::ast_node! {
    /// `GENERATED ALWAYS AS {IDENTITY [(seq options)] | (expr) [STORED |
    /// VIRTUAL]}`.
    #[derive(Debug)]
    pub struct GeneratedAlwaysConstraint {
        #[tok(GENERATED, ALWAYS, AS, this)]
        pub body: GeneratedBody,
    }
}

recursa::ast_node! {
    /// `GENERATED BY DEFAULT AS IDENTITY [(seq options)]`. gram.y takes the
    /// expression form only with `ALWAYS`.
    #[derive(Debug)]
    #[tok(GENERATED, BY, DEFAULT, AS, this)]
    pub struct GeneratedByDefaultConstraint {
        pub identity: GeneratedIdentityTail,
    }
}

recursa::ast_node! {
    /// What follows `GENERATED ALWAYS AS`.
    ///
    /// Variant ordering: `IDENTITY` or `(` decides; `Stored` (which ends in
    /// `STORED`) before `Virtual`, whose keyword is optional.
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum GeneratedBody {
        Identity(GeneratedIdentityTail),
        Stored(GeneratedStoredTail),
        /// Added in 18: `(expr) [VIRTUAL]`, gram.y `opt_virtual_or_stored`
        /// with `VIRTUAL` or empty (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 1; REL_18_6 gram.y `ColConstraintElem`).
        #[config(since = pg18)]
        Virtual(GeneratedVirtualTail),
    }
}

// Added in 18: see `GeneratedBody::Virtual`.
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// `(expr) [VIRTUAL]` — a virtual generated column. From 18 a
    /// generated column without `STORED` is virtual, so the keyword is
    /// optional.
    #[derive(Debug)]
    pub struct GeneratedVirtualTail {
        #[tok(LPAREN, this, RPAREN)]
        pub expr: crate::ast::shared::expr::Expr,
        /// Whether the source spells `VIRTUAL`.
        #[presence(VIRTUAL)]
        pub virtual_keyword: bool,
    }
}

recursa::ast_node! {
    /// `IDENTITY [(seq options)]`.
    #[derive(Debug)]
    #[tok(IDENTITY, this)]
    pub struct GeneratedIdentityTail {
        pub seq_options: Option<IdentitySeqOptionList>,
    }
}

recursa::ast_node! {
    /// `(expr) STORED`.
    #[derive(Debug)]
    pub struct GeneratedStoredTail {
        #[tok(LPAREN, this, RPAREN, STORED)]
        pub expr: crate::ast::shared::expr::Expr,
    }
}

recursa::ast_node! {
    /// `COMPRESSION method` column clause. Sets the compression method
    /// (e.g. `pglz`, `lz4`) for a toastable column.
    #[derive(Debug)]
    pub struct CompressionConstraint {
        #[tok(COMPRESSION, this)]
        pub method: literal::Ident,
    }
}

recursa::ast_node! {
    /// DEFAULT expr column constraint.
    #[derive(Debug)]
    pub struct DefaultConstraint {
        /// gram.y `ColConstraintElem: DEFAULT b_expr`: the restricted
        /// expression, which has no `AND`, `OR`, `LIKE`, `BETWEEN`, `IN` or
        /// `IS NULL` extender, so `DEFAULT 1 NOT NULL` ends the default at
        /// `NOT`.
        #[tok(DEFAULT, this)]
        pub expr: crate::ast::shared::expr::BExpr,
    }
}

recursa::ast_node! {
    /// Column constraint kind (without the optional `CONSTRAINT name` prefix).
    ///
    /// Variant ordering for longest-match-wins:
    /// - GeneratedIdentity (`GENERATED`) first (unique keyword)
    /// - PrimaryKey (`PRIMARY KEY`) before others (unique keyword)
    /// - NotNull (`NOT NULL`) before others
    /// - References, Unique, Default, Check all start with distinct keywords
    #[derive(Debug)]
    pub enum ColumnConstraintKind {
        Generated(GeneratedConstraint),
        PrimaryKey(PrimaryKeyConstraint),
        #[tok(NOT, NULL)]
        NotNull,
        /// Added in 18: gram.y `ColConstraintElem: NOT NULL_P opt_no_inherit`
        /// with `NO INHERIT` (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 7; REL_18_6 gram.y `ColConstraintElem`).
        #[config(since = pg18)]
        #[tok(NOT, NULL, NO, INHERIT)]
        NotNullNoInherit,
        #[tok(NULL)]
        /// Bare `NULL` — redundant (columns are nullable by default) but
        /// syntactically accepted.
        Null,
        /// gram.y `ColConstraint: ConstraintAttr`: `[NOT] DEFERRABLE` and
        /// `INITIALLY {DEFERRED | IMMEDIATE}` are entries of the column's
        /// constraint list in their own right, not a tail of the constraint
        /// before them, so `UNIQUE NOT DEFERRABLE NOT NULL` needs no
        /// two-token decision after `UNIQUE`.
        Attr(ColumnConstraintAttr),
        Unique(UniqueConstraint),
        References(ReferencesConstraint),
        Default(DefaultConstraint),
        Check(CheckConstraint),
        Compression(CompressionConstraint),
        // Added in 16: see `StorageConstraint`.
        #[config(since = pg16)]
        Storage(StorageConstraint),
    }
}

// Added in 16: research, PostgreSQL 16, "Changes to existing statements"
// (REL_16_15 gram.y `column_storage`: `STORAGE ColId | STORAGE DEFAULT`). In
// 15, only `ALTER ... SET STORAGE ColId` takes a storage mode.
#[cfg(feature = "since-pg16")]
recursa::ast_node! {
    /// Column STORAGE mode — gram.y `column_storage: STORAGE ColId | STORAGE
    /// DEFAULT`. The grammar takes any `ColId`, and only `ALTER TABLE`'s parse
    /// analysis (`tablecmds.c`) rejects a word other than `plain`, `external`,
    /// `extended` or `main`, so the admission set is `ColId`, not that word
    /// list (principle 9).
    ///
    /// Variant ordering: `Default` first, although `DEFAULT` is a reserved
    /// keyword that `ColId` never admits.
    #[derive(Debug)]
    pub enum ColumnStorageMode {
        /// Added in 16: the shape that the widening of `ALTER TABLE ... SET
        /// STORAGE` adds. Before 16 the argument is a `ColId`, which never
        /// admits the reserved word `DEFAULT` (REL_15_19 gram.y
        /// `alter_table_cmd: ... SET STORAGE ColId`; REL_16_15 2472,
        /// commit b9424d014).
        #[config(since = pg16)]
        #[tok(DEFAULT)]
        Default,
        Name(crate::tokens::ColId),
    }
}

// Added in 16: research, PostgreSQL 16, "Changes to existing statements"
// (REL_16_15 gram.y `columnDef ... opt_column_storage`, commit 784cedda0).
#[cfg(feature = "since-pg16")]
recursa::ast_node! {
    /// `STORAGE mode` column-level storage specifier (used inline in CREATE
    /// TABLE column definitions).
    #[derive(Debug)]
    pub struct StorageConstraint {
        #[tok(STORAGE, this)]
        pub mode: ColumnStorageMode,
    }
}

recursa::ast_node! {
    /// Optional `CONSTRAINT name` prefix shared by column-level and
    /// table-level constraints.
    #[derive(Debug)]
    pub struct ConstraintNamePrefix {
        #[tok(CONSTRAINT, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// A column constraint with its optional `CONSTRAINT name` prefix.
    #[derive(Debug)]
    pub struct ColumnConstraint {
        pub name: Option<ConstraintNamePrefix>,
        pub kind: ColumnConstraintKind,
    }
}

recursa::ast_node! {
    /// `COLLATE "name"` clause used after a column's type.
    #[derive(Debug)]
    pub struct CollateClause {
        /// gram.y `opt_collate_clause: COLLATE any_name`.
        #[tok(COLLATE, this)]
        pub name: crate::ast::shared::names::QualifiedName,
    }
}

recursa::ast_node! {
    /// One entry in a column-level `OPTIONS (name 'value', ...)` clause —
    /// Postgres' `generic_option_elem` (`generic_option_name generic_option_arg`).
    ///
    /// The name is a `ColLabel` (any identifier-or-keyword), and the argument is
    /// a single-quoted string constant (`Sconst`).
    #[derive(Debug)]
    pub struct GenericOption {
        pub name: literal::AliasName,
        pub value: crate::ast::utility::copy::CopySconst,
    }
}

recursa::ast_node! {
    /// Postgres' `create_generic_options`: `OPTIONS (generic_option_list)`.
    /// Used by CREATE FOREIGN DATA WRAPPER, CREATE SERVER, CREATE FOREIGN TABLE,
    /// CREATE USER MAPPING, IMPORT FOREIGN SCHEMA, and column-level options on
    /// foreign-table columns.
    #[derive(Debug)]
    #[tok(OPTIONS, LPAREN, this, RPAREN)]
    pub struct CreateGenericOptions {
        #[sep(COMMA)]
        pub list: one_or_many!(GenericOption),
    }
}

recursa::ast_node! {
    /// A column definition: `name type [COLLATE "..."] [OPTIONS (...)] [constraints...]`.
    ///
    /// The `column_options` slot models Postgres' `create_generic_options` on
    /// `columnDef` — used in CREATE FOREIGN TABLE column lists.
    #[derive(Debug)]
    pub struct ColumnDef {
        pub name: crate::tokens::ColId,
        pub type_name: crate::ast::shared::expr::CastType,
        pub collate: Option<CollateClause>,
        pub column_options: Option<CreateGenericOptions>,
        pub constraints: zero_or_many!(ColumnConstraint),
    }
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

recursa::ast_node! {
    /// `USING INDEX name` — gram.y `ExistingIndex`. The named index must
    /// already exist on the table; used by `PRIMARY KEY USING INDEX name` and
    /// `UNIQUE USING INDEX name` table constraint forms.
    #[derive(Debug)]
    pub struct ExistingIndex {
        #[tok(USING, INDEX, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// Body of a table-level `PRIMARY KEY` / `UNIQUE` constraint — either the
    /// `(cols) [INCLUDE (…)]` column-list form or the `USING INDEX name`
    /// existing-index form (gram.y `ConstraintElem` `PRIMARY KEY (cols) …`
    /// vs `PRIMARY KEY ExistingIndex …`, and the analogous `UNIQUE` pair).
    ///
    /// Variant ordering: `UsingIndex` first because its first token (`USING`)
    /// is disjoint from `(`; declaration order is then for clarity.
    #[derive(Debug)]
    pub enum IndexedConstraintBody {
        /// `USING INDEX name` — bind constraint to an existing index.
        UsingIndex(ExistingIndex),
        /// `(cols) [INCLUDE (…)]` — declare the constraint on columns.
        Columns(IndexedConstraintColumns),
    }
}

recursa::ast_node! {
    /// Parenthesized column list in a PRIMARY KEY or UNIQUE constraint.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct IndexedConstraintColumnList(
        // A narrowing, not an addition (ADR 0010, `docs/minimum-version.md`):
        // 18 accepts fewer lists than 14, so a list that 18 accepts needs no
        // version above the baseline. Both arms only remove; neither records.
        #[cfg(not(feature = "since-pg18"))]
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(crate::tokens::ColId),
        /// From 18 the list is gram.y `columnList`, which is not empty: an
        /// empty list would let `(WITHOUT OVERLAPS)` parse, which REL_18_6
        /// gram.y `ConstraintElem` rejects.
        #[cfg(feature = "since-pg18")]
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
        /// Added in 18: gram.y `opt_without_overlaps` after the last key
        /// column, a temporal key (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 2; REL_18_6 gram.y `ConstraintElem`).
        #[config(since = pg18)]
        #[presence(WITHOUT, OVERLAPS)]
        pub bool,
    );
}

recursa::ast_node! {
    /// `(cols) [INCLUDE (…)] [WITH (...)] [USING INDEX TABLESPACE name]` — the
    /// column-list branch of a PK/UNIQUE constraint body. Per gram.y
    /// `ConstraintElem`'s `UNIQUE … '(' columnList ')' opt_c_include
    /// opt_definition OptConsTableSpace ConstraintAttributeSpec` rule.
    #[derive(Debug)]
    pub struct IndexedConstraintColumns {
        pub columns: IndexedConstraintColumnList,
        pub include: Option<IncludeColumns>,
        /// `WITH (storage_param = value, ...)` — gram.y's `opt_definition`.
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        /// `USING INDEX TABLESPACE name` — gram.y's `OptConsTableSpace`.
        pub index_tablespace: Option<UsingIndexTablespace>,
    }
}

recursa::ast_node! {
    /// `PRIMARY KEY {(cols) [INCLUDE (…)] | USING INDEX name}` — table-level
    /// constraint. Per gram.y `ConstraintElem`:
    /// `PRIMARY KEY '(' columnList ')' opt_c_include … ConstraintAttributeSpec`
    /// or `PRIMARY KEY ExistingIndex ConstraintAttributeSpec`.
    #[derive(Debug)]
    pub struct TablePrimaryKey {
        #[tok(PRIMARY, KEY, this)]
        pub body: IndexedConstraintBody,
        /// gram.y `ConstraintAttributeSpec`.
        pub attrs: zero_or_many!(IndexConstraintAttr),
    }
}

recursa::ast_node! {
    /// `INCLUDE (col, ...)` covering-index clause used on PRIMARY KEY / UNIQUE
    /// table constraints and on CREATE INDEX.
    #[derive(Debug)]
    #[tok(INCLUDE, LPAREN, this, RPAREN)]
    pub struct IncludeColumns {
        #[sep(COMMA)]
        pub columns: zero_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `UNIQUE {(cols) [INCLUDE (…)] | USING INDEX name}` — table-level
    /// constraint. Per gram.y `ConstraintElem`:
    /// `UNIQUE … '(' columnList ')' opt_c_include … ConstraintAttributeSpec`
    /// or `UNIQUE ExistingIndex ConstraintAttributeSpec`. The `USING INDEX`
    /// branch has no `NULLS [NOT] DISTINCT` qualifier (PG infers it from the
    /// existing index definition).
    #[derive(Debug)]
    #[tok(UNIQUE, this)]
    pub struct TableUnique {
        /// `NULLS [NOT] DISTINCT` qualifier — only meaningful for the
        /// `(cols)` branch but accepted before either body for parsing
        /// simplicity. If present alongside `USING INDEX`, PG rejects at
        /// semantic time; the diff oracle handles that case. Added in 15:
        /// research, PostgreSQL 15, "Changes to existing statements".
        #[config(since = pg15)]
        pub nulls: Option<NullsDistinctQualifier>,
        pub body: IndexedConstraintBody,
        /// gram.y `ConstraintAttributeSpec`.
        pub attrs: zero_or_many!(IndexConstraintAttr),
    }
}

recursa::ast_node! {
    /// Parenthesized local-column list in a table-level foreign-key constraint.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ForeignKeyColumnList(
        // A narrowing, not an addition (ADR 0010, `docs/minimum-version.md`):
        // 18 accepts fewer lists than 14, so a list that 18 accepts needs no
        // version above the baseline. Both arms only remove; neither records.
        #[cfg(not(feature = "since-pg18"))]
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(crate::tokens::ColId),
        /// From 18 the list is gram.y `columnList`, which is not empty: an
        /// empty list would let `(, PERIOD c)` parse, which REL_18_6 gram.y
        /// `ConstraintElem` rejects.
        #[cfg(feature = "since-pg18")]
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
        /// Added in 18: gram.y `optionalPeriodName`, the `PERIOD` column of a
        /// temporal foreign key (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 3; REL_18_6 gram.y `ConstraintElem`).
        #[config(since = pg18)]
        pub Option<PeriodColumn>,
    );
}

// Added in 18: gram.y `optionalPeriodName: ',' PERIOD columnElem`
// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18, item 3).
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// `, PERIOD column` — the last column of a temporal foreign key, on the
    /// referencing and on the referenced side.
    #[derive(Debug)]
    pub struct PeriodColumn {
        #[tok(COMMA, PERIOD, this)]
        pub column: crate::tokens::ColId,
    }
}

// Added in 18: gram.y `opt_column_and_period_list`
// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18, item 3).
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// gram.y `opt_column_and_period_list: '(' columnList optionalPeriodName
    /// ')'`: the referenced columns of a table-level foreign key. Only the
    /// table-level form takes `PERIOD`; a column-level `REFERENCES` keeps
    /// [`ReferencedColumnList`] (`opt_column_list`).
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ForeignKeyReferencedColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
        pub Option<PeriodColumn>,
    );
}

// Added in 18: the referenced side of a table-level foreign key, which differs
// from a column-level `REFERENCES` in its column list
// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18, item 3).
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// `REFERENCES table [(col, ... [, PERIOD col])] [MATCH ...] [ON ...]` —
    /// gram.y `ConstraintElem`'s `REFERENCES qualified_name
    /// opt_column_and_period_list key_match key_actions`. The fields have the
    /// names of [`ReferencesConstraint`].
    #[derive(Debug)]
    pub struct ForeignKeyReferences {
        #[tok(REFERENCES, this)]
        pub table: crate::ast::shared::names::QualifiedName,
        pub columns: Option<ForeignKeyReferencedColumnList>,
        pub match_clause: Option<MatchClause>,
        pub actions: zero_or_many!(OnAction),
    }
}

recursa::ast_node! {
    /// `FOREIGN KEY (col, ...) REFERENCES table [(col, ...)] [MATCH ...] [ON ...] [DEFERRABLE ...] [INITIALLY ...]`
    #[derive(Debug)]
    #[tok(FOREIGN, KEY, this)]
    pub struct TableForeignKey {
        pub columns: ForeignKeyColumnList,
        // A widening, not an addition (ADR 0010, `docs/minimum-version.md`):
        // the newer type accepts everything the older one does, so both arms
        // only remove and neither records. The gate for what 18 adds belongs
        // to those shapes.
        #[cfg(not(feature = "since-pg18"))]
        pub references: ReferencesConstraint,
        /// From 18 the referenced column list can end with `PERIOD col`,
        /// which a column-level `REFERENCES` cannot (REL_18_6 gram.y
        /// `opt_column_and_period_list`).
        #[cfg(feature = "since-pg18")]
        pub references: ForeignKeyReferences,
        /// gram.y `ConstraintAttributeSpec` after `key_actions`.
        pub attrs: zero_or_many!(ForeignKeyConstraintAttr),
    }
}

recursa::ast_node! {
    /// One entry in an EXCLUDE constraint's exclusion list: `index_elem WITH any_operator`.
    ///
    /// Postgres' `ExclusionConstraintElem`. The operator may also appear wrapped
    /// in `OPERATOR(...)` for the benefit of `ruleutils.c`; we accept both forms
    /// via [`ExclusionOperator`].
    #[derive(Debug)]
    pub struct ExclusionConstraintElem {
        pub elem: crate::ast::ddl::index::IndexElem,
        /// gram.y writes a plain `WITH` here, so PostgreSQL rejects an operator
        /// qualified by a schema named `time` or `ordinality`; pg-sql accepts
        /// it, as it did before the marker.
        pub with: crate::ast::shared::flags::AnyWith,
        pub op: ExclusionOperator,
    }
}

recursa::ast_node! {
    /// The operator slot of an exclusion constraint element.
    ///
    /// Two forms per `gram.y::ExclusionConstraintElem`:
    /// - `any_operator`                — bare operator name.
    /// - `OPERATOR ( any_operator )`   — same operator decorated with `OPERATOR(...)`.
    ///
    /// Variant ordering: `Decorated` starts with the `OPERATOR` keyword; `Plain`
    /// starts with an operator-name token. Their first sets are disjoint.
    #[derive(Debug)]
    pub enum ExclusionOperator {
        Decorated(ExclusionOperatorDecorated),
        Plain(crate::ast::shared::names::QualifiedOperatorName),
    }
}

recursa::ast_node! {
    /// `OPERATOR ( any_operator )` decorated form of an exclusion operator.
    #[derive(Debug)]
    pub struct ExclusionOperatorDecorated {
        #[tok(OPERATOR, LPAREN, this, RPAREN)]
        pub name: crate::ast::shared::names::QualifiedOperatorName,
    }
}

recursa::ast_node! {
    /// `WHERE (predicate)` clause on an EXCLUDE constraint — Postgres'
    /// `OptWhereClause` in `gram.y`. The parens are mandatory (unlike the regular
    /// `WHERE expr` form used by SELECT).
    #[derive(Debug)]
    pub struct ExclusionWhereClause {
        #[tok(WHERE, LPAREN, this, RPAREN)]
        pub expr: crate::ast::shared::expr::Expr,
    }
}

recursa::ast_node! {
    /// Parenthesized, non-empty list of exclusion-constraint elements.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ExclusionConstraintList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(ExclusionConstraintElem),
    );
}

recursa::ast_node! {
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
    #[derive(Debug)]
    #[tok(EXCLUDE, this)]
    pub struct TableExclude {
        /// `access_method_clause` — `USING method` is optional (defaults to gist).
        pub using: Option<crate::ast::ddl::index::UsingMethod>,
        pub exclusions: ExclusionConstraintList,
        /// `INCLUDE (col, ...)` covering-index clause.
        pub include: Option<IncludeColumns>,
        /// `WITH (param = value, ...)` storage parameters (`opt_definition`).
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        /// `USING INDEX TABLESPACE name` (`OptConsTableSpace`).
        pub index_tablespace: Option<UsingIndexTablespace>,
        /// `WHERE (expr)` partial-constraint predicate (parens mandatory).
        pub where_clause: Option<ExclusionWhereClause>,
        /// gram.y `ConstraintAttributeSpec`.
        pub attrs: zero_or_many!(IndexConstraintAttr),
    }
}

recursa::ast_node! {
    /// A table-level constraint kind.
    ///
    /// Variant ordering: `PRIMARY KEY` (PRIMARY), `FOREIGN KEY` (FOREIGN),
    /// `UNIQUE`, `CHECK`, `EXCLUDE` — all start with distinct unique keywords
    /// so order is not strictly required for disambiguation.
    #[derive(Debug)]
    pub enum TableConstraintKind {
        PrimaryKey(TablePrimaryKey),
        ForeignKey(TableForeignKey),
        Unique(TableUnique),
        Check(TableCheck),
        Exclude(TableExclude),
        /// Added in 18: gram.y `ConstraintElem: NOT NULL_P ColId
        /// ConstraintAttributeSpec` (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 6; REL_18_6 gram.y `ConstraintElem`).
        #[config(since = pg18)]
        NotNull(TableNotNull),
    }
}

// Added in 18: a table-level `NOT NULL` constraint
// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18, item 6).
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// `NOT NULL column [NO INHERIT] [NOT VALID]` — a table-level not-null
    /// constraint, which can have a name.
    #[derive(Debug)]
    pub struct TableNotNull {
        #[tok(NOT, NULL, this)]
        pub column: crate::tokens::ColId,
        /// gram.y `ConstraintAttributeSpec`.
        pub attrs: zero_or_many!(TableNotNullAttr),
    }
}

recursa::ast_node! {
    /// A table-level constraint with optional `CONSTRAINT name` prefix.
    #[derive(Debug)]
    pub struct TableConstraint {
        pub name: Option<ConstraintNamePrefix>,
        pub kind: TableConstraintKind,
    }
}

recursa::ast_node! {
    /// A single `INCLUDING` / `EXCLUDING` option on a `LIKE` source table clause.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `INCLUDING what`.
    #[derive(Debug)]
    pub struct IncludingOption {
        #[tok(INCLUDING, this)]
        pub what: LikeOptionKind,
    }
}

recursa::ast_node! {
    /// `EXCLUDING what`.
    #[derive(Debug)]
    pub struct ExcludingOption {
        #[tok(EXCLUDING, this)]
        pub what: LikeOptionKind,
    }
}

recursa::ast_node! {
    /// One option on a `LIKE table` clause.
    #[derive(Debug)]
    pub enum LikeOption {
        Including(IncludingOption),
        Excluding(ExcludingOption),
    }
}

recursa::ast_node! {
    /// `LIKE source_table [INCLUDING/EXCLUDING option ...]` clause in a column
    /// list body. Copies column definitions (and optionally other properties)
    /// from an existing table.
    #[derive(Debug)]
    pub struct LikeClause {
        #[tok(LIKE, this)]
        pub source: crate::ast::shared::names::QualifiedName,
        pub options: zero_or_many!(LikeOption),
    }
}

recursa::ast_node! {
    /// One item in a CREATE TABLE column list: a `LIKE table` clause, a
    /// table-level constraint, or a column definition.
    ///
    /// Variant ordering: the `Like` variant starts with the `LIKE` keyword and
    /// must come first (its leading token is otherwise an infix operator in
    /// expressions, so it can't collide with `Column` which starts with an
    /// ident). `Constraint` must come before `Column` because its leading
    /// tokens (`CONSTRAINT`, `PRIMARY`, `UNIQUE`, `FOREIGN`, `CHECK`) are
    /// keywords, while a `Column` starts with an identifier.
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum ColumnOrConstraint {
        Like(LikeClause),
        Constraint(TableConstraint),
        Column(ColumnDef),
    }
}

recursa::ast_node! {
    /// Optional TEMP or TEMPORARY keyword.
    #[derive(Debug)]
    pub enum TempKw {
        #[tok(TEMP)]
        Temp,
        #[tok(TEMPORARY)]
        Temporary,
    }
}

recursa::ast_node! {
    /// INHERITS clause: `INHERITS (parent, ...)`
    #[derive(Debug)]
    #[tok(INHERITS, LPAREN, this, RPAREN)]
    pub struct InheritsClause {
        #[sep(COMMA)]
        pub parents: zero_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `TABLESPACE name` clause on CREATE TABLE / CREATE INDEX, placing the
    /// relation into a non-default tablespace.
    #[derive(Debug)]
    pub struct TablespaceClause {
        #[tok(TABLESPACE, this)]
        pub name: literal::Ident,
    }
}

/// Legacy OIDS choice exposed by [`ColumnsBody::with_oids`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WithOidsClause {
    WithOids,
    WithoutOids,
}

recursa::ast_node! {
    /// Parenthesized storage parameters following `WITH` on CREATE TABLE.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ColumnsStorageParams(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(crate::ast::ddl::index::StorageParam),
    );
}

recursa::ast_node! {
    /// Payload after the common CREATE TABLE `WITH` prefix.
    #[derive(Debug)]
    pub enum ColumnsWithValue {
        #[tok(OIDS)]
        Oids,
        Storage(ColumnsStorageParams),
    }
}

recursa::ast_node! {
    /// CREATE TABLE's mutually exclusive `WITH OIDS`, `WITH (...)`, and
    /// `WITHOUT OIDS` clauses.
    ///
    /// Factoring `WITH` before choosing `OIDS` or `(` lets the parser use the
    /// second token for disambiguation instead of committing one optional field
    /// before it can try the other.
    #[derive(Debug)]
    pub enum ColumnsWithClause {
        With(#[tok(WITH, this)] ColumnsWithValue),
        #[tok(WITHOUT, OIDS)]
        WithoutOids,
    }
}

recursa::ast_node! {
    /// `USING access_method` clause on CREATE TABLE, selecting a non-default
    /// table access method (e.g. `heap`, `heap2`).
    #[derive(Debug)]
    pub struct UsingAccessMethodClause {
        #[tok(USING, this)]
        pub method: literal::Ident,
    }
}

recursa::ast_node! {
    /// Column-based table body: `(cols_and_constraints) [INHERITS (...)] [PARTITION BY ...]`
    #[derive(Debug)]
    pub struct ColumnsBody {
        pub columns: TableElementList,
        pub inherits: Option<InheritsClause>,
        pub partition_by: Option<PartitionByClause>,
        pub using: Option<UsingAccessMethodClause>,
        pub with: Option<ColumnsWithClause>,
        pub on_commit: Option<OnCommitClause>,
        pub tablespace: Option<TablespaceClause>,
    }
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

recursa::ast_node! {
    /// Parenthesized list of zero or more table elements.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct TableElementList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(ColumnOrConstraint),
    );
}

recursa::ast_node! {
    /// `ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP }` for temp tables.
    ///
    /// Variant ordering: distinct first tokens (`PRESERVE` / `DELETE` / `DROP`),
    /// so order is for clarity.
    #[derive(Debug)]
    pub struct OnCommitClause {
        #[tok(ON, COMMIT, this)]
        pub action: OnCommitAction,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum OnCommitAction {
        #[tok(PRESERVE, ROWS)]
        PreserveRows,
        #[tok(DELETE, ROWS)]
        DeleteRows,
        #[tok(DROP)]
        Drop,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    #[allow(clippy::large_enum_variant)]
    pub enum PartitionColumnOption {
        Constraint(TableConstraint),
        Column(PartitionColumnOptionDef),
    }
}

recursa::ast_node! {
    /// Per-partition column option: `name [WITH OPTIONS] [COLLATE "..."]
    /// [constraints...]`. Overrides constraints/collation for a column
    /// inherited from the partitioned parent table.
    #[derive(Debug)]
    pub struct PartitionColumnOptionDef {
        pub name: literal::Ident,
        #[presence(WITH, OPTIONS)]
        pub with_options: bool,
        pub collate: Option<CollateClause>,
        pub constraints: zero_or_many!(ColumnConstraint),
    }
}

recursa::ast_node! {
    /// Optional parenthesized column-option list on typed and partition tables.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct PartitionColumnOptionList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(PartitionColumnOption),
    );
}

recursa::ast_node! {
    /// Partition-of table body: `PARTITION OF parent [(col_options, ...)] FOR VALUES IN (...) [PARTITION BY ...]`
    ///
    /// The optional `(col_options, ...)` list is a per-partition override of
    /// column constraints (e.g. `b NOT NULL`, `b DEFAULT 1`, `CONSTRAINT c CHECK
    /// (...)`), reusing the same `ColumnOrConstraint` grammar as a columns-based
    /// table body.
    #[derive(Debug)]
    pub struct PartitionOfBody {
        #[tok(PARTITION, OF, this)]
        pub parent: crate::ast::shared::names::QualifiedName,
        pub column_options: Option<PartitionColumnOptionList>,
        pub for_values: Option<ForValuesClause>,
        #[presence(DEFAULT)]
        pub default: bool,
        pub partition_by: Option<PartitionByClause>,
        pub using: Option<UsingAccessMethodClause>,
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        pub on_commit: Option<OnCommitClause>,
        pub tablespace: Option<TablespaceClause>,
    }
}

recursa::ast_node! {
    /// `OF type_name [(column_options)]` — typed-table body.
    ///
    /// Creates a table whose columns are derived from a composite type.
    /// Optional column options override constraints/defaults from the type.
    #[derive(Debug)]
    pub struct OfTypeBody {
        #[tok(OF, this)]
        pub type_name: crate::ast::shared::names::QualifiedName,
        pub column_options: Option<PartitionColumnOptionList>,
    }
}

recursa::ast_node! {
    /// The source of a CTAS body after `AS` — a query, or a prepared statement
    /// invoked with `EXECUTE name [(args)]`.
    ///
    /// gram.y routes the second spelling through its own production
    /// (`ExecuteStmt: CREATE OptTemp TABLE create_as_target AS EXECUTE name
    /// execute_param_clause opt_with_data`), but on the surface it is a plain
    /// alternative in the same position, sharing the `WITH [NO] DATA` tail. The
    /// argument list is the same `execute_param_clause` as the standalone
    /// `EXECUTE` statement, so [`ExecuteStmt`] is reused whole.
    ///
    /// Variant ordering: `Execute` begins with the `EXECUTE` keyword while
    /// `Query` begins with `SELECT` / `TABLE` / `VALUES` / `WITH` / `(`, so the
    /// first-token sets are disjoint and order is for clarity.
    ///
    /// [`ExecuteStmt`]: crate::ast::tcl::prepared::ExecuteStmt
    #[derive(Debug)]
    pub enum CtasSource {
        Execute(boxed!(crate::ast::tcl::prepared::ExecuteStmt)),
        Query(boxed!(crate::ast::dml::values::Subquery)),
    }
}

recursa::ast_node! {
    /// AS-query table body: `AS { SELECT ... | EXECUTE name [(args)] } [WITH [NO] DATA]`.
    #[derive(Debug)]
    pub struct AsQueryBody {
        /// Optional `WITH (param = value, ...)` storage parameters before `AS`.
        pub with_storage: Option<crate::ast::ddl::index::WithStorage>,
        /// Optional `TABLESPACE name` before `AS`.
        pub tablespace: Option<TablespaceClause>,
        #[tok(AS, this)]
        pub source: CtasSource,
        pub with_data: Option<WithDataClause>,
    }
}

recursa::ast_node! {
    /// `WITH DATA` or `WITH NO DATA` modifier on a CTAS query.
    ///
    /// Variant ordering: `NoData` (`WITH NO DATA`, longer) before `Data`.
    #[derive(Debug)]
    pub enum WithDataClause {
        #[tok(WITH, NO, DATA)]
        NoData,
        #[tok(WITH, DATA)]
        Data,
    }
}

recursa::ast_node! {
    /// `(col, col, ...) [ON COMMIT ...] AS source [WITH [NO] DATA]` — CTAS with
    /// column list. The source is a query or an `EXECUTE` of a prepared
    /// statement, as in `CREATE TABLE t (a) AS EXECUTE data_sel WITH DATA`.
    #[derive(Debug)]
    pub struct ColumnsAsQueryBody {
        pub columns: CtasColumnList,
        pub on_commit: Option<OnCommitClause>,
        #[tok(AS, this)]
        pub source: CtasSource,
        pub with_data: Option<WithDataClause>,
    }
}

recursa::ast_node! {
    /// Required, non-empty CTAS output-column list.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CtasColumnList {
        #[sep(COMMA)]
        pub columns: one_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// The body of a CREATE TABLE statement after `CREATE [TEMP] TABLE name`.
    ///
    /// Variant ordering: AsQuery (`AS`) and PartitionOf (`PARTITION`) start with
    /// keywords; Columns starts with `(`. Longest-match-wins disambiguates.
    #[derive(Debug)]
    pub enum CreateTableBody {
        AsQuery(AsQueryBody),
        PartitionOf(PartitionOfBody),
        /// `OF type_name [(column_options)]` — typed table.
        /// Distinct first token `OF`, no ambiguity with other variants.
        OfType(OfTypeBody),
        /// `(col, ...) AS query` — CTAS with explicit column list.
        /// Listed before `Columns` so the `( ... ) AS` form wins over the
        /// columns-only `( ... )` form via longer match.
        ColumnsAsQuery(ColumnsAsQueryBody),
        Columns(ColumnsBody),
    }
}

recursa::ast_node! {
    /// ```sql
    /// CREATE [TEMP] TABLE statement.
    /// ```
    #[derive(Debug)]
    #[tok(CREATE, this)]
    pub struct CreateTableStmt {
        pub temp: Option<TempKw>,
        #[tok(this, TABLE)]
        #[presence(UNLOGGED)]
        pub unlogged: bool,
        #[presence(IF, NOT, EXISTS)]
        pub if_not_exists: bool,
        pub name: crate::ast::shared::names::QualifiedName,
        /// `USING am` between the table name and an `AS query` body, e.g.
        /// `CREATE TABLE t USING heap2 AS SELECT ...`. When the body starts
        /// with `(`, this clause is absent and `USING` appears after the
        /// column list inside `ColumnsBody`.
        pub using: Option<UsingAccessMethodClause>,
        pub body: CreateTableBody,
    }
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

recursa::ast_node! {
    /// One partition key item: `{ column_name | ( expr ) } [COLLATE collation] [opclass_name]`.
    ///
    /// The `opclass` operator class name is a trailing identifier (e.g.
    /// `point_ops`, `int4_ops`) that binds the column/expression to a specific
    /// operator class for the partition strategy.
    #[derive(Debug)]
    pub struct PartitionKeyItem {
        pub target: crate::ast::ddl::index::IndexTarget,
        /// gram.y `part_elem: ... opt_collate opt_qualified_name`: both are
        /// `any_name`.
        #[tok(COLLATE, this)]
        pub collate: Option<crate::ast::shared::names::QualifiedName>,
        pub opclass: Option<crate::ast::shared::names::QualifiedName>,
    }
}

recursa::ast_node! {
    /// PARTITION BY LIST (col) clause.
    #[derive(Debug)]
    pub struct PartitionByClause {
        #[tok(PARTITION, BY, this)]
        pub strategy: literal::AliasName,
        /// Partition key items — may be plain column names or expressions like
        /// `((a+b)/2)`, optionally followed by a trailing opclass name.
        pub columns: PartitionKeyList,
    }
}

recursa::ast_node! {
    /// Parenthesized, comma-separated partition key list (`part_params`).
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct PartitionKeyList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(PartitionKeyItem),
    );
}

recursa::ast_node! {
    /// FOR VALUES IN (val, ...) clause — legacy name kept for backward compat
    /// with partition.rs own tests; the general form lives in `ForValuesClause`.
    #[derive(Debug)]
    #[tok(FOR, VALUES, IN, LPAREN, this, RPAREN)]
    pub struct ForValuesInClause {
        #[sep(COMMA)]
        pub values: zero_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// `FROM (...) TO (...)` range partition spec.
    #[derive(Debug)]
    pub struct FromToSpec {
        #[tok(FROM, this)]
        pub from_values: PartitionValues,
        #[tok(TO, this)]
        pub to_values: PartitionValues,
    }
}

recursa::ast_node! {
    /// Parenthesized value list in a partition bound.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct PartitionValues(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(Expr),
    );
}

recursa::ast_node! {
    /// `IN (val, ...)` list partition spec.
    #[derive(Debug)]
    #[tok(IN, LPAREN, this, RPAREN)]
    pub struct InListSpec {
        #[sep(COMMA)]
        pub values: zero_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// `MODULUS n` entry.
    #[derive(Debug)]
    pub struct ModulusEntry {
        #[tok(MODULUS, this)]
        pub value: Expr,
    }
}

recursa::ast_node! {
    /// `REMAINDER n` entry.
    #[derive(Debug)]
    pub struct RemainderEntry {
        #[tok(REMAINDER, this)]
        pub value: Expr,
    }
}

recursa::ast_node! {
    /// One item in `WITH (...)` for hash partitioning: MODULUS n or REMAINDER n.
    #[derive(Debug)]
    pub enum HashPartItem {
        Modulus(ModulusEntry),
        Remainder(RemainderEntry),
    }
}

recursa::ast_node! {
    /// `WITH (MODULUS n, REMAINDER m)` hash partition spec.
    #[derive(Debug)]
    #[tok(WITH, LPAREN, this, RPAREN)]
    pub struct WithModulusSpec {
        #[sep(COMMA)]
        pub items: zero_or_many!(HashPartItem),
    }
}

recursa::ast_node! {
    /// Body after `FOR VALUES` in a PARTITION OF clause. Variant ordering:
    /// `From` starts with `FROM`, `In` starts with `IN`, `With` starts with `WITH` —
    /// all distinct keywords, so peek disambiguation is trivial.
    #[derive(Debug)]
    pub enum ForValuesSpec {
        From(FromToSpec),
        In(InListSpec),
        With(WithModulusSpec),
    }
}

recursa::ast_node! {
    /// Full `FOR VALUES ...` clause in a `PARTITION OF ...` body.
    #[derive(Debug)]
    pub struct ForValuesClause {
        #[tok(FOR, VALUES, this)]
        pub spec: ForValuesSpec,
    }
}

recursa::ast_node! {
    /// Column definition in partition table: `name type`.
    #[derive(Debug)]
    pub struct PartitionColumnDef {
        pub name: literal::Ident,
        pub type_name: TypeName,
    }
}

recursa::ast_node! {
    /// Parenthesized column-definition list of a standalone partitioned table.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct PartitionColumnDefList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(PartitionColumnDef),
    );
}

recursa::ast_node! {
    /// CREATE TABLE with PARTITION BY: `CREATE TABLE name (cols) PARTITION BY strategy (cols)`.
    #[derive(Debug)]
    pub struct CreatePartitionedTableStmt {
        #[tok(CREATE, TABLE, this)]
        pub name: literal::Ident,
        pub columns: PartitionColumnDefList,
        pub partition_by: PartitionByClause,
    }
}

recursa::ast_node! {
    /// CREATE TABLE ... PARTITION OF parent FOR VALUES IN (...) [PARTITION BY ...].
    #[derive(Debug)]
    pub struct CreatePartitionOfStmt {
        #[tok(CREATE, TABLE, this)]
        pub name: literal::Ident,
        #[tok(PARTITION, OF, this)]
        pub parent: literal::Ident,
        pub for_values: ForValuesInClause,
        pub partition_by: Option<PartitionByClause>,
    }
}

// -----------------------------------------------------------------------
// DROP TABLE — folded in from the former `ast/drop_table.rs`.
// -----------------------------------------------------------------------

recursa::ast_node! {
    /// ```sql
    /// DROP TABLE [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
    /// ```
    #[derive(Debug)]
    #[tok(DROP, TABLE, this)]
    pub struct DropTableStmt {
        #[presence(IF, EXISTS)]
        pub if_exists: bool,
        #[sep(COMMA)]
        /// gram.y `any_name_list`: one or more names.
        pub names: one_or_many!(QualifiedName),
        pub behavior: Option<DropBehavior>,
    }
}

// =========================================================================
// ALTER/DROP TABLE — appended from simple_stmts.rs during physical extraction.
// =========================================================================

recursa::ast_node! {
    /// `ALTER TABLE ...` — Postgres' `AlterTableStmt` (table object kind), plus
    /// the table-shaped branches of `RenameStmt` and `AlterObjectSchemaStmt`.
    ///
    /// pg-sql keeps one LR production family for `ALTER TABLE`, so this one struct
    /// covers every shape that begins with those two keywords:
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
    #[derive(Debug)]
    pub struct AlterTableStmt {
        #[tok(ALTER, TABLE, this)]
        pub body: AlterTableBody,
    }
}

recursa::ast_node! {
    /// Body of `ALTER TABLE ...` — either the bulk-relocate `ALL IN TABLESPACE`
    /// form or a per-relation form.
    ///
    /// Variant ordering: `All` (starts with `ALL`) before `Single` (starts with
    /// `IF` / `ONLY` / `qualified_name`, never `ALL`).
    #[derive(Debug)]
    pub enum AlterTableBody {
        All(AllInTablespaceBody),
        Single(AlterTableSingle),
    }
}

recursa::ast_node! {
    /// Per-relation `ALTER TABLE` body: `[IF EXISTS] [ONLY] name [*] action`.
    ///
    /// The relation reference is Postgres' `relation_expr`: a `qualified_name`
    /// optionally prefixed by `ONLY` and/or suffixed by `*`. The `ONLY (name)`
    /// parenthesised form is not exercised by any corpus statement, so it is
    /// not modelled.
    #[derive(Debug)]
    pub struct AlterTableSingle {
        pub if_exists: Option<IfExists>,
        pub relation: crate::ast::shared::names::RelationExpr,
        pub action: AlterTableSingleAction,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterTableSingleAction {
        RenameConstraint(AlterTableRenameConstraint),
        RenameColumn(RenameColumnClause),
        Rename(RenameTo),
        SetSchema(SetSchemaClause),
        Partition(PartitionCmd),
        Cmds(AlterTableCmds),
    }
}

recursa::ast_node! {
    /// `RENAME CONSTRAINT old TO new` — Postgres' `RenameStmt` branch for table
    /// constraints.
    #[derive(Debug)]
    pub struct AlterTableRenameConstraint {
        #[tok(RENAME, CONSTRAINT, this)]
        pub old_name: literal::Ident,
        #[tok(TO, this)]
        pub new_name: literal::Ident,
    }
}

recursa::ast_node! {
    /// Comma-separated `alter_table_cmds` on ALTER TABLE.
    #[derive(Debug)]
    pub struct AlterTableCmds {
        #[sep(COMMA)]
        pub cmds: one_or_many!(AlterTableCmd),
    }
}

recursa::ast_node! {
    /// Postgres' `partition_cmd`: a single `ATTACH PARTITION` or `DETACH
    /// PARTITION` action on a partitioned table.
    ///
    /// Variant ordering: `Attach` (ATTACH) and `Detach` (DETACH) have disjoint
    /// first tokens, so order is for clarity.
    #[derive(Debug)]
    pub enum PartitionCmd {
        Attach(AttachPartitionCmd),
        Detach(DetachPartitionCmd),
    }
}

recursa::ast_node! {
    /// `ATTACH PARTITION qualified_name partition_bound_spec` — adds an existing
    /// table as a partition of the target partitioned table.
    #[derive(Debug)]
    pub struct AttachPartitionCmd {
        #[tok(ATTACH, PARTITION, this)]
        pub name: QualifiedName,
        pub bound: PartitionBoundSpec,
    }
}

recursa::ast_node! {
    /// `DETACH PARTITION qualified_name [CONCURRENTLY | FINALIZE]` — removes a
    /// partition from its parent.
    #[derive(Debug)]
    pub struct DetachPartitionCmd {
        #[tok(DETACH, PARTITION, this)]
        pub name: QualifiedName,
        pub mode: Option<DetachPartitionMode>,
    }
}

recursa::ast_node! {
    /// Trailing mode keyword on `DETACH PARTITION`: `CONCURRENTLY` (the default
    /// nonblocking detach) or `FINALIZE` (completes a previously-CONCURRENTLY
    /// detached partition).
    #[derive(Debug)]
    pub enum DetachPartitionMode {
        #[tok(CONCURRENTLY)]
        Concurrently,
        #[tok(FINALIZE)]
        Finalize,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum PartitionBoundSpec {
        #[tok(DEFAULT)]
        Default,
        ForValues(crate::ast::ddl::table::ForValuesClause),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum AlterTableCmd {
        // ADD ... — longer prefixes first.
        AddColumnIfNotExists(AddColumnIfNotExistsCmd),
        AddIfNotExists(AddIfNotExistsCmd),
        AddColumn(AddColumnCmd),
        AddConstraint(AddTableConstraintCmd),
        AddColumnBare(AddColumnBareCmd),
        // ALTER CONSTRAINT ...
        AlterConstraint(AlterConstraintCmd),
        /// Added in 18: gram.y `alter_table_cmd: ALTER CONSTRAINT name
        /// INHERIT` (docs/research/postgres-14-19-sql-syntax-changes.md,
        /// PostgreSQL 18, item 5; REL_18_6 gram.y `alter_table_cmd`).
        #[config(since = pg18)]
        AlterConstraintInherit(AlterConstraintInheritCmd),
        // ALTER [COLUMN] colname ...
        AlterColumn(AlterColumnCmd),
        // DROP ... — longer prefixes first.
        DropConstraintIfExists(DropConstraintIfExistsCmd),
        DropConstraint(DropConstraintCmd),
        DropColumnIfExists(DropColumnIfExistsCmd),
        DropColumn(DropColumnCmd),
        // ENABLE / DISABLE variants — longer prefixes first.
        EnableReplicaTrigger(EnableReplicaTriggerCmd),
        EnableAlwaysTrigger(EnableAlwaysTriggerCmd),
        EnableReplicaRule(EnableReplicaRuleCmd),
        EnableAlwaysRule(EnableAlwaysRuleCmd),
        EnableTrigger(EnableTriggerCmd),
        EnableRule(EnableRuleCmd),
        EnableRowSecurity(EnableRowSecurityCmd),
        DisableTrigger(DisableTriggerCmd),
        DisableRule(DisableRuleCmd),
        DisableRowSecurity(DisableRowSecurityCmd),
        // FORCE / NO FORCE ROW LEVEL SECURITY — NO FORCE listed first.
        NoForceRowSecurity(NoForceRowSecurityCmd),
        ForceRowSecurity(ForceRowSecurityCmd),
        // CLUSTER ON / SET WITHOUT CLUSTER.
        ClusterOn(ClusterOnCmd),
        // SET ... variants — longest prefixes first.
        SetWithoutCluster(SetWithoutClusterCmd),
        SetWithoutOids(SetWithoutOidsCmd),
        SetLogged(SetLoggedCmd),
        SetUnlogged(SetUnloggedCmd),
        /// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
        /// (REL_15_19 gram.y `alter_table_cmd: SET ACCESS METHOD name`).
        #[config(since = pg15)]
        SetAccessMethod(SetAccessMethodClause),
        SetTablespace(SetTablespaceClause),
        SetReloptions(SetReloptions),
        ResetReloptions(ResetReloptions),
        // REPLICA IDENTITY ...
        ReplicaIdentity(ReplicaIdentityCmd),
        // INHERIT / NO INHERIT — NO INHERIT listed first.
        NoInherit(NoInheritCmd),
        Inherit(InheritCmd),
        // OF / NOT OF.
        NotOf(NotOfCmd),
        Of(OfCmd),
        // OWNER TO role.
        Owner(OwnerTo),
        // VALIDATE CONSTRAINT name.
        ValidateConstraint(ValidateConstraintCmd),
        // [NO] DEPENDS ON EXTENSION name.
        DependsOnExtension(DependsOnExtension),
        // OPTIONS (...)  — foreign-table alter_generic_options. Listed last so
        // every keyword-led variant above wins first.
        GenericOptions(AlterGenericOptions),
    }
}

recursa::ast_node! {
    /// `ADD COLUMN IF NOT EXISTS columnDef`.
    #[derive(Debug)]
    pub struct AddColumnIfNotExistsCmd {
        #[tok(ADD, COLUMN, this)]
        pub if_not_exists: IfNotExists,
        pub column_def: crate::ast::ddl::table::ColumnDef,
    }
}

recursa::ast_node! {
    /// `ADD IF NOT EXISTS columnDef`.
    #[derive(Debug)]
    pub struct AddIfNotExistsCmd {
        #[tok(ADD, this)]
        pub if_not_exists: IfNotExists,
        pub column_def: crate::ast::ddl::table::ColumnDef,
    }
}

recursa::ast_node! {
    /// `ADD COLUMN columnDef`.
    #[derive(Debug)]
    pub struct AddColumnCmd {
        #[tok(ADD, COLUMN, this)]
        pub column_def: crate::ast::ddl::table::ColumnDef,
    }
}

recursa::ast_node! {
    /// `ADD TableConstraint [NOT VALID]` — table-level constraint with optional
    /// `NOT VALID` marker (Postgres routes this through `ConstraintAttributeSpec`
    /// on the constraint).
    ///
    /// The `NOT VALID` modifier is part of the constraint's attribute list in
    /// gram.y; pg-sql models it as a trailing `Option` on this `AlterTableCmd`
    /// variant for symmetry with the corpus' usage (it only ever sits at the end).
    #[derive(Debug)]
    pub struct AddTableConstraintCmd {
        /// gram.y `ADD_P TableConstraint`: `NOT VALID` is an entry of the
        /// constraint's own `ConstraintAttributeSpec`, not a suffix of the
        /// command.
        #[tok(ADD, this)]
        pub constraint: crate::ast::ddl::table::TableConstraint,
    }
}

recursa::ast_node! {
    /// `ADD columnDef` (no `COLUMN` keyword, no `IF NOT EXISTS`).
    ///
    /// Listed last in the ADD family because every column definition begins with
    /// a bareword (the column name), which would otherwise greedily swallow
    /// `COLUMN`, `IF`, `CONSTRAINT`, etc.
    #[derive(Debug)]
    pub struct AddColumnBareCmd {
        #[tok(ADD, this)]
        pub column_def: crate::ast::ddl::table::ColumnDef,
    }
}

recursa::ast_node! {
    /// `ALTER CONSTRAINT name [DEFERRABLE | NOT DEFERRABLE] [INITIALLY {DEFERRED
    /// | IMMEDIATE}]` — Postgres' `AT_AlterConstraint` action.
    #[derive(Debug)]
    pub struct AlterConstraintCmd {
        #[tok(ALTER, CONSTRAINT, this)]
        pub name: literal::Ident,
        /// gram.y `ConstraintAttributeSpec`.
        pub attrs: zero_or_many!(AlterConstraintAttr),
    }
}

// Added in 18: see `AlterTableCmd::AlterConstraintInherit`.
#[cfg(feature = "since-pg18")]
recursa::ast_node! {
    /// `ALTER CONSTRAINT name INHERIT` — makes a not-null constraint
    /// inheritable again.
    #[derive(Debug)]
    pub struct AlterConstraintInheritCmd {
        #[tok(ALTER, CONSTRAINT, this, INHERIT)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `ALTER [COLUMN] colname …` — the big `ALTER COLUMN` cmd. The `colname`
    /// can also be a numeric column index for `SET STATISTICS` (used on indexes,
    /// not on tables — but accepted here for symmetry).
    #[derive(Debug)]
    pub struct AlterColumnCmd {
        #[tok(ALTER, optional(COLUMN), this)]
        pub col_ref: ColumnRef,
        pub action: AlterColumnAction,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterColumnAction {
        // SET ... — longest prefixes first.
        // Added in 17: see `AlterColSetExpression`.
        #[config(since = pg17)]
        SetExpressionAs(AlterColSetExpression),
        SetDataType(AlterColSetDataType),
        SetStatistics(AlterColSetStatistics),
        SetCompression(AlterColSetCompression),
        SetStorage(AlterColSetStorage),
        SetNotNull(AlterColSetNotNull),
        SetDefault(AlterColSetDefault),
        SetReloptions(SetReloptions),
        // DROP ... — longest prefixes first.
        DropExpression(AlterColDropExpression),
        DropIdentity(AlterColDropIdentity),
        DropNotNull(AlterColDropNotNull),
        DropDefault(AlterColDropDefault),
        // ADD GENERATED ... AS IDENTITY [(opts)]
        AddIdentity(AlterColAddIdentity),
        // RESET (reloptions)
        ResetReloptions(ResetReloptions),
        // TYPE Typename [COLLATE …] [USING expr] — without leading SET DATA.
        Type(AlterColTypeBare),
        // alter_identity_column_option_list — chained SET GENERATED / SET
        // seq_option / RESTART [WITH n] items (single- or multi-element).
        IdentityOpts(AlterIdentityOpts),
        // FOREIGN-TABLE column OPTIONS (...).
        GenericOptions(AlterGenericOptions),
    }
}

recursa::ast_node! {
    /// One element of `alter_identity_column_option_list` (gram.y):
    /// `SET GENERATED {ALWAYS|BY DEFAULT}` | `SET seq_option` | `RESTART [WITH n]`.
    ///
    /// Variant ordering: `SetGenerated` (`SET GENERATED …`) before
    /// `SetSeqOption` (`SET …seq_option`) so the more specific `SET GENERATED`
    /// 2-token peek commits first; both share the leading `SET`. `Restart`
    /// has a disjoint leading `RESTART` token.
    #[derive(Debug)]
    pub enum AlterIdentityOption {
        SetGenerated(AlterColSetGenerated),
        SetSeqOption(AlterColSetSeqOption),
        Restart(AlterColRestart),
    }
}

recursa::ast_node! {
    /// `alter_identity_column_option_list` — one or more
    /// [`AlterIdentityOption`] items in sequence, no separator.
    #[derive(Debug)]
    pub struct AlterIdentityOpts {
        pub items: one_or_many!(AlterIdentityOption),
    }
}

// Added in 17: research, PostgreSQL 17, "Changes to existing statements"
// (REL_17_11 gram.y 2441).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `SET EXPRESSION AS (expr)` — adjust a generated column's expression.
    #[derive(Debug)]
    pub struct AlterColSetExpression {
        #[tok(SET, EXPRESSION, AS, LPAREN, this, RPAREN)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `[SET DATA] TYPE Typename [COLLATE name] [USING expr]` — change a column's
    /// type. The `SET DATA` is mandatory in this variant (the leading-`SET` form);
    /// the bare `TYPE …` form is `AlterColTypeBare`.
    #[derive(Debug)]
    pub struct AlterColSetDataType {
        #[tok(SET, DATA, TYPE, this)]
        pub type_name: CastType,
        pub collate: Option<crate::ast::ddl::table::CollateClause>,
        pub using: Option<AlterColUsing>,
    }
}

recursa::ast_node! {
    /// `SET STATISTICS { SignedIconst | DEFAULT }` — adjust per-column statistics
    /// target.
    #[derive(Debug)]
    pub struct AlterColSetStatistics {
        #[tok(SET, STATISTICS, this)]
        pub value: SetStatisticsValue,
    }
}

recursa::ast_node! {
    /// `SET COMPRESSION { name | DEFAULT }` — change a column's compression
    /// method.
    #[derive(Debug)]
    pub struct AlterColSetCompression {
        #[tok(SET, COMPRESSION, this)]
        pub target: ColumnCompressionTarget,
    }
}

recursa::ast_node! {
    /// `SET DEFAULT expr` — set a column's default expression.
    #[derive(Debug)]
    pub struct AlterColSetDefault {
        #[tok(SET, DEFAULT, this)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `DROP DEFAULT` — drop a column's default expression.
    #[derive(Debug)]
    pub enum AlterColDropDefault {
        #[tok(DROP, DEFAULT)]
        Value,
    }
}

recursa::ast_node! {
    /// `USING expr` clause on `ALTER COLUMN … TYPE …`.
    #[derive(Debug)]
    pub struct AlterColUsing {
        #[tok(USING, this)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `TYPE Typename [COLLATE name] [USING expr]` — change a column's type
    /// without the leading `SET DATA`. Postgres accepts both spellings.
    #[derive(Debug)]
    pub struct AlterColTypeBare {
        #[tok(TYPE, this)]
        pub type_name: CastType,
        pub collate: Option<crate::ast::ddl::table::CollateClause>,
        pub using: Option<AlterColUsing>,
    }
}

recursa::ast_node! {
    /// `SET GENERATED { ALWAYS | BY DEFAULT }` — change the identity column
    /// generation mode.
    #[derive(Debug)]
    pub struct AlterColSetGenerated {
        #[tok(SET, GENERATED, this)]
        pub mode: crate::ast::ddl::table::GeneratedIdentityMode,
    }
}

recursa::ast_node! {
    /// `SET STORAGE { PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT }` — adjust a
    /// column's TOAST storage strategy.
    #[derive(Debug)]
    pub struct AlterColSetStorage {
        // 16 accepts `DEFAULT` (`column_storage`): research, PostgreSQL 16,
        // "Changes to existing statements" (commit b9424d014). REL_15_19 gram.y
        // has `ALTER opt_column ColId SET STORAGE ColId`.
        // A widening, not an addition (ADR 0010, `docs/minimum-version.md`):
        // the newer type accepts everything the older one does, so both arms
        // only remove and neither records. The gate for what 16 adds belongs
        // to those shapes.
        #[cfg(feature = "since-pg16")]
        #[tok(SET, STORAGE, this)]
        pub mode: crate::ast::ddl::table::ColumnStorageMode,
        #[cfg(not(feature = "since-pg16"))]
        #[tok(SET, STORAGE, this)]
        pub mode: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `SET NOT NULL` — add a NOT NULL marker on the column.
    #[derive(Debug)]
    pub enum AlterColSetNotNull {
        #[tok(SET, NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
    /// One `SET seqOpt` action on an identity column —
    /// `SET { START WITH | INCREMENT BY | MINVALUE | MAXVALUE | CACHE | CYCLE |
    /// NO MINVALUE | NO MAXVALUE | NO CYCLE }`.
    ///
    /// Reuses `IdentitySeqOption` from `create_table.rs` so the full sequence-
    /// option set is supported (and the formatter is shared).
    #[derive(Debug)]
    pub struct AlterColSetSeqOption {
        #[tok(SET, this)]
        pub option: crate::ast::ddl::table::IdentitySeqOption,
    }
}

recursa::ast_node! {
    /// `DROP EXPRESSION [IF EXISTS]` — remove a generated column's expression,
    /// turning it into a regular column.
    #[derive(Debug)]
    #[tok(DROP, EXPRESSION, this)]
    pub struct AlterColDropExpression {
        pub if_exists: Option<IfExists>,
    }
}

recursa::ast_node! {
    /// `DROP IDENTITY [IF EXISTS]` — remove an identity-column property.
    #[derive(Debug)]
    #[tok(DROP, IDENTITY, this)]
    pub struct AlterColDropIdentity {
        pub if_exists: Option<IfExists>,
    }
}

recursa::ast_node! {
    /// `DROP NOT NULL` — remove a NOT NULL marker.
    #[derive(Debug)]
    pub enum AlterColDropNotNull {
        #[tok(DROP, NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
    /// `ADD GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY [(seq_options)]` —
    /// add an identity property to an existing column.
    #[derive(Debug)]
    pub struct AlterColAddIdentity {
        #[tok(ADD, this)]
        pub identity: crate::ast::ddl::table::GeneratedIdentityConstraint,
    }
}

recursa::ast_node! {
    /// `RESTART [WITH NumericOnly]` — restart an identity column's sequence.
    #[derive(Debug)]
    #[tok(RESTART, this)]
    pub struct AlterColRestart {
        pub value: Option<RestartWith>,
    }
}

recursa::ast_node! {
    /// `[WITH] NumericOnly` — the value portion of `RESTART`.
    #[derive(Debug)]
    pub struct RestartWith {
        #[tok(optional(WITH), this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `DROP CONSTRAINT IF EXISTS name [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropConstraintIfExistsCmd {
        #[tok(DROP, CONSTRAINT, this)]
        pub if_exists: IfExists,
        pub name: literal::Ident,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `DROP CONSTRAINT name [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropConstraintCmd {
        #[tok(DROP, CONSTRAINT, this)]
        pub name: literal::Ident,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `DROP [COLUMN] IF EXISTS colname [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropColumnIfExistsCmd {
        #[tok(DROP, optional(COLUMN), this)]
        pub if_exists: IfExists,
        pub name: literal::Ident,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `DROP [COLUMN] colname [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    pub struct DropColumnCmd {
        #[tok(DROP, optional(COLUMN), this)]
        pub name: literal::Ident,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `ENABLE TRIGGER { name | ALL | USER }`.
    #[derive(Debug)]
    pub struct EnableTriggerCmd {
        #[tok(ENABLE, TRIGGER, this)]
        pub target: TriggerOrRuleTarget,
    }
}

recursa::ast_node! {
    /// `ENABLE ALWAYS TRIGGER name`.
    #[derive(Debug)]
    pub struct EnableAlwaysTriggerCmd {
        #[tok(ENABLE, ALWAYS, TRIGGER, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `ENABLE REPLICA TRIGGER name`.
    #[derive(Debug)]
    pub struct EnableReplicaTriggerCmd {
        #[tok(ENABLE, REPLICA, TRIGGER, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `DISABLE TRIGGER { name | ALL | USER }`.
    #[derive(Debug)]
    pub struct DisableTriggerCmd {
        #[tok(DISABLE, TRIGGER, this)]
        pub target: TriggerOrRuleTarget,
    }
}

recursa::ast_node! {
    /// `ENABLE RULE name`.
    #[derive(Debug)]
    pub struct EnableRuleCmd {
        #[tok(ENABLE, RULE, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `ENABLE ALWAYS RULE name`.
    #[derive(Debug)]
    pub struct EnableAlwaysRuleCmd {
        #[tok(ENABLE, ALWAYS, RULE, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `ENABLE REPLICA RULE name`.
    #[derive(Debug)]
    pub struct EnableReplicaRuleCmd {
        #[tok(ENABLE, REPLICA, RULE, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `DISABLE RULE name`.
    #[derive(Debug)]
    pub struct DisableRuleCmd {
        #[tok(DISABLE, RULE, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// Trigger-action target on `ENABLE TRIGGER` / `DISABLE TRIGGER`:
    /// `ALL` (every trigger), `USER` (every non-internal trigger), or a named
    /// trigger.
    ///
    /// Variant ordering: keyword variants (`All` / `User`) before `Name` (ident),
    /// since `ALL` and `USER` are hard keywords that won't lex as `Ident`.
    #[derive(Debug)]
    pub enum TriggerOrRuleTarget {
        #[tok(ALL)]
        All,
        #[tok(USER)]
        User,
        Name(literal::Ident),
    }
}

recursa::ast_node! {
    /// `ENABLE ROW LEVEL SECURITY`.
    #[derive(Debug)]
    pub enum EnableRowSecurityCmd {
        #[tok(ENABLE, ROW, LEVEL, SECURITY)]
        Value,
    }
}

recursa::ast_node! {
    /// `DISABLE ROW LEVEL SECURITY`.
    #[derive(Debug)]
    pub enum DisableRowSecurityCmd {
        #[tok(DISABLE, ROW, LEVEL, SECURITY)]
        Value,
    }
}

recursa::ast_node! {
    /// `FORCE ROW LEVEL SECURITY`.
    #[derive(Debug)]
    pub enum ForceRowSecurityCmd {
        #[tok(FORCE, ROW, LEVEL, SECURITY)]
        Value,
    }
}

recursa::ast_node! {
    /// `NO FORCE ROW LEVEL SECURITY`.
    #[derive(Debug)]
    pub enum NoForceRowSecurityCmd {
        #[tok(NO, FORCE, ROW, LEVEL, SECURITY)]
        Value,
    }
}

recursa::ast_node! {
    /// `CLUSTER ON indexname`.
    #[derive(Debug)]
    pub struct ClusterOnCmd {
        #[tok(CLUSTER, ON, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `SET WITHOUT CLUSTER`.
    #[derive(Debug)]
    pub enum SetWithoutClusterCmd {
        #[tok(SET, WITHOUT, CLUSTER)]
        Value,
    }
}

recursa::ast_node! {
    /// `SET WITHOUT OIDS`.
    #[derive(Debug)]
    pub enum SetWithoutOidsCmd {
        #[tok(SET, WITHOUT, OIDS)]
        Value,
    }
}

recursa::ast_node! {
    /// `SET LOGGED`.
    #[derive(Debug)]
    pub enum SetLoggedCmd {
        #[tok(SET, LOGGED)]
        Value,
    }
}

recursa::ast_node! {
    /// `SET UNLOGGED`.
    #[derive(Debug)]
    pub enum SetUnloggedCmd {
        #[tok(SET, UNLOGGED)]
        Value,
    }
}

recursa::ast_node! {
    /// `REPLICA IDENTITY { DEFAULT | NOTHING | FULL | USING INDEX name }`.
    #[derive(Debug)]
    pub struct ReplicaIdentityCmd {
        #[tok(REPLICA, IDENTITY, this)]
        pub kind: ReplicaIdentityKind,
    }
}

recursa::ast_node! {
    /// One of `DEFAULT`, `NOTHING`, `FULL`, or `USING INDEX name`.
    ///
    /// Variant ordering: keyword-only variants (single tokens, disjoint) first;
    /// `UsingIndex` (`USING INDEX`) last — it has a unique `USING` prefix.
    #[derive(Debug)]
    pub enum ReplicaIdentityKind {
        #[tok(DEFAULT)]
        Default,
        #[tok(NOTHING)]
        Nothing,
        #[tok(FULL)]
        Full,
        UsingIndex(ReplicaIdentityUsingIndex),
    }
}

recursa::ast_node! {
    /// `USING INDEX name` — the index-backed REPLICA IDENTITY.
    #[derive(Debug)]
    pub struct ReplicaIdentityUsingIndex {
        #[tok(USING, INDEX, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `INHERIT parent`.
    #[derive(Debug)]
    pub struct InheritCmd {
        #[tok(INHERIT, this)]
        pub parent: QualifiedName,
    }
}

recursa::ast_node! {
    /// `NO INHERIT parent`.
    #[derive(Debug)]
    pub struct NoInheritCmd {
        #[tok(NO, INHERIT, this)]
        pub parent: QualifiedName,
    }
}

recursa::ast_node! {
    /// `OF type_name`.
    #[derive(Debug)]
    pub struct OfCmd {
        #[tok(OF, this)]
        pub type_name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `NOT OF` — drop the typed-table relationship.
    #[derive(Debug)]
    pub enum NotOfCmd {
        #[tok(NOT, OF)]
        Value,
    }
}

recursa::ast_node! {
    /// `VALIDATE CONSTRAINT name`.
    #[derive(Debug)]
    pub struct ValidateConstraintCmd {
        #[tok(VALIDATE, CONSTRAINT, this)]
        pub name: literal::Ident,
    }
}
