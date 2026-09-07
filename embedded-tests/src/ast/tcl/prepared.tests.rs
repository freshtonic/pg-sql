#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn prepare_plain_is_modelled() {
        let stmt = parse_stmt::<PrepareStmt>("PREPARE q1 AS SELECT 1 AS a");
        let stmt = stmt.ast();
        let body = match &stmt.body {
            PrepareStmtBody::Standard(s) => s,
            PrepareStmtBody::Transaction(_) => panic!("expected standard PREPARE body"),
        };
        assert_eq!(body.name.text(), "q1");
        assert!(body.types.is_none());
        assert_eq!(
            roundtrip::<PrepareStmt>("PREPARE q1 AS SELECT 1 AS a"),
            "PREPARE q1 AS SELECT 1 AS a"
        );
    }

    #[test]
    fn prepare_with_types_keeps_type_list() {
        let stmt = parse_stmt::<PrepareStmt>("PREPARE q2(text) AS SELECT $1");
        let stmt = stmt.ast();
        let body = match &stmt.body {
            PrepareStmtBody::Standard(s) => s,
            PrepareStmtBody::Transaction(_) => panic!("expected standard PREPARE body"),
        };
        assert!(body.types.is_some());
        reparse_stable::<PrepareStmt>("PREPARE q2(text) AS SELECT $1");
    }

    #[test]
    fn prepare_multiple_types_roundtrips() {
        let stmt = parse_stmt::<PrepareStmt>("PREPARE q3(text, int, boolean) AS SELECT $1");
        let stmt = stmt.ast();
        let body = match &stmt.body {
            PrepareStmtBody::Standard(s) => s,
            PrepareStmtBody::Transaction(_) => panic!("expected standard PREPARE body"),
        };
        assert!(body.types.is_some());
        reparse_stable::<PrepareStmt>("PREPARE q3(text, int, boolean) AS SELECT $1");
    }

    #[test]
    fn prepare_insert_is_modelled() {
        let stmt = parse_stmt::<PrepareStmt>("PREPARE p AS INSERT INTO t VALUES (1)");
        let stmt = stmt.ast();
        assert!(matches!(
            stmt.body,
            PrepareStmtBody::Standard(ref s) if matches!(s.body, PreparableStmt::Insert(_))
        ));
        reparse_stable::<PrepareStmt>("PREPARE p AS INSERT INTO t VALUES (1)");
    }

    /// `PREPARE TRANSACTION 'gid'` is the two-phase-commit transaction form
    /// (distinct from `PREPARE name [(types)] AS stmt`). The discriminator
    /// is the `TRANSACTION` keyword vs an identifier name after `PREPARE`.
    #[test]
    fn prepare_transaction_is_modelled() {
        let stmt = parse_stmt::<PrepareStmt>("PREPARE TRANSACTION 'regress_foo1'");
        let stmt = stmt.ast();
        assert!(matches!(stmt.body, PrepareStmtBody::Transaction(_)));
        assert_eq!(
            roundtrip::<PrepareStmt>("PREPARE TRANSACTION 'regress_foo1'"),
            "PREPARE TRANSACTION 'regress_foo1'"
        );
    }

    #[test]
    fn execute_plain_is_modelled() {
        let stmt = parse_stmt::<ExecuteStmt>("EXECUTE q1");
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "q1");
        assert!(stmt.params.is_none());
        assert_eq!(roundtrip::<ExecuteStmt>("EXECUTE q1"), "EXECUTE q1");
    }

    #[test]
    fn execute_with_params_keeps_params() {
        let stmt = parse_stmt::<ExecuteStmt>("EXECUTE q2('postgres')");
        let stmt = stmt.ast();
        assert!(stmt.params.is_some());
        reparse_stable::<ExecuteStmt>("EXECUTE q2('postgres')");
    }

    #[test]
    fn deallocate_name_is_modelled() {
        let stmt = parse_stmt::<DeallocateStmt>("DEALLOCATE q1");
        let stmt = stmt.ast();
        assert!(matches!(stmt.target, DeallocateTarget::Name(_)));
        assert_eq!(
            roundtrip::<DeallocateStmt>("DEALLOCATE q1"),
            "DEALLOCATE q1"
        );
    }

    #[test]
    fn deallocate_prepare_name_keeps_prepare() {
        assert_eq!(
            roundtrip::<DeallocateStmt>("DEALLOCATE PREPARE q1"),
            "DEALLOCATE PREPARE q1"
        );
    }

    #[test]
    fn deallocate_all_is_modelled() {
        let stmt = parse_stmt::<DeallocateStmt>("DEALLOCATE ALL");
        let stmt = stmt.ast();
        assert!(matches!(stmt.target, DeallocateTarget::All));
        assert_eq!(
            roundtrip::<DeallocateStmt>("DEALLOCATE ALL"),
            "DEALLOCATE ALL"
        );
    }
}
