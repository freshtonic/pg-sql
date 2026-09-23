/// MERGE statement AST.
///
/// ```sql
/// MERGE INTO [ONLY] target [[AS] alias]
/// USING source [[AS] alias] ON condition
/// WHEN [NOT] MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN
///     { UPDATE SET ... | DELETE | DO NOTHING
///     | INSERT [INTO target] [(cols)] { VALUES (...) [, (...)] | DEFAULT VALUES } }
/// [RETURNING ...]
/// ```
use crate::ast::dml::select::TableRef;
#[cfg(feature = "since-pg17")]
use crate::ast::dml::update::ReturningClause;
use crate::ast::dml::update::SetAssignment;
use crate::ast::shared::expr::Expr;
use crate::tokens::literal;

recursa::ast_node! {
    /// `AND cond` qualifier on a WHEN clause.
    #[derive(Debug)]
    pub struct AndCondition {
        #[tok(AND, this)]
        pub condition: Expr,
    }
}

recursa::ast_node! {
    /// `UPDATE SET set_clause_list`: gram.y:12518 `merge_update`.
    #[derive(Debug)]
    #[tok(UPDATE, SET, this)]
    pub struct UpdateAction {
        #[sep(COMMA)]
        pub assignments: one_or_many!(SetAssignment),
    }
}

recursa::ast_node! {
    /// The action of a `WHEN MATCHED` or `WHEN NOT MATCHED BY SOURCE` clause:
    /// gram.y:12459 gives `merge_when_tgt_matched` the actions `merge_update`,
    /// `merge_delete` and `DO NOTHING`, and never an insert.
    #[derive(Debug)]
    pub enum MatchedAction {
        Update(UpdateAction),
        #[tok(DELETE)]
        Delete,
        #[tok(DO, NOTHING)]
        DoNothing,
    }
}

recursa::ast_node! {
    /// gram.y:12592 `merge_values_clause: VALUES '(' expr_list ')'`: exactly
    /// one row, with at least one value. `INSERT VALUES (1, 1), (2, 2)` is a
    /// syntax error in a `MERGE`, where the same text is an `INSERT`
    /// statement's `values_clause`.
    #[derive(Debug)]
    #[tok(VALUES, LPAREN, this, RPAREN)]
    pub struct MergeValuesClause {
        #[sep(COMMA)]
        pub values: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// gram.y `insert_column_list` in a `merge_insert`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct MergeInsertColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(literal::AliasName),
    );
}

recursa::ast_node! {
    /// gram.y:12544 `merge_insert`: `INSERT [(columns)] [OVERRIDING {SYSTEM |
    /// USER} VALUE] merge_values_clause | INSERT DEFAULT VALUES`. There is no
    /// `INTO`: the target is the statement's.
    ///
    /// Variant ordering: `Default` leads with `DEFAULT`; `Values` with `(`,
    /// `OVERRIDING` or `VALUES`.
    #[derive(Debug)]
    pub enum InsertAction {
        /// `INSERT DEFAULT VALUES`, which takes no column list and no
        /// `OVERRIDING`.
        #[tok(INSERT, DEFAULT, VALUES)]
        Default,
        Values(#[tok(INSERT, this)] MergeInsertValues),
    }
}

recursa::ast_node! {
    /// `[(columns)] [OVERRIDING {SYSTEM | USER} VALUE] merge_values_clause`.
    #[derive(Debug)]
    pub struct MergeInsertValues {
        pub columns: Option<MergeInsertColumnList>,
        pub overriding: Option<crate::ast::dml::insert::OverridingClause>,
        pub values: MergeValuesClause,
    }
}

recursa::ast_node! {
    /// The action of a `WHEN NOT MATCHED [BY TARGET]` clause: gram.y:12459
    /// gives `merge_when_tgt_not_matched` the actions `merge_insert` and `DO
    /// NOTHING`, and never an update or a delete.
    #[derive(Debug)]
    pub enum NotMatchedAction {
        Insert(InsertAction),
        #[tok(DO, NOTHING)]
        DoNothing,
    }
}

recursa::ast_node! {
    /// gram.y:12503 `merge_when_tgt_matched: WHEN MATCHED | WHEN NOT MATCHED
    /// BY SOURCE`: the clauses whose row exists in the target, with the
    /// actions that need one.
    ///
    /// Variant ordering: the two lead with `WHEN MATCHED` and `WHEN NOT`.
    #[derive(Debug)]
    pub enum MatchedKind {
        #[tok(WHEN, MATCHED)]
        Matched,
        // Added in 17: research, PostgreSQL 17, "Changes to existing statements"
        // (REL_17_11 gram.y 12503 `merge_when_tgt_matched`).
        #[cfg(feature = "since-pg17")]
        #[tok(WHEN, NOT, MATCHED, BY, SOURCE)]
        NotMatchedBySource,
    }
}

recursa::ast_node! {
    /// gram.y:12508 `merge_when_tgt_not_matched: WHEN NOT MATCHED | WHEN NOT
    /// MATCHED BY TARGET`: the clauses whose row is absent from the target.
    /// `BY TARGET` is the spelled-out default.
    #[derive(Debug)]
    #[cfg_attr(feature = "since-pg17", tok(WHEN, NOT, MATCHED, this))]
    #[cfg_attr(not(feature = "since-pg17"), tok(WHEN, NOT, MATCHED))]
    pub struct NotMatchedKind {
        // Added in 17: research, PostgreSQL 17, "Changes to existing
        // statements" (REL_17_11 gram.y 12508 `merge_when_tgt_not_matched`).
        // REL_16_15 gram.y 12289 has only `WHEN NOT MATCHED`.
        #[cfg(feature = "since-pg17")]
        #[presence(BY, TARGET)]
        pub by_target: bool,
    }
}

recursa::ast_node! {
    /// `merge_when_tgt_matched opt_merge_when_condition THEN {merge_update |
    /// merge_delete | DO NOTHING}`.
    #[derive(Debug)]
    pub struct WhenMatched {
        pub kind: MatchedKind,
        pub and: Option<AndCondition>,
        #[tok(THEN, this)]
        pub action: MatchedAction,
    }
}

recursa::ast_node! {
    /// `merge_when_tgt_not_matched opt_merge_when_condition THEN {merge_insert
    /// | DO NOTHING}`.
    #[derive(Debug)]
    pub struct WhenNotMatched {
        pub kind: NotMatchedKind,
        pub and: Option<AndCondition>,
        #[tok(THEN, this)]
        pub action: NotMatchedAction,
    }
}

recursa::ast_node! {
    /// gram.y:12459 `merge_when_clause`. gram.y splits the clauses by whether
    /// the target row exists, because that decides the actions: `WHEN MATCHED`
    /// and `WHEN NOT MATCHED BY SOURCE` update, delete or do nothing; `WHEN NOT
    /// MATCHED [BY TARGET]` inserts or does nothing. A clause with the wrong
    /// action is a syntax error, which merge.sql tests.
    ///
    /// Variant ordering: both lead with `WHEN`; the parser decides at `BY
    /// SOURCE` or at the token after `MATCHED`.
    #[derive(Debug)]
    pub enum WhenClause {
        NotMatched(WhenNotMatched),
        Matched(WhenMatched),
    }
}

recursa::ast_node! {
    /// `AS alias` on a MERGE target — gram.y `relation_expr_opt_alias:
    /// relation_expr AS ColId`.
    #[derive(Debug)]
    pub struct MergeAsAlias {
        #[tok(AS, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Alias of a MERGE target — gram.y `relation_expr_opt_alias`, which has
    /// no column list.
    ///
    /// `SET` is a `ColId`, yet the bare form never takes it: gram.y gives the
    /// first `relation_expr_opt_alias` rule the `UMINUS` precedence, which is
    /// above `SET`'s, so the parser reduces the empty alias instead of
    /// shifting the keyword (REL_17_11 gram.y 13801-13810). MERGE, DELETE and
    /// UPDATE share that state, so `MERGE INTO t set` is a syntax error.
    ///
    /// Variant ordering: `WithAs` (`AS ident`) before `Bare` (`ident`).
    #[derive(Debug)]
    pub enum MergeTableAlias {
        WithAs(MergeAsAlias),
        Bare(literal::RelationAliasName),
    }
}

recursa::ast_node! {
    /// gram.y `relation_expr_opt_alias`, the target of `MERGE INTO`.
    #[derive(Debug)]
    pub struct MergeTarget {
        pub relation: crate::ast::shared::names::RelationExpr,
        pub alias: Option<MergeTableAlias>,
    }
}

recursa::ast_node! {
    /// MERGE statement.
    #[derive(Debug)]
    pub struct MergeStmt {
        #[tok(MERGE, INTO, this)]
        pub target: boxed!(MergeTarget),
        #[tok(USING, this)]
        pub source: boxed!(TableRef),
        #[tok(ON, this)]
        pub condition: boxed!(Expr),
        /// PostgreSQL's `merge_when_list` is one-or-more.
        pub when_clauses: one_or_many!(WhenClause),
        // Added in 17: research, PostgreSQL 17, "Changes to existing statements"
        // (REL_17_11 gram.y 12433 `MergeStmt ... returning_clause`).
        #[cfg(feature = "since-pg17")]
        pub returning: Option<boxed!(ReturningClause)>,
    }
}
