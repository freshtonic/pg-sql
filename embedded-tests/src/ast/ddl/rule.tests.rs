#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_rule_rename() {
        let lexed = crate::lex("ALTER RULE InsertRule ON rule_v1 RENAME TO NewInsertRule");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterRuleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_rule_nothing() {
        let stmt: CreateRuleStmt =
            parse_stmt("CREATE RULE r AS ON INSERT TO tbl DO INSTEAD NOTHING");
        assert_eq!(stmt.name.text(), "r");
        assert_eq!(stmt.table.object(), "tbl");
        assert!(!stmt.or_replace);
        assert!(matches!(stmt.event, RuleEvent::Insert));
        assert!(matches!(
            stmt.do_clause.instead_also,
            Some(RuleInsteadAlso::Instead)
        ));
        assert!(matches!(stmt.do_clause.actions, RuleActions::Nothing));
    }

    #[test]
    fn create_or_replace_rule_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE OR REPLACE RULE r AS ON INSERT TO tbl DO INSTEAD INSERT INTO other VALUES (1)",
        );
    }

    #[test]
    fn create_rule_with_where_clause_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE RULE r AS ON UPDATE TO tbl WHERE NEW.a <> OLD.a DO INSERT INTO log VALUES (NEW.a)",
        );
    }

    #[test]
    fn create_rule_select_event_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            r#"CREATE RULE "_RETURN" AS ON SELECT TO v DO INSTEAD SELECT 1"#,
        );
    }

    #[test]
    fn create_rule_do_also_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE RULE r AS ON DELETE TO tbl DO ALSO DELETE FROM other WHERE a = OLD.a",
        );
    }

    #[test]
    fn create_rule_multi_action_paren_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE RULE r AS ON UPDATE TO tbl DO ALSO (UPDATE other SET a = NEW.a; DELETE FROM log WHERE a = OLD.a)",
        );
    }
}
