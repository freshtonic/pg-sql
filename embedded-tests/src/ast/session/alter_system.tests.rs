#[cfg(test)]
mod tests {
    use crate::ast::session::alter_system::{AlterSystemAction, AlterSystemStmt};
    use crate::ast::session::set_reset::GenericReset;

    /// gram.y `AlterSystemStmt: ALTER SYSTEM_P SET generic_set`
    /// (REL_17_11:11485). `generic_set` is `var_name TO|= var_list` and
    /// `var_name TO|= DEFAULT`, so no `set_rest_more` special form and no
    /// `LOCAL` or `SESSION` scope belongs here. The production is the same in
    /// every target version.
    #[test]
    fn alter_system_set_generic_set() {
        crate::ast::test_support::check_statement_forms(
            &[
                "ALTER SYSTEM SET work_mem = '64MB'",
                "ALTER SYSTEM SET work_mem TO '64MB'",
                "ALTER SYSTEM SET enable_seqscan = off",
                "ALTER SYSTEM SET enable_seqscan TO on",
                "ALTER SYSTEM SET enable_sort = true",
                "ALTER SYSTEM SET extra_float_digits = -1",
                "ALTER SYSTEM SET seq_page_cost = 1.5",
                "ALTER SYSTEM SET search_path = a, b, c",
                "ALTER SYSTEM SET search_path TO DEFAULT",
                "ALTER SYSTEM SET search_path = DEFAULT",
                "ALTER SYSTEM SET \"Weird Name\" = 1",
                "ALTER SYSTEM SET a.b = 1",
                "ALTER SYSTEM SET \"a\".\"b\" = 1",
            ],
            &[
                "ALTER SYSTEM",
                "ALTER SYSTEM SET x",
                "ALTER SYSTEM SET LOCAL x = 1",
                "ALTER SYSTEM SET SESSION x = 1",
                "ALTER SYSTEM SET TIME ZONE 'UTC'",
                "ALTER SYSTEM SET ROLE bob",
                "ALTER SYSTEM SET SESSION AUTHORIZATION bob",
                "ALTER SYSTEM SET XML OPTION DOCUMENT",
                "ALTER SYSTEM SET SCHEMA 'public'",
                "ALTER SYSTEM SET NAMES 'utf8'",
                "ALTER SYSTEM SET x FROM CURRENT",
                "ALTER SYSTEM SET TRANSACTION ISOLATION LEVEL SERIALIZABLE",
                "ALTER SYSTEM SET x = 1 SET y = 2",
            ],
        );
    }

    /// gram.y `AlterSystemStmt: ALTER SYSTEM_P RESET generic_reset`
    /// (REL_17_11:11485). `generic_reset` is `var_name | ALL`, which is
    /// narrower than the `reset_rest` of a top-level `RESET`: the three
    /// spelled-out names are syntax errors after `ALTER SYSTEM`.
    #[test]
    fn alter_system_reset_generic_reset() {
        crate::ast::test_support::check_statement_forms(
            &[
                "ALTER SYSTEM RESET work_mem",
                "ALTER SYSTEM RESET ALL",
                "ALTER SYSTEM RESET a.b",
                "ALTER SYSTEM RESET a.b.c",
                "ALTER SYSTEM RESET \"Weird Name\"",
            ],
            &[
                "ALTER SYSTEM RESET",
                "ALTER SYSTEM RESET TIME ZONE",
                "ALTER SYSTEM RESET SESSION AUTHORIZATION",
                "ALTER SYSTEM RESET TRANSACTION ISOLATION LEVEL",
                "ALTER SYSTEM RESET ALL, x",
            ],
        );
    }

    /// The two halves reach the two `AlterSystemStmt` branches.
    #[test]
    fn alter_system_action_shapes() {
        let reset = crate::ast::test_support::parse_stmt::<AlterSystemStmt>("ALTER SYSTEM RESET ALL");
        let AlterSystemAction::Reset(reset) = &reset.ast().action else {
            panic!("RESET expected");
        };
        assert!(matches!(reset.target, GenericReset::All));

        let set = crate::ast::test_support::parse_stmt::<AlterSystemStmt>("ALTER SYSTEM SET work_mem = '64MB'");
        let AlterSystemAction::Set(set) = &set.ast().action else {
            panic!("SET expected");
        };
        assert_eq!(set.rest.param.object(), "work_mem");
    }
}
