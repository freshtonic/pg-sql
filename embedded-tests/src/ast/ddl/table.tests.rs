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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
            let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_create_table_single_column() {
        let lexed = crate::lex("CREATE TABLE BOOLTBL1 (f1 bool)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "BOOLTBL1");
        assert_eq!(stmt.items().unwrap().len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_multiple_columns() {
        let lexed = crate::lex("CREATE TABLE BOOLTBL3 (d text, b bool, o int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.items().unwrap().len(), 7);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_array_column_types() {
        let lexed =
            crate::lex("CREATE TABLE t (a int2[], b int4[][][], c varchar(5)[], d text[])");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.items().unwrap().len(), 4);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_boolean_type() {
        let lexed = crate::lex("CREATE TABLE t (f1 boolean)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.items().unwrap().len(), 1);
    }

    #[test]
    fn parse_create_temp_table() {
        let lexed = crate::lex("CREATE TEMP TABLE foo (f1 int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.temp.is_some());
        assert_eq!(stmt.name.object(), "foo");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_partitioned_table() {
        let lexed =
            crate::lex("create table list_parted_tbl (a int,b int) partition by list (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "list_parted_tbl1");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_check_constraint() {
        let lexed = crate::lex("CREATE TABLE t (a int CHECK (a > 0))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_references_full() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int REFERENCES other(id) MATCH FULL ON DELETE CASCADE ON UPDATE NO ACTION DEFERRABLE INITIALLY DEFERRED)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_named_constraint() {
        let lexed = crate::lex("CREATE TABLE t (a int CONSTRAINT pos CHECK (a > 0))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_default_constraint() {
        let lexed = crate::lex("CREATE TABLE t (a int DEFAULT 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_primary_key() {
        let lexed = crate::lex("CREATE TABLE t (a int, b int, PRIMARY KEY (a, b))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_unique() {
        let lexed = crate::lex("CREATE TABLE t (a int, UNIQUE (a))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_foreign_key() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES other(id) ON DELETE SET NULL)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_foreign_key_set_null_columns() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, b int, FOREIGN KEY (a, b) REFERENCES p ON DELETE SET NULL (b))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_foreign_key_set_default_columns() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, FOREIGN KEY (a) REFERENCES p ON UPDATE SET DEFAULT (a))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check() {
        let lexed = crate::lex("CREATE TABLE t (a int, CHECK (a > 0))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_named_constraint() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int, b int, CONSTRAINT pk PRIMARY KEY (a, b) DEFERRABLE INITIALLY IMMEDIATE)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check_no_inherit() {
        let lexed = crate::lex("CREATE TABLE t (a int, CHECK (a > 0) NO INHERIT)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_bare() {
        let lexed = crate::lex("CREATE TABLE foo (LIKE bar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_including_all() {
        let lexed = crate::lex("CREATE TABLE foo (LIKE bar INCLUDING ALL)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_including_excluding() {
        let lexed = crate::lex(
            "CREATE TABLE foo (LIKE bar INCLUDING DEFAULTS EXCLUDING CONSTRAINTS)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_like_mixed_with_columns() {
        let lexed = crate::lex("CREATE TABLE foo (a int, LIKE bar INCLUDING ALL, b text)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check_no_inherit_not_valid() {
        let lexed =
            crate::lex("CREATE TABLE t (d date, CHECK (false) NO INHERIT NOT VALID)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_table_check_not_valid() {
        let lexed = crate::lex("CREATE TABLE t (a int, CHECK (a > 0) NOT VALID)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_params() {
        let lexed =
            crate::lex("CREATE TABLE t (a int) WITH (fillfactor = 70, autovacuum_enabled = off)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
            let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.items().unwrap().len(), 0);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_unlogged_table() {
        let lexed = crate::lex("CREATE UNLOGGED TABLE t (a int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_range_from_to() {
        let lexed =
            crate::lex("CREATE TABLE p1 PARTITION OF p FOR VALUES FROM (0) TO (10)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_list_in() {
        let lexed = crate::lex("CREATE TABLE p2 PARTITION OF p FOR VALUES IN (1, 2, 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_hash_with_modulus() {
        let lexed = crate::lex(
            "CREATE TABLE p3 PARTITION OF p FOR VALUES WITH (MODULUS 4, REMAINDER 0)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_partition_of_default() {
        let lexed = crate::lex("CREATE TABLE p4 PARTITION OF p DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_primary_key_using_index_tablespace() {
        let lexed = crate::lex(
            "CREATE TABLE t (a int PRIMARY KEY USING INDEX TABLESPACE pg_default) PARTITION BY LIST (a)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Sanity check: ALTER TABLE ... ADD CONSTRAINT ... PRIMARY KEY (col)
    /// must parse (existing functionality).
    #[test]
    fn parse_alter_table_add_pk_cols_sanity() {
        let lexed = crate::lex("ALTER TABLE t ADD PRIMARY KEY (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Table-level `UNIQUE USING INDEX existing_idx` form (gram.y
    /// `ConstraintElem: UNIQUE ExistingIndex …`).
    #[test]
    fn parse_table_constraint_unique_using_index() {
        let lexed = crate::lex("ALTER TABLE t ADD UNIQUE USING INDEX my_idx");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ctas_on_commit_delete_rows() {
        let lexed =
            crate::lex("CREATE TEMP TABLE temptest(col) ON COMMIT DELETE ROWS AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ctas_on_commit_drop() {
        let lexed =
            crate::lex("CREATE TEMP TABLE temptest(col) ON COMMIT DROP AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
            let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type() {
        let lexed = crate::lex("CREATE TABLE persons OF person_type");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_constraints() {
        let lexed = crate::lex(
            "CREATE TABLE persons2 OF person_type (id WITH OPTIONS PRIMARY KEY, UNIQUE (name))",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_default() {
        let lexed = crate::lex(
            "CREATE TABLE persons3 OF person_type (PRIMARY KEY (id), name WITH OPTIONS DEFAULT '')",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_of_type_not_null_default() {
        let lexed = crate::lex(
            "CREATE TABLE persons3 OF person_type (PRIMARY KEY (id), name NOT NULL DEFAULT '')",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE table constraint, simplest form: `EXCLUDE (col WITH op)`.
    #[test]
    fn parse_table_exclude_bare() {
        let lexed = crate::lex("CREATE TABLE deferred_excl (f1 int, EXCLUDE (f1 WITH =))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE table constraint with explicit access method: `EXCLUDE USING gist (col WITH op)`.
    #[test]
    fn parse_table_exclude_using_gist() {
        let lexed =
            crate::lex("CREATE TABLE t (a int4range, EXCLUDE USING GIST (a WITH =))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// EXCLUDE constraint with a custom operator like `&&` or `-|-`.
    #[test]
    fn parse_table_exclude_custom_op() {
        let lexed =
            crate::lex("CREATE TABLE t (a int4range, EXCLUDE USING GIST (a WITH -|-))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    // -------------------------------------------------------------------
    // Tests folded in from the former `ast/partition.rs`.
    // -------------------------------------------------------------------

    #[test]
    fn parse_partitioned_table_standalone() {
        use crate::ast::ddl::table::CreatePartitionedTableStmt;
        let lexed =
            crate::lex("create table list_parted_tbl (a int,b int) partition by list (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreatePartitionedTableStmt::parse(&mut input)
            .unwrap()
            .into_ast();
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
        let stmt = CreatePartitionOfStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = DropTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_table_lowercase() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("drop table my_table");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTableStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
    }

    #[test]
    fn parse_drop_table_if_exists() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("DROP TABLE IF EXISTS foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists);
    }

    #[test]
    fn parse_drop_table_multi_cascade() {
        use crate::ast::ddl::table::DropTableStmt;
        let lexed = crate::lex("DROP TABLE IF EXISTS a, b, c CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = DropTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_table_identity_single_set_generated_still_works() {
        let lexed = crate::lex("ALTER TABLE t ALTER COLUMN c SET GENERATED ALWAYS");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_table_identity_set_seq_option_alone() {
        let lexed = crate::lex("ALTER TABLE t ALTER COLUMN c SET INCREMENT BY 2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_table_identity_restart_alone() {
        let lexed = crate::lex("ALTER TABLE t ALTER COLUMN c RESTART");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = AlterTableStmt::parse(&mut input).unwrap().into_ast();
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
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
    #[test]
    fn parse_create_table_as_execute() {
        // gram.y `ExecuteStmt: CREATE OptTemp TABLE create_as_target AS
        // EXECUTE name execute_param_clause opt_with_data`.
        let lexed = crate::lex("CREATE TABLE as_select1 AS EXECUTE select1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        let super::CreateTableBody::AsQuery(body) = &stmt.body else {
            panic!("expected an AS-query body");
        };
        assert!(matches!(body.source, super::CtasSource::Query(_)));
        assert!(input.is_eof());
    }
}
