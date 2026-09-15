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
use crate::ast::dml::select::{PlainTable, TableRef};
use crate::ast::dml::update::{ReturningClause, SetAssignment};
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
    /// `BY SOURCE` or `BY TARGET`.
    #[derive(Debug)]
    pub enum NotMatchedBy {
        #[tok(BY, SOURCE)]
        Source,
        #[tok(BY, TARGET)]
        Target,
    }
}

recursa::ast_node! {
    /// `UPDATE SET col = expr, ...` action body (the part after THEN).
    #[derive(Debug)]
    #[tok(UPDATE, SET, this)]
    pub struct UpdateAction {
        #[sep(COMMA)]
        pub assignments: one_or_many!(SetAssignment),
    }
}

recursa::ast_node! {
    /// Action allowed after `WHEN MATCHED ... THEN`.
    ///
    /// Variant ordering: `DoNothing` (`DO NOTHING`) and `Update` (`UPDATE`) and
    /// `Delete` (`DELETE`) all start with distinct keywords, so order is by
    /// declaration only.
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
    /// A single row of values: `(expr, ...)`.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ValueRow(#[sep(COMMA)] pub zero_or_many!(Expr));
}

recursa::ast_node! {
    /// `VALUES (row), (row), ...` body.
    #[derive(Debug)]
    #[tok(VALUES, this)]
    pub struct InsertValuesBody {
        #[sep(COMMA)]
        pub rows: zero_or_many!(ValueRow),
    }
}

recursa::ast_node! {
    /// Body of an INSERT inside MERGE: `VALUES ...` or `DEFAULT VALUES`.
    ///
    /// Variant ordering: `Default` (`DEFAULT VALUES`) is matched before
    /// `Values` (`VALUES`) since they begin with different keywords.
    #[derive(Debug)]
    pub enum InsertBody {
        #[tok(DEFAULT, VALUES)]
        Default,
        Values(InsertValuesBody),
    }
}

recursa::ast_node! {
    /// Optional `INTO target_name` after `INSERT`.
    #[derive(Debug)]
    pub struct InsertInto {
        #[tok(INTO, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Parenthesized `insert_column_list` on a MERGE `INSERT` action.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct MergeInsertColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(literal::AliasName),
    );
}

recursa::ast_node! {
    /// `INSERT [INTO target] [(cols)] { VALUES ... | DEFAULT VALUES }`
    #[derive(Debug)]
    #[tok(INSERT, this)]
    pub struct InsertAction {
        pub into: Option<InsertInto>,
        pub columns: Option<MergeInsertColumnList>,
        /// `OVERRIDING {SYSTEM|USER} VALUE` between the columns and the body.
        pub overriding: Option<crate::ast::dml::insert::OverridingClause>,
        pub body: InsertBody,
    }
}

recursa::ast_node! {
    /// Action allowed after `WHEN NOT MATCHED ... THEN`.
    ///
    /// `WHEN NOT MATCHED [BY TARGET]` takes an `INSERT` (or `DO NOTHING`);
    /// `WHEN NOT MATCHED BY SOURCE` takes an `UPDATE` / `DELETE` instead
    /// (the target row exists, the source row does not). Which `by` form
    /// permits which action is a semantic rule, so all four are accepted
    /// grammatically.
    #[derive(Debug)]
    pub enum NotMatchedAction {
        Insert(InsertAction),
        Update(UpdateAction),
        #[tok(DELETE)]
        Delete,
        #[tok(DO, NOTHING)]
        DoNothing,
    }
}

recursa::ast_node! {
    /// `WHEN NOT MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN action`.
    #[derive(Debug)]
    #[tok(WHEN, NOT, MATCHED, this)]
    pub struct WhenNotMatched {
        pub by: Option<NotMatchedBy>,
        pub and: Option<AndCondition>,
        #[tok(THEN, this)]
        pub action: NotMatchedAction,
    }
}

recursa::ast_node! {
    /// `WHEN MATCHED [AND cond] THEN action`.
    #[derive(Debug)]
    #[tok(WHEN, MATCHED, this)]
    pub struct WhenMatched {
        pub and: Option<AndCondition>,
        #[tok(THEN, this)]
        pub action: MatchedAction,
    }
}

recursa::ast_node! {
    /// A WHEN clause in MERGE.
    ///
    /// Variant ordering: `NotMatched` (`WHEN NOT MATCHED`) is longer than
    /// `Matched` (`WHEN MATCHED`); list it first.
    #[derive(Debug)]
    pub enum WhenClause {
        NotMatched(WhenNotMatched),
        Matched(WhenMatched),
    }
}

recursa::ast_node! {
    /// MERGE statement.
    #[derive(Debug)]
    pub struct MergeStmt {
        #[tok(MERGE, INTO, this)]
        pub target: boxed!(PlainTable),
        #[tok(USING, this)]
        pub source: boxed!(TableRef),
        #[tok(ON, this)]
        pub condition: boxed!(Expr),
        /// PostgreSQL's `merge_when_list` is one-or-more.
        pub when_clauses: one_or_many!(WhenClause),
        pub returning: Option<boxed!(ReturningClause)>,
    }
}
