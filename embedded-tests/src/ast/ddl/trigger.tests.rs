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
        let stmt = DropTriggerStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = DropEventTriggerStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterTriggerStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_trigger_minimal() {
        let stmt: CreateTriggerStmt =
            parse_stmt("CREATE TRIGGER t BEFORE INSERT ON tbl FOR EACH ROW EXECUTE PROCEDURE f()");
        assert_eq!(stmt.name.text(), "t");
        assert!(matches!(stmt.timing, TriggerActionTime::Before));
        assert_eq!(stmt.table.object(), "tbl");
        assert!(!stmt.or_replace);
        assert!(stmt.referencing.is_none());
        assert!(stmt.when_clause.is_none());
    }

    #[test]
    fn parse_create_or_replace_trigger_modelled() {
        let stmt: CreateTriggerStmt = parse_stmt(
            "CREATE OR REPLACE TRIGGER my_trig BEFORE INSERT ON my_table FOR EACH ROW EXECUTE PROCEDURE funcB()",
        );
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
        let stmt: CreateConstraintTriggerStmt = parse_stmt(
            "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON tbl FOR EACH ROW EXECUTE PROCEDURE f()",
        );
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

    #[test]
    fn parse_create_event_trigger_minimal() {
        let stmt: CreateEventTriggerStmt = parse_stmt(
            "CREATE EVENT TRIGGER undroppable ON sql_drop EXECUTE PROCEDURE undroppable()",
        );
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
}
