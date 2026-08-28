/// UPDATE statement AST.
///
/// `UPDATE table SET col = expr [, ...] [FROM ...] [WHERE ...] [RETURNING ...]`
use recursa::seq::Seq0;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::dml::select::{FromClause, WhereClause};
use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::{literal, punct};

use crate::tokens::keyword::*;

/// Single SET assignment: `col = expr`, `col[idx] = expr`,
/// `col[lo:hi] = expr`, `alias.col = expr`, or any chain thereof.
///
/// The target is a column name plus an optional indirection chain
/// (Postgres `set_target: ColId opt_indirection`). The `.field` form of
/// indirection also covers the `alias.col` left-hand side that
/// `ON CONFLICT DO UPDATE` permits inside `INSERT ... AS alias`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SingleAssignment<'input> {
    pub column: literal::Ident<'input>,
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
    pub eq: punct::Eq,
    pub value: Expr<'input>,
}

/// One entry in a multi-column SET target list — Postgres
/// `set_target: ColId opt_indirection`. The indirection chain admits the
/// same `[idx]`, `[lo:hi]`, and `.field` elements as `SingleAssignment`,
/// so `SET (f2[1], f1, tag) = (...)` (rules.sql) parses cleanly.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetTarget<'input> {
    pub column: literal::Ident<'input>,
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
}

/// Tuple SET assignment: `(col, ...) = expr` — Postgres
/// `'(' set_target_list ')' '=' a_expr`. Each item in the list is a
/// `set_target` (`ColId opt_indirection`), so subscripts and field
/// accessors are admitted on individual columns.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TupleAssignment<'input> {
    pub columns: recursa::surrounded::Surrounded<
        punct::LParen,
        Seq0<SetTarget<'input>, punct::Comma>,
        punct::RParen,
    >,
    pub eq: punct::Eq,
    pub values: Expr<'input>,
}

/// A single SET assignment: `col = expr` or `(col, ...) = (expr, ...)`
///
/// Variant ordering: Tuple starts with `(` which is longer than a bare
/// identifier, so longest-match-wins picks it when parens are present.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetAssignment<'input> {
    Tuple(TupleAssignment<'input>),
    Single(SingleAssignment<'input>),
}

/// RETURNING clause: `RETURNING expr, ...`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReturningClause<'input> {
    pub returning: RETURNING,
    pub items: Seq0<crate::ast::dml::select::SelectItem<'input>, punct::Comma>,
}

/// `[AS] alias` on an UPDATE target table.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UpdateTableAlias<'input> {
    pub r#as: Option<AS>,
    pub name: literal::Ident<'input>,
}

/// UPDATE statement: `UPDATE [ONLY] table [alias] SET assignments [FROM ...] [WHERE ...] [RETURNING ...]`
///
/// The optional `ONLY` modifier excludes inheritance children — Postgres'
/// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
/// not exercised by any UPDATE corpus statement, so it is not modelled (matches
/// the `TruncateRelation` / `LockRelation` shape).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dml"])]
#[format_tokens(group(consistent))]
pub struct UpdateStmt<'input> {
    pub update: UPDATE,
    pub only: Option<ONLY>,
    pub table_name: QualifiedName<'input>,
    pub alias: Option<UpdateTableAlias<'input>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub set: SET,
    #[format_tokens(indent)]
    pub assignments: Seq0<SetAssignment<'input>, punct::Comma>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub from_clause: Option<Box<FromClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub returning: Option<Box<ReturningClause<'input>>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_update_qualified_table() {
        let mut input = crate::tokens::test_input("UPDATE pg_catalog.pg_class SET relname = '123'");
        let stmt = UpdateStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "pg_class");
        assert!(input.is_empty());
    }

    /// A SET target may carry an indirection chain — `[idx]`, `[lo:hi]`,
    /// `.field` (Postgres `set_target: ColId opt_indirection`).
    #[test]
    fn parse_update_set_target_indirection() {
        for src in [
            "UPDATE t SET a[1:2] = '{16,25}'",
            "UPDATE t SET b[1:1][1:1][1:2] = '{1}', c[2:2] = '{x}'",
            "UPDATE t SET alias.col = 1",
        ] {
            let mut input = crate::tokens::test_input(src);
            UpdateStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_update_simple() {
        let mut input = crate::tokens::test_input("UPDATE y SET a = a + 1");
        let stmt = UpdateStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "y");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_update_with_returning() {
        let mut input = crate::tokens::test_input("UPDATE y SET a = a + 1 RETURNING *");
        let stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(stmt.returning.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_update_with_from_where() {
        let mut input = crate::tokens::test_input(
            "UPDATE y SET a = y.a - 10 FROM t WHERE y.a > 20 AND t.a = y.a RETURNING y.a",
        );
        let stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
        assert!(stmt.where_clause.is_some());
        assert!(stmt.returning.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_update_subscript_assignment() {
        let mut input = crate::tokens::test_input("UPDATE t SET e[0] = '1.1'");
        let _stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_update_subscript_assignment_one() {
        let mut input = crate::tokens::test_input("UPDATE t SET e[1] = '2.2'");
        let _stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_update_plain_assignment_still_parses() {
        let mut input = crate::tokens::test_input("UPDATE t SET col = 'x'");
        let _stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_update_tuple_set() {
        let mut input = crate::tokens::test_input(
            "UPDATE parent SET (k, v) = (SELECT k, v FROM simpletup WHERE simpletup.k = parent.k)",
        );
        let _stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// A multi-column SET target list may carry an indirection chain on
    /// individual items — Postgres `set_target: ColId opt_indirection`,
    /// admitted by `'(' set_target_list ')' '=' a_expr`. The rules.sql
    /// regression has `SET (f2[1], f1, tag) = (...)`.
    #[test]
    fn parse_update_tuple_set_with_indirection() {
        let mut input = crate::tokens::test_input(
            "UPDATE trgt SET (f2[1], f1, tag) = (SELECT 1, 2, 'updated'::varchar)",
        );
        let _stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// `UPDATE ONLY tab SET ...` excludes inheritance children — Postgres'
    /// `relation_expr` in `gram.y`. The `ONLY` qualifier appears immediately
    /// before the target table name (after the `UPDATE` keyword).
    #[test]
    fn parse_update_only() {
        let mut input =
            crate::tokens::test_input("UPDATE ONLY a SET aa = 'zzzzz' WHERE aa = 'aaaaa'");
        let stmt = UpdateStmt::parse(&mut input).unwrap();
        assert!(stmt.only.is_some(), "ONLY qualifier should be parsed");
        assert_eq!(stmt.table_name.object(), "a");
        assert!(input.is_empty());
    }
}
