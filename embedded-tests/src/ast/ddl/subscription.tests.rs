#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_subscription() {
        let lexed = crate::lex("DROP SUBSCRIPTION sub1 CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropSubscriptionStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.text(), "sub1");
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    /// `ALTER SUBSCRIPTION name SET (origin = 'value')` — string-valued
    /// def_arg, sanity test for the SetDef path.
    #[test]
    fn parse_alter_subscription_set_origin_string() {
        let lexed = crate::lex("ALTER SUBSCRIPTION regress_testsub4 SET (origin = 'none')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterSubscriptionStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// `ALTER SUBSCRIPTION name SET (origin = any)` — `any` is a reserved
    /// keyword used as a `def_arg` value (gram.y `def_arg` accepts
    /// `reserved_keyword`). subscription.sql corpus uses this.
    #[test]
    fn parse_alter_subscription_set_origin_any() {
        let lexed = crate::lex("ALTER SUBSCRIPTION regress_testsub4 SET (origin = any)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterSubscriptionStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn create_subscription_connection_publication_roundtrips() {
        let stmt = parse_stmt::<CreateSubscriptionStmt>(
            "CREATE SUBSCRIPTION regress_testsub CONNECTION 'testconn' PUBLICATION testpub WITH (connect = false)",
        );
        let stmt = stmt.ast();
        assert_eq!(stmt.name.text(), "regress_testsub");
        assert_eq!(stmt.publication_clause.names.len(), 1);
        assert!(stmt.with.is_some());
        reparse_stable::<CreateSubscriptionStmt>(
            "CREATE SUBSCRIPTION regress_testsub CONNECTION 'testconn' PUBLICATION testpub WITH (connect = false)",
        );
    }

    #[test]
    fn create_subscription_multi_publication_roundtrips() {
        reparse_stable::<CreateSubscriptionStmt>(
            "CREATE SUBSCRIPTION s CONNECTION 'dbname=x' PUBLICATION p1, p2, p3 WITH (connect = false)",
        );
    }

    /// Added in 19: `CREATE SUBSCRIPTION name SERVER name PUBLICATION ...`,
    /// `ALTER SUBSCRIPTION name SERVER name` and `ALTER SUBSCRIPTION name
    /// REFRESH SEQUENCES` (gram.y b73d13c:11045, 11084, 11104; research
    /// PostgreSQL 19, "Changes to existing statements").
    #[cfg(feature = "since-pg19")]
    #[test]
    fn subscription_server_and_refresh_sequences_from_19() {
        crate::ast::test_support::check_statement_forms(
            &[
                "CREATE SUBSCRIPTION s SERVER fsrv PUBLICATION p",
                "CREATE SUBSCRIPTION s SERVER fsrv PUBLICATION p, q WITH (enabled = false)",
                "ALTER SUBSCRIPTION s SERVER fsrv",
                "ALTER SUBSCRIPTION s REFRESH SEQUENCES",
                "ALTER SUBSCRIPTION s REFRESH PUBLICATION",
            ],
            &[
                "CREATE SUBSCRIPTION s SERVER fsrv CONNECTION 'x' PUBLICATION p",
                "CREATE SUBSCRIPTION s SERVER 'fsrv' PUBLICATION p",
                "ALTER SUBSCRIPTION s REFRESH SEQUENCES WITH (copy_data = false)",
                "ALTER SUBSCRIPTION s SERVER",
            ],
        );
    }

    /// Before 19 a subscription has only `CONNECTION` and `REFRESH
    /// PUBLICATION` (the same research entry).
    #[cfg(not(feature = "since-pg19"))]
    #[test]
    fn reject_subscription_server_before_19() {
        crate::ast::test_support::check_statement_forms(
            &["CREATE SUBSCRIPTION s CONNECTION 'x' PUBLICATION p"],
            &[
                "CREATE SUBSCRIPTION s SERVER fsrv PUBLICATION p",
                "ALTER SUBSCRIPTION s SERVER fsrv",
                "ALTER SUBSCRIPTION s REFRESH SEQUENCES",
            ],
        );
    }
}
