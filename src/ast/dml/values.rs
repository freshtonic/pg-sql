/// VALUES statement, TABLE statement, and set operation support (UNION, EXCEPT, INTERSECT).
use crate::ast::dml::select::{SelectStmt, ValuesBody};

recursa::ast_node! {
    /// TABLE statement: gram.y `simple_select: TABLE relation_expr`. Its ORDER BY
    /// and LIMIT tails belong to the enclosing [`QueryBody`], as they do for
    /// every `simple_select`.
    #[derive(Debug)]
    pub struct TableStmt {
        #[tok(TABLE, this)]
        pub relation: crate::ast::shared::names::RelationExpr,
    }
}

recursa::ast_node! {
    /// gram.y `set_quantifier: ALL | DISTINCT | /*EMPTY*/`, the present forms.
    /// `DISTINCT` is the default and changes nothing; `ALL` keeps duplicates.
    #[derive(Debug)]
    pub enum SetQuantifier {
        #[tok(ALL)]
        All,
        #[tok(DISTINCT)]
        Distinct,
    }
}

recursa::ast_node! {
    /// `UNION [ALL | DISTINCT]`. The keyword is on the node and not on the
    /// optional field, so the operator is never empty.
    #[derive(Debug)]
    #[tok(UNION, this)]
    pub struct UnionOp {
        pub quantifier: Option<SetQuantifier>,
    }
}

recursa::ast_node! {
    /// `EXCEPT [ALL | DISTINCT]`.
    #[derive(Debug)]
    #[tok(EXCEPT, this)]
    pub struct ExceptOp {
        pub quantifier: Option<SetQuantifier>,
    }
}

recursa::ast_node! {
    /// `INTERSECT [ALL | DISTINCT]`.
    #[derive(Debug)]
    #[tok(INTERSECT, this)]
    pub struct IntersectOp {
        pub quantifier: Option<SetQuantifier>,
    }
}

recursa::ast_node! {
    /// gram.y `SelectStmt`: `select_no_parens` with its `with_clause` forms, the
    /// query every subquery position accepts (`(WITH ... SELECT ... ORDER BY 1)`).
    /// The statement level keeps its `with_clause` factored in
    /// `WithStatement`, because there the CTE list is also the prefix of
    /// `INSERT`, `UPDATE`, `DELETE` and `MERGE`.
    #[derive(Debug)]
    pub struct Subquery {
        pub with: Option<crate::ast::shared::with_clause::WithClause>,
        pub body: QueryBody,
    }
}

recursa::ast_node! {
    /// gram.y `select_no_parens` without `with_clause`: a `select_clause`
    /// followed by `opt_sort_clause`, `opt_select_limit` and
    /// `opt_for_locking_clause`. The tails apply to the whole set-operation
    /// chain and belong to no `simple_select` member, so
    /// `SELECT 1 UNION SELECT 2 ORDER BY 1` sorts the union.
    #[derive(Debug)]
    pub struct QueryBody {
        pub clause: SelectClause,
        #[pretty(break_before = soft)]
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
        /// The LIMIT / OFFSET / FETCH FIRST tail and the locking clause, in either
        /// of gram.y's two orders.
        #[pretty(break_before = soft)]
        pub limit_locking: Option<boxed!(crate::ast::dml::select::LimitLockingClause)>,
    }
}

impl<'input> QueryBody<'input> {
    /// The `select_limit` tail, wherever it stands relative to the locking
    /// clause.
    pub fn limit_offset(&self) -> Option<&crate::ast::dml::select::LimitOffsetClause<'input>> {
        self.limit_locking.as_deref()?.limit_offset()
    }

    /// The locking items, in source order. Empty for no locking clause and
    /// for `FOR READ ONLY`.
    pub fn locking_items(&self) -> &[crate::ast::dml::select::ForUpdateClause<'input>] {
        self.limit_locking
            .as_deref()
            .and_then(|tail| tail.locking())
            .map_or(&[], |locking| locking.items())
    }
}

recursa::ast_node! {
    /// gram.y `select_with_parens` (gram.y:12682):
    ///
    /// ```text
    /// select_with_parens: '(' select_no_parens ')' | '(' select_with_parens ')'
    /// ```
    ///
    /// The second production needs no separate variant here: [`Subquery`] is
    /// `select_no_parens`, and its `select_clause` admits a further
    /// `SelectWithParens`, so `((SELECT 1))` reduces through this one rule.
    ///
    /// This is the single nonterminal that [`SelectClause`] and [`SimpleSelect`]
    /// share. gram.y factors it for the same reason (gram.y:12658): spelling the
    /// parenthesized query once is what stops the two from carrying duplicate
    /// copies of every production below them.
    #[derive(Debug)]
    pub struct SelectWithParens {
        pub open: crate::ast::shared::expr::ParenthesizedOpen,
        pub inner: boxed!(Subquery),
        pub close: crate::ast::shared::expr::ParenthesizedClose,
    }
}

recursa::ast_node! {
    /// gram.y `select_clause: simple_select | select_with_parens`
    /// (gram.y:12757), with `simple_select`'s productions (gram.y:12790) as
    /// variants, the way [`crate::ast::shared::expr::Expr`] holds `c_expr`'s.
    ///
    /// A set operation is an ordinary left-recursive rule, as it is in gram.y:
    /// `select_clause UNION set_quantifier select_clause` (gram.y:12844-12854).
    /// The declared precedence of `crate::tokens` decides a chain, level for
    /// level with gram.y:830-831 `%left UNION EXCEPT` and `%left INTERSECT`:
    /// `INTERSECT` binds tighter, and every operator associates to the left.
    /// `a INTERSECT b UNION c` is `(a INTERSECT b) UNION c`, `a UNION b
    /// INTERSECT c` is `a UNION (b INTERSECT c)`, and `a UNION b EXCEPT c` is
    /// `(a UNION b) EXCEPT c`. The tree is PostgreSQL's `SetOperationStmt`
    /// tree, so a consumer never regroups a chain. Only parentheses override
    /// it, and they are explicit as [`SelectClause::Parens`].
    ///
    /// Each operator is a Node, so its rule ends in no terminal of its own and
    /// carries the level of its keyword through `#[parse(prec = ...)]`.
    ///
    /// The right operand is a `select_clause`, never a sorted or limited
    /// query: `opt_sort_clause` and the other tails belong to [`QueryBody`].
    #[derive(Debug)]
    #[flat(pool)]
    pub enum SelectClause {
        /// gram.y:12844 `select_clause UNION set_quantifier select_clause`.
        #[parse(prec = UNION)]
        Union(
            boxed!(Self),
            #[pretty(break_before = soft)] UnionOp,
            boxed!(Self),
        ),
        /// gram.y:12852 `select_clause EXCEPT set_quantifier select_clause`.
        #[parse(prec = EXCEPT)]
        Except(
            boxed!(Self),
            #[pretty(break_before = soft)] ExceptOp,
            boxed!(Self),
        ),
        /// gram.y:12848 `select_clause INTERSECT set_quantifier select_clause`.
        #[parse(prec = INTERSECT)]
        Intersect(
            boxed!(Self),
            #[pretty(break_before = soft)] IntersectOp,
            boxed!(Self),
        ),
        /// gram.y:12791 `SELECT opt_all_clause opt_target_list ...` and its
        /// `distinct_clause` twin.
        Select(boxed!(SelectStmt)),
        /// gram.y `simple_select: values_clause`.
        Values(ValuesBody),
        /// gram.y `simple_select: TABLE relation_expr`.
        Table(TableStmt),
        /// gram.y `select_clause: select_with_parens`.
        Parens(SelectWithParens),
    }
}
