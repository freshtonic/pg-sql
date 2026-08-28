//! Transaction-control statements: BEGIN, START, COMMIT, ROLLBACK, END,
//! ABORT, SET TRANSACTION, SET CONSTRAINTS. SAVEPOINT/RELEASE live in
//! `tcl/savepoint.rs`; PREPARE/EXECUTE/DEALLOCATE in `tcl/prepared.rs`.

use recursa::seq::{Seq0, Seq1};
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};

// --- Transaction control ---

/// Isolation level following `ISOLATION LEVEL`.
///
/// Variant ordering: multi-word forms before single-word `Serializable`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum IsolationLevelKind {
    RepeatableRead((REPEATABLE, READ)),
    ReadCommitted((READ, COMMITTED)),
    ReadUncommitted((READ, UNCOMMITTED)),
    Serializable(SERIALIZABLE),
}

/// `ISOLATION LEVEL level` transaction mode.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IsolationLevelMode {
    pub isolation: ISOLATION,
    pub level_kw: LEVEL,
    pub level: IsolationLevelKind,
}

/// A single transaction mode.
///
/// Variant ordering: multi-word before single, and `NotDeferrable` (NOT
/// DEFERRABLE) before bare `Deferrable`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TransactionMode<'input> {
    IsolationLevel(IsolationLevelMode),
    ReadOnly((READ, ONLY)),
    ReadWrite((READ, WRITE)),
    NotDeferrable((NOT, DEFERRABLE)),
    Deferrable(DEFERRABLE),
    Snapshot(SnapshotMode<'input>),
}

/// `SNAPSHOT 'snapshot_id'` — import a serializable transaction snapshot.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SnapshotMode<'input> {
    pub snapshot: SNAPSHOT,
    pub id: literal::StringLit<'input>,
}

/// Optional `WORK | TRANSACTION` suffix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WorkOrTransaction {
    Work(WORK),
    Transaction(TRANSACTION),
}

/// BEGIN [WORK | TRANSACTION] [transaction_mode [, ...]]
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct BeginStmt<'input> {
    pub begin: BEGIN,
    pub work: Option<WorkOrTransaction>,
    pub modes: Option<Seq0<TransactionMode<'input>, punct::Comma>>,
}

/// END [WORK | TRANSACTION] — alias for COMMIT.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct EndStmt {
    pub end: END,
    pub work: Option<WorkOrTransaction>,
}

/// ABORT [WORK | TRANSACTION] — alias for ROLLBACK.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct AbortStmt {
    pub abort: ABORT,
    pub work: Option<WorkOrTransaction>,
}

/// START TRANSACTION [transaction_mode [, ...]]
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct StartTransactionStmt<'input> {
    pub start: START,
    pub transaction: TRANSACTION,
    pub modes: Option<Seq0<TransactionMode<'input>, punct::Comma>>,
}

/// SET TRANSACTION transaction_mode [, ...]
/// SET SESSION CHARACTERISTICS AS TRANSACTION transaction_mode [, ...]
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct SetTransactionStmt<'input> {
    pub set: SET,
    pub target: SetTransactionTarget,
    pub modes: Seq0<TransactionMode<'input>, punct::Comma>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetTransactionTarget {
    SessionCharacteristics((SESSION, CHARACTERISTICS, AS, TRANSACTION)),
    Transaction(TRANSACTION),
}

/// `SET CONSTRAINTS { ALL | name [, …] } { DEFERRED | IMMEDIATE }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct SetConstraintsStmt<'input> {
    pub set: SET,
    pub constraints: CONSTRAINTS,
    pub target: SetConstraintsTarget<'input>,
    pub mode: DeferredMode,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetConstraintsTarget<'input> {
    All(ALL),
    /// Per gram.y `constraints_set_list: qualified_name_list`. Schema-qualified
    /// names are required for cross-schema constraints (`fkpart3.fkey`).
    Names(Seq1<QualifiedName<'input>, punct::Comma>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DeferredMode {
    Deferred(DEFERRED),
    Immediate(IMMEDIATE),
}

/// `AND [NO] CHAIN` transaction-chaining suffix on `COMMIT` / `ROLLBACK`.
///
/// Variant ordering: `AND NO CHAIN` (3 tokens) before `AND CHAIN` (2 tokens)
/// so the longer form wins longest-match disambiguation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TransactionChain {
    NoChain((AND, NO, CHAIN)),
    Chain((AND, CHAIN)),
}

/// `WORK | TRANSACTION` followed by an optional `AND [NO] CHAIN` chain clause —
/// the `opt_transaction opt_transaction_chain` form of `COMMIT` / `ROLLBACK`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommitWithWork {
    pub work: WorkOrTransaction,
    pub chain: Option<TransactionChain>,
}

/// `PREPARED 'gid'` — the two-phase-commit form of `COMMIT` / `ROLLBACK`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PreparedGid<'input> {
    pub prepared: PREPARED,
    pub gid: literal::StringLit<'input>,
}

/// Body of a `COMMIT` statement after the `COMMIT` keyword.
///
/// Variant ordering: first-sets are disjoint (`PREPARED`, `WORK`/`TRANSACTION`,
/// `AND`), so order does not affect disambiguation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CommitBody<'input> {
    Prepared(PreparedGid<'input>),
    WithWork(CommitWithWork),
    Chain(TransactionChain),
}

/// COMMIT \[WORK | TRANSACTION\] \[AND \[NO\] CHAIN\]
/// COMMIT PREPARED 'gid'
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct CommitStmt<'input> {
    pub commit: COMMIT,
    pub body: Option<CommitBody<'input>>,
}

/// `TO [SAVEPOINT] name` — the savepoint target of `ROLLBACK TO`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RollbackToClause<'input> {
    pub to: TO,
    pub savepoint: Option<SAVEPOINT>,
    pub name: crate::tokens::ColId<'input>,
}

/// What follows `WORK`/`TRANSACTION` in a `ROLLBACK`: either a `TO` savepoint
/// clause or an `AND [NO] CHAIN` clause. First-sets (`TO` vs `AND`) are
/// disjoint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RollbackAfterWork<'input> {
    To(RollbackToClause<'input>),
    Chain(TransactionChain),
}

/// `WORK | TRANSACTION` followed by an optional `TO`/`AND CHAIN` clause —
/// the `opt_transaction (opt_transaction_chain | TO ...)` form of `ROLLBACK`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RollbackWithWork<'input> {
    pub work: WorkOrTransaction,
    pub after: Option<RollbackAfterWork<'input>>,
}

/// Body of a `ROLLBACK` statement after the `ROLLBACK` keyword.
///
/// Variant ordering: first-sets are disjoint (`PREPARED`, `WORK`/`TRANSACTION`,
/// `TO`, `AND`), so order does not affect disambiguation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RollbackBody<'input> {
    Prepared(PreparedGid<'input>),
    WithWork(RollbackWithWork<'input>),
    To(RollbackToClause<'input>),
    Chain(TransactionChain),
}

/// ROLLBACK \[WORK | TRANSACTION\] \[AND \[NO\] CHAIN\]
/// ROLLBACK \[WORK | TRANSACTION\] TO \[SAVEPOINT\] name
/// ROLLBACK PREPARED 'gid'
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct RollbackStmt<'input> {
    pub rollback: ROLLBACK,
    pub body: Option<RollbackBody<'input>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_end_stmt() {
        let mut input = crate::tokens::test_input("END");
        let _stmt = EndStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_abort_stmt() {
        let mut input = crate::tokens::test_input("ABORT");
        let _stmt = AbortStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_abort_work() {
        let mut input = crate::tokens::test_input("ABORT WORK");
        let _stmt = AbortStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_start_transaction_read_write() {
        let mut input = crate::tokens::test_input("START TRANSACTION READ WRITE");
        let _stmt = StartTransactionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_transaction_modes() {
        let mut input = crate::tokens::test_input(
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY, DEFERRABLE",
        );
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_transaction_read_write() {
        let mut input = crate::tokens::test_input("SET TRANSACTION READ WRITE");
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_session_characteristics() {
        let mut input =
            crate::tokens::test_input("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY");
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_transaction_snapshot() {
        let mut input = crate::tokens::test_input("SET TRANSACTION SNAPSHOT 'FFF-FFF-F'");
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_constraints_all_deferred() {
        let mut input = crate::tokens::test_input("SET CONSTRAINTS ALL DEFERRED");
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// `SET CONSTRAINTS qualified_name [, …] mode` — `constraints_set_list`
    /// is `qualified_name_list`, so schema-qualified names like
    /// `fkpart3.fkey` must parse (foreign_key.sql corpus).
    #[test]
    fn parse_set_constraints_qualified_name_deferred() {
        let mut input = crate::tokens::test_input("SET CONSTRAINTS fkpart3.fkey DEFERRED");
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_constraints_multiple_names_immediate() {
        let mut input =
            crate::tokens::test_input("SET CONSTRAINTS schema_a.c1, schema_b.c2 IMMEDIATE");
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_begin_isolation() {
        let mut input = crate::tokens::test_input("BEGIN ISOLATION LEVEL SERIALIZABLE");
        let _stmt = BeginStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn commit_bare_has_no_body() {
        let mut input = crate::tokens::test_input("COMMIT");
        let stmt = CommitStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
        assert!(stmt.body.is_none());
    }

    #[test]
    fn commit_work_keeps_work_keyword() {
        assert_eq!(roundtrip::<CommitStmt>("COMMIT WORK"), "COMMIT WORK");
    }

    #[test]
    fn commit_transaction_and_chain_roundtrips() {
        assert_eq!(
            roundtrip::<CommitStmt>("COMMIT TRANSACTION AND CHAIN"),
            "COMMIT TRANSACTION AND CHAIN"
        );
    }

    #[test]
    fn commit_and_no_chain_roundtrips() {
        assert_eq!(
            roundtrip::<CommitStmt>("COMMIT AND NO CHAIN"),
            "COMMIT AND NO CHAIN"
        );
    }

    #[test]
    fn commit_prepared_roundtrips() {
        assert_eq!(
            roundtrip::<CommitStmt>("COMMIT PREPARED 'regress_foo2'"),
            "COMMIT PREPARED 'regress_foo2'"
        );
    }

    #[test]
    fn rollback_bare_has_no_body() {
        let mut input = crate::tokens::test_input("ROLLBACK");
        let stmt = RollbackStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
        assert!(stmt.body.is_none());
    }

    #[test]
    fn rollback_work_and_chain_roundtrips() {
        assert_eq!(
            roundtrip::<RollbackStmt>("ROLLBACK WORK AND CHAIN"),
            "ROLLBACK WORK AND CHAIN"
        );
    }

    #[test]
    fn rollback_to_savepoint_roundtrips() {
        assert_eq!(
            roundtrip::<RollbackStmt>("ROLLBACK TO SAVEPOINT one"),
            "ROLLBACK TO SAVEPOINT one"
        );
    }

    #[test]
    fn rollback_to_name_roundtrips() {
        assert_eq!(
            roundtrip::<RollbackStmt>("ROLLBACK TO sp"),
            "ROLLBACK TO sp"
        );
    }

    #[test]
    fn rollback_prepared_roundtrips() {
        assert_eq!(
            roundtrip::<RollbackStmt>("ROLLBACK PREPARED 'regress_foo1'"),
            "ROLLBACK PREPARED 'regress_foo1'"
        );
    }
}
