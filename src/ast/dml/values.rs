/// VALUES statement, TABLE statement, and set operation support (UNION, EXCEPT, INTERSECT).
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::dml::select::SelectBody;
use crate::tokens::keyword::*;
use crate::tokens::punct;
use recursa_diagram::railroad;

/// TABLE statement: `TABLE tablename [ORDER BY ...] [LIMIT ...] [OFFSET ...]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dql"])]
pub struct TableStmt<'input> {
    pub table: TABLE,
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset_1: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
    pub limit_offset_2: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
}

/// Set operation type.
///
/// Variant ordering: longer keyword sequences first within each group
/// so longest-match-wins picks UNION ALL over bare UNION, etc.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetOp {
    UnionAll((UNION, ALL)),
    UnionDistinct((UNION, DISTINCT)),
    ExceptAll((EXCEPT, ALL)),
    IntersectAll((INTERSECT, ALL)),
    Union(UNION),
    Except(EXCEPT),
    Intersect(INTERSECT),
}

/// A set operation combiner: `UNION [ALL|DISTINCT] | EXCEPT [ALL] | INTERSECT [ALL]`
/// followed by the right-hand query.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetOpCombiner<'input> {
    pub op: SetOp,
    pub right: Box<Subquery<'input>>,
}

/// A compound query: a query body optionally followed by a set operation.
/// This allows chaining: `VALUES ... UNION ALL SELECT ... EXCEPT TABLE ...`
/// Paren variant handles `(WITH ... SELECT ... UNION ...)` grouping.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dql"])]
pub enum Subquery<'input> {
    Paren(CompoundParen<'input>),
    Table(TableStmt<'input>),
    Body(CompoundBody<'input>),
}

/// Parenthesized compound query with optional set operation continuation.
/// e.g., `(SELECT ... UNION ALL ...) EXCEPT ...`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CompoundParen<'input> {
    pub inner: Surrounded<punct::LParen, Box<Subquery<'input>>, punct::RParen>,
    pub set_op: Option<SetOpCombiner<'input>>,
    /// Optional trailing `ORDER BY ...` applied to the parenthesized query.
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset_1: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
    pub limit_offset_2: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
}

/// A SELECT or VALUES body with optional set operation continuation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
        let mut input = crate::tokens::test_input("TABLE int8_tbl");
        let stmt = TableStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "int8_tbl");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_values_standalone() {
        let mut input = crate::tokens::test_input("VALUES (1,2), (3,4), (7,8)");
        let body = CompoundBody::parse(&mut input).unwrap();
        assert!(body.set_op.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_values_union_all_select() {
        let mut input = crate::tokens::test_input("VALUES (1,2) UNION ALL SELECT 3, 4");
        let body = CompoundBody::parse(&mut input).unwrap();
        assert!(body.set_op.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_values_union_all_table() {
        let mut input = crate::tokens::test_input("VALUES (1,2) UNION ALL TABLE t");
        let body = CompoundBody::parse(&mut input).unwrap();
        assert!(body.set_op.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_table_stmt_order_by() {
        let mut input = crate::tokens::test_input(
            "TABLE information_schema.enabled_roles ORDER BY role_name COLLATE \"C\"",
        );
        let stmt = TableStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
        assert!(input.is_empty());
    }
}
