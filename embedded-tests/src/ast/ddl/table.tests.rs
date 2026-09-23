#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_create_table_identity_seq_options() {
        let lexed = crate::lex(
            "CREATE TABLE t (id int GENERATED ALWAYS AS IDENTITY (START WITH 44))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_temp_table_on_commit() {
        for src in [
            "CREATE TEMP TABLE t (a int) ON COMMIT PRESERVE ROWS",
            "CREATE TEMP TABLE t (a int) ON COMMIT DELETE ROWS",
            "CREATE TEMP TABLE t (a int) ON COMMIT DROP",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
            let _stmt = _stmt_parsed.ast();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_create_table_single_column() {
        let lexed = crate::lex("CREATE TABLE BOOLTBL1 (f1 bool)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.object(), "BOOLTBL1");
        assert_eq!(stmt.items().unwrap().len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_multiple_columns() {
        let lexed = crate::lex("CREATE TABLE BOOLTBL3 (d text, b bool, o int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.object(), "BOOLTBL3");
        assert_eq!(stmt.items().unwrap().len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_ctas_with_column_list() {
        // Regression: matview.sql uses `CREATE TABLE foo(a, b) AS VALUES(1, 10)`.
        let lexed = crate::lex("CREATE TABLE mvtest_foo(a, b) AS VALUES(1, 10)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(
            stmt.body,
            super::CreateTableBody::ColumnsAsQuery(_)
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_time_zone_types() {
        // Regression: brin.sql brintest table uses `time without time zone`,
        // `timestamp with time zone`, `bit varying(16)` as column types.
        let lexed = crate::lex(
            "CREATE TABLE t (a time without time zone, b timestamp with time zone, c time with time zone, d timestamp without time zone, e bit varying(16), f bit(10), g character)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.items().unwrap().len(), 7);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_array_column_types() {
        let lexed = crate::lex("CREATE TABLE t (a int2[], b int4[][][], c varchar(5)[], d text[])");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.items().unwrap().len(), 4);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_boolean_type() {
        let lexed = crate::lex("CREATE TABLE t (f1 boolean)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.items().unwrap().len(), 1);
    }

    #[test]
    fn parse_create_temp_table() {
        let lexed = crate::lex("CREATE TEMP TABLE foo (f1 int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.temp.is_some());
        assert_eq!(stmt.name.object(), "foo");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_partitioned_table() {
        let lexed = crate::lex("create table list_parted_tbl (a int,b int) partition by list (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.object(), "list_parted_tbl");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_partition_of() {
        let lexed = crate::lex(
            "create table list_parted_tbl1 partition of list_parted_tbl for values in (1) partition by list(b)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.object(), "list_parted_tbl1");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_check_constraint() {
        let lexed = crate::lex("CREATE TABLE t (a int CHECK (a > 0))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_references_full() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int REFERENCES other(id) MATCH FULL ON DELETE CASCADE ON UPDATE NO ACTION DEFERRABLE INITIALLY DEFERRED)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_named_constraint() {
        let lexed = crate::lex("CREATE TABLE t (a int CONSTRAINT pos CHECK (a > 0))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_default_constraint() {
        let lexed = crate::lex("CREATE TABLE t (a int DEFAULT 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_primary_key() {
        let lexed = crate::lex("CREATE TABLE t (a int, b int, PRIMARY KEY (a, b))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_unique() {
        let lexed = crate::lex("CREATE TABLE t (a int, UNIQUE (a))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_foreign_key() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES other(id) ON DELETE SET NULL)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    // The column list is added in 15: research, PostgreSQL 15, "Changes to
    // existing statements" (REL_15_19 gram.y `key_action`).
    #[cfg(feature = "since-pg15")]
    #[test]
    fn parse_table_foreign_key_set_null_columns() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, b int, FOREIGN KEY (a, b) REFERENCES p ON DELETE SET NULL (b))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    // The column list is added in 15: research, PostgreSQL 15, "Changes to
    // existing statements" (REL_15_19 gram.y `key_action`).
    #[cfg(feature = "since-pg15")]
    #[test]
    fn parse_table_foreign_key_set_default_columns() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES p ON UPDATE SET DEFAULT (a))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check() {
        let lexed = crate::lex("CREATE TABLE t (a int, CHECK (a > 0))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_named_constraint() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, b int, CONSTRAINT pk PRIMARY KEY (a, b) DEFERRABLE INITIALLY IMMEDIATE)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check_no_inherit() {
        let lexed = crate::lex("CREATE TABLE t (a int, CHECK (a > 0) NO INHERIT)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_bare() {
        let lexed = crate::lex("CREATE TABLE foo (LIKE bar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_including_all() {
        let lexed = crate::lex("CREATE TABLE foo (LIKE bar INCLUDING ALL)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_including_excluding() {
        let lexed = crate::lex(
            "CREATE TABLE foo (LIKE bar INCLUDING DEFAULTS EXCLUDING CONSTRAINTS)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_mixed_with_columns() {
        let lexed = crate::lex("CREATE TABLE foo (a int, LIKE bar INCLUDING ALL, b text)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check_no_inherit_not_valid() {
        let lexed = crate::lex("CREATE TABLE t (d date, CHECK (false) NO INHERIT NOT VALID)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check_not_valid() {
        let lexed = crate::lex("CREATE TABLE t (a int, CHECK (a > 0) NOT VALID)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_params() {
        let lexed = crate::lex("CREATE TABLE t (a int) WITH (fillfactor = 70, autovacuum_enabled = off)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let super::CreateTableBody::Columns(body) = &stmt.body else {
            panic!("expected columns body");
        };
        assert_eq!(body.with_storage().unwrap().len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_legacy_oids_clauses() {
        for (src, expected) in [
            (
                "CREATE TABLE with_oids (a int) WITH OIDS",
                super::WithOidsClause::WithOids,
            ),
            (
                "CREATE TABLE without_oids (a int) WITHOUT OIDS",
                super::WithOidsClause::WithoutOids,
            ),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
            let stmt = stmt_parsed.ast();
            let super::CreateTableBody::Columns(body) = &stmt.body else {
                panic!("expected columns body for {src:?}");
            };
            assert_eq!(body.with_oids(), Some(expected));
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_create_temp_table_empty_columns() {
        let lexed = crate::lex("CREATE TEMP TABLE nocols()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.items().unwrap().len(), 0);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_unlogged_table() {
        let lexed = crate::lex("CREATE UNLOGGED TABLE t (a int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.unlogged);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_unlogged_table_qualified() {
        let lexed = crate::lex("CREATE UNLOGGED TABLE public.t (a int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        // This uses unqualified Ident only; restrict to the unqualified form.
        let _stmt = CreateTableStmt::parse(&mut input);
    }

    #[test]
    fn parse_column_with_collate() {
        let lexed = crate::lex("CREATE TABLE foo (a text COLLATE \"C\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_range_from_to() {
        let lexed = crate::lex("CREATE TABLE p1 PARTITION OF p FOR VALUES FROM (0) TO (10)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_list_in() {
        let lexed = crate::lex("CREATE TABLE p2 PARTITION OF p FOR VALUES IN (1, 2, 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_hash_with_modulus() {
        let lexed = crate::lex(
            "CREATE TABLE p3 PARTITION OF p FOR VALUES WITH (MODULUS 4, REMAINDER 0)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_default() {
        let lexed = crate::lex("CREATE TABLE p4 PARTITION OF p DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_primary_key_using_index_tablespace() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int PRIMARY KEY USING INDEX TABLESPACE pg_default) PARTITION BY LIST (a)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// Sanity check: ALTER TABLE ... ADD CONSTRAINT ... PRIMARY KEY (col)
    /// must parse (existing functionality).
    #[test]
    fn parse_alter_table_add_pk_cols_sanity() {
        let lexed = crate::lex("ALTER TABLE t ADD PRIMARY KEY (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// Table-level `PRIMARY KEY USING INDEX existing_idx` constraint form
    /// (gram.y `ConstraintElem: PRIMARY KEY ExistingIndex …`). Distinct
    /// from the `PRIMARY KEY (cols)` form modelled by `TablePrimaryKey`.
    #[test]
    fn parse_table_constraint_primary_key_using_index() {
        let lexed = crate::lex("ALTER TABLE t ADD PRIMARY KEY USING INDEX my_idx");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// Table-level `UNIQUE USING INDEX existing_idx` form (gram.y
    /// `ConstraintElem: UNIQUE ExistingIndex …`).
    #[test]
    fn parse_table_constraint_unique_using_index() {
        let lexed = crate::lex("ALTER TABLE t ADD UNIQUE USING INDEX my_idx");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// `ADD CONSTRAINT name PRIMARY KEY USING INDEX existing_idx` — the
    /// named-constraint form.
    #[test]
    fn parse_table_constraint_named_primary_key_using_index() {
        let lexed = crate::lex(
            "ALTER TABLE t ADD CONSTRAINT my_pkey PRIMARY KEY USING INDEX my_idx",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ctas_on_commit_delete_rows() {
        let lexed = crate::lex("CREATE TEMP TABLE temptest(col) ON COMMIT DELETE ROWS AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ctas_on_commit_drop() {
        let lexed = crate::lex("CREATE TEMP TABLE temptest(col) ON COMMIT DROP AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_on_commit() {
        for src in [
            "CREATE TEMP TABLE t1 PARTITION OF p FOR VALUES IN (1) ON COMMIT DELETE ROWS",
            "CREATE TEMP TABLE t2 PARTITION OF p FOR VALUES IN (2) ON COMMIT DROP",
            "CREATE TEMP TABLE t3 PARTITION OF p FOR VALUES IN (1) ON COMMIT PRESERVE ROWS",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
            let _stmt = _stmt_parsed.ast();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_partition_of_multiple_column_options() {
        let lexed = crate::lex(
            "CREATE TABLE child PARTITION OF parent (a NOT NULL, b DEFAULT 1) FOR VALUES IN (1)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type() {
        let lexed = crate::lex("CREATE TABLE persons OF person_type");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt.body, super::CreateTableBody::OfType(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_with_options() {
        let lexed = crate::lex(
            "CREATE TABLE personsx OF person_type (myname WITH OPTIONS NOT NULL)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_constraints() {
        let lexed = crate::lex(
            "CREATE TABLE persons2 OF person_type (id WITH OPTIONS PRIMARY KEY, UNIQUE (name))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_default() {
        let lexed = crate::lex(
            "CREATE TABLE persons3 OF person_type (PRIMARY KEY (id), name WITH OPTIONS DEFAULT '')",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_not_null_default() {
        let lexed = crate::lex(
            "CREATE TABLE persons3 OF person_type (PRIMARY KEY (id), name NOT NULL DEFAULT '')",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE table constraint, simplest form: `EXCLUDE (col WITH op)`.
    #[test]
    fn parse_table_exclude_bare() {
        let lexed = crate::lex("CREATE TABLE deferred_excl (f1 int, EXCLUDE (f1 WITH =))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE table constraint with explicit access method: `EXCLUDE USING gist (col WITH op)`.
    #[test]
    fn parse_table_exclude_using_gist() {
        let lexed = crate::lex("CREATE TABLE t (a int4range, EXCLUDE USING GIST (a WITH =))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE constraint with multiple index elements: `EXCLUDE USING GIST (a WITH =, b WITH =)`.
    #[test]
    fn parse_table_exclude_multi_elements() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int4range, b int4range, EXCLUDE USING GIST (a WITH =, b WITH =))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE constraint with a custom operator like `&&` or `-|-`.
    #[test]
    fn parse_table_exclude_custom_op() {
        let lexed = crate::lex("CREATE TABLE t (a int4range, EXCLUDE USING GIST (a WITH -|-))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE constraint with `WHERE (predicate)` partial-index clause.
    #[test]
    fn parse_table_exclude_with_where() {
        let lexed = crate::lex(
            "CREATE TABLE t (f4 int, EXCLUDE USING btree (f4 WITH =) WHERE (f4 IS NOT NULL))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    // -------------------------------------------------------------------
    // Tests folded in from the former `ast/partition.rs`.
    // -------------------------------------------------------------------

    #[test]
    fn parse_partitioned_table_standalone() {
        use crate::ast::ddl::table::CreatePartitionedTableStmt;
        let lexed = crate::lex("create table list_parted_tbl (a int,b int) partition by list (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreatePartitionedTableStmt::parse(&mut input)
            .unwrap()
            ;
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.text(), "list_parted_tbl");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_standalone() {
        use crate::ast::ddl::table::CreatePartitionOfStmt;
        let lexed = crate::lex(
            "create table list_parted_tbl1 partition of list_parted_tbl for values in (1) partition by list(b)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreatePartitionOfStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.text(), "list_parted_tbl1");
        assert_eq!(stmt.parent.text(), "list_parted_tbl");
        assert!(stmt.partition_by.is_some());
        assert!(input.is_eof());
    }

    // -------------------------------------------------------------------
    // Tests folded in from the former `ast/drop_table.rs`.
    // -------------------------------------------------------------------

    #[test]
    fn parse_drop_table() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("DROP TABLE BOOLTBL1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_table_lowercase() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("drop table my_table");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.names.len(), 1);
    }

    #[test]
    fn parse_drop_table_if_exists() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("DROP TABLE IF EXISTS foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_exists);
    }

    #[test]
    fn parse_drop_table_multi_cascade() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("DROP TABLE IF EXISTS a, b, c CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.if_exists);
        assert_eq!(stmt.names.len(), 3);
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_table_qualified() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("DROP TABLE schema1.foo RESTRICT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = DropTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
    /// Multi-element `alter_identity_column_option_list` (gram.y) — the
    /// `SET GENERATED …`, `SET seq_option`, and `RESTART …` clauses can
    /// chain in a single `ALTER COLUMN` action. identity.sql corpus uses
    /// this.
    #[test]
    fn parse_alter_table_set_generated_set_increment_restart() {
        let lexed = crate::lex(
            "ALTER TABLE pitest2 ALTER COLUMN f3 SET GENERATED BY DEFAULT SET INCREMENT BY 2 RESTART",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_table_identity_single_set_generated_still_works() {
        let lexed = crate::lex("ALTER TABLE t ALTER COLUMN c SET GENERATED ALWAYS");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_table_identity_set_seq_option_alone() {
        let lexed = crate::lex("ALTER TABLE t ALTER COLUMN c SET INCREMENT BY 2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_table_identity_restart_alone() {
        let lexed = crate::lex("ALTER TABLE t ALTER COLUMN c RESTART");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// gram.y `reloption_elem` includes `ColLabel '=' def_arg`. PG accepts
    /// `RESET (name = value)` even though it ignores the value. Reloptions.sql
    /// has `ALTER TABLE reloptions_test RESET (fillfactor=12)` — must not
    /// surface as a file-level parse error.
    #[test]
    fn parse_alter_table_reset_reloptions_with_value() {
        let lexed = crate::lex(
            "ALTER TABLE reloptions_test RESET (fillfactor=12, toast.autovacuum_enabled=off)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = AlterTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// gram.y `Typename` accepts `expr_list` typmods, including negative
    /// integers like `numeric(3, -6)`. numeric.sql corpus needs this.
    #[test]
    fn parse_create_table_numeric_negative_typmod() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::lex("CREATE TABLE num_typemod_test (millions numeric(3, -6))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }
    #[test]
    fn parse_create_table_as_execute() {
        // gram.y `ExecuteStmt: CREATE OptTemp TABLE create_as_target AS
        // EXECUTE name execute_param_clause opt_with_data`.
        let lexed = crate::lex("CREATE TABLE as_select1 AS EXECUTE select1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let super::CreateTableBody::AsQuery(body) = &stmt.body else {
            panic!("expected an AS-query body");
        };
        let super::CtasSource::Execute(execute) = &body.source else {
            panic!("expected an EXECUTE source");
        };
        assert_eq!(execute.name.text(), "select1");
        assert!(execute.params.is_none());
        assert!(body.with_data.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_as_execute_params_with_no_data() {
        let lexed = crate::lex(
            "CREATE TEMPORARY TABLE q5_prep_nodata AS EXECUTE q5(200, 'DTAAAA') WITH NO DATA",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let super::CreateTableBody::AsQuery(body) = &stmt.body else {
            panic!("expected an AS-query body");
        };
        let super::CtasSource::Execute(execute) = &body.source else {
            panic!("expected an EXECUTE source");
        };
        assert_eq!(execute.params.as_ref().unwrap().params.len(), 2);
        assert!(matches!(body.with_data, Some(super::WithDataClause::NoData)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_columns_as_execute() {
        let lexed = crate::lex(
            "CREATE TABLE selinto_schema.tbl_withdata3 (a) AS EXECUTE data_sel WITH DATA",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let super::CreateTableBody::ColumnsAsQuery(body) = &stmt.body else {
            panic!("expected a columns AS-query body");
        };
        assert_eq!(body.columns.columns.len(), 1);
        assert!(matches!(body.source, super::CtasSource::Execute(_)));
        assert!(matches!(body.with_data, Some(super::WithDataClause::Data)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_tablespace_as_execute() {
        let lexed = crate::lex(
            "CREATE TABLE testschema.asexecute TABLESPACE regress_tblspace AS EXECUTE selectsource(2)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let super::CreateTableBody::AsQuery(body) = &stmt.body else {
            panic!("expected an AS-query body");
        };
        assert!(body.tablespace.is_some());
        assert!(matches!(body.source, super::CtasSource::Execute(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_as_query_still_parses() {
        let lexed = crate::lex("CREATE TABLE t AS SELECT 1 WITH NO DATA");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateTableStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let super::CreateTableBody::AsQuery(body) = &stmt.body else {
            panic!("expected an AS-query body");
        };
        assert!(matches!(body.source, super::CtasSource::Query(_)));
        assert!(input.is_eof());
    }

    // Added in 17: research, PostgreSQL 17, "Changes to existing statements"
    // (REL_17_11 gram.y 2441, 2470, 2480, 2882).
    #[cfg(feature = "since-pg17")]
    #[test]
    fn alter_table_17_column_and_access_method_forms() {
        crate::ast::test_support::assert_statements_parse(&[
            "ALTER TABLE t ALTER COLUMN g SET EXPRESSION AS (a * 2)",
            "ALTER TABLE t ALTER g SET EXPRESSION AS (a * 2)",
            "ALTER TABLE t ALTER c SET STATISTICS DEFAULT",
            "ALTER INDEX i ALTER 1 SET STATISTICS DEFAULT",
            "ALTER TABLE t SET ACCESS METHOD DEFAULT",
            "ALTER MATERIALIZED VIEW v SET ACCESS METHOD DEFAULT",
        ]);
    }

    // Added in 17, so rejected before 17: research, PostgreSQL 17, "Changes to existing statements".
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn alter_table_17_forms_are_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "ALTER TABLE t ALTER COLUMN g SET EXPRESSION AS (a * 2)",
            "ALTER TABLE t ALTER g SET EXPRESSION AS (a * 2)",
            "ALTER TABLE t ALTER c SET STATISTICS DEFAULT",
            "ALTER INDEX i ALTER 1 SET STATISTICS DEFAULT",
            "ALTER TABLE t SET ACCESS METHOD DEFAULT",
            "ALTER MATERIALIZED VIEW v SET ACCESS METHOD DEFAULT",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "ALTER TABLE t ALTER COLUMN g DROP EXPRESSION",
            "ALTER TABLE t ALTER c SET STATISTICS -1",
            "ALTER INDEX i ALTER 1 SET STATISTICS 5",
        ]);
        // `SET ACCESS METHOD name` is added in 15: research, PostgreSQL 15,
        // "Changes to existing statements".
        #[cfg(feature = "since-pg15")]
        crate::ast::test_support::assert_statements_parse(&["ALTER TABLE t SET ACCESS METHOD heap"]);
    }

    // --- PostgreSQL 18 (docs/research/postgres-14-19-sql-syntax-changes.md,
    // PostgreSQL 18, items 1 to 7) ---

    /// Whether `src` parses as one complete statement.
    fn statement_parses(src: &str) -> bool {
        let lexed = crate::lex(src);
        if lexed.errors().count() > 0 {
            return false;
        }
        let mut input = lexed.input();
        crate::ast::Statement::parse(&mut input).is_ok() && input.is_eof()
    }

    /// The constraints of the first column of a `CREATE TABLE`.
    #[cfg(feature = "since-pg18")]
    fn first_column_kinds(src: &'static str) -> Vec<String> {
        let parsed = crate::ast::test_support::parse_stmt::<CreateTableStmt>(src);
        let stmt = parsed.ast();
        let items = stmt.items().expect("a column body");
        let ColumnOrConstraint::Column(column) = &items[0] else {
            panic!("the first item of {src:?} is a column");
        };
        column
            .constraints
            .iter()
            .map(|constraint| format!("{:?}", constraint.kind))
            .collect()
    }

    /// The table constraint kinds of a `CREATE TABLE`.
    #[cfg(feature = "since-pg18")]
    fn table_constraints(src: &'static str) -> Vec<String> {
        let parsed = crate::ast::test_support::parse_stmt::<CreateTableStmt>(src);
        let stmt = parsed.ast();
        stmt.items()
            .expect("a column body")
            .iter()
            .filter_map(|item| match item {
                ColumnOrConstraint::Constraint(constraint) => {
                    Some(format!("{:?}", constraint.kind))
                }
                _ => None,
            })
            .collect()
    }

    #[cfg(feature = "since-pg18")]
    #[test]
    fn parse_virtual_generated_column() {
        for (src, spelled) in [
            ("CREATE TABLE t (c int GENERATED ALWAYS AS (a * 2) VIRTUAL)", true),
            ("CREATE TABLE t (c int GENERATED ALWAYS AS (a * 2))", false),
        ] {
            let parsed = crate::ast::test_support::parse_stmt::<CreateTableStmt>(src);
            let stmt = parsed.ast();
            let ColumnOrConstraint::Column(column) = &stmt.items().unwrap()[0] else {
                panic!("{src:?} holds a column");
            };
            let ColumnConstraintKind::Generated(generated) = &column.constraints[0].kind else {
                panic!("{src:?} holds a generated column");
            };
            let GeneratedBody::Virtual(tail) = &generated.body else {
                panic!("{src:?} is a virtual generated column");
            };
            assert_eq!(tail.virtual_keyword, spelled, "{src:?}");
            crate::ast::test_support::reparse_stable::<CreateTableStmt>(src);
        }
        // `STORED` stays the stored form.
        let kinds = first_column_kinds("CREATE TABLE t (c int GENERATED ALWAYS AS (a) STORED)");
        assert!(kinds[0].contains("Stored"), "{kinds:?}");
        crate::ast::test_support::reparse_stable::<AlterTableStmt>(
            "ALTER TABLE t ADD COLUMN c int GENERATED ALWAYS AS (a) VIRTUAL",
        );
    }

    #[cfg(feature = "since-pg18")]
    #[test]
    fn parse_temporal_keys_without_overlaps() {
        for src in [
            "CREATE TABLE t (id int, v daterange, PRIMARY KEY (id, v WITHOUT OVERLAPS))",
            "CREATE TABLE t (id int, v daterange, UNIQUE (id, v WITHOUT OVERLAPS))",
        ] {
            let kinds = table_constraints(src);
            assert_eq!(kinds.len(), 1, "{src:?}");
            crate::ast::test_support::reparse_stable::<CreateTableStmt>(src);
        }
        let parsed = crate::ast::test_support::parse_stmt::<CreateTableStmt>(
            "CREATE TABLE t (id int, PRIMARY KEY (id, v WITHOUT OVERLAPS))",
        );
        let stmt = parsed.ast();
        let ColumnOrConstraint::Constraint(constraint) = &stmt.items().unwrap()[1] else {
            panic!("the second item is a constraint");
        };
        let TableConstraintKind::PrimaryKey(key) = &constraint.kind else {
            panic!("a primary key");
        };
        let IndexedConstraintBody::Columns(columns) = &key.body else {
            panic!("a column list");
        };
        assert_eq!(columns.columns.len(), 2);
        assert!(columns.columns.1, "WITHOUT OVERLAPS is recorded");
        crate::ast::test_support::reparse_stable::<AlterTableStmt>(
            "ALTER TABLE t ADD CONSTRAINT k UNIQUE (id, v WITHOUT OVERLAPS)",
        );
    }

    #[cfg(feature = "since-pg18")]
    #[test]
    fn parse_temporal_foreign_key_period() {
        let src = "CREATE TABLE t (id int, v daterange, \
                   FOREIGN KEY (id, PERIOD v) REFERENCES p (id, PERIOD v))";
        let parsed = crate::ast::test_support::parse_stmt::<CreateTableStmt>(src);
        let stmt = parsed.ast();
        let ColumnOrConstraint::Constraint(constraint) = &stmt.items().unwrap()[2] else {
            panic!("the third item is a constraint");
        };
        let TableConstraintKind::ForeignKey(key) = &constraint.kind else {
            panic!("a foreign key");
        };
        assert_eq!(key.columns.len(), 1);
        assert_eq!(key.columns.1.as_ref().unwrap().column.text(), "v");
        let referenced = key.references.columns.as_ref().unwrap();
        assert_eq!(referenced.len(), 1);
        assert_eq!(referenced.1.as_ref().unwrap().column.text(), "v");
        crate::ast::test_support::reparse_stable::<CreateTableStmt>(src);
        // `period` is still an ordinary column name.
        crate::ast::test_support::reparse_stable::<CreateTableStmt>(
            "CREATE TABLE t (period int, FOREIGN KEY (period, PERIOD v) REFERENCES p (period))",
        );
        crate::ast::test_support::reparse_stable::<AlterTableStmt>(
            "ALTER TABLE t ADD FOREIGN KEY (id, PERIOD v) REFERENCES p (id, PERIOD v) ON DELETE CASCADE",
        );
        // A column-level `REFERENCES` has no `PERIOD` (gram.y `opt_column_list`).
        assert!(!statement_parses("CREATE TABLE t (a int REFERENCES p (a, PERIOD b))"));
    }

    #[cfg(feature = "since-pg18")]
    #[test]
    fn parse_enforced_constraint_attributes() {
        let kinds = first_column_kinds(
            "CREATE TABLE t (a int CHECK (a > 0) NOT ENFORCED REFERENCES p ENFORCED)",
        );
        assert_eq!(kinds.len(), 4, "{kinds:?}");
        assert_eq!(kinds[1], "Attr(NotEnforced)");
        assert_eq!(kinds[3], "Attr(Enforced)");
        let kinds = table_constraints(
            "CREATE TABLE t (a int, CHECK (a > 0) NOT ENFORCED, \
             FOREIGN KEY (a) REFERENCES p ENFORCED)",
        );
        assert!(kinds[0].contains("NotEnforced"), "{kinds:?}");
        assert!(
            kinds[1].contains("Enforced") && !kinds[1].contains("NotEnforced"),
            "{kinds:?}"
        );
        crate::ast::test_support::reparse_stable::<CreateTableStmt>(
            "CREATE TABLE t (a int CHECK (a > 0) NOT ENFORCED, CHECK (a < 9) ENFORCED)",
        );
    }

    #[cfg(feature = "since-pg18")]
    #[test]
    fn parse_alter_constraint_enforced_and_inherit() {
        for src in [
            "ALTER TABLE t ALTER CONSTRAINT c NOT ENFORCED",
            "ALTER TABLE t ALTER CONSTRAINT c ENFORCED",
            "ALTER TABLE t ALTER CONSTRAINT c NO INHERIT",
            "ALTER TABLE t ALTER CONSTRAINT c INHERIT",
        ] {
            crate::ast::test_support::reparse_stable::<AlterTableStmt>(src);
        }
        let rendered = format!(
            "{:?}",
            crate::ast::test_support::parse_stmt::<AlterTableStmt>(
                "ALTER TABLE t ALTER CONSTRAINT c INHERIT"
            )
            .ast()
        );
        assert!(rendered.contains("AlterConstraintInherit"), "{rendered}");
        // `INHERIT` takes no constraint attributes after it.
        assert!(!statement_parses("ALTER TABLE t ALTER CONSTRAINT c INHERIT DEFERRABLE"));
    }

    #[cfg(feature = "since-pg18")]
    #[test]
    fn parse_not_null_constraints_of_18() {
        let kinds = table_constraints(
            "CREATE TABLE t (a int, b int, NOT NULL a, CONSTRAINT nn NOT NULL b NO INHERIT NOT VALID)",
        );
        assert_eq!(kinds.len(), 2);
        assert!(kinds.iter().all(|kind| kind.starts_with("NotNull")), "{kinds:?}");
        let kinds = first_column_kinds("CREATE TABLE t (a int NOT NULL NO INHERIT DEFAULT 1)");
        assert_eq!(kinds[0], "NotNullNoInherit");
        for src in [
            "CREATE TABLE t (a int, CONSTRAINT nn NOT NULL a NOT VALID)",
            "CREATE TABLE t (a int NOT NULL NO INHERIT)",
            "CREATE TABLE p2 PARTITION OF p (NOT NULL a) FOR VALUES IN (1)",
        ] {
            crate::ast::test_support::reparse_stable::<CreateTableStmt>(src);
        }
        crate::ast::test_support::reparse_stable::<AlterTableStmt>("ALTER TABLE t ADD NOT NULL a");
        assert!(!statement_parses("ALTER TABLE t ADD NOT NULL (a)"));
    }

    #[test]
    fn the_keywords_of_18_are_names() {
        // `enforced`, `objects`, `period` and `virtual` are unreserved,
        // bare-label keywords in 18 and ordinary names before 18.
        for src in [
            "CREATE TABLE virtual (period int, enforced int, objects int)",
            "CREATE TABLE t (a int, FOREIGN KEY (a, period) REFERENCES p (a, period))",
        ] {
            crate::ast::test_support::reparse_stable::<CreateTableStmt>(src);
        }
        assert!(statement_parses("SELECT 1 virtual, 1 enforced, 1 objects, 1 period"));
    }

    // Before 18 PostgreSQL rejects each of these forms
    // (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
    // items 1 to 7).
    #[cfg(not(feature = "since-pg18"))]
    #[test]
    fn the_table_forms_of_18_are_rejected_before_18() {
        for src in [
            "CREATE TABLE t (c int GENERATED ALWAYS AS (a) VIRTUAL)",
            "CREATE TABLE t (c int GENERATED ALWAYS AS (a))",
            "CREATE TABLE t (id int, PRIMARY KEY (id, v WITHOUT OVERLAPS))",
            "CREATE TABLE t (id int, UNIQUE (id, v WITHOUT OVERLAPS))",
            "CREATE TABLE t (id int, FOREIGN KEY (id, PERIOD v) REFERENCES p (id, PERIOD v))",
            "CREATE TABLE t (a int CHECK (a > 0) NOT ENFORCED)",
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES p ENFORCED)",
            "ALTER TABLE t ALTER CONSTRAINT c NOT ENFORCED",
            "ALTER TABLE t ALTER CONSTRAINT c INHERIT",
            "CREATE TABLE t (a int, NOT NULL a)",
            "ALTER TABLE t ADD NOT NULL a",
            "CREATE TABLE t (a int NOT NULL NO INHERIT)",
        ] {
            assert!(!statement_parses(src), "{src:?} must not parse before 18");
        }
    }

    // Added in 16: research, PostgreSQL 16, "Changes to existing statements"
    // (REL_16_15 gram.y `column_storage`, commits 784cedda0, b9424d014).
    #[cfg(feature = "since-pg16")]
    #[test]
    fn column_storage_forms() {
        crate::ast::test_support::assert_statements_parse(&[
            "CREATE TABLE t (a text STORAGE EXTERNAL)",
            "CREATE TABLE t (a text STORAGE DEFAULT)",
            "CREATE TABLE t (a text STORAGE plain COMPRESSION lz4)",
            "ALTER TABLE t ADD COLUMN a text STORAGE MAIN",
            "ALTER TABLE t ALTER a SET STORAGE DEFAULT",
        ]);
    }

    // gram.y `column_storage: STORAGE ColId | STORAGE DEFAULT` (REL_16_15
    // 3781, REL_17_11 3845) takes a name, not the four TOAST words. Only
    // `tablecmds.c` knows the word list, so the raw parser accepts any
    // `ColId` and rejects the reserved words that `ColId` excludes.
    #[cfg(feature = "since-pg16")]
    #[test]
    fn column_storage_takes_any_col_id() {
        crate::ast::test_support::check_statement_forms(
            &[
                "CREATE TABLE t (a int STORAGE banana)",
                "CREATE TABLE t (a int STORAGE \"Banana\")",
                "CREATE TABLE t (a int STORAGE between)",
                "ALTER TABLE t ALTER COLUMN a SET STORAGE banana",
            ],
            &[
                "CREATE TABLE t (a int STORAGE select)",
                "ALTER TABLE t ALTER COLUMN a SET STORAGE select",
            ],
        );
    }

    // Added in 16, so rejected before 16. REL_15_19 gram.y has no
    // `column_storage`, and `ALTER opt_column ColId SET STORAGE ColId` takes a
    // name, which excludes the reserved word `DEFAULT`.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn column_storage_is_rejected_before_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "CREATE TABLE t (a text STORAGE EXTERNAL)",
            "CREATE TABLE t (a text STORAGE DEFAULT)",
            "CREATE TABLE t (a text STORAGE plain COMPRESSION lz4)",
            "ALTER TABLE t ADD COLUMN a text STORAGE MAIN",
            "ALTER TABLE t ALTER a SET STORAGE DEFAULT",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "ALTER TABLE t ALTER a SET STORAGE EXTERNAL",
            "ALTER TABLE t ALTER COLUMN a SET STORAGE main",
            "ALTER TABLE t ALTER a SET STORAGE foo",
            "ALTER TABLE t ALTER a SET STORAGE \"default\"",
            "CREATE TABLE t (LIKE u INCLUDING STORAGE)",
        ]);
    }
}
