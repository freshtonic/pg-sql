//! TRUNCATE statement.

use recursa::seq::Seq1;
use recursa_diagram::railroad;

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;
use crate::tokens::punct;
use crate::tokens::soft_keyword::{CONTINUE, RESTART};

// --- TRUNCATE ---

/// `{ RESTART | CONTINUE } IDENTITY` — Postgres' `opt_restart_seqs`.
#[derive(recursa::Node, Debug, Clone)]
pub enum RestartSeqs {
    #[tok(RESTART, IDENTITY)] Restart,
    #[tok(CONTINUE, IDENTITY)] Continue,
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
pub struct TruncateStmt<'input> {
    #[tok(TRUNCATE, optional(TABLE), this)]
    #[sep(COMMA)]
    pub relations: recursa::Vec1<TruncateRelation<'input> >,
    pub restart_seqs: Option<RestartSeqs>,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn truncate_bare_is_modelled() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE t");
        assert!(stmt.table.is_none());
        assert_eq!(stmt.relations.len(), 1);
        assert!(stmt.restart_seqs.is_none());
        assert!(stmt.behavior.is_none());
        reparse_stable::<TruncateStmt>("TRUNCATE t");
    }

    #[test]
    fn truncate_with_table_keyword_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE TABLE pk");
        assert!(stmt.table.is_some());
        reparse_stable::<TruncateStmt>("TRUNCATE TABLE pk");
    }

    #[test]
    fn truncate_only_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE ONLY trunc_f");
        assert_eq!(stmt.relations.len(), 1);
        reparse_stable::<TruncateStmt>("TRUNCATE ONLY trunc_f");
    }

    #[test]
    fn truncate_multiple_relations_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE ONLY trunc_fb, ONLY trunc_fa");
        assert_eq!(stmt.relations.len(), 2);
        reparse_stable::<TruncateStmt>("TRUNCATE ONLY trunc_fb, ONLY trunc_fa");
    }

    #[test]
    fn truncate_cascade_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE TABLE truncate_a CASCADE");
        assert!(stmt.behavior.is_some());
        reparse_stable::<TruncateStmt>("TRUNCATE TABLE truncate_a CASCADE");
    }

    #[test]
    fn truncate_restrict_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE TABLE truncate_a RESTRICT");
    }

    #[test]
    fn truncate_restart_identity_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE truncate_a RESTART IDENTITY");
        assert!(stmt.restart_seqs.is_some());
        reparse_stable::<TruncateStmt>("TRUNCATE truncate_a RESTART IDENTITY");
    }

    #[test]
    fn truncate_continue_identity_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE truncate_a CONTINUE IDENTITY");
    }

    #[test]
    fn truncate_restart_identity_cascade_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE truncate_a RESTART IDENTITY CASCADE");
    }
}
