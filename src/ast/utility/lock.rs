//! LOCK statement.

use crate::ast::shared::names::QualifiedName;

// --- LOCK ---

/// A single relation reference in a `LOCK` statement — Postgres'
/// `relation_expr`.
///
/// `ONLY name` excludes inheritance children; a trailing `*` makes the
/// (default) inheritance behaviour explicit. The `ONLY (name)` parenthesised
/// form is not exercised by any corpus statement, so it is not modelled.
#[derive(recursa::Node, Debug, Clone)]
pub struct LockRelation<'input> {
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    #[presence(STAR)]
    #[pretty(break_before = soft)]
    pub star: bool,
}

/// A `LOCK` lock-mode name — Postgres' `lock_type`.
///
/// Variant ordering: the three-word `SHARE ROW EXCLUSIVE` /
/// `SHARE UPDATE EXCLUSIVE` forms precede the two-word `ROW EXCLUSIVE` and the
/// bare `SHARE`, so longest-match-wins picks the most specific spelling.
#[derive(recursa::Node, Debug, Clone)]
pub enum LockType {
    #[tok(ACCESS, SHARE)]
    AccessShare,
    #[tok(ACCESS, EXCLUSIVE)]
    AccessExclusive,
    #[tok(SHARE, ROW, EXCLUSIVE)]
    ShareRowExclusive,
    #[tok(SHARE, UPDATE, EXCLUSIVE)]
    ShareUpdateExclusive,
    #[tok(ROW, SHARE)]
    RowShare,
    #[tok(ROW, EXCLUSIVE)]
    RowExclusive,
    #[tok(SHARE)]
    Share,
    #[tok(EXCLUSIVE)]
    Exclusive,
}

/// The `IN lock_type MODE` clause on a `LOCK` statement — Postgres' `opt_lock`.
#[derive(recursa::Node, Debug, Clone)]
pub struct LockMode {
    #[tok(IN, this, MODE)]
    pub lock_type: LockType,
}

/// ```sql
/// LOCK [TABLE] name [, ...] [IN mode MODE] [NOWAIT]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct LockStmt<'input> {
    #[tok(LOCK, this)]
    #[presence(TABLE)]
    pub table: bool,
    #[sep(COMMA)]
    pub relations: recursa::Vec1<LockRelation<'input>>,
    pub mode: Option<LockMode>,
    #[presence(NOWAIT)]
    pub nowait: bool,
}
