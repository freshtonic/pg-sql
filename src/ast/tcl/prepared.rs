//! PREPARE / EXECUTE / DEALLOCATE statements (including the two-phase-commit
//! `PREPARE TRANSACTION 'gid'` form).

use recursa::seq::Seq0;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PreparableStmt<'input> {
    Query(Box<crate::ast::dml::values::Subquery<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Merge(Box<crate::ast::dml::merge::MergeStmt<'input>>),
}

/// `( typename [, ...] )` parameter-type list on a `PREPARE` statement
/// (`prep_type_clause` in `gram.y`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PrepareTypes<'input> {
    pub types: Surrounded<punct::LParen, TypeNameList<'input>, punct::RParen>,
}

/// Body of a standard `PREPARE name [(types)] AS stmt` statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PrepareStandardBody<'input> {
    pub name: literal::AliasName<'input>,
    pub types: Option<PrepareTypes<'input>>,
    pub r#as: AS,
    pub body: PreparableStmt<'input>,
}

/// `PREPARE TRANSACTION 'gid'` — the two-phase-commit transaction-prepare
/// form (`gram.y::TransactionStmt: PREPARE TRANSACTION Sconst`). Distinct
/// from the `PREPARE name … AS stmt` form modelled by
/// [`PrepareStandardBody`].
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PrepareTransactionBody<'input> {
    pub transaction: TRANSACTION,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PrepareStmtBody<'input> {
    Transaction(PrepareTransactionBody<'input>),
    Standard(PrepareStandardBody<'input>),
}

/// ```sql
/// PREPARE name [ (typename [, ...]) ] AS PreparableStmt
/// PREPARE TRANSACTION 'gid'
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct PrepareStmt<'input> {
    pub prepare: PREPARE,
    pub body: PrepareStmtBody<'input>,
}

/// `( expr [, ...] )` argument list on an `EXECUTE` statement
/// (`execute_param_clause` in `gram.y`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExecuteParams<'input> {
    pub params: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
}

/// ```sql
/// EXECUTE name [ (expr [, ...]) ]
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct ExecuteStmt<'input> {
    pub execute: EXECUTE,
    pub name: literal::AliasName<'input>,
    pub params: Option<ExecuteParams<'input>>,
}

/// Target of a `DEALLOCATE` statement: a named prepared statement or `ALL`.
///
/// Variant ordering: `All` (the `ALL` keyword) before `Name` so the reserved
/// word is not swallowed as a statement name.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DeallocateTarget<'input> {
    All(ALL),
    Name(literal::AliasName<'input>),
}

/// ```sql
/// DEALLOCATE [PREPARE] { name | ALL }
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct DeallocateStmt<'input> {
    pub deallocate: DEALLOCATE,
    pub prepare: Option<PREPARE>,
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
