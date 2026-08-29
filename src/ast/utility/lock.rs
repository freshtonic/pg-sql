//! LOCK statement.

use recursa::seq::Seq1;
use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;
use crate::tokens::punct;

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
    pub star: bool,
}

/// A `LOCK` lock-mode name — Postgres' `lock_type`.
///
/// Variant ordering: the three-word `SHARE ROW EXCLUSIVE` /
/// `SHARE UPDATE EXCLUSIVE` forms precede the two-word `ROW EXCLUSIVE` and the
/// bare `SHARE`, so longest-match-wins picks the most specific spelling.
#[derive(recursa::Node, Debug, Clone)]
pub enum LockType {
    #[tok(ACCESS, SHARE)] AccessShare,
    #[tok(ACCESS, EXCLUSIVE)] AccessExclusive,
    #[tok(SHARE, ROW, EXCLUSIVE)] ShareRowExclusive,
    #[tok(SHARE, UPDATE, EXCLUSIVE)] ShareUpdateExclusive,
    #[tok(ROW, SHARE)] RowShare,
    #[tok(ROW, EXCLUSIVE)] RowExclusive,
    #[tok(SHARE)] Share,
    #[tok(EXCLUSIVE)] Exclusive,
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
    #[tok(LOCK, optional(TABLE), this)]
    #[sep(COMMA)]
    pub relations: recursa::Vec1<LockRelation<'input> >,
    pub mode: Option<LockMode>,
    #[presence(NOWAIT)]
    pub nowait: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn lock_plain_relation_is_modelled() {
        let stmt: LockStmt = parse_stmt("LOCK atestc");
        assert!(stmt.table.is_none());
        assert!(stmt.mode.is_none());
        assert!(stmt.nowait.is_none());
        assert_eq!(roundtrip::<LockStmt>("LOCK atestc"), "LOCK atestc");
    }

    #[test]
    fn lock_table_keyword_roundtrips() {
        let stmt: LockStmt = parse_stmt("LOCK TABLE fast_emp4000");
        assert!(stmt.table.is_some());
        assert_eq!(
            roundtrip::<LockStmt>("LOCK TABLE fast_emp4000"),
            "LOCK TABLE fast_emp4000"
        );
    }

    #[test]
    fn lock_only_roundtrips() {
        assert_eq!(
            roundtrip::<LockStmt>("LOCK TABLE ONLY lock_tbl1"),
            "LOCK TABLE ONLY lock_tbl1"
        );
    }

    #[test]
    fn lock_inheritance_star_roundtrips() {
        assert_eq!(
            roundtrip::<LockStmt>("LOCK TABLE lock_tbl1 * IN ACCESS EXCLUSIVE MODE"),
            "LOCK TABLE lock_tbl1 * IN ACCESS EXCLUSIVE MODE"
        );
    }

    #[test]
    fn lock_with_access_exclusive_mode_roundtrips() {
        let stmt: LockStmt = parse_stmt("LOCK atest1 IN ACCESS EXCLUSIVE MODE");
        assert!(stmt.mode.is_some());
        assert_eq!(
            roundtrip::<LockStmt>("LOCK atest1 IN ACCESS EXCLUSIVE MODE"),
            "LOCK atest1 IN ACCESS EXCLUSIVE MODE"
        );
    }

    #[test]
    fn lock_with_share_row_exclusive_mode_roundtrips() {
        assert_eq!(
            roundtrip::<LockStmt>("LOCK lock_tbl1 IN SHARE ROW EXCLUSIVE MODE"),
            "LOCK lock_tbl1 IN SHARE ROW EXCLUSIVE MODE"
        );
    }

    #[test]
    fn lock_with_share_update_exclusive_mode_roundtrips() {
        assert_eq!(
            roundtrip::<LockStmt>("LOCK TABLE lock_tbl1 IN SHARE UPDATE EXCLUSIVE MODE"),
            "LOCK TABLE lock_tbl1 IN SHARE UPDATE EXCLUSIVE MODE"
        );
    }

    #[test]
    fn lock_with_nowait_keeps_nowait() {
        let stmt: LockStmt = parse_stmt("LOCK TABLE lock_tbl1 IN ACCESS EXCLUSIVE MODE NOWAIT");
        assert!(stmt.nowait.is_some());
        assert_eq!(
            roundtrip::<LockStmt>("LOCK TABLE lock_tbl1 IN ACCESS EXCLUSIVE MODE NOWAIT"),
            "LOCK TABLE lock_tbl1 IN ACCESS EXCLUSIVE MODE NOWAIT"
        );
    }

    #[test]
    fn lock_multiple_relations_roundtrips() {
        assert_eq!(
            roundtrip::<LockStmt>("LOCK TABLE a, b, c"),
            "LOCK TABLE a, b, c"
        );
    }
}
