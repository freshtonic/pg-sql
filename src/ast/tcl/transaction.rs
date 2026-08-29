//! Transaction-control statements: BEGIN, START, COMMIT, ROLLBACK, END,
//! ABORT, SET TRANSACTION, SET CONSTRAINTS. SAVEPOINT/RELEASE live in
//! `tcl/savepoint.rs`; PREPARE/EXECUTE/DEALLOCATE in `tcl/prepared.rs`.

use recursa::seq::{Seq0, Seq1};
use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};

// --- Transaction control ---

/// Isolation level following `ISOLATION LEVEL`.
///
/// Variant ordering: multi-word forms before single-word `Serializable`.
#[derive(recursa::Node, Debug, Clone)]
pub enum IsolationLevelKind {
    #[tok(REPEATABLE, READ)] RepeatableRead,
    #[tok(READ, COMMITTED)] ReadCommitted,
    #[tok(READ, UNCOMMITTED)] ReadUncommitted,
    #[tok(SERIALIZABLE)] Serializable,
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
    #[tok(READ, ONLY)] ReadOnly,
    #[tok(READ, WRITE)] ReadWrite,
    #[tok(NOT, DEFERRABLE)] NotDeferrable,
    #[tok(DEFERRABLE)] Deferrable,
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
    #[tok(WORK)] Work,
    #[tok(TRANSACTION)] Transaction,
}

/// BEGIN [WORK | TRANSACTION] [transaction_mode [, ...]]
#[derive(recursa::Node, Debug, Clone)]
pub struct BeginStmt<'input> {
    #[tok(BEGIN, this)]
    pub work: Option<WorkOrTransaction>,
    #[sep(COMMA)]
    pub modes: Option<Vec<TransactionMode<'input> >>,
}

/// END [WORK | TRANSACTION] — alias for COMMIT.
#[derive(recursa::Node, Debug, Clone)]
pub struct EndStmt {
    #[tok(END, this)]
    pub work: Option<WorkOrTransaction>,
}

/// ABORT [WORK | TRANSACTION] — alias for ROLLBACK.
#[derive(recursa::Node, Debug, Clone)]
pub struct AbortStmt {
    #[tok(ABORT, this)]
    pub work: Option<WorkOrTransaction>,
}

/// START TRANSACTION [transaction_mode [, ...]]
#[derive(recursa::Node, Debug, Clone)]
pub struct StartTransactionStmt<'input> {
    #[tok(START, TRANSACTION, this)]
    #[sep(COMMA)]
    pub modes: Option<Vec<TransactionMode<'input> >>,
}

/// SET TRANSACTION transaction_mode [, ...]
/// SET SESSION CHARACTERISTICS AS TRANSACTION transaction_mode [, ...]
#[derive(recursa::Node, Debug, Clone)]
pub struct SetTransactionStmt<'input> {
    #[tok(SET, this)]
    pub target: SetTransactionTarget,
    #[sep(COMMA)]
    pub modes: Vec<TransactionMode<'input> >,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SetTransactionTarget {
    #[tok(SESSION, CHARACTERISTICS, AS, TRANSACTION)] SessionCharacteristics,
    #[tok(TRANSACTION)] Transaction,
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
    #[tok(ALL)] All,
    /// Per gram.y `constraints_set_list: qualified_name_list`. Schema-qualified
    /// names are required for cross-schema constraints (`fkpart3.fkey`).
    Names(#[sep(COMMA)] recursa::Vec1<QualifiedName<'input> >),
}

#[derive(recursa::Node, Debug, Clone)]
pub enum DeferredMode {
    #[tok(DEFERRED)] Deferred,
    #[tok(IMMEDIATE)] Immediate,
}

/// `AND [NO] CHAIN` transaction-chaining suffix on `COMMIT` / `ROLLBACK`.
///
/// Variant ordering: `AND NO CHAIN` (3 tokens) before `AND CHAIN` (2 tokens)
/// so the longer form wins longest-match disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum TransactionChain {
    #[tok(AND, NO, CHAIN)] NoChain,
    #[tok(AND, CHAIN)] Chain,
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
pub struct CommitStmt<'input> {
    #[tok(COMMIT, this)]
    pub body: Option<CommitBody<'input>>,
}

/// `TO [SAVEPOINT] name` — the savepoint target of `ROLLBACK TO`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RollbackToClause<'input> {
    #[tok(TO, optional(SAVEPOINT), this)]
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
pub struct RollbackStmt<'input> {
    #[tok(ROLLBACK, this)]
    pub body: Option<RollbackBody<'input>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_end_stmt() {
        let lexed = crate::tokens::lex("END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = EndStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_abort_stmt() {
        let lexed = crate::tokens::lex("ABORT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AbortStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_abort_work() {
        let lexed = crate::tokens::lex("ABORT WORK");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AbortStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_start_transaction_read_write() {
        let lexed = crate::tokens::lex("START TRANSACTION READ WRITE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = StartTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_transaction_modes() {
        let lexed = crate::tokens::lex("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY, DEFERRABLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_transaction_read_write() {
        let lexed = crate::tokens::lex("SET TRANSACTION READ WRITE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_characteristics() {
        let lexed = crate::tokens::lex("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_transaction_snapshot() {
        let lexed = crate::tokens::lex("SET TRANSACTION SNAPSHOT 'FFF-FFF-F'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_constraints_all_deferred() {
        let lexed = crate::tokens::lex("SET CONSTRAINTS ALL DEFERRED");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// `SET CONSTRAINTS qualified_name [, …] mode` — `constraints_set_list`
    /// is `qualified_name_list`, so schema-qualified names like
    /// `fkpart3.fkey` must parse (foreign_key.sql corpus).
    #[test]
    fn parse_set_constraints_qualified_name_deferred() {
        let lexed = crate::tokens::lex("SET CONSTRAINTS fkpart3.fkey DEFERRED");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_constraints_multiple_names_immediate() {
        let lexed = crate::tokens::lex("SET CONSTRAINTS schema_a.c1, schema_b.c2 IMMEDIATE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_begin_isolation() {
        let lexed = crate::tokens::lex("BEGIN ISOLATION LEVEL SERIALIZABLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = BeginStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn commit_bare_has_no_body() {
        let lexed = crate::tokens::lex("COMMIT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CommitStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
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
        let lexed = crate::tokens::lex("ROLLBACK");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = RollbackStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
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
