//! VACUUM statement and the shared `VacuumOption(s)` AST nodes used by
//! VACUUM/REINDEX/CLUSTER for their `( option [= value], ... )` lists.

#[cfg(not(feature = "since-pg18"))]
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
    #[derive(Debug)]
    pub struct VacuumRelation {
        // A widening, not an addition (ADR 0010, `docs/minimum-version.md`):
        // the newer type accepts everything the older one does, so both arms
        // only remove and neither records. The gate for what 18 adds belongs
        // to those shapes.
        #[cfg(not(feature = "since-pg18"))]
        pub name: QualifiedName,
        /// From 18 gram.y `vacuum_relation` names a `relation_expr`, so
        /// `ONLY name` and `name *` are accepted
        /// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
        /// item 9; REL_18_6 gram.y `vacuum_relation`).
        #[cfg(feature = "since-pg18")]
        pub name: crate::ast::shared::names::RelationExpr,
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
    /// The name of the relation. From 18 the relation is a `relation_expr`,
    /// and this is the name inside it.
    pub fn relation_name(&self) -> &crate::ast::shared::names::QualifiedName<'input> {
        #[cfg(not(feature = "since-pg18"))]
        return &self.name;
        #[cfg(feature = "since-pg18")]
        return self.name.name();
    }
}
