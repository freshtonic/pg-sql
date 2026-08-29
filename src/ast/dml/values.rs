/// VALUES statement, TABLE statement, and set operation support (UNION, EXCEPT, INTERSECT).

use crate::ast::dml::select::SelectBody;
use crate::tokens::keyword::*;
use crate::tokens::punct;
use recursa_diagram::railroad;

/// TABLE statement: `TABLE tablename [ORDER BY ...] [LIMIT ...] [OFFSET ...]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableStmt<'input> {
    #[tok(TABLE, this)]
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset_1: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
    pub limit_offset_2: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
}

/// Set operation type.
///
/// Variant ordering: longer keyword sequences first within each group
/// so longest-match-wins picks UNION ALL over bare UNION, etc.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetOp {
    #[tok(UNION, ALL)] UnionAll,
    #[tok(UNION, DISTINCT)] UnionDistinct,
    #[tok(EXCEPT, ALL)] ExceptAll,
    #[tok(INTERSECT, ALL)] IntersectAll,
    #[tok(UNION)] Union,
    #[tok(EXCEPT)] Except,
    #[tok(INTERSECT)] Intersect,
}

/// A set operation combiner: `UNION [ALL|DISTINCT] | EXCEPT [ALL] | INTERSECT [ALL]`
/// followed by the right-hand query.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetOpCombiner<'input> {
    pub op: SetOp,
    pub right: Box<Subquery<'input>>,
}

/// A compound query: a query body optionally followed by a set operation.
/// This allows chaining: `VALUES ... UNION ALL SELECT ... EXCEPT TABLE ...`
/// Paren variant handles `(WITH ... SELECT ... UNION ...)` grouping.
#[derive(recursa::Node, Debug, Clone)]
pub enum Subquery<'input> {
    Paren(CompoundParen<'input>),
    Table(TableStmt<'input>),
    Body(CompoundBody<'input>),
}

/// Parenthesized compound query with optional set operation continuation.
/// e.g., `(SELECT ... UNION ALL ...) EXCEPT ...`
#[derive(recursa::Node, Debug, Clone)]
pub struct CompoundParen<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub inner:  Box<Subquery<'input>> ,
    pub set_op: Option<SetOpCombiner<'input>>,
    /// Optional trailing `ORDER BY ...` applied to the parenthesized query.
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset_1: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
    pub limit_offset_2: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
}

/// A SELECT or VALUES body with optional set operation continuation.
#[derive(recursa::Node, Debug, Clone)]
pub struct CompoundBody<'input> {
    pub body: SelectBody<'input>,
    pub set_op: Option<SetOpCombiner<'input>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::dml::values::{CompoundBody, TableStmt};

    #[test]
    fn parse_table_stmt() {
        let lexed = crate::tokens::lex("TABLE int8_tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = TableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "int8_tbl");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_standalone() {
        let lexed = crate::tokens::lex("VALUES (1,2), (3,4), (7,8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body = CompoundBody::parse(&mut input).unwrap().into_ast();
        assert!(body.set_op.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_union_all_select() {
        let lexed = crate::tokens::lex("VALUES (1,2) UNION ALL SELECT 3, 4");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body = CompoundBody::parse(&mut input).unwrap().into_ast();
        assert!(body.set_op.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_values_union_all_table() {
        let lexed = crate::tokens::lex("VALUES (1,2) UNION ALL TABLE t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let body = CompoundBody::parse(&mut input).unwrap().into_ast();
        assert!(body.set_op.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_stmt_order_by() {
        let lexed = crate::tokens::lex("TABLE information_schema.enabled_roles ORDER BY role_name COLLATE \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = TableStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.order_by.is_some());
        assert!(input.is_eof());
    }
}
