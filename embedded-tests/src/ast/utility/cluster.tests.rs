#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

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

    #[test]
    fn cluster_verbose_roundtrips() {
        let stmt = parse_stmt::<ClusterStmt>("CLUSTER VERBOSE clstr_2");
        let stmt = stmt.ast();
        assert!(stmt.verbose);
        reparse_stable::<ClusterStmt>("CLUSTER VERBOSE clstr_2");
    }

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
}
