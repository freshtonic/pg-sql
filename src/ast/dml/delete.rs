/// DELETE FROM statement AST.
use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::ReturningClause;
use crate::ast::shared::names::QualifiedName;

/// Table alias with explicit AS keyword: `AS alias`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DeleteAsAlias<'input> {
    #[tok(AS, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// Table alias in DELETE FROM: either `AS alias` or bare `alias`.
///
/// gram.y's `relation_expr_opt_alias` spells both forms with `ColId`, not
/// with `BareColLabel`: a reserved keyword such as `USING` or `NULL` can
/// never open the alias, which is what keeps `USING ...` a using-clause.
///
/// Variant ordering: WithAs (`AS ident`) has a longer first_pattern than
/// Bare (`ident`), so longest-match-wins picks it when AS is present.
#[derive(recursa::Node, Debug, Clone)]
pub enum DeleteTableAlias<'input> {
    WithAs(DeleteAsAlias<'input>),
    Bare(crate::tokens::ColId<'input>),
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

/// `USING table, ...` clause in DELETE statements.
///
/// `USING` leads the whole from-list, so it is declared on the struct;
/// gram.y's `using_clause: USING from_list` makes the list non-empty.
#[derive(recursa::Node, Debug, Clone)]
#[tok(USING, this)]
pub struct DeleteUsingClause<'input> {
    #[sep(COMMA)]
    pub tables: recursa::Vec1<crate::ast::dml::select::TableRef<'input>>,
}

/// DELETE FROM statement: `DELETE FROM [ONLY] table [alias] [USING ...] [WHERE expr] [RETURNING ...]`.
///
/// The optional `ONLY` modifier excludes inheritance children — Postgres'
/// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
/// not exercised by any DELETE corpus statement, so it is not modelled (matches
/// the `TruncateRelation` / `LockRelation` shape).
#[derive(recursa::Node, Debug, Clone)]
#[pretty(group = consistent)]
pub struct DeleteStmt<'input> {
    #[tok(DELETE, FROM, this)]
    #[presence(ONLY)]
    pub only: bool,
    pub table_name: QualifiedName<'input>,
    /// Greedy: a leading ABSENT starts this element instead of ending `DeleteStmt` (bison shift preference).
    #[greedy(ABSENT)]
    pub alias: Option<Box<DeleteTableAlias<'input>>>,
    #[pretty(break_before = soft)]
    pub using_clause: Option<Box<DeleteUsingClause<'input>>>,
    #[pretty(break_before = soft)]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    /// Greedy: a leading RETURNING starts this element instead of ending `DeleteStmt` (bison shift preference).
    #[greedy(RETURNING)]
    #[pretty(break_before = soft)]
    pub returning: Option<Box<ReturningClause<'input>>>,
}
