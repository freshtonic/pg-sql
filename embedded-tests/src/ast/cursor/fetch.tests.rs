#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn close_cursor_is_modelled() {
        let stmt = parse_stmt::<CloseStmt>("CLOSE foo1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.target, CloseTarget::Cursor(_)));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE foo1"), "CLOSE foo1");
    }

    #[test]
    fn close_all_is_modelled() {
        let stmt = parse_stmt::<CloseStmt>("CLOSE ALL");
        let stmt = stmt.ast();
        assert!(matches!(stmt.target, CloseTarget::All));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE ALL"), "CLOSE ALL");
    }

    /// The bare count of `fetch_args`, and the count after `FORWARD` and
    /// `BACKWARD`, are each a `SignedIconst` (REL_17_11 gram.y 7450, 7477,
    /// 7504; `SignedIconst` 17299). Every target version from 14 has the
    /// same three productions, so there is no version gate.
    #[test]
    fn fetch_signed_count_is_modelled() {
        let stmt = parse_stmt::<FetchStmt>("FETCH -5 c");
        let stmt = stmt.ast();
        assert!(matches!(stmt.direction, Some(FetchDirection::Count(_))));

        let stmt = parse_stmt::<MoveStmt>("MOVE FORWARD +2 FROM c");
        let stmt = stmt.ast();
        assert!(matches!(stmt.direction, Some(FetchDirection::Forward(_))));
        assert!(stmt.source.is_some());
    }

    /// `FETCH`/`MOVE` accept a sign only where gram.y writes `SignedIconst`:
    /// the bare count, `ABSOLUTE`, `RELATIVE`, `FORWARD` and `BACKWARD`. The
    /// keyword directions (`NEXT`, `PRIOR`, `FIRST`, `LAST`, `ALL`) take no
    /// count, and `SignedIconst` takes an `Iconst`, never an `FCONST`.
    #[test]
    fn fetch_and_move_signed_count_forms() {
        check_statement_forms(
            &[
                "FETCH -5 c",
                "FETCH +5 c",
                "FETCH - 5 FROM c",
                "FETCH -5 IN c",
                "MOVE -5 c",
                "MOVE +5 FROM c",
                "FETCH ABSOLUTE -1 c",
                "FETCH RELATIVE +1 IN c",
                "FETCH FORWARD -1 c",
                "FETCH FORWARD +1 FROM c",
                "FETCH BACKWARD -1 c",
                "MOVE BACKWARD +1 IN c",
                "FETCH FORWARD ALL c",
                "FETCH ALL c",
            ],
            &[
                "FETCH - c",
                "FETCH + c",
                "FETCH +- 5 c",
                "FETCH -5.0 c",
                "FETCH ABSOLUTE -5.0 c",
                "FETCH FORWARD -1.5 c",
                "FETCH NEXT -1 c",
                "FETCH PRIOR -1 c",
                "FETCH FIRST -1 c",
                "FETCH LAST -1 c",
                "FETCH ALL -1 c",
                "FETCH FORWARD ALL -1 c",
                "MOVE NEXT -1 c",
                "FETCH -5 -5 c",
            ],
        );
    }
}
