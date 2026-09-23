/// DELETE FROM statement AST.
use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::ReturningClause;

recursa::ast_node! {
    /// Table alias with explicit AS keyword: `AS alias`.
    #[derive(Debug)]
    pub struct DeleteAsAlias {
        #[tok(AS, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Table alias in DELETE FROM: either `AS alias` or bare `alias`.
    ///
    /// gram.y's `relation_expr_opt_alias` spells both forms with `ColId`, not
    /// with `BareColLabel`: a reserved keyword such as `USING` or `NULL` can
    /// never open the alias, which is what keeps `USING ...` a using-clause.
    ///
    /// `SET` is a `ColId`, yet the bare form never takes it: gram.y gives the
    /// first `relation_expr_opt_alias` rule the `UMINUS` precedence, which is
    /// above `SET`'s, so the parser reduces the empty alias instead of
    /// shifting the keyword ("Given `UPDATE foo set set ...`, we have to
    /// decide without looking any further ahead", REL_17_11 gram.y 13801-13810
    /// and `%nonassoc IDENT … SET …` 886-887). The state is the one the three
    /// statements share, so `DELETE FROM t set` is a syntax error as much as
    /// `UPDATE t set SET a = 1` is. `AS set` keeps working.
    ///
    /// Variant ordering: WithAs (`AS ident`) has a longer first_pattern than
    /// Bare (`ident`), so longest-match-wins picks it when AS is present.
    #[derive(Debug)]
    pub enum DeleteTableAlias {
        WithAs(DeleteAsAlias),
        Bare(crate::tokens::literal::RelationAliasName),
    }
}

impl<'input> DeleteTableAlias<'input> {
    /// Returns the alias name regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            DeleteTableAlias::WithAs(alias) => {
                let crate::tokens::ColId::Text(text) = &alias.name;
                text.text()
            }
            DeleteTableAlias::Bare(name) => name.text(),
        }
    }
}

recursa::ast_node! {
    /// `USING table, ...` clause in DELETE statements.
    ///
    /// `USING` leads the whole from-list, so it is declared on the struct;
    /// gram.y's `using_clause: USING from_list` makes the list non-empty.
    #[derive(Debug)]
    #[tok(USING, this)]
    pub struct DeleteUsingClause {
        #[sep(COMMA)]
        pub tables: one_or_many!(crate::ast::dml::select::TableRef),
    }
}

recursa::ast_node! {
    /// DELETE FROM statement: `DELETE FROM [ONLY] table [alias] [USING ...] [WHERE expr] [RETURNING ...]`.
    ///
    /// The optional `ONLY` modifier excludes inheritance children — Postgres'
    /// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
    /// not exercised by any DELETE corpus statement, so it is not modelled (matches
    /// the `TruncateRelation` / `LockRelation` shape).
    #[derive(Debug)]
    #[pretty(group = consistent)]
    pub struct DeleteStmt {
        /// gram.y `DELETE FROM relation_expr_opt_alias`.
        #[tok(DELETE, FROM, this)]
        pub relation: crate::ast::shared::names::RelationExpr,
        pub alias: Option<boxed!(DeleteTableAlias)>,
        #[pretty(break_before = soft)]
        pub using_clause: Option<boxed!(DeleteUsingClause)>,
        #[pretty(break_before = soft)]
        pub where_clause: Option<boxed!(WhereClause)>,
        #[pretty(break_before = soft)]
        pub returning: Option<boxed!(ReturningClause)>,
    }
}
