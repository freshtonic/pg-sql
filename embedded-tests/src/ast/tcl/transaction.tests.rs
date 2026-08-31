#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_end_stmt() {
        let lexed = crate::lex("END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = EndStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_abort_stmt() {
        let lexed = crate::lex("ABORT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AbortStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_abort_work() {
        let lexed = crate::lex("ABORT WORK");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AbortStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_start_transaction_read_write() {
        let lexed = crate::lex("START TRANSACTION READ WRITE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = StartTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_transaction_modes() {
        let lexed = crate::lex(
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY, DEFERRABLE",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_transaction_read_write() {
        let lexed = crate::lex("SET TRANSACTION READ WRITE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_characteristics() {
        let lexed = crate::lex("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_transaction_snapshot() {
        let lexed = crate::lex("SET TRANSACTION SNAPSHOT 'FFF-FFF-F'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTransactionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_constraints_all_deferred() {
        let lexed = crate::lex("SET CONSTRAINTS ALL DEFERRED");
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
        let lexed = crate::lex("SET CONSTRAINTS fkpart3.fkey DEFERRED");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_constraints_multiple_names_immediate() {
        let lexed = crate::lex("SET CONSTRAINTS schema_a.c1, schema_b.c2 IMMEDIATE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetConstraintsStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_begin_isolation() {
        let lexed = crate::lex("BEGIN ISOLATION LEVEL SERIALIZABLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = BeginStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn commit_bare_has_no_body() {
        let lexed = crate::lex("COMMIT");
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
        let lexed = crate::lex("ROLLBACK");
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
