//! Transaction-control statements: BEGIN, START, COMMIT, ROLLBACK, END,
//! ABORT, SET TRANSACTION, SET CONSTRAINTS. SAVEPOINT/RELEASE live in
//! `tcl/savepoint.rs`; PREPARE/EXECUTE/DEALLOCATE in `tcl/prepared.rs`.

use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

// --- Transaction control ---

/// Isolation level following `ISOLATION LEVEL`.
///
/// Variant ordering: multi-word forms before single-word `Serializable`.
#[derive(recursa::Node, Debug, Clone)]
pub enum IsolationLevelKind {
    #[tok(REPEATABLE, READ)]
    RepeatableRead,
    #[tok(READ, COMMITTED)]
    ReadCommitted,
    #[tok(READ, UNCOMMITTED)]
    ReadUncommitted,
    #[tok(SERIALIZABLE)]
    Serializable,
}

/// `ISOLATION LEVEL level` transaction mode.
#[derive(recursa::Node, Debug, Clone)]
pub struct IsolationLevelMode {
    #[tok(ISOLATION, LEVEL, this)]
    pub level: IsolationLevelKind,
}

/// A single transaction mode.
///
/// Variant ordering: multi-word before single, and `NotDeferrable` (NOT
/// DEFERRABLE) before bare `Deferrable`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransactionMode<'input> {
    IsolationLevel(IsolationLevelMode),
    #[tok(READ, ONLY)]
    ReadOnly,
    #[tok(READ, WRITE)]
    ReadWrite,
    #[tok(NOT, DEFERRABLE)]
    NotDeferrable,
    #[tok(DEFERRABLE)]
    Deferrable,
    Snapshot(SnapshotMode<'input>),
}

/// `SNAPSHOT 'snapshot_id'` — import a serializable transaction snapshot.
#[derive(recursa::Node, Debug, Clone)]
pub struct SnapshotMode<'input> {
    #[tok(SNAPSHOT, this)]
    pub id: literal::StringLit<'input>,
}

/// Optional `WORK | TRANSACTION` suffix.
#[derive(recursa::Node, Debug, Clone)]
pub enum WorkOrTransaction {
    #[tok(WORK)]
    Work,
    #[tok(TRANSACTION)]
    Transaction,
}

/// BEGIN [WORK | TRANSACTION] [transaction_mode [, ...]]
#[derive(recursa::Node, Debug, Clone)]
#[tok(BEGIN, this)]
pub struct BeginStmt<'input> {
    pub work: Option<WorkOrTransaction>,
    #[sep(COMMA)]
    pub modes: Option<recursa::Vec1<TransactionMode<'input>>>,
}

/// END [WORK | TRANSACTION] — alias for COMMIT.
#[derive(recursa::Node, Debug, Clone)]
#[tok(END, this)]
pub struct EndStmt {
    pub work: Option<WorkOrTransaction>,
}

/// ABORT [WORK | TRANSACTION] — alias for ROLLBACK.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ABORT, this)]
pub struct AbortStmt {
    pub work: Option<WorkOrTransaction>,
}

/// START TRANSACTION [transaction_mode [, ...]]
#[derive(recursa::Node, Debug, Clone)]
#[tok(START, TRANSACTION, this)]
pub struct StartTransactionStmt<'input> {
    #[sep(COMMA)]
    pub modes: Option<recursa::Vec1<TransactionMode<'input>>>,
}

/// SET TRANSACTION transaction_mode [, ...]
/// SET SESSION CHARACTERISTICS AS TRANSACTION transaction_mode [, ...]
#[derive(recursa::Node, Debug, Clone)]
pub struct SetTransactionStmt<'input> {
    #[tok(SET, this)]
    pub target: SetTransactionTarget,
    #[sep(COMMA)]
    pub modes: Vec<TransactionMode<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SetTransactionTarget {
    #[tok(SESSION, CHARACTERISTICS, AS, TRANSACTION)]
    SessionCharacteristics,
    #[tok(TRANSACTION)]
    Transaction,
}

/// `SET CONSTRAINTS { ALL | name [, …] } { DEFERRED | IMMEDIATE }`
#[derive(recursa::Node, Debug, Clone)]
pub struct SetConstraintsStmt<'input> {
    #[tok(SET, CONSTRAINTS, this)]
    pub target: SetConstraintsTarget<'input>,
    pub mode: DeferredMode,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SetConstraintsTarget<'input> {
    #[tok(ALL)]
    All,
    /// Per gram.y `constraints_set_list: qualified_name_list`. Schema-qualified
    /// names are required for cross-schema constraints (`fkpart3.fkey`).
    Names(#[sep(COMMA)] recursa::Vec1<QualifiedName<'input>>),
}

#[derive(recursa::Node, Debug, Clone)]
pub enum DeferredMode {
    #[tok(DEFERRED)]
    Deferred,
    #[tok(IMMEDIATE)]
    Immediate,
}

/// `AND [NO] CHAIN` transaction-chaining suffix on `COMMIT` / `ROLLBACK`.
///
/// Variant ordering: `AND NO CHAIN` (3 tokens) before `AND CHAIN` (2 tokens)
/// so the longer form wins longest-match disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransactionChain {
    #[tok(AND, NO, CHAIN)]
    NoChain,
    #[tok(AND, CHAIN)]
    Chain,
}

/// `WORK | TRANSACTION` followed by an optional `AND [NO] CHAIN` chain clause —
/// the `opt_transaction opt_transaction_chain` form of `COMMIT` / `ROLLBACK`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommitWithWork {
    pub work: WorkOrTransaction,
    pub chain: Option<TransactionChain>,
}

/// `PREPARED 'gid'` — the two-phase-commit form of `COMMIT` / `ROLLBACK`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PreparedGid<'input> {
    #[tok(PREPARED, this)]
    pub gid: literal::StringLit<'input>,
}

/// Body of a `COMMIT` statement after the `COMMIT` keyword.
///
/// Variant ordering: first-sets are disjoint (`PREPARED`, `WORK`/`TRANSACTION`,
/// `AND`), so order does not affect disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum CommitBody<'input> {
    Prepared(PreparedGid<'input>),
    WithWork(CommitWithWork),
    Chain(TransactionChain),
}

/// COMMIT \[WORK | TRANSACTION\] \[AND \[NO\] CHAIN\]
/// COMMIT PREPARED 'gid'
#[derive(recursa::Node, Debug, Clone)]
#[tok(COMMIT, this)]
pub struct CommitStmt<'input> {
    pub body: Option<CommitBody<'input>>,
}

/// `TO [SAVEPOINT] name` — the savepoint target of `ROLLBACK TO`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RollbackToClause<'input> {
    #[tok(TO, this)]
    #[presence(SAVEPOINT)]
    pub savepoint: bool,
    pub name: crate::tokens::ColId<'input>,
}

/// What follows `WORK`/`TRANSACTION` in a `ROLLBACK`: either a `TO` savepoint
/// clause or an `AND [NO] CHAIN` clause. First-sets (`TO` vs `AND`) are
/// disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum RollbackAfterWork<'input> {
    To(RollbackToClause<'input>),
    Chain(TransactionChain),
}

/// `WORK | TRANSACTION` followed by an optional `TO`/`AND CHAIN` clause —
/// the `opt_transaction (opt_transaction_chain | TO ...)` form of `ROLLBACK`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RollbackWithWork<'input> {
    pub work: WorkOrTransaction,
    pub after: Option<RollbackAfterWork<'input>>,
}

/// Body of a `ROLLBACK` statement after the `ROLLBACK` keyword.
///
/// Variant ordering: first-sets are disjoint (`PREPARED`, `WORK`/`TRANSACTION`,
/// `TO`, `AND`), so order does not affect disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum RollbackBody<'input> {
    Prepared(PreparedGid<'input>),
    WithWork(RollbackWithWork<'input>),
    To(RollbackToClause<'input>),
    Chain(TransactionChain),
}

/// ROLLBACK \[WORK | TRANSACTION\] \[AND \[NO\] CHAIN\]
/// ROLLBACK \[WORK | TRANSACTION\] TO \[SAVEPOINT\] name
/// ROLLBACK PREPARED 'gid'
#[derive(recursa::Node, Debug, Clone)]
#[tok(ROLLBACK, this)]
pub struct RollbackStmt<'input> {
    pub body: Option<RollbackBody<'input>>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/tcl/transaction.tests.rs"
));
