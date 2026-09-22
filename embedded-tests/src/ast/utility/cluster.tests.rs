#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    // The 17 shape of `ClusterStmt`: research, PostgreSQL 17, "Changes to existing statements".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn cluster_bare_is_modelled() {
        let stmt = parse_stmt::<ClusterStmt>("CLUSTER");
        let stmt = stmt.ast();
        assert!(stmt.options.is_none());
        assert!(!stmt.verbose);
        assert!(stmt.target.is_none());
        reparse_stable::<ClusterStmt>("CLUSTER");
    }

    #[test]
    fn cluster_table_only_roundtrips() {
        reparse_stable::<ClusterStmt>("CLUSTER clstr_2");
    }

    #[test]
    fn cluster_table_using_index_roundtrips() {
        reparse_stable::<ClusterStmt>("CLUSTER clstr_2 USING clstr_2_pkey");
    }

    // The 17 shape of `ClusterStmt`: see `cluster_bare_is_modelled`.
    #[cfg(feature = "since-pg17")]
    #[test]
    fn cluster_verbose_roundtrips() {
        let stmt = parse_stmt::<ClusterStmt>("CLUSTER VERBOSE clstr_2");
        let stmt = stmt.ast();
        assert!(stmt.verbose);
        reparse_stable::<ClusterStmt>("CLUSTER VERBOSE clstr_2");
    }

    // The 17 shape of `ClusterStmt`: see `cluster_bare_is_modelled`.
    #[cfg(feature = "since-pg17")]
    #[test]
    fn cluster_options_roundtrips() {
        let stmt = parse_stmt::<ClusterStmt>("CLUSTER (VERBOSE) clstr_2 USING clstr_2_pkey");
        let stmt = stmt.ast();
        assert!(stmt.options.is_some());
        reparse_stable::<ClusterStmt>("CLUSTER (VERBOSE) clstr_2 USING clstr_2_pkey");
    }

    #[test]
    fn cluster_legacy_index_on_table_roundtrips() {
        // pre-8.3 form: CLUSTER [VERBOSE] index ON table
        reparse_stable::<ClusterStmt>("CLUSTER clstr_2_pkey ON clstr_2");
    }

    // `CLUSTER (options)` with no table is added in 17: research, PostgreSQL 17, "Changes to
    // existing statements" (REL_17_11 gram.y 11739).
    #[cfg(feature = "since-pg17")]
    #[test]
    fn cluster_options_without_a_table_roundtrips() {
        let stmt = parse_stmt::<ClusterStmt>("CLUSTER (VERBOSE)");
        let stmt = stmt.ast();
        assert!(stmt.options.is_some());
        assert!(stmt.target.is_none());
        reparse_stable::<ClusterStmt>("CLUSTER (VERBOSE)");
    }

    // Before 17, the option list and its table are one form: research, PostgreSQL 17,
    // "Changes to existing statements" (REL_16_15 gram.y 11570-11600).
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn cluster_forms_before_17() {
        let stmt = parse_stmt::<ClusterStmt>("CLUSTER");
        let ClusterStmt::Plain(plain) = stmt.ast() else {
            panic!("bare CLUSTER has no option list");
        };
        assert!(!plain.verbose);
        assert!(plain.target.is_none());

        let stmt = parse_stmt::<ClusterStmt>("CLUSTER VERBOSE clstr_2");
        let ClusterStmt::Plain(plain) = stmt.ast() else {
            panic!("CLUSTER VERBOSE has no option list");
        };
        assert!(plain.verbose);

        let stmt = parse_stmt::<ClusterStmt>("CLUSTER (VERBOSE) clstr_2 USING clstr_2_pkey");
        let ClusterStmt::Options(options) = stmt.ast() else {
            panic!("expected the option-list form");
        };
        assert!(options.target.using_index.is_some());
        reparse_stable::<ClusterStmt>("CLUSTER (VERBOSE) clstr_2 USING clstr_2_pkey");
        reparse_stable::<ClusterStmt>("CLUSTER VERBOSE clstr_2_pkey ON clstr_2");
    }

    // Removed before 17: research, PostgreSQL 17, "Changes to existing statements".
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn cluster_options_without_a_table_is_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "CLUSTER (VERBOSE)",
            "CLUSTER (VERBOSE) clstr_2_pkey ON clstr_2",
        ]);
    }
}
