/// UPDATE statement AST.
///
/// `UPDATE table SET col = expr [, ...] [FROM ...] [WHERE ...] [RETURNING ...]`
use crate::ast::dml::select::{FromClause, WhereClause};
use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

recursa::ast_node! {
    /// Single SET assignment: `col = expr`, `col[idx] = expr`,
    /// `col[lo:hi] = expr`, `alias.col = expr`, or any chain thereof.
    ///
    /// The target is a column name plus an optional indirection chain
    /// (Postgres `set_target: ColId opt_indirection`). The `.field` form of
    /// indirection also covers the `alias.col` left-hand side that
    /// `ON CONFLICT DO UPDATE` permits inside `INSERT ... AS alias`.
    #[derive(Debug)]
    pub struct SingleAssignment {
        pub column: literal::Ident,
        pub indirection: zero_or_many!(crate::ast::shared::expr::IndirectionEl),
        #[tok(EQ, this)]
        pub value: Expr,
    }
}

recursa::ast_node! {
    /// One entry in a multi-column SET target list — Postgres
    /// `set_target: ColId opt_indirection`. The indirection chain admits the
    /// same `[idx]`, `[lo:hi]`, and `.field` elements as `SingleAssignment`,
    /// so `SET (f2[1], f1, tag) = (...)` (rules.sql) parses cleanly.
    #[derive(Debug)]
    pub struct SetTarget {
        pub column: literal::Ident,
        pub indirection: zero_or_many!(crate::ast::shared::expr::IndirectionEl),
    }
}

recursa::ast_node! {
    /// Tuple SET assignment: `(col, ...) = expr` — Postgres
    /// `'(' set_target_list ')' '=' a_expr`. Each item in the list is a
    /// `set_target` (`ColId opt_indirection`), so subscripts and field
    /// accessors are admitted on individual columns.
    #[derive(Debug)]
    pub struct TupleAssignment {
        pub columns: SetTargetList,
        #[tok(EQ, this)]
        pub values: Expr,
    }
}

recursa::ast_node! {
    /// Parenthesized, comma-separated targets on the left side of a tuple SET.
    ///
    /// The delimiters wrap the list as a whole. Attaching them to the repeated
    /// field would require a fresh pair of parentheses around every target.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct SetTargetList(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(SetTarget),
    );
}

recursa::ast_node! {
    /// A single SET assignment: `col = expr` or `(col, ...) = (expr, ...)`
    ///
    /// Variant ordering: Tuple starts with `(` which is longer than a bare
    /// identifier, so longest-match-wins picks it when parens are present.
    #[derive(Debug)]
    pub enum SetAssignment {
        Tuple(TupleAssignment),
        Single(SingleAssignment),
    }
}

recursa::ast_node! {
    /// RETURNING clause: `RETURNING expr, ...`
    #[derive(Debug)]
    #[tok(RETURNING, this)]
    pub struct ReturningClause {
        #[sep(COMMA)]
        pub items: zero_or_many!(crate::ast::dml::select::SelectItem),
    }
}

recursa::ast_node! {
    /// `AS alias` on an UPDATE target table.
    #[derive(Debug)]
    pub struct UpdateTableAliasWithAs {
        #[tok(AS, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Required `SET` plus its comma-separated assignments.
    ///
    /// Keeping `SET` on this wrapper makes it occur once for the whole list.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(SET, this)]
    pub struct SetClause(
        #[sep(COMMA)]
        #[pretty(indent)]
        #[deref]
        pub zero_or_many!(SetAssignment),
    );
}

recursa::ast_node! {
    /// Optional target-table alias before the required SET clause.
    ///
    /// PostgreSQL admits `SET` as a `ColId`, but gives it precedence as the UPDATE
    /// clause keyword in this position. The bare-alias admission excludes exactly
    /// that keyword; the explicit `AS` form continues to accept it.
    #[derive(Debug)]
    pub enum UpdateTableAlias {
        WithAs(UpdateTableAliasWithAs),
        Bare(literal::UpdateAliasName),
    }
}

impl UpdateTableAlias<'_> {
    /// Raw alias text regardless of whether `AS` was present.
    pub fn name(&self) -> &str {
        match self {
            UpdateTableAlias::WithAs(alias) => alias.name.text(),
            UpdateTableAlias::Bare(name) => name.text(),
        }
    }
}

recursa::ast_node! {
    /// UPDATE statement: `UPDATE [ONLY] table [alias] SET assignments [FROM ...] [WHERE ...] [RETURNING ...]`
    ///
    /// The optional `ONLY` modifier excludes inheritance children — Postgres'
    /// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
    /// not exercised by any UPDATE corpus statement, so it is not modelled (matches
    /// the `TruncateRelation` / `LockRelation` shape).
    #[derive(Debug)]
    #[pretty(group = consistent)]
    pub struct UpdateStmt {
        #[tok(UPDATE, this)]
        #[presence(ONLY)]
        pub only: bool,
        pub table_name: QualifiedName,
        pub alias: Option<UpdateTableAlias>,
        #[pretty(break_before = soft)]
        pub assignments: SetClause,
        #[pretty(break_before = soft)]
        pub from_clause: Option<boxed!(FromClause)>,
        #[pretty(break_before = soft)]
        pub where_clause: Option<boxed!(WhereClause)>,
        #[pretty(break_before = soft)]
        pub returning: Option<boxed!(ReturningClause)>,
    }
}
