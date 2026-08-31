#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn lock_plain_relation_is_modelled() {
        let stmt: LockStmt = parse_stmt("LOCK atestc");
        assert!(stmt.mode.is_none());
        assert!(!stmt.nowait);
        assert_eq!(roundtrip::<LockStmt>("LOCK atestc"), "LOCK atestc");
    }

    #[test]
    fn lock_table_keyword_roundtrips() {
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
        assert!(stmt.nowait);
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
