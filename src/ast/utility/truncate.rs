//! TRUNCATE statement.

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;

// --- TRUNCATE ---

/// `{ RESTART | CONTINUE } IDENTITY` — Postgres' `opt_restart_seqs`.
#[derive(recursa::Node, Debug, Clone)]
pub enum RestartSeqs {
    #[tok(RESTART, IDENTITY)]
    Restart,
    #[tok(CONTINUE, IDENTITY)]
    Continue,
}

/// A single relation reference in a `TRUNCATE` statement — Postgres'
/// `relation_expr`.
///
/// `ONLY name` excludes inheritance children; a trailing `*` makes the
/// (default) inheritance behaviour explicit. The `ONLY (name)` parenthesised
/// form is not exercised by any TRUNCATE corpus statement, so it is not
/// modelled (matches the `LockRelation` shape).
#[derive(recursa::Node, Debug, Clone)]
pub struct TruncateRelation<'input> {
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    pub star: bool,
}

/// ```sql
/// TRUNCATE [TABLE] [ONLY] name [*] [, ...]
///     [ { RESTART | CONTINUE } IDENTITY ]
///     [ CASCADE | RESTRICT ]
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(TRUNCATE, optional(TABLE), this)]
pub struct TruncateStmt<'input> {
    #[sep(COMMA)]
    pub relations: recursa::Vec1<TruncateRelation<'input>>,
    pub restart_seqs: Option<RestartSeqs>,
    pub behavior: Option<DropBehavior>,
}
