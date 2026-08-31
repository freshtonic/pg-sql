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
/// Variant ordering: WithAs (`AS ident`) has a longer first_pattern than
/// Bare (`ident`), so longest-match-wins picks it when AS is present.
#[derive(recursa::Node, Debug, Clone)]
pub enum DeleteTableAlias<'input> {
    WithAs(DeleteAsAlias<'input>),
    Bare(crate::tokens::BareColLabel<'input>),
}

impl<'input> DeleteTableAlias<'input> {
    /// Returns the alias name regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            DeleteTableAlias::WithAs(a) => match &a.name {
                crate::tokens::ColId::Text(text) => text.text(),
            },
            DeleteTableAlias::Bare(ident) => match ident {
                crate::tokens::BareColLabel::Text(text) => text.text(),
            },
        }
    }
}

/// `USING table, ...` clause in DELETE statements.
#[derive(recursa::Node, Debug, Clone)]
pub struct DeleteUsingClause<'input> {
    #[tok(USING, this)]
    #[sep(COMMA)]
    pub tables: Vec<crate::ast::dml::select::TableRef<'input>>,
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
    pub alias: Option<Box<DeleteTableAlias<'input>>>,
    #[pretty(break_before = soft)]
    pub using_clause: Option<Box<DeleteUsingClause<'input>>>,
    #[pretty(break_before = soft)]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    #[pretty(break_before = soft)]
    pub returning: Option<Box<ReturningClause<'input>>>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/dml/delete.tests.rs"
));
