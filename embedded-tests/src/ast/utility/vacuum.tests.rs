#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_vacuum_full() {
        let lexed = crate::lex("VACUUM (FULL) tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = VacuumStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_vacuum_full_freeze() {
        let lexed = crate::lex("VACUUM (FULL, FREEZE) tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VacuumStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_vacuum_parallel_value() {
        let lexed = crate::lex("VACUUM (PARALLEL 2) tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = VacuumStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn vacuum_bare_is_modelled() {
        let stmt = parse_stmt::<VacuumStmt>("VACUUM");
        let stmt = stmt.ast();
        assert!(!stmt.full);
        assert!(!stmt.freeze);
        assert!(!stmt.verbose);
        assert!(!stmt.analyze);
        assert!(stmt.options.is_none());
        assert!(stmt.relations.is_none());
        reparse_stable::<VacuumStmt>("VACUUM");
    }

    #[test]
    fn vacuum_full_legacy_roundtrips() {
        let stmt = parse_stmt::<VacuumStmt>("VACUUM FULL vactst");
        let stmt = stmt.ast();
        assert!(stmt.full);
        reparse_stable::<VacuumStmt>("VACUUM FULL vactst");
    }

    #[test]
    fn vacuum_full_analyze_roundtrips() {
        let stmt = parse_stmt::<VacuumStmt>("VACUUM ANALYZE vacparted");
        let stmt = stmt.ast();
        assert!(stmt.analyze);
        reparse_stable::<VacuumStmt>("VACUUM ANALYZE vacparted");
    }

    #[test]
    fn vacuum_full_freeze_legacy_roundtrips() {
        reparse_stable::<VacuumStmt>("VACUUM FULL FREEZE vactst");
    }

    #[test]
    fn vacuum_verbose_analyze_roundtrips() {
        reparse_stable::<VacuumStmt>("VACUUM VERBOSE ANALYZE vactst");
    }

    #[test]
    fn vacuum_analyze_columns_roundtrips() {
        let stmt = parse_stmt::<VacuumStmt>("VACUUM ANALYZE vacparted(a, b, a)");
        let stmt = stmt.ast();
        assert_eq!(stmt.relations.as_ref().unwrap().len(), 1);
        reparse_stable::<VacuumStmt>("VACUUM ANALYZE vacparted(a, b, a)");
    }

    #[test]
    fn vacuum_multi_targets_roundtrips() {
        let stmt = parse_stmt::<VacuumStmt>("VACUUM ANALYZE vactst, vacparted (a)");
        let stmt = stmt.ast();
        assert_eq!(stmt.relations.as_ref().unwrap().len(), 2);
        reparse_stable::<VacuumStmt>("VACUUM ANALYZE vactst, vacparted (a)");
    }

    #[test]
    fn vacuum_options_with_target_roundtrips() {
        let stmt = parse_stmt::<VacuumStmt>("VACUUM (FULL, FREEZE) vactst");
        let stmt = stmt.ast();
        assert!(stmt.options.is_some());
        assert!(stmt.relations.is_some());
        reparse_stable::<VacuumStmt>("VACUUM (FULL, FREEZE) vactst");
    }

    #[test]
    fn vacuum_parallel_option_with_value_roundtrips() {
        reparse_stable::<VacuumStmt>("VACUUM (PARALLEL 2) pvactst");
    }
}
