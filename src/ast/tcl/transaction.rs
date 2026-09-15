//! Transaction-control statements: BEGIN, START, COMMIT, ROLLBACK, END,
//! ABORT, SET TRANSACTION, SET CONSTRAINTS. SAVEPOINT/RELEASE live in
//! `tcl/savepoint.rs`; PREPARE/EXECUTE/DEALLOCATE in `tcl/prepared.rs`.

use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

// --- Transaction control ---

recursa::ast_node! {
    /// Isolation level following `ISOLATION LEVEL`.
    ///
    /// Variant ordering: multi-word forms before single-word `Serializable`.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `ISOLATION LEVEL level` transaction mode.
    #[derive(Debug)]
    pub struct IsolationLevelMode {
        #[tok(ISOLATION, LEVEL, this)]
        pub level: IsolationLevelKind,
    }
}

recursa::ast_node! {
    /// A single transaction mode.
    ///
    /// Variant ordering: multi-word before single, and `NotDeferrable` (NOT
    /// DEFERRABLE) before bare `Deferrable`.
    #[derive(Debug)]
    pub enum TransactionMode {
        IsolationLevel(IsolationLevelMode),
        #[tok(READ, ONLY)]
        ReadOnly,
        #[tok(READ, WRITE)]
        ReadWrite,
        #[tok(NOT, DEFERRABLE)]
        NotDeferrable,
        #[tok(DEFERRABLE)]
        Deferrable,
        Snapshot(SnapshotMode),
    }
}

recursa::ast_node! {
    /// `SNAPSHOT 'snapshot_id'` — import a serializable transaction snapshot.
    #[derive(Debug)]
    pub struct SnapshotMode {
        #[tok(SNAPSHOT, this)]
        pub id: literal::StringLit,
    }
}

recursa::ast_node! {
    /// Optional `WORK | TRANSACTION` suffix.
    #[derive(Debug)]
    pub enum WorkOrTransaction {
        #[tok(WORK)]
        Work,
        #[tok(TRANSACTION)]
        Transaction,
    }
}

recursa::ast_node! {
    /// BEGIN [WORK | TRANSACTION] [transaction_mode [, ...]]
    #[derive(Debug)]
    #[tok(BEGIN, this)]
    pub struct BeginStmt {
        pub work: Option<WorkOrTransaction>,
        #[sep(COMMA)]
        pub modes: Option<one_or_many!(TransactionMode)>,
    }
}

recursa::ast_node! {
    /// END [WORK | TRANSACTION] — alias for COMMIT.
    #[derive(Debug)]
    #[tok(END, this)]
    pub struct EndStmt {
        pub work: Option<WorkOrTransaction>,
    }
}

recursa::ast_node! {
    /// ABORT [WORK | TRANSACTION] — alias for ROLLBACK.
    #[derive(Debug)]
    #[tok(ABORT, this)]
    pub struct AbortStmt {
        pub work: Option<WorkOrTransaction>,
    }
}

recursa::ast_node! {
    /// START TRANSACTION [transaction_mode [, ...]]
    #[derive(Debug)]
    #[tok(START, TRANSACTION, this)]
    pub struct StartTransactionStmt {
        #[sep(COMMA)]
        pub modes: Option<one_or_many!(TransactionMode)>,
    }
}

recursa::ast_node! {
    /// `TRANSACTION transaction_mode [, ...]`, the transaction branch of
    /// PostgreSQL's `set_rest`.
    #[derive(Debug)]
    pub struct SetTransactionRest {
        #[tok(TRANSACTION, this)]
        pub modes: TransactionModeList,
    }
}

recursa::ast_node! {
    /// `SESSION CHARACTERISTICS AS TRANSACTION transaction_mode [, ...]`, the
    /// other transaction branch of PostgreSQL's `set_rest`.
    #[derive(Debug)]
    pub struct SetSessionCharacteristicsRest {
        #[tok(SESSION, CHARACTERISTICS, AS, TRANSACTION, this)]
        pub modes: TransactionModeList,
    }
}

recursa::ast_node! {
    /// One or more comma-separated transaction modes.
    ///
    /// This owns the repetition so the prefixes attached by the two `set_rest`
    /// forms above are consumed once. A direct attachment on `Vec1` would make
    /// the generated table expect that prefix again after every comma.
    #[derive(Debug, derive_more :: Deref)]
    pub struct TransactionModeList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(TransactionMode),
    );
}

recursa::ast_node! {
    /// `SET CONSTRAINTS { ALL | name [, …] } { DEFERRED | IMMEDIATE }`
    #[derive(Debug)]
    pub struct SetConstraintsStmt {
        #[tok(SET, CONSTRAINTS, this)]
        pub target: SetConstraintsTarget,
        pub mode: DeferredMode,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SetConstraintsTarget {
        #[tok(ALL)]
        All,
        /// Per gram.y `constraints_set_list: qualified_name_list`. Schema-qualified
        /// names are required for cross-schema constraints (`fkpart3.fkey`).
        Names(#[sep(COMMA)] one_or_many!(QualifiedName)),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum DeferredMode {
        #[tok(DEFERRED)]
        Deferred,
        #[tok(IMMEDIATE)]
        Immediate,
    }
}

recursa::ast_node! {
    /// `AND [NO] CHAIN` transaction-chaining suffix on `COMMIT` / `ROLLBACK`.
    ///
    /// Variant ordering: `AND NO CHAIN` (3 tokens) before `AND CHAIN` (2 tokens)
    /// so the longer form wins longest-match disambiguation.
    #[derive(Debug)]
    pub enum TransactionChain {
        #[tok(AND, NO, CHAIN)]
        NoChain,
        #[tok(AND, CHAIN)]
        Chain,
    }
}

recursa::ast_node! {
    /// `WORK | TRANSACTION` followed by an optional `AND [NO] CHAIN` chain clause —
    /// the `opt_transaction opt_transaction_chain` form of `COMMIT` / `ROLLBACK`.
    #[derive(Debug)]
    pub struct CommitWithWork {
        pub work: WorkOrTransaction,
        pub chain: Option<TransactionChain>,
    }
}

recursa::ast_node! {
    /// `PREPARED 'gid'` — the two-phase-commit form of `COMMIT` / `ROLLBACK`.
    #[derive(Debug)]
    pub struct PreparedGid {
        #[tok(PREPARED, this)]
        pub gid: literal::StringLit,
    }
}

recursa::ast_node! {
    /// Body of a `COMMIT` statement after the `COMMIT` keyword.
    ///
    /// Variant ordering: first-sets are disjoint (`PREPARED`, `WORK`/`TRANSACTION`,
    /// `AND`), so order does not affect disambiguation.
    #[derive(Debug)]
    pub enum CommitBody {
        Prepared(PreparedGid),
        WithWork(CommitWithWork),
        Chain(TransactionChain),
    }
}

recursa::ast_node! {
    /// COMMIT \[WORK | TRANSACTION\] \[AND \[NO\] CHAIN\]
    /// COMMIT PREPARED 'gid'
    #[derive(Debug)]
    #[tok(COMMIT, this)]
    pub struct CommitStmt {
        pub body: Option<CommitBody>,
    }
}

recursa::ast_node! {
    /// `TO [SAVEPOINT] name` — the savepoint target of `ROLLBACK TO`.
    #[derive(Debug)]
    #[tok(TO, this)]
    pub struct RollbackToClause {
        #[presence(SAVEPOINT)]
        pub savepoint: bool,
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// What follows `WORK`/`TRANSACTION` in a `ROLLBACK`: either a `TO` savepoint
    /// clause or an `AND [NO] CHAIN` clause. First-sets (`TO` vs `AND`) are
    /// disjoint.
    #[derive(Debug)]
    pub enum RollbackAfterWork {
        To(RollbackToClause),
        Chain(TransactionChain),
    }
}

recursa::ast_node! {
    /// `WORK | TRANSACTION` followed by an optional `TO`/`AND CHAIN` clause —
    /// the `opt_transaction (opt_transaction_chain | TO ...)` form of `ROLLBACK`.
    #[derive(Debug)]
    pub struct RollbackWithWork {
        pub work: WorkOrTransaction,
        pub after: Option<RollbackAfterWork>,
    }
}

recursa::ast_node! {
    /// Body of a `ROLLBACK` statement after the `ROLLBACK` keyword.
    ///
    /// Variant ordering: first-sets are disjoint (`PREPARED`, `WORK`/`TRANSACTION`,
    /// `TO`, `AND`), so order does not affect disambiguation.
    #[derive(Debug)]
    pub enum RollbackBody {
        Prepared(PreparedGid),
        WithWork(RollbackWithWork),
        To(RollbackToClause),
        Chain(TransactionChain),
    }
}

recursa::ast_node! {
    /// ROLLBACK \[WORK | TRANSACTION\] \[AND \[NO\] CHAIN\]
    /// ROLLBACK \[WORK | TRANSACTION\] TO \[SAVEPOINT\] name
    /// ROLLBACK PREPARED 'gid'
    #[derive(Debug)]
    #[tok(ROLLBACK, this)]
    pub struct RollbackStmt {
        pub body: Option<RollbackBody>,
    }
}
