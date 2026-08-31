#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn truncate_bare_is_modelled() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE t");
        assert_eq!(stmt.relations.len(), 1);
        assert!(stmt.restart_seqs.is_none());
        assert!(stmt.behavior.is_none());
        reparse_stable::<TruncateStmt>("TRUNCATE t");
    }

    #[test]
    fn truncate_with_table_keyword_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE TABLE pk");
    }

    #[test]
    fn truncate_only_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE ONLY trunc_f");
        assert_eq!(stmt.relations.len(), 1);
        reparse_stable::<TruncateStmt>("TRUNCATE ONLY trunc_f");
    }

    #[test]
    fn truncate_multiple_relations_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE ONLY trunc_fb, ONLY trunc_fa");
        assert_eq!(stmt.relations.len(), 2);
        reparse_stable::<TruncateStmt>("TRUNCATE ONLY trunc_fb, ONLY trunc_fa");
    }

    #[test]
    fn truncate_cascade_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE TABLE truncate_a CASCADE");
        assert!(stmt.behavior.is_some());
        reparse_stable::<TruncateStmt>("TRUNCATE TABLE truncate_a CASCADE");
    }

    #[test]
    fn truncate_restrict_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE TABLE truncate_a RESTRICT");
    }

    #[test]
    fn truncate_restart_identity_roundtrips() {
        let stmt: TruncateStmt = parse_stmt("TRUNCATE truncate_a RESTART IDENTITY");
        assert!(stmt.restart_seqs.is_some());
        reparse_stable::<TruncateStmt>("TRUNCATE truncate_a RESTART IDENTITY");
    }

    #[test]
    fn truncate_continue_identity_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE truncate_a CONTINUE IDENTITY");
    }

    #[test]
    fn truncate_restart_identity_cascade_roundtrips() {
        reparse_stable::<TruncateStmt>("TRUNCATE truncate_a RESTART IDENTITY CASCADE");
    }
}
