#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_merge_basic() {
        let sql = "MERGE INTO m USING (select 0 k, 'v' v) o ON m.k = o.k WHEN MATCHED THEN UPDATE SET v = 'updated' WHEN NOT MATCHED THEN INSERT VALUES(o.k, o.v)";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.when_clauses.len(), 2);
        assert!(input.is_eof());

        let sql = "MERGE INTO target USING source ON target.id = source.id";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(
            MergeStmt::parse(&mut input).is_err(),
            "PostgreSQL requires at least one MERGE WHEN clause",
        );
    }

    #[test]
    fn parse_merge_target_alias() {
        let sql = "MERGE INTO target t USING source s ON t.tid = s.sid WHEN MATCHED THEN DELETE";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_when_matched_and() {
        let sql =
            "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED AND t.a = 2 THEN UPDATE SET b = s.b";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_not_matched_by_source_default_values() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN NOT MATCHED BY SOURCE THEN INSERT DEFAULT VALUES";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// `WHEN NOT MATCHED BY SOURCE` accepts `UPDATE` / `DELETE` (PG17),
    /// and a MERGE may carry a `RETURNING` clause.
    #[test]
    fn parse_merge_not_matched_by_source_update_delete() {
        for src in [
            "MERGE INTO t USING s ON t.a = s.a \
             WHEN NOT MATCHED BY SOURCE THEN DELETE",
            "MERGE INTO t USING s ON t.a = s.a \
             WHEN NOT MATCHED BY SOURCE AND s.b = 1 THEN UPDATE SET b = 0",
            "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DELETE \
             RETURNING merge_action(), t.*",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            MergeStmt::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn parse_merge_do_nothing_both() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DO NOTHING WHEN NOT MATCHED THEN DO NOTHING";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_insert_multi_values() {
        let sql =
            "MERGE INTO t USING s ON t.a = s.a WHEN NOT MATCHED THEN INSERT VALUES (1,1), (2,2)";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_insert_into_default_values() {
        let sql = "MERGE INTO target t USING source s ON t.tid = s.sid WHEN NOT MATCHED THEN INSERT INTO target DEFAULT VALUES";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
