/// DELETE FROM statement AST.
use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::ReturningClause;
use crate::ast::shared::names::QualifiedName;

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
    /// Variant ordering: WithAs (`AS ident`) has a longer first_pattern than
    /// Bare (`ident`), so longest-match-wins picks it when AS is present.
    #[derive(Debug)]
    pub enum DeleteTableAlias {
        WithAs(DeleteAsAlias),
        Bare(crate::tokens::ColId),
    }
}

impl<'input> DeleteTableAlias<'input> {
    /// Returns the alias name regardless of variant.
    pub fn name(&self) -> &str {
        let (DeleteTableAlias::WithAs(DeleteAsAlias {
            name: crate::tokens::ColId::Text(text),
        })
        | DeleteTableAlias::Bare(crate::tokens::ColId::Text(text))) = self;
        text.text()
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
        #[tok(DELETE, FROM, this)]
        #[presence(ONLY)]
        pub only: bool,
        pub table_name: QualifiedName,
        pub alias: Option<boxed!(DeleteTableAlias)>,
        #[pretty(break_before = soft)]
        pub using_clause: Option<boxed!(DeleteUsingClause)>,
        #[pretty(break_before = soft)]
        pub where_clause: Option<boxed!(WhereClause)>,
        #[pretty(break_before = soft)]
        pub returning: Option<boxed!(ReturningClause)>,
    }
}
