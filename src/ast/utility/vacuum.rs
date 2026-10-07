//! VACUUM statement and the shared `VacuumOption(s)` AST nodes used by
//! VACUUM/REINDEX/CLUSTER for their `( option [= value], ... )` lists.

use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

recursa::ast_node! {
    /// A single option inside a VACUUM/REINDEX `( ... )` list: `name [value]`.
    ///
    /// The option name may be any SQL word (including keywords like `FULL`,
    /// `FREEZE`, `PARALLEL`) so it uses `AliasName`. The value matches gram.y's
    /// `utility_option_arg`: `opt_boolean_or_string | NumericOnly | EMPTY` —
    /// i.e. `ON`, `OFF`, `TRUE`, `FALSE`, `DEFAULT`, a numeric (signed or not),
    /// a string literal, or an identifier. We model that with `SetValue`, which
    /// is the same vocabulary `SET` accepts.
    #[derive(Debug)]
    pub struct VacuumOption {
        pub name: literal::AliasName,
        pub value: Option<crate::ast::session::set_reset::SetValue>,
    }
}

recursa::ast_node! {
    /// Parenthesized options list: `( opt [= val] [, ...] )`.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct VacuumOptions {
        #[sep(COMMA)]
        pub list: zero_or_many!(VacuumOption),
    }
}

// -----------------------------------------------------------------------
// VACUUM statement itself.
// -----------------------------------------------------------------------

// --- VACUUM ---

recursa::ast_node! {
    /// A single VACUUM/ANALYZE relation target — Postgres' `vacuum_relation`.
    ///
    /// `qualified_name [(column [, ...])]`. The optional column list applies to
    /// `VACUUM ANALYZE` to scope the analyze to specific columns.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct VacuumColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
    );
}

recursa::ast_node! {
    /// The relation of one gram.y `vacuum_relation`: a `qualified_name` before
    /// 18 (REL_17_11 gram.y 11913), and a `relation_expr` from 18 (REL_18_6
    /// gram.y 12021; docs/research/postgres-14-19-sql-syntax-changes.md,
    /// PostgreSQL 18, item 9).
    ///
    /// The change is a widening, so the requirement belongs to the two shapes
    /// 18 adds and not to the relation (ADR 0010,
    /// `docs/minimum-version.md`). `VACUUM t` therefore records nothing, and
    /// `VACUUM ONLY t` and `VACUUM t *` each record 18. The shared
    /// [`RelationExpr`](crate::ast::shared::names::RelationExpr) cannot carry
    /// those gates, because `ONLY t` and `t *` are legal in 14 everywhere else
    /// it appears.
    ///
    /// Variant ordering: `Only` leads with the keyword, `Named` with a name.
    #[derive(Debug)]
    pub enum VacuumRelationName {
        /// `ONLY name` or `ONLY ( name )`, added in 18 with the widening.
        #[config(since = pg18)]
        Only(crate::ast::shared::names::OnlyRelation),
        /// `name` in every version, and `name *` from 18.
        Named(VacuumInheritedRelation),
    }
}

recursa::ast_node! {
    /// `name [*]` inside a gram.y `vacuum_relation`: the relation with its
    /// inheritance children, which is the default.
    #[derive(Debug)]
    pub struct VacuumInheritedRelation {
        pub name: QualifiedName,
        /// Added in 18 with the widening of gram.y `vacuum_relation` to a
        /// `relation_expr` (REL_18_6 gram.y 12021, `extended_relation_expr:
        /// qualified_name '*'`). REL_17_11 gram.y 11913 takes only a
        /// `qualified_name`.
        #[config(since = pg18)]
        #[presence(STAR)]
        #[pretty(break_before = soft)]
        pub star: bool,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct VacuumRelation {
        pub name: VacuumRelationName,
        pub columns: Option<VacuumColumnList>,
    }
}

recursa::ast_node! {
    /// `VACUUM` statement supporting both forms in Postgres' `gram.y`:
    ///
    /// ```sql
    /// VACUUM [FULL] [FREEZE] [VERBOSE] [ANALYZE] [vacuum_relation [, ...]]
    /// VACUUM '(' option [, ...] ')' [vacuum_relation [, ...]]
    /// ```
    ///
    /// In the parenthesised form, `options` is `Some` and the four legacy
    /// keyword fields are all `None`. In the legacy form, `options` is `None`
    /// and any combination of `FULL` / `FREEZE` / `VERBOSE` / `ANALYZE` is
    /// permitted in that fixed declaration order. Both forms share the optional
    /// trailing relation list.
    #[derive(Debug)]
    #[tok(VACUUM, this)]
    pub struct VacuumStmt {
        pub options: Option<VacuumOptions>,
        #[presence(FULL)]
        pub full: bool,
        #[presence(FREEZE)]
        pub freeze: bool,
        #[presence(VERBOSE)]
        pub verbose: bool,
        #[presence(ANALYZE)]
        pub analyze: bool,
        #[sep(COMMA)]
        pub relations: Option<one_or_many!(VacuumRelation)>,
    }
}

impl<'input> VacuumRelation<'input> {
    /// The name of the relation, whichever form wraps it.
    pub fn relation_name(&self) -> &QualifiedName<'input> {
        self.name.name()
    }
}

impl<'input> VacuumRelationName<'input> {
    /// The relation's name, whichever form wraps it.
    pub fn name(&self) -> &QualifiedName<'input> {
        match self {
            // The `ONLY` form exists only from 18, where gram.y
            // `vacuum_relation` names a `relation_expr`.
            #[cfg(feature = "since-pg18")]
            Self::Only(only) => only.name(),
            Self::Named(relation) => &relation.name,
        }
    }

    /// Whether `ONLY` excludes the inheritance children.
    pub fn is_only(&self) -> bool {
        match self {
            #[cfg(feature = "since-pg18")]
            Self::Only(_) => true,
            Self::Named(_) => false,
        }
    }
}
