#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_trigger_on_table() {
        let lexed = crate::lex("DROP TRIGGER IF EXISTS trg ON my_table CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropTriggerStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.name.text(), "trg");
        assert_eq!(stmt.table.object(), "my_table");
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_event_trigger() {
        let lexed = crate::lex("DROP EVENT TRIGGER et1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropEventTriggerStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_trigger_rename() {
        let lexed = crate::lex(
            "ALTER TRIGGER modified_a ON main_table RENAME TO modified_modified_a",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTriggerStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_trigger_minimal() {
        let stmt = parse_stmt::<CreateTriggerStmt>("CREATE TRIGGER t BEFORE INSERT ON tbl FOR EACH ROW EXECUTE PROCEDURE f()");
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "t");
        assert!(matches!(stmt.timing, TriggerActionTime::Before));
        assert_eq!(stmt.table.object(), "tbl");
        assert!(!stmt.or_replace);
        assert!(stmt.referencing.is_none());
        assert!(stmt.when_clause.is_none());
    }

    #[test]
    fn parse_create_or_replace_trigger_modelled() {
        let stmt = parse_stmt::<CreateTriggerStmt>(
            "CREATE OR REPLACE TRIGGER my_trig BEFORE INSERT ON my_table FOR EACH ROW EXECUTE PROCEDURE funcB()",
        );
        let stmt = stmt.ast();
        assert!(stmt.or_replace);
        assert_eq!(stmt.name.text(), "my_trig");
    }

    #[test]
    fn create_trigger_after_update_or_delete_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER UPDATE OR DELETE ON tbl FOR EACH STATEMENT EXECUTE FUNCTION f()",
        );
    }

    #[test]
    fn create_trigger_update_of_columns_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t BEFORE UPDATE OF a, b ON tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_instead_of_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t INSTEAD OF INSERT ON v FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_truncate_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t BEFORE TRUNCATE ON tbl FOR EACH STATEMENT EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_when_clause_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER INSERT ON tbl FOR EACH ROW WHEN (NEW.a = 123) EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_referencing_old_new_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER UPDATE ON tbl REFERENCING OLD TABLE AS oldtable NEW TABLE AS newtable FOR EACH STATEMENT EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_trigger_with_args_roundtrips() {
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t BEFORE INSERT ON tbl FOR EACH STATEMENT EXECUTE PROCEDURE f('hello', 42)",
        );
    }

    #[test]
    fn create_trigger_default_for_each_roundtrips() {
        // `FOR EACH ROW`/`STATEMENT` is optional — when omitted, defaults to
        // STATEMENT per the SQL standard. Our AST mirrors source verbatim.
        reparse_stable::<CreateTriggerStmt>(
            "CREATE TRIGGER t AFTER UPDATE ON tbl EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn parse_create_constraint_trigger_minimal() {
        let stmt = parse_stmt::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "t");
        assert_eq!(stmt.table.object(), "tbl");
        assert!(stmt.constraint_attrs.is_empty());
    }

    #[test]
    fn create_constraint_trigger_initially_deferred_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER UPDATE ON tbl INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION f()",
        );
    }

    #[test]
    fn create_constraint_trigger_deferrable_initially_deferred_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_constraint_trigger_multi_event_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT OR UPDATE OR DELETE ON s.tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_constraint_trigger_from_table_roundtrips() {
        reparse_stable::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FROM other DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE PROCEDURE f()",
        );
    }

    // The raw parser rejects NOT VALID and NO INHERIT on a CONSTRAINT
    // TRIGGER in every version: `processCASbits` gets no pointer for them
    // (REL_17_11 gram.y 5947-5949; REL_18_6 gram.y 6062-6064), and in 19 the
    // `CreateTrigStmt` action rejects them (b73d13c gram.y 6128-6139).
    #[test]
    fn constraint_trigger_rejects_not_valid_and_no_inherit() {
        check_statement_forms(
            &[
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl NOT DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION f()",
            ],
            &[
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl NOT VALID FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl NO INHERIT FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl DEFERRABLE NOT VALID FOR EACH ROW EXECUTE FUNCTION f()",
            ],
        );
    }

    // Added in 19: `CREATE CONSTRAINT TRIGGER ... ENFORCED`
    // (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 19,
    // "Changes to existing statements"; commit 87251e11496; b73d13c gram.y
    // 6126-6145 and 6164-6166). NOT ENFORCED stays rejected.
    #[cfg(feature = "since-pg19")]
    #[test]
    fn constraint_trigger_enforced_from_19() {
        let stmt = parse_stmt::<CreateConstraintTriggerStmt>(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl ENFORCED FOR EACH ROW EXECUTE FUNCTION f()",
        );
        let stmt = stmt.ast();
        assert_eq!(stmt.constraint_attrs.len(), 1);
        assert!(matches!(stmt.constraint_attrs[0], ConstraintTriggerAttr::Enforced));
        check_statement_forms(
            &[
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl ENFORCED FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FROM o DEFERRABLE ENFORCED INITIALLY DEFERRED FOR EACH ROW EXECUTE PROCEDURE f()",
            ],
            &[
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl NOT ENFORCED FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl ENFORCED NOT VALID FOR EACH ROW EXECUTE FUNCTION f()",
            ],
        );
    }

    // Before 19 the raw parser rejects ENFORCED and NOT ENFORCED on a
    // CONSTRAINT TRIGGER. 18 has the keyword, but `processCASbits` gets no
    // `is_enforced` pointer (REL_18_6 gram.y 6062-6064 and 19513-19546).
    #[cfg(not(feature = "since-pg19"))]
    #[test]
    fn constraint_trigger_enforced_is_rejected_before_19() {
        check_statement_forms(
            &[],
            &[
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl ENFORCED FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl NOT ENFORCED FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl DEFERRABLE ENFORCED FOR EACH ROW EXECUTE FUNCTION f()",
            ],
        );
    }

    #[test]
    fn parse_create_event_trigger_minimal() {
        let stmt = parse_stmt::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER undroppable ON sql_drop EXECUTE PROCEDURE undroppable()",
        );
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "undroppable");
        assert!(stmt.when_filters.is_none());
    }

    #[test]
    fn create_event_trigger_when_tag_in_roundtrips() {
        reparse_stable::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER t ON sql_drop WHEN TAG IN ('drop table', 'drop function') EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_event_trigger_when_multi_filter_roundtrips() {
        reparse_stable::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER t ON ddl_command_start WHEN TAG IN ('CREATE TABLE') AND TAG IN ('ALTER TABLE') EXECUTE PROCEDURE f()",
        );
    }

    #[test]
    fn create_event_trigger_execute_function_roundtrips() {
        reparse_stable::<CreateEventTriggerStmt>(
            "CREATE EVENT TRIGGER t ON ddl_command_end EXECUTE FUNCTION f()",
        );
    }

    // The `CreateTrigStmt` action stops on `OR REPLACE` in the constraint arm
    // ("CREATE OR REPLACE CONSTRAINT TRIGGER is not supported"), so the raw
    // parser rejects it in every target version (REL_14_24 gram.y
    // `CreateTrigStmt`; b73d13c gram.y 6096). The plain-trigger arm keeps it.
    #[test]
    fn constraint_trigger_rejects_or_replace() {
        check_statement_forms(
            &[
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FOR EACH ROW EXECUTE FUNCTION f()",
                "CREATE OR REPLACE TRIGGER t AFTER INSERT ON tbl FOR EACH ROW EXECUTE FUNCTION f()",
            ],
            &[
                "CREATE OR REPLACE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FOR EACH ROW EXECUTE FUNCTION f()",
            ],
        );
    }
}
