/// INSERT INTO statement AST.
///
/// Supports: `INSERT INTO table [(cols)] source [ON CONFLICT ...] [RETURNING ...]`
/// where source is DEFAULT VALUES, VALUES rows, or SELECT query.
use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::{ReturningClause, SetAssignment};
use crate::ast::dml::values::Subquery;
use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;

recursa::ast_node! {
    /// `[AS] alias` on INSERT target table, e.g. `INSERT INTO t AS x`.
    #[derive(Debug)]
    pub struct InsertTableAlias {
        #[tok(AS, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `OVERRIDING {SYSTEM | USER} VALUE` clause on an INSERT statement.
    ///
    /// Variant ordering: distinct first tokens (`SYSTEM` vs `USER`), so
    /// declaration order is cosmetic.
    #[derive(Debug)]
    pub struct OverridingClause {
        #[tok(OVERRIDING, this, VALUE)]
        pub which: OverridingKind,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum OverridingKind {
        #[tok(SYSTEM)]
        System,
        #[tok(USER)]
        User,
    }
}

recursa::ast_node! {
    /// Multiple value rows: `VALUES (row1), (row2), ...`
    #[derive(Debug)]
    #[tok(VALUES, this)]
    pub struct InsertValueRows {
        #[sep(COMMA)]
        pub rows: zero_or_many!(ValueList),
    }
}

recursa::ast_node! {
    /// Insert source: `DEFAULT VALUES` or a query.
    ///
    /// PostgreSQL's query form includes `VALUES`, so the `Subquery` AST retains
    /// the distinction between SELECT, VALUES, TABLE, and parenthesized sources
    /// without declaring the VALUES language twice at this enum boundary.
    /// gram.y `insert_rest`: `SelectStmt`, `OVERRIDING ... SelectStmt`,
    /// `'(' insert_column_list ')' [OVERRIDING ...] SelectStmt` or
    /// `DEFAULT VALUES`, as two alternatives that both begin by shifting the
    /// parenthesis marker when a `(` follows the table name.
    ///
    /// Variant ordering: `Columns` starts with `(`; `Plain` with `OVERRIDING`,
    /// `DEFAULT` or a query (which may also start with `(`, decided by the
    /// token after it).
    #[derive(Debug)]
    pub enum InsertRest {
        Columns(InsertColumnsRest),
        Plain(InsertPlainRest),
    }
}

recursa::ast_node! {
    /// `'(' insert_column_list ')' [OVERRIDING ...] source`.
    #[derive(Debug)]
    pub struct InsertColumnsRest {
        pub columns: boxed!(ColumnList),
        pub overriding: Option<OverridingClause>,
        #[pretty(break_before = soft)]
        pub source: boxed!(InsertSource),
    }
}

recursa::ast_node! {
    /// `[OVERRIDING ...] source`.
    #[derive(Debug)]
    pub struct InsertPlainRest {
        pub overriding: Option<OverridingClause>,
        pub source: boxed!(InsertSource),
    }
}

impl<'input> InsertRest<'input> {
    /// The explicit column list, when written.
    pub fn columns(&self) -> Option<&ColumnList<'input>> {
        match self {
            InsertRest::Columns(rest) => Some(&rest.columns),
            InsertRest::Plain(_) => None,
        }
    }

    /// The `OVERRIDING` clause, when written.
    pub fn overriding(&self) -> Option<&OverridingClause> {
        match self {
            InsertRest::Columns(rest) => rest.overriding.as_ref(),
            InsertRest::Plain(rest) => rest.overriding.as_ref(),
        }
    }

    /// The row source.
    pub fn source(&self) -> &InsertSource<'input> {
        match self {
            InsertRest::Columns(rest) => &rest.source,
            InsertRest::Plain(rest) => &rest.source,
        }
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum InsertSource {
        #[tok(DEFAULT, VALUES)]
        Default,
        Select(boxed!(Subquery)),
    }
}

recursa::ast_node! {
    /// DO UPDATE SET ... [WHERE ...] action.
    #[derive(Debug)]
    #[tok(DO, UPDATE, SET, this)]
    pub struct DoUpdateAction {
        #[sep(COMMA)]
        pub assignments: one_or_many!(SetAssignment),
        pub where_clause: Option<WhereClause>,
    }
}

recursa::ast_node! {
    /// ON CONFLICT action: DO UPDATE SET ... [WHERE ...] or DO NOTHING.
    ///
    /// Variant ordering: DoUpdate (`DO UPDATE SET`) is longer than
    /// DoNothing (`DO NOTHING`), but both start with `DO` and diverge
    /// at the next keyword, so the regex disambiguates.
    #[derive(Debug)]
    pub enum ConflictAction {
        DoUpdate(boxed!(DoUpdateAction)),
        #[tok(DO, NOTHING)]
        DoNothing,
        /// Added in 19: `ON CONFLICT opt_conf_expr DO SELECT
        /// opt_for_locking_strength where_clause` (gram.y b73d13c:12580;
        /// research PostgreSQL 19, "Changes to existing statements").
        #[cfg(feature = "since-pg19")]
        DoSelect(DoSelectAction),
    }
}

#[cfg(feature = "since-pg19")]
recursa::ast_node! {
    /// `FOR {UPDATE | NO KEY UPDATE | SHARE | KEY SHARE}` — gram.y
    /// `for_locking_strength`, with no `OF` list and no wait clause, as
    /// `opt_for_locking_strength` (b73d13c:13826) takes it.
    #[derive(Debug)]
    pub struct ForLockingStrength {
        #[tok(FOR, this)]
        pub mode: crate::ast::dml::select::LockingMode,
    }
}

#[cfg(feature = "since-pg19")]
recursa::ast_node! {
    /// `DO SELECT [FOR lock_strength] [WHERE ...]`, added in 19.
    #[derive(Debug)]
    #[tok(DO, SELECT, this)]
    pub struct DoSelectAction {
        pub strength: Option<ForLockingStrength>,
        pub where_clause: Option<WhereClause>,
    }
}

recursa::ast_node! {
    /// One entry in an `ON CONFLICT (...)` target list.
    ///
    /// Matches the index-element grammar: an expression (plain column name,
    /// qualified name, parenthesized expression, or function call) optionally
    /// followed by a `COLLATE "name"` clause and an optional opclass ident.
    #[derive(Debug)]
    pub struct ConflictTargetItem {
        pub target: crate::ast::ddl::index::IndexTarget,
        pub collate: Option<crate::ast::ddl::table::CollateClause>,
        pub opclass: Option<crate::tokens::ColId>,
    }
}

recursa::ast_node! {
    /// `ON CONSTRAINT name` arbiter form of `opt_conf_expr` — names a unique
    /// or exclusion constraint directly instead of inferring from column list.
    /// Per gram.y `opt_conf_expr: ON CONSTRAINT name`.
    #[derive(Debug)]
    pub struct OnConflictConstraint {
        #[tok(ON, CONSTRAINT, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Parenthesized `index_params` arbiter list on `ON CONFLICT`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ConflictTargetList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(ConflictTargetItem),
    );
}

recursa::ast_node! {
    /// Arbiter specification for `ON CONFLICT` — Postgres' `opt_conf_expr`.
    ///
    /// Variant ordering: each variant has a distinct first token (`(` vs `ON`),
    /// so order is for clarity.
    #[derive(Debug)]
    pub enum ConflictTarget {
        /// `( index_params )` — the inferring-by-columns form.
        Index(ConflictTargetList),
        /// `ON CONSTRAINT name` — the named-constraint form.
        Constraint(OnConflictConstraint),
    }
}

recursa::ast_node! {
    /// ON CONFLICT clause: `ON CONFLICT [(col, ...) | ON CONSTRAINT name]
    /// DO UPDATE SET ... | DO NOTHING`
    #[derive(Debug)]
    #[tok(ON, CONFLICT, this)]
    pub struct OnConflictClause {
        pub target: Option<ConflictTarget>,
        /// `WHERE predicate` after the arbiter target list, restricting the
        /// partial-index arbiter to matching rows. Only valid for the
        /// index-params form (gram.y attaches `where_clause` to that branch
        /// only); attached at the outer struct so the enum stays simple.
        pub where_clause: Option<WhereClause>,
        pub action: ConflictAction,
    }
}

recursa::ast_node! {
    /// INSERT INTO statement with optional ON CONFLICT and RETURNING.
    #[derive(Debug)]
    #[pretty(group = consistent)]
    pub struct InsertStmt {
        #[tok(INSERT, INTO, this)]
        pub table_name: QualifiedName,
        /// Optional `[AS] alias` after the target table, used to rebind the
        /// target in ON CONFLICT DO UPDATE expressions.
        pub alias: Option<InsertTableAlias>,
        /// gram.y `insert_rest`.
        #[pretty(break_before = soft)]
        pub rest: InsertRest,
        #[pretty(break_before = soft)]
        pub on_conflict: Option<boxed!(OnConflictClause)>,
        #[pretty(break_before = soft)]
        pub returning: Option<boxed!(ReturningClause)>,
    }
}

recursa::ast_node! {
    /// One target column of an INSERT column list: a column name plus an
    /// optional indirection chain — `f2[1]`, `f3.if1`, `a[1:5]` (Postgres
    /// `insert_column_item: ColId opt_indirection`).
    #[derive(Debug)]
    pub struct InsertColumnItem {
        pub name: crate::tokens::ColId,
        pub indirection: zero_or_many!(crate::ast::shared::expr::IndirectionEl),
    }
}

recursa::ast_node! {
    /// Column list: `(col1, col2[1], col3.field, ...)`.
    #[derive(Debug, derive_more :: Deref)]
    pub struct ColumnList {
        /// The shared parenthesis markers: after `INSERT INTO t` a `(` opens
        /// either this list or a parenthesized query, and both reduce the same
        /// marker before the next token decides (gram.y `insert_rest`).
        pub open: crate::ast::shared::expr::ParenthesizedOpen,
        /// gram.y `insert_column_list`: one or more items.
        #[sep(COMMA)]
        #[deref]
        pub items: one_or_many!(InsertColumnItem),
        pub close: crate::ast::shared::expr::ParenthesizedClose,
    }
}

recursa::ast_node! {
    /// Value list: `(col1, col2, ...)`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ValueList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(Expr),
    );
}
