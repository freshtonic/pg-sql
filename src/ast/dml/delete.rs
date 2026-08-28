/// DELETE FROM statement AST.
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::ReturningClause;
use crate::ast::shared::names::QualifiedName;

use crate::tokens::keyword::*;
/// Table alias with explicit AS keyword: `AS alias`.
#[recursa::ast]
pub struct DeleteAsAlias<'input> {
    pub r#as: AS,
    pub name: crate::tokens::ColId<'input>,
}

/// Table alias in DELETE FROM: either `AS alias` or bare `alias`.
///
/// Variant ordering: WithAs (`AS ident`) has a longer first_pattern than
/// Bare (`ident`), so longest-match-wins picks it when AS is present.
#[recursa::ast]
pub enum DeleteTableAlias<'input> {
    WithAs(DeleteAsAlias<'input>),
    Bare(crate::tokens::BareColLabel<'input>),
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
#[recursa::ast]
pub struct DeleteUsingClause<'input> {
    pub using: USING,
    pub tables:
        recursa::seq::Seq0<crate::ast::dml::select::TableRef<'input>, crate::tokens::punct::Comma>,
}

/// DELETE FROM statement: `DELETE FROM [ONLY] table [alias] [USING ...] [WHERE expr] [RETURNING ...]`.
///
/// The optional `ONLY` modifier excludes inheritance children — Postgres'
/// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
/// not exercised by any DELETE corpus statement, so it is not modelled (matches
/// the `TruncateRelation` / `LockRelation` shape).
#[recursa::ast(meta_tags = ["dml"])]
#[format_tokens(group(consistent))]
pub struct DeleteStmt<'input> {
    pub delete: DELETE,
    pub from: FROM,
    pub only: Option<ONLY>,
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
        let mut input = crate::tokens::test_input("DELETE FROM pg_catalog.pg_class");
        let stmt = DeleteStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "pg_class");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_delete_simple() {
        let mut input = crate::tokens::test_input("DELETE FROM delete_test WHERE a > 25");
        let stmt = DeleteStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_delete_with_as_alias() {
        let mut input = crate::tokens::test_input("DELETE FROM delete_test AS dt WHERE dt.a > 75");
        let stmt = DeleteStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(matches!(
            stmt.alias.as_deref(),
            Some(DeleteTableAlias::WithAs(_))
        ));
        assert_eq!(stmt.alias.as_ref().unwrap().name(), "dt");
        assert!(stmt.where_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_delete_with_bare_alias() {
        let mut input =
            crate::tokens::test_input("DELETE FROM delete_test dt WHERE delete_test.a > 25");
        let stmt = DeleteStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "delete_test");
        assert!(matches!(
            stmt.alias.as_deref(),
            Some(DeleteTableAlias::Bare(_))
        ));
        assert_eq!(stmt.alias.as_ref().unwrap().name(), "dt");
        assert!(stmt.where_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_delete_no_where() {
        let mut input = crate::tokens::test_input("DELETE FROM t");
        let stmt = DeleteStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "t");
        assert!(stmt.alias.is_none());
        assert!(stmt.where_clause.is_none());
        assert!(input.is_empty());
    }

    /// `DELETE FROM ONLY tab` excludes inheritance children — `relation_expr`
    /// in `gram.y`. The `ONLY` qualifier appears immediately before the
    /// target table name.
    #[test]
    fn parse_delete_from_only() {
        let mut input = crate::tokens::test_input("DELETE FROM ONLY c WHERE aa = 'new'");
        let stmt = DeleteStmt::parse(&mut input).unwrap();
        assert!(stmt.only.is_some(), "ONLY qualifier should be parsed");
        assert_eq!(stmt.table_name.object(), "c");
        assert!(input.is_empty());
    }
}
