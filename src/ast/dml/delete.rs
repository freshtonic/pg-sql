/// DELETE FROM statement AST.
use recursa_diagram::railroad;

use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::ReturningClause;
use crate::ast::shared::names::QualifiedName;

use crate::tokens::keyword::*;
/// Table alias with explicit AS keyword: `AS alias`.
#[derive(recursa::Node)]
pub struct DeleteAsAlias<'input> {
    #[tok(AS, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// Table alias in DELETE FROM: either `AS alias` or bare `alias`.
///
/// Variant ordering: WithAs (`AS ident`) has a longer first_pattern than
/// Bare (`ident`), so longest-match-wins picks it when AS is present.
#[derive(recursa::Node)]
pub enum DeleteTableAlias<'input> {
    WithAs(DeleteAsAlias<'input>),
    Bare(#[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(BareColLabel))] crate::tokens::BareColLabel<'input>),
}

impl<'input> DeleteTableAlias<'input> {
    /// Returns the alias name regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            DeleteTableAlias::WithAs(a) => a.name.text(),
            DeleteTableAlias::Bare(ident) => ident.text(),
        }
    }
}

/// `USING table, ...` clause in DELETE statements.
#[derive(recursa::Node)]
pub struct DeleteUsingClause<'input> {
    #[tok(USING, this)]
    #[sep(COMMA)]
    pub tables:
        recursa::seq::Vec<crate::ast::dml::select::TableRef<'input> >,
}

/// DELETE FROM statement: `DELETE FROM [ONLY] table [alias] [USING ...] [WHERE expr] [RETURNING ...]`.
///
/// The optional `ONLY` modifier excludes inheritance children — Postgres'
/// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
/// not exercised by any DELETE corpus statement, so it is not modelled (matches
/// the `TruncateRelation` / `LockRelation` shape).
#[derive(recursa::Node)]
#[format_tokens(group(consistent))]
pub struct DeleteStmt<'input> {
    #[tok(DELETE, FROM, this)]
    #[presence(ONLY)]
    pub only: bool,
    pub table_name: QualifiedName<'input>,
    pub alias: Option<Box<DeleteTableAlias<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub using_clause: Option<Box<DeleteUsingClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub returning: Option<Box<ReturningClause<'input>>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::dml::delete::{DeleteStmt, DeleteTableAlias};

    #[test]
    fn parse_delete_qualified_table() {
        let lexed = crate::tokens::lex("DELETE FROM pg_catalog.pg_class");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "pg_class");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_simple() {
        let lexed = crate::tokens::lex("DELETE FROM delete_test WHERE a > 25");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_with_as_alias() {
        let lexed = crate::tokens::lex("DELETE FROM delete_test AS dt WHERE dt.a > 75");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(matches!(
            stmt.alias.as_deref(),
            Some(DeleteTableAlias::WithAs(_))
        ));
        assert_eq!(stmt.alias.as_ref().unwrap().name(), "dt");
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_with_bare_alias() {
        let lexed = crate::tokens::lex("DELETE FROM delete_test dt WHERE delete_test.a > 25");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(matches!(
            stmt.alias.as_deref(),
            Some(DeleteTableAlias::Bare(_))
        ));
        assert_eq!(stmt.alias.as_ref().unwrap().name(), "dt");
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_delete_no_where() {
        let lexed = crate::tokens::lex("DELETE FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "t");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_none());
        assert!(input.is_eof());
    }

    /// `DELETE FROM ONLY tab` excludes inheritance children — `relation_expr`
    /// in `gram.y`. The `ONLY` qualifier appears immediately before the
    /// target table name.
    #[test]
    fn parse_delete_from_only() {
        let lexed = crate::tokens::lex("DELETE FROM ONLY c WHERE aa = 'new'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DeleteStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.only.is_some(), "ONLY qualifier should be parsed");
        assert_eq!(stmt.table_name.object(), "c");
        assert!(input.is_eof());
    }
}
