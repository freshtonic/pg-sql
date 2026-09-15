/// CREATE INDEX / DROP INDEX statement AST.
pub use crate::ast::shared::flags::{DropBehavior, IfExists, IfNotExists};

use crate::ast::dml::select::{NullsOrder, SortDir, WhereClause};
use crate::ast::session::set_reset::SetValue;
use crate::ast::shared::expr::{Expr, FunctionApplicationExpr, JsonFuncExpr};
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
recursa::ast_node! {
    #[allow(unused_imports)]
    // ---------------------------------------------------------------------------
    /// Index access method: `USING method_name`.
    ///
    /// The method name can be an identifier or one of the built-in method
    /// keywords (`btree`, `gin`, ...). We accept `literal::AliasName` so both
    /// identifiers and keywords are allowed in this position.
    #[derive(Debug)]
    pub struct UsingMethod {
        #[tok(USING, this)]
        pub method: literal::AliasName,
    }
}

recursa::ast_node! {
    /// A single opclass option: `name = value`.
    #[derive(Debug)]
    pub struct OpclassOption {
        pub name: literal::AliasName,
        #[tok(EQ, this)]
        pub value: Expr,
    }
}

recursa::ast_node! {
    /// Parenthesized opclass option list: `(name = value, ...)`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct OpclassOptions(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(OpclassOption),
    );
}

recursa::ast_node! {
    /// Opclass name plus optional options: `int4_ops [(opt = val, ...)]`.
    #[derive(Debug)]
    pub struct OpclassSpec {
        pub name: crate::tokens::ColId,
        pub options: Option<OpclassOptions>,
    }
}

recursa::ast_node! {
    /// A storage parameter entry: `name [= value]`.
    #[derive(Debug)]
    pub struct StorageParam {
        pub name: StorageParamName,
        pub value: Option<StorageParamValue>,
    }
}

recursa::ast_node! {
    /// Storage parameter name: either a bare word or `namespace.word`.
    ///
    /// Parsing the first word unconditionally keeps the optional suffix's first
    /// token (`.`) disjoint from the first word. In particular, a bare name does
    /// not speculatively commit to the qualified form and then require a dot.
    #[derive(Debug)]
    pub struct StorageParamName {
        pub head: literal::AliasName,
        pub qualified_tail: Option<StorageParamQualifiedTail>,
    }
}

recursa::ast_node! {
    /// `.name` suffix on a qualified storage parameter name.
    #[derive(Debug)]
    pub struct StorageParamQualifiedTail {
        #[tok(DOT, this)]
        pub name: literal::AliasName,
    }
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

recursa::ast_node! {
    /// `= value` suffix for a storage parameter.
    ///
    /// The value is a permissive SetValue (keywords like `off`, `on`, string/numeric
    /// literals, identifiers) rather than a full `Expr` — storage param values are
    /// simple literals and `Expr::ColumnRef` rejects keywords like `off`.
    #[derive(Debug)]
    pub struct StorageParamValue {
        #[tok(EQ, this)]
        pub value: SetValue,
    }
}

recursa::ast_node! {
    /// `WITH (name = value, ...)` storage parameters clause.
    #[derive(Debug)]
    #[tok(WITH, LPAREN, this, RPAREN)]
    pub struct WithStorage {
        #[sep(COMMA)]
        pub params: zero_or_many!(StorageParam),
    }
}

recursa::ast_node! {
    /// `INCLUDE (col, ...)` covering-index clause.
    #[derive(Debug)]
    #[tok(INCLUDE, LPAREN, this, RPAREN)]
    pub struct IncludeClause {
        #[sep(COMMA)]
        pub columns: zero_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum IndexTarget {
        Expr(#[tok(LPAREN, this, RPAREN)] boxed!(Expr)),
        Json(boxed!(JsonFuncExpr)),
        /// gram.y `func_expr_windowless`: no `WITHIN GROUP`, `FILTER` or `OVER`
        /// suffix, which are also operator class names after the call.
        Func(boxed!(FunctionApplicationExpr)),
        Col(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `COLLATE "name"` on an index element.
    #[derive(Debug)]
    pub struct IndexCollate {
        #[tok(COLLATE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// An index element:
    /// `column_or_expr [COLLATE "name"] [opclass [(options)]] [ASC|DESC] [NULLS FIRST|LAST]`.
    #[derive(Debug)]
    pub struct IndexElem {
        pub target: IndexTarget,
        pub collate: Option<IndexCollate>,
        pub opclass: Option<OpclassSpec>,
        pub dir: Option<SortDir>,
        pub nulls: Option<NullsOrder>,
    }
}

recursa::ast_node! {
    /// Parenthesized, comma-separated index-element list.
    ///
    /// The legacy grammar represented this as `Seq0`, so this wrapper retains a
    /// zero-or-more [`Vec`] while applying the delimiters to the whole list rather
    /// than to each element.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct IndexElementList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(IndexElem),
    );
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub struct CreateIndexStmt {
        #[tok(CREATE, this, INDEX)]
        #[presence(UNIQUE)]
        pub unique: bool,
        #[presence(CONCURRENTLY)]
        pub concurrently: bool,
        pub if_not_exists: Option<IfNotExists>,
        pub name: Option<crate::tokens::ColId>,
        #[tok(ON, this)]
        #[presence(ONLY)]
        /// Optional `ONLY` modifier — restricts the index to the named table
        /// without descending into inheritance children (partitioned tables).
        pub only: bool,
        pub table_name: crate::ast::shared::names::QualifiedName,
        pub using: Option<boxed!(UsingMethod)>,
        pub columns: IndexElementList,
        pub include: Option<boxed!(IncludeClause)>,
        pub nulls_distinct: Option<NullsDistinctClause>,
        pub with_storage: Option<boxed!(WithStorage)>,
        pub tablespace: Option<crate::ast::ddl::table::TablespaceClause>,
        pub where_clause: Option<boxed!(WhereClause)>,
    }
}

recursa::ast_node! {
    /// `NULLS [NOT] DISTINCT` modifier on a unique index.
    ///
    /// Variant ordering: `NotDistinct` (`NULLS NOT DISTINCT`, longer) before
    /// `Distinct` (`NULLS DISTINCT`, shorter).
    #[derive(Debug)]
    pub enum NullsDistinctClause {
        #[tok(NULLS, NOT, DISTINCT)]
        NotDistinct,
        #[tok(NULLS, DISTINCT)]
        Distinct,
    }
}

recursa::ast_node! {
    /// DROP INDEX statement:
    ///
    /// ```sql
    /// DROP INDEX [CONCURRENTLY] [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
    /// ```
    #[derive(Debug)]
    #[tok(DROP, INDEX, this)]
    pub struct DropIndexStmt {
        #[presence(CONCURRENTLY)]
        pub concurrently: bool,
        pub if_exists: Option<IfExists>,
        #[sep(COMMA)]
        /// gram.y `any_name_list`: one or more names.
        pub names: one_or_many!(crate::ast::shared::names::QualifiedName),
        pub behavior: Option<DropBehavior>,
    }
}

// =========================================================================
// ALTER/DROP INDEX — appended from simple_stmts.rs during physical extraction.
// =========================================================================

recursa::ast_node! {
    /// `SET (storage_param = value, ...)` action shared by ALTER INDEX /
    /// ALTER VIEW / ALTER MATERIALIZED VIEW / ALTER TABLE — modifies storage
    /// parameters. Differs from `WithStorage` (`WITH (...)` on CREATE) only in
    /// the leading keyword.
    #[derive(Debug)]
    #[tok(SET, LPAREN, this, RPAREN)]
    pub struct SetReloptions {
        #[sep(COMMA)]
        pub params: one_or_many!(crate::ast::ddl::index::StorageParam),
    }
}

recursa::ast_node! {
    /// `RESET (param_name [= value], ...)` action shared by ALTER INDEX /
    /// ALTER VIEW / ALTER MATERIALIZED VIEW / ALTER TABLE — removes storage
    /// parameters. Postgres' gram.y `reloption_elem` allows
    /// `ColLabel [. ColLabel] [= def_arg]`, so the syntax accepts `name = value`
    /// in RESET too (PG ignores the value semantically). Modeled via the same
    /// `StorageParam` type used by `WITH (...)`.
    #[derive(Debug)]
    #[tok(RESET, LPAREN, this, RPAREN)]
    pub struct ResetReloptions {
        #[sep(COMMA)]
        pub params: one_or_many!(crate::ast::ddl::index::StorageParam),
    }
}

recursa::ast_node! {
    /// `ATTACH PARTITION qualified_name` — Postgres' `index_partition_cmd` (the
    /// single ALTER INDEX form that takes a partition operation).
    #[derive(Debug)]
    pub struct AttachPartitionClause {
        #[tok(ATTACH, PARTITION, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// A column reference inside `ALTER INDEX … ALTER COLUMN col_ref …`:
    /// either an integer column position (`SignedIconst`) or a column name
    /// (`Ident`). Postgres' gram.y has two productions; we union them as one
    /// enum so the surrounding action struct can be derived.
    ///
    /// Variant ordering: `Number` first (lex token kind disjoint from
    /// `Ident`), then `Name`.
    #[derive(Debug)]
    pub enum ColumnRef {
        Number(SignedIconst),
        Name(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `SET STATISTICS …` tail of an ALTER COLUMN command.
    #[derive(Debug)]
    pub struct AlterColumnStatisticsAction {
        #[tok(SET, STATISTICS, this)]
        pub value: SetStatisticsValue,
    }
}

recursa::ast_node! {
    /// Action following the shared `ALTER [COLUMN] col_ref` prefix.
    #[derive(Debug)]
    pub enum AlterColumnIndexAction {
        Statistics(AlterColumnStatisticsAction),
        Reloptions(SetReloptions),
    }
}

recursa::ast_node! {
    /// One `ALTER COLUMN …` cmd on ALTER INDEX. The two forms (`SET
    /// STATISTICS` and `SET (params)`) both start with `ALTER … SET`; the
    /// disambiguation token is `STATISTICS` vs `(`.
    ///
    /// The common prefix is represented once so the LR alternatives part at the
    /// keyword or parenthesis immediately following `SET`, rather than duplicating
    /// a potentially multi-token signed column reference.
    #[derive(Debug)]
    pub struct AlterColumnIndexCmd {
        #[tok(ALTER, optional(COLUMN), this)]
        pub col_ref: ColumnRef,
        pub action: AlterColumnIndexAction,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterIndexAction {
        SetTablespace(SetTablespaceClause),
        SetReloptions(SetReloptions),
        ResetReloptions(ResetReloptions),
        Attach(AttachPartitionClause),
        AlterColumn(AlterColumnIndexCmd),
        Depends(DependsOnExtension),
        Rename(RenameTo),
    }
}

recursa::ast_node! {
    /// `ALL IN TABLESPACE name [OWNED BY role_list] SET TABLESPACE new
    /// [NOWAIT]` — Postgres' bulk-relocate action on ALTER INDEX (and ALTER
    /// MATERIALIZED VIEW). Moves every index in the named tablespace to a new
    /// tablespace, optionally filtered by owner role(s).
    #[derive(Debug)]
    pub struct AllInTablespaceBody {
        #[tok(ALL, IN, TABLESPACE, this)]
        pub source: crate::tokens::ColId,
        pub owned_by: Option<OwnedByRoles>,
        pub set_tablespace: SetTablespaceClause,
        #[presence(NOWAIT)]
        pub nowait: bool,
    }
}

recursa::ast_node! {
    /// `OWNED BY role_list` — owner filter on the bulk `ALL IN TABLESPACE`
    /// action.
    #[derive(Debug)]
    pub struct OwnedByRoles {
        #[tok(OWNED, BY, this)]
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// `ALTER INDEX [IF EXISTS] name action` plus the bulk `ALTER INDEX ALL IN
    /// TABLESPACE …` form. The two top-level shapes share the leading `ALTER
    /// INDEX` keywords, so they sit on either side of a single enum and share that
    /// LR prefix once.
    ///
    /// Variant ordering: `All` (starts with `ALL`) before `Single`
    /// (starts with `[IF EXISTS] qualified_name` — never `ALL`).
    #[derive(Debug)]
    pub enum AlterIndexBody {
        All(AllInTablespaceBody),
        Single(AlterIndexSingle),
    }
}

recursa::ast_node! {
    /// `[IF EXISTS] name action` — the per-index branch of ALTER INDEX.
    #[derive(Debug)]
    pub struct AlterIndexSingle {
        pub if_exists: Option<IfExists>,
        pub name: QualifiedName,
        pub action: AlterIndexAction,
    }
}

recursa::ast_node! {
    /// `ALTER INDEX [IF EXISTS] name action`
    /// `ALTER INDEX ALL IN TABLESPACE name [OWNED BY role_list] SET TABLESPACE
    ///   new [NOWAIT]` — the two top-level shapes of Postgres' `AlterTableStmt`
    /// branches that begin with `ALTER INDEX …`, plus the index branches of
    /// `RenameStmt` / `AlterObjectDependsStmt` (single form).
    #[derive(Debug)]
    pub struct AlterIndexStmt {
        #[tok(ALTER, INDEX, this)]
        pub body: AlterIndexBody,
    }
}
