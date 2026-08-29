//! PREPARE / EXECUTE / DEALLOCATE statements (including the two-phase-commit
//! `PREPARE TRANSACTION 'gid'` form).

use recursa::seq::Seq0;
use recursa_diagram::railroad;

use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::TypeNameList;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};

// --- PREPARE / EXECUTE / DEALLOCATE ---

/// A statement that can be the body of `PREPARE name AS ...` — Postgres'
/// `PreparableStmt`: `SELECT | INSERT | UPDATE | DELETE | MERGE`.
///
/// `Query` uses `Subquery`, which already models Postgres' full `SelectStmt`
/// grammar (`SELECT`, set operations, `VALUES`, `TABLE`, and `WITH`). The
/// other four variants have disjoint leading keywords, so variant order does
/// not affect disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum PreparableStmt<'input> {
    Query(Box<crate::ast::dml::values::Subquery<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Merge(Box<crate::ast::dml::merge::MergeStmt<'input>>),
}

/// `( typename [, ...] )` parameter-type list on a `PREPARE` statement
/// (`prep_type_clause` in `gram.y`).
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareTypes<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub types:  TypeNameList<'input> ,
}

/// Body of a standard `PREPARE name [(types)] AS stmt` statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareStandardBody<'input> {
    pub name: literal::AliasName<'input>,
    pub types: Option<PrepareTypes<'input>>,
    #[tok(AS, this)]
    pub body: PreparableStmt<'input>,
}

/// `PREPARE TRANSACTION 'gid'` — the two-phase-commit transaction-prepare
/// form (`gram.y::TransactionStmt: PREPARE TRANSACTION Sconst`). Distinct
/// from the `PREPARE name … AS stmt` form modelled by
/// [`PrepareStandardBody`].
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareTransactionBody<'input> {
    #[tok(TRANSACTION, this)]
    pub gid: literal::StringLit<'input>,
}

/// Body of a `PREPARE` statement.
///
/// Variant ordering: both variants share a 1-token first-set. `Transaction`
/// matches only the `TRANSACTION` keyword; `Standard` matches any word
/// (`AliasName::Bare` accepts every keyword including `TRANSACTION` — see
/// `tokens::literal::BareAliasName::peek`). Declaration order is the
/// tiebreaker, so `Transaction` must come first to win when the source is
/// literally `PREPARE TRANSACTION 'gid'`.
#[derive(recursa::Node, Debug, Clone)]
pub enum PrepareStmtBody<'input> {
    Transaction(PrepareTransactionBody<'input>),
    Standard(PrepareStandardBody<'input>),
}

/// ```sql
/// PREPARE name [ (typename [, ...]) ] AS PreparableStmt
/// PREPARE TRANSACTION 'gid'
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct PrepareStmt<'input> {
    #[tok(PREPARE, this)]
    pub body: PrepareStmtBody<'input>,
}

/// `( expr [, ...] )` argument list on an `EXECUTE` statement
/// (`execute_param_clause` in `gram.y`).
#[derive(recursa::Node, Debug, Clone)]
pub struct ExecuteParams<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params:  Vec<Expr<'input> > ,
}

/// ```sql
/// EXECUTE name [ (expr [, ...]) ]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct ExecuteStmt<'input> {
    #[tok(EXECUTE, this)]
    pub name: literal::AliasName<'input>,
    pub params: Option<ExecuteParams<'input>>,
}

/// Target of a `DEALLOCATE` statement: a named prepared statement or `ALL`.
///
/// Variant ordering: `All` (the `ALL` keyword) before `Name` so the reserved
/// word is not swallowed as a statement name.
#[derive(recursa::Node, Debug, Clone)]
pub enum DeallocateTarget<'input> {
    #[tok(ALL)] All,
    Name(literal::AliasName<'input>),
}

/// ```sql
/// DEALLOCATE [PREPARE] { name | ALL }
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct DeallocateStmt<'input> {
    #[tok(DEALLOCATE, optional(PREPARE), this)]
    pub target: DeallocateTarget<'input>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn prepare_plain_is_modelled() {
        let stmt: PrepareStmt = parse_stmt("PREPARE q1 AS SELECT 1 AS a");
        let body = match &stmt.body {
            PrepareStmtBody::Standard(s) => s,
            PrepareStmtBody::Transaction(_) => panic!("expected standard PREPARE body"),
        };
        assert_eq!(body.name.text(), "q1");
        assert!(body.types.is_none());
        assert_eq!(
            roundtrip::<PrepareStmt>("PREPARE q1 AS SELECT 1 AS a"),
            "PREPARE q1 AS SELECT 1 AS a"
        );
    }

    #[test]
    fn prepare_with_types_keeps_type_list() {
        let stmt: PrepareStmt = parse_stmt("PREPARE q2(text) AS SELECT $1");
        let body = match &stmt.body {
            PrepareStmtBody::Standard(s) => s,
            PrepareStmtBody::Transaction(_) => panic!("expected standard PREPARE body"),
        };
        assert!(body.types.is_some());
        reparse_stable::<PrepareStmt>("PREPARE q2(text) AS SELECT $1");
    }

    #[test]
    fn prepare_multiple_types_roundtrips() {
        let stmt: PrepareStmt = parse_stmt("PREPARE q3(text, int, boolean) AS SELECT $1");
        let body = match &stmt.body {
            PrepareStmtBody::Standard(s) => s,
            PrepareStmtBody::Transaction(_) => panic!("expected standard PREPARE body"),
        };
        assert!(body.types.is_some());
        reparse_stable::<PrepareStmt>("PREPARE q3(text, int, boolean) AS SELECT $1");
    }

    #[test]
    fn prepare_insert_is_modelled() {
        let stmt: PrepareStmt = parse_stmt("PREPARE p AS INSERT INTO t VALUES (1)");
        assert!(matches!(
            stmt.body,
            PrepareStmtBody::Standard(ref s) if matches!(s.body, PreparableStmt::Insert(_))
        ));
        reparse_stable::<PrepareStmt>("PREPARE p AS INSERT INTO t VALUES (1)");
    }

    /// `PREPARE TRANSACTION 'gid'` is the two-phase-commit transaction form
    /// (distinct from `PREPARE name [(types)] AS stmt`). The discriminator
    /// is the `TRANSACTION` keyword vs an identifier name after `PREPARE`.
    #[test]
    fn prepare_transaction_is_modelled() {
        let stmt: PrepareStmt = parse_stmt("PREPARE TRANSACTION 'regress_foo1'");
        assert!(matches!(stmt.body, PrepareStmtBody::Transaction(_)));
        assert_eq!(
            roundtrip::<PrepareStmt>("PREPARE TRANSACTION 'regress_foo1'"),
            "PREPARE TRANSACTION 'regress_foo1'"
        );
    }

    #[test]
    fn execute_plain_is_modelled() {
        let stmt: ExecuteStmt = parse_stmt("EXECUTE q1");
        assert_eq!(stmt.name.text(), "q1");
        assert!(stmt.params.is_none());
        assert_eq!(roundtrip::<ExecuteStmt>("EXECUTE q1"), "EXECUTE q1");
    }

    #[test]
    fn execute_with_params_keeps_params() {
        let stmt: ExecuteStmt = parse_stmt("EXECUTE q2('postgres')");
        assert!(stmt.params.is_some());
        reparse_stable::<ExecuteStmt>("EXECUTE q2('postgres')");
    }

    #[test]
    fn deallocate_name_is_modelled() {
        let stmt: DeallocateStmt = parse_stmt("DEALLOCATE q1");
        assert!(stmt.prepare.is_none());
        assert!(matches!(stmt.target, DeallocateTarget::Name(_)));
        assert_eq!(
            roundtrip::<DeallocateStmt>("DEALLOCATE q1"),
            "DEALLOCATE q1"
        );
    }

    #[test]
    fn deallocate_prepare_name_keeps_prepare() {
        let stmt: DeallocateStmt = parse_stmt("DEALLOCATE PREPARE q1");
        assert!(stmt.prepare.is_some());
        assert_eq!(
            roundtrip::<DeallocateStmt>("DEALLOCATE PREPARE q1"),
            "DEALLOCATE PREPARE q1"
        );
    }

    #[test]
    fn deallocate_all_is_modelled() {
        let stmt: DeallocateStmt = parse_stmt("DEALLOCATE ALL");
        assert!(matches!(stmt.target, DeallocateTarget::All(_)));
        assert_eq!(
            roundtrip::<DeallocateStmt>("DEALLOCATE ALL"),
            "DEALLOCATE ALL"
        );
    }
}
