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
        let _stmt_parsed = AlterRuleStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_rule_nothing() {
        let stmt = parse_stmt::<CreateRuleStmt>("CREATE RULE r AS ON INSERT TO tbl DO INSTEAD NOTHING");
        let stmt = stmt.ast();
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
    /// gram.y `RuleActionList: NOTHING | RuleActionStmt | '(' RuleActionMulti
    /// ')'`: a `(` after `DO [ALSO | INSTEAD]` opens the action list, so a single
    /// action cannot lead with a parenthesized query. Nothing else is
    /// excluded: the right operand of a set operation is a full
    /// `select_clause`, and PostgreSQL 17.9 accepts `DO ALSO SELECT 1 UNION
    /// (SELECT 2)`. The nesting is the one `SelectClause` declares.
    #[test]
    fn parse_rule_action_set_operations_with_parenthesized_right_operands() {
        use crate::ast::dml::values::SelectClause;

        fn shape(clause: &SelectClause<'_>) -> String {
            match clause {
                SelectClause::Union(l, _, r) => format!("Union({},{})", shape(l), shape(r)),
                SelectClause::Except(l, _, r) => format!("Except({},{})", shape(l), shape(r)),
                SelectClause::Intersect(l, _, r) => {
                    format!("Intersect({},{})", shape(l), shape(r))
                }
                SelectClause::Select(_) => "s".to_owned(),
                SelectClause::Values(_) => "v".to_owned(),
                SelectClause::Table(_) => "t".to_owned(),
                SelectClause::Parens(parens) => format!("({})", shape(&parens.inner.body.clause)),
            }
        }

        for (src, expected) in [
            ("SELECT 1 UNION (SELECT 2)", "Union(s,(s))"),
            ("SELECT 1 UNION (SELECT 2) UNION SELECT 3", "Union(Union(s,(s)),s)"),
            ("SELECT 1 INTERSECT (SELECT 2 UNION SELECT 3)", "Intersect(s,(Union(s,s)))"),
            ("SELECT 1 UNION SELECT 2 INTERSECT (SELECT 3)", "Union(s,Intersect(s,(s)))"),
            ("SELECT 1 INTERSECT SELECT 2 UNION SELECT 3", "Union(Intersect(s,s),s)"),
            ("VALUES (1) EXCEPT ((SELECT 2))", "Except(v,((s)))"),
            ("TABLE t UNION ALL (TABLE u)", "Union(t,(t))"),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed =
                RuleQuery::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            assert_eq!(shape(&parsed.ast().clause), expected, "for {src:?}");
        }

        for src in [
            "CREATE RULE r AS ON INSERT TO t DO ALSO SELECT 1 UNION (SELECT 2)",
            "CREATE RULE r AS ON INSERT TO t DO INSTEAD SELECT 1 EXCEPT (SELECT 2) ORDER BY 1",
            "CREATE RULE r AS ON INSERT TO t DO ALSO (SELECT 1 UNION (SELECT 2))",
            "CREATE RULE r AS ON INSERT TO t DO ALSO (SELECT 1; SELECT 2 UNION (SELECT 3))",
            "CREATE RULE r AS ON INSERT TO t DO INSTEAD (SELECT 1)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            CreateRuleStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

}
