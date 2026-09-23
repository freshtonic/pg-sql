// MERGE is added in 15: research, PostgreSQL 15, "New statements"
// (REL_15_19 gram.y `MergeStmt`).
#[cfg(feature = "since-pg15")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_merge_basic() {
        let sql = "MERGE INTO m USING (select 0 k, 'v' v) o ON m.k = o.k WHEN MATCHED THEN UPDATE SET v = 'updated' WHEN NOT MATCHED THEN INSERT VALUES(o.k, o.v)";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = MergeStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
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
        let _stmt_parsed = MergeStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_when_matched_and() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED AND t.a = 2 THEN UPDATE SET b = s.b";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = MergeStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    // `BY SOURCE` and `RETURNING` are added in 17: research, PostgreSQL 17,
    // "Changes to existing statements".
    #[cfg(feature = "since-pg17")]
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
                ;
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
        let _stmt_parsed = MergeStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    // `BY SOURCE`, `BY TARGET` and `RETURNING` are added in 17: research, PostgreSQL 17,
    // "Changes to existing statements".
    #[cfg(feature = "since-pg17")]
    /// gram.y:12459 `merge_when_clause` pairs each match kind with its actions:
    /// `WHEN MATCHED` and `WHEN NOT MATCHED BY SOURCE` (gram.y:12503
    /// `merge_when_tgt_matched`) update, delete or do nothing; `WHEN NOT
    /// MATCHED [BY TARGET]` (gram.y:12508) inserts or does nothing.
    /// `merge_insert` (gram.y:12544) has no `INTO`, and `DEFAULT VALUES` takes
    /// no column list and no `OVERRIDING`; `merge_values_clause`
    /// (gram.y:12592) is one row. merge.sql tests these as syntax errors, and
    /// PostgreSQL 17.9 agrees with every line here.
    #[test]
    fn parse_merge_when_clauses_with_gram_y_actions() {
        use crate::ast::dml::merge::{InsertAction, MatchedKind, NotMatchedAction, WhenClause};

        let head = "MERGE INTO t USING s ON t.k = s.k ";
        for tail in [
            "WHEN MATCHED THEN UPDATE SET v = s.v",
            "WHEN MATCHED THEN DELETE",
            "WHEN MATCHED AND s.v > 1 THEN DO NOTHING",
            "WHEN NOT MATCHED BY SOURCE THEN UPDATE SET v = 1",
            "WHEN NOT MATCHED BY SOURCE THEN DELETE",
            "WHEN NOT MATCHED BY SOURCE AND t.v > 1 THEN DO NOTHING",
            "WHEN NOT MATCHED THEN INSERT VALUES (s.k, s.v)",
            "WHEN NOT MATCHED THEN INSERT (k, v) VALUES (s.k, s.v)",
            "WHEN NOT MATCHED THEN INSERT (k) OVERRIDING SYSTEM VALUE VALUES (1)",
            "WHEN NOT MATCHED THEN INSERT OVERRIDING USER VALUE VALUES (1)",
            "WHEN NOT MATCHED THEN INSERT DEFAULT VALUES",
            "WHEN NOT MATCHED BY TARGET THEN INSERT VALUES (1)",
            "WHEN NOT MATCHED BY TARGET AND s.v > 1 THEN DO NOTHING",
            "WHEN MATCHED THEN DELETE WHEN NOT MATCHED BY SOURCE THEN UPDATE SET v = 0 \
             WHEN NOT MATCHED THEN INSERT VALUES (s.k, s.v) RETURNING merge_action(), t.*",
        ] {
            let src: &'static str = format!("{head}{tail}").leak();
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            MergeStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }

        // The tree names the match kind and the action's shape.
        let lexed = crate::lex(
            "MERGE INTO t USING s ON true WHEN NOT MATCHED BY SOURCE THEN DELETE \
             WHEN NOT MATCHED BY TARGET THEN INSERT (k) VALUES (1) \
             WHEN NOT MATCHED THEN INSERT DEFAULT VALUES",
        );
        let mut input = lexed.input();
        let parsed = MergeStmt::parse(&mut input).unwrap();
        let clauses = parsed.ast().when_clauses.as_slice();
        assert!(matches!(
            clauses,
            [WhenClause::Matched(source), WhenClause::NotMatched(target), WhenClause::NotMatched(plain)]
                if matches!(source.kind, MatchedKind::NotMatchedBySource)
                    && target.kind.by_target
                    && !plain.kind.by_target
                    && matches!(&target.action, NotMatchedAction::Insert(InsertAction::Values(values)) if values.columns.is_some())
                    && matches!(&plain.action, NotMatchedAction::Insert(InsertAction::Default))
        ));

        for tail in [
            "WHEN MATCHED THEN INSERT VALUES (1)",
            "WHEN MATCHED THEN INSERT DEFAULT VALUES",
            "WHEN NOT MATCHED BY SOURCE THEN INSERT VALUES (1)",
            "WHEN NOT MATCHED BY SOURCE THEN INSERT DEFAULT VALUES",
            "WHEN NOT MATCHED THEN UPDATE SET v = 1",
            "WHEN NOT MATCHED THEN DELETE",
            "WHEN NOT MATCHED BY TARGET THEN UPDATE SET v = 1",
            "WHEN NOT MATCHED BY TARGET THEN DELETE",
            "WHEN NOT MATCHED THEN INSERT INTO t VALUES (1)",
            "WHEN NOT MATCHED THEN INSERT VALUES (1, 1), (2, 2)",
            "WHEN NOT MATCHED THEN INSERT VALUES ()",
            "WHEN NOT MATCHED THEN INSERT (k) DEFAULT VALUES",
            "WHEN NOT MATCHED THEN INSERT OVERRIDING SYSTEM VALUE DEFAULT VALUES",
            "WHEN MATCHED THEN",
            "",
        ] {
            let src: &'static str = format!("{head}{tail}").trim_end().to_owned().leak();
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = MergeStmt::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    // Before 17, the WHEN clauses are `WHEN MATCHED` and `WHEN NOT MATCHED`
    // only: research, PostgreSQL 17, "Changes to existing statements" (REL_16_15 gram.y
    // 12274-12315).
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn parse_merge_when_clauses_before_17() {
        let lexed = crate::lex(
            "MERGE INTO t USING s ON t.k = s.k WHEN MATCHED AND s.v > 1 THEN UPDATE SET v = s.v \
             WHEN MATCHED THEN DELETE WHEN NOT MATCHED THEN INSERT (k) VALUES (s.k) \
             WHEN NOT MATCHED THEN DO NOTHING",
        );
        let mut input = lexed.input();
        let parsed = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_eof());
        assert!(matches!(
            parsed.ast().when_clauses.as_slice(),
            [
                WhenClause::Matched(update),
                WhenClause::Matched(_),
                WhenClause::NotMatched(insert),
                WhenClause::NotMatched(_),
            ] if matches!(update.kind, MatchedKind::Matched)
                && matches!(&insert.action, NotMatchedAction::Insert(InsertAction::Values(_)))
        ));
    }

    // Added in 17, so rejected before 17: research, PostgreSQL 17, "Changes to existing
    // statements" (`BY SOURCE`, `BY TARGET`, `RETURNING`).
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn merge_17_clauses_are_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "MERGE INTO t USING s ON t.k = s.k WHEN NOT MATCHED BY SOURCE THEN DELETE",
            "MERGE INTO t USING s ON t.k = s.k WHEN NOT MATCHED BY SOURCE THEN DO NOTHING",
            "MERGE INTO t USING s ON t.k = s.k WHEN NOT MATCHED BY TARGET THEN INSERT DEFAULT VALUES",
            "MERGE INTO t USING s ON t.k = s.k WHEN MATCHED THEN DELETE RETURNING *",
            "MERGE INTO t USING s ON t.k = s.k WHEN MATCHED THEN DELETE RETURNING merge_action()",
            "WITH m AS (MERGE INTO t USING s ON true WHEN MATCHED THEN DELETE RETURNING *) SELECT 1",
        ]);
    }

    // The MERGE target is gram.y `relation_expr_opt_alias`, whose alias-free
    // rule has the `UMINUS` precedence, above `SET`'s, so `set` never opens
    // the bare alias (REL_17_11 gram.y 13801-13810). The target also takes no
    // column list, which `alias_clause` in a FROM item does.
    #[test]
    fn merge_bare_target_alias_never_takes_set() {
        crate::ast::test_support::check_statement_forms(
            &[
                "MERGE INTO t AS set USING u ON true WHEN MATCHED THEN DO NOTHING",
                "MERGE INTO t x USING u ON true WHEN MATCHED THEN DO NOTHING",
            ],
            &[
                "MERGE INTO t set USING u ON true WHEN MATCHED THEN DO NOTHING",
                "MERGE INTO t x (a) USING u ON true WHEN MATCHED THEN DO NOTHING",
            ],
        );
    }
}
