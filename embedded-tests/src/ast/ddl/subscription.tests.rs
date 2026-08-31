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
        let stmt = DropSubscriptionStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterSubscriptionStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterSubscriptionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn create_subscription_connection_publication_roundtrips() {
        let stmt: CreateSubscriptionStmt = parse_stmt(
            "CREATE SUBSCRIPTION regress_testsub CONNECTION 'testconn' PUBLICATION testpub WITH (connect = false)",
        );
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
}
