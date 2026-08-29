/// UPDATE statement AST.
///
/// `UPDATE table SET col = expr [, ...] [FROM ...] [WHERE ...] [RETURNING ...]`
use recursa::seq::Seq0;
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
#[derive(recursa::Node, Debug, Clone)]
pub struct SingleAssignment<'input> {
    pub column: literal::Ident<'input>,
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
    #[tok(EQ, this)]
    pub value: Expr<'input>,
}

/// One entry in a multi-column SET target list — Postgres
/// `set_target: ColId opt_indirection`. The indirection chain admits the
/// same `[idx]`, `[lo:hi]`, and `.field` elements as `SingleAssignment`,
/// so `SET (f2[1], f1, tag) = (...)` (rules.sql) parses cleanly.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetTarget<'input> {
    pub column: literal::Ident<'input>,
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
}

/// Tuple SET assignment: `(col, ...) = expr` — Postgres
/// `'(' set_target_list ')' '=' a_expr`. Each item in the list is a
/// `set_target` (`ColId opt_indirection`), so subscripts and field
/// accessors are admitted on individual columns.
#[derive(recursa::Node, Debug, Clone)]
pub struct TupleAssignment<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: recursa::surrounded::

        Vec<SetTarget<'input> >

    ,
    #[tok(EQ, this)]
    pub values: Expr<'input>,
}

/// A single SET assignment: `col = expr` or `(col, ...) = (expr, ...)`
///
/// Variant ordering: Tuple starts with `(` which is longer than a bare
/// identifier, so longest-match-wins picks it when parens are present.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetAssignment<'input> {
    Tuple(TupleAssignment<'input>),
    Single(SingleAssignment<'input>),
}

/// RETURNING clause: `RETURNING expr, ...`
#[derive(recursa::Node, Debug, Clone)]
pub struct ReturningClause<'input> {
    #[tok(RETURNING, this)]
    #[sep(COMMA)]
    pub items: Vec<crate::ast::dml::select::SelectItem<'input> >,
}

/// `[AS] alias` on an UPDATE target table.
#[derive(recursa::Node, Debug, Clone)]
pub struct UpdateTableAlias<'input> {
    #[tok(optional(AS), this)]
    pub name: literal::Ident<'input>,
}

/// UPDATE statement: `UPDATE [ONLY] table [alias] SET assignments [FROM ...] [WHERE ...] [RETURNING ...]`
///
/// The optional `ONLY` modifier excludes inheritance children — Postgres'
/// `relation_expr` in `gram.y`. The legacy `ONLY (name)` parenthesised form is
/// not exercised by any UPDATE corpus statement, so it is not modelled (matches
/// the `TruncateRelation` / `LockRelation` shape).
#[derive(recursa::Node, Debug, Clone)]
#[format_tokens(group(consistent))]
pub struct UpdateStmt<'input> {
    #[tok(UPDATE, this)]
    #[presence(ONLY)]
    pub only: bool,
    pub table_name: QualifiedName<'input>,
    pub alias: Option<UpdateTableAlias<'input>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    #[tok(SET, this)]
    #[sep(COMMA)]
    #[format_tokens(indent)]
    pub assignments: Vec<SetAssignment<'input> >,
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
        let lexed = crate::tokens::lex("UPDATE pg_catalog.pg_class SET relname = '123'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "pg_class");
        assert!(input.is_eof());
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
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            UpdateStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_update_simple() {
        let lexed = crate::tokens::lex("UPDATE y SET a = a + 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.table_name.object(), "y");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_with_returning() {
        let lexed = crate::tokens::lex("UPDATE y SET a = a + 1 RETURNING *");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.returning.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_with_from_where() {
        let lexed = crate::tokens::lex("UPDATE y SET a = y.a - 10 FROM t WHERE y.a > 20 AND t.a = y.a RETURNING y.a");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.from_clause.is_some());
        assert!(stmt.where_clause.is_some());
        assert!(stmt.returning.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_subscript_assignment() {
        let lexed = crate::tokens::lex("UPDATE t SET e[0] = '1.1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_subscript_assignment_one() {
        let lexed = crate::tokens::lex("UPDATE t SET e[1] = '2.2'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_plain_assignment_still_parses() {
        let lexed = crate::tokens::lex("UPDATE t SET col = 'x'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_update_tuple_set() {
        let lexed = crate::tokens::lex("UPDATE parent SET (k, v) = (SELECT k, v FROM simpletup WHERE simpletup.k = parent.k)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// A multi-column SET target list may carry an indirection chain on
    /// individual items — Postgres `set_target: ColId opt_indirection`,
    /// admitted by `'(' set_target_list ')' '=' a_expr`. The rules.sql
    /// regression has `SET (f2[1], f1, tag) = (...)`.
    #[test]
    fn parse_update_tuple_set_with_indirection() {
        let lexed = crate::tokens::lex("UPDATE trgt SET (f2[1], f1, tag) = (SELECT 1, 2, 'updated'::varchar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// `UPDATE ONLY tab SET ...` excludes inheritance children — Postgres'
    /// `relation_expr` in `gram.y`. The `ONLY` qualifier appears immediately
    /// before the target table name (after the `UPDATE` keyword).
    #[test]
    fn parse_update_only() {
        let lexed = crate::tokens::lex("UPDATE ONLY a SET aa = 'zzzzz' WHERE aa = 'aaaaa'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = UpdateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.only.is_some(), "ONLY qualifier should be parsed");
        assert_eq!(stmt.table_name.object(), "a");
        assert!(input.is_eof());
    }
}
