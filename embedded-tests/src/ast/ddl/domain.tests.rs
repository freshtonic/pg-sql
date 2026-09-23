#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_domain_simple() {
        let lexed = crate::lex("CREATE DOMAIN domaintext text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.object(), "domaintext");
        assert!(stmt.quals.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_check_default_notnull() {
        let lexed = crate::lex("CREATE DOMAIN dcheck varchar(15) NOT NULL DEFAULT 'a' CHECK (VALUE = 'a')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.quals.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_named_constraint() {
        let lexed = crate::lex("CREATE DOMAIN testdomain1 AS int CONSTRAINT unsigned CHECK (value > 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.quals.len(), 1);
        let DomainQual::Constraint(constraint) = &stmt.quals[0] else {
            panic!("expected a named domain constraint");
        };
        assert!(constraint.name.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_array_with_size() {
        // `int4[1]` — `[N]` array bound, exercised by domain.sql.
        let lexed = crate::lex("CREATE DOMAIN domainint4arr int4[1]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.type_name.array_suffixes.len(), 1);
        assert!(input.is_eof());
    }
    // `ALTER DOMAIN ... ADD NOT NULL` is added in 17: research, PostgreSQL 17, "Changes to existing statements".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_alter_domain_add_not_null() {
        // `ALTER DOMAIN d ADD NOT NULL` — bare NOT NULL domain constraint
        // (gram.y `AlterDomainStmt: ALTER DOMAIN_P any_name ADD_P TableConstraint`).
        let lexed = crate::lex("alter domain connotnull add not null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = AlterDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let AlterDomainAction::Add(add) = &stmt.action else {
            panic!("expected an ADD action, got {:?}", stmt.action);
        };
        assert!(add.constraint.name.is_none());
        let AlterDomainConstraintElem::NotNull(not_null) = &add.constraint.elem else {
            panic!("expected a NOT NULL constraint element");
        };
        assert!(not_null.attrs.is_empty());
        assert!(input.is_eof());
    }

    // Added in 17: see `parse_alter_domain_add_not_null`.
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_alter_domain_add_named_not_null() {
        let lexed = crate::lex("alter domain connotnull add constraint constr1 not null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = AlterDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let AlterDomainAction::Add(add) = &stmt.action else {
            panic!("expected an ADD action, got {:?}", stmt.action);
        };
        assert_eq!(add.constraint.name.as_ref().unwrap().name.text(), "constr1");
        assert!(matches!(
            add.constraint.elem,
            AlterDomainConstraintElem::NotNull(_)
        ));
        assert!(input.is_eof());
    }

    // Added in 17: see `parse_alter_domain_add_not_null`.
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_alter_domain_add_not_null_with_attrs() {
        // The `ConstraintAttributeSpec` tail is still available on the
        // NOT NULL arm of `DomainConstraintElem`. `NOT DEFERRABLE` is one of
        // the entries that `processCASbits` never rejects, in any version.
        let lexed = crate::lex("alter domain connotnull add not null not deferrable");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = AlterDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        let AlterDomainAction::Add(add) = &stmt.action else {
            panic!("expected an ADD action, got {:?}", stmt.action);
        };
        let AlterDomainConstraintElem::NotNull(not_null) = &add.constraint.elem else {
            panic!("expected a NOT NULL constraint element");
        };
        assert_eq!(not_null.attrs.len(), 1);
        assert!(input.is_eof());
    }

    // Before 17, `ALTER DOMAIN ... ADD` takes a `TableConstraint`: research, PostgreSQL 17,
    // "Changes to existing statements" (REL_16_15 gram.y 11392). Execution
    // rejects the kinds other than `CHECK`.
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn alter_domain_add_table_constraint_before_17() {
        for (src, unique) in [
            ("ALTER DOMAIN d ADD UNIQUE (a)", true),
            ("ALTER DOMAIN d ADD CONSTRAINT c PRIMARY KEY (a)", false),
            ("ALTER DOMAIN d ADD FOREIGN KEY (a) REFERENCES t", false),
            ("ALTER DOMAIN d ADD EXCLUDE USING gist (a WITH =)", false),
            ("ALTER DOMAIN d ADD CONSTRAINT c CHECK (VALUE > 0) NOT VALID", false),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let stmt_parsed =
                AlterDomainStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            let AlterDomainAction::Add(add) = &stmt_parsed.ast().action else {
                panic!("expected an ADD action for {src:?}");
            };
            assert_eq!(
                matches!(add.constraint.kind, crate::ast::ddl::table::TableConstraintKind::Unique(_)),
                unique,
                "{src:?}"
            );
        }
    }

    // `NOT NULL` is not a `TableConstraint`, so 16 rejects it here: research, PostgreSQL 17,
    // "Changes to existing statements".
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn alter_domain_add_not_null_is_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "ALTER DOMAIN d ADD NOT NULL",
            "ALTER DOMAIN d ADD CONSTRAINT nn NOT NULL",
        ]);
    }

    // gram.y `CreateDomainStmt` ends in `ColQualList`, the same list a column
    // of a table takes, so every `ColConstraintElem` and every
    // `ConstraintAttr` parses here. Execution, not the raw parser, rejects the
    // kinds a domain cannot hold (REL_17_11 gram.y `CreateDomainStmt` 11509,
    // `ColQualList` 3854, `ColConstraint` 3859).
    #[test]
    fn create_domain_takes_the_column_qualification_list() {
        crate::ast::test_support::check_statement_forms(
            &[
                "CREATE DOMAIN d AS int CHECK (VALUE > 0) NO INHERIT",
                "CREATE DOMAIN d AS int CONSTRAINT c CHECK (VALUE > 0) NO INHERIT",
                "CREATE DOMAIN d AS int DEFERRABLE",
                "CREATE DOMAIN d AS int NOT DEFERRABLE",
                "CREATE DOMAIN d AS int INITIALLY DEFERRED",
                "CREATE DOMAIN d AS int INITIALLY IMMEDIATE",
                "CREATE DOMAIN d AS int NOT NULL DEFERRABLE INITIALLY DEFERRED",
                "CREATE DOMAIN d AS int UNIQUE",
                "CREATE DOMAIN d AS int CONSTRAINT c UNIQUE",
                "CREATE DOMAIN d AS int PRIMARY KEY",
                "CREATE DOMAIN d AS int REFERENCES t",
                "CREATE DOMAIN d AS int REFERENCES t (a) MATCH FULL ON DELETE CASCADE ON UPDATE SET NULL",
                "CREATE DOMAIN d AS int GENERATED ALWAYS AS IDENTITY",
                "CREATE DOMAIN d AS int GENERATED BY DEFAULT AS IDENTITY",
                "CREATE DOMAIN d AS int GENERATED ALWAYS AS IDENTITY (START WITH 4)",
                "CREATE DOMAIN d AS int GENERATED ALWAYS AS (1) STORED",
                "CREATE DOMAIN d AS int UNIQUE WITH (fillfactor = 10) USING INDEX TABLESPACE ts",
                "CREATE DOMAIN d AS int PRIMARY KEY WITH (fillfactor = 10)",
                "CREATE DOMAIN d AS int PRIMARY KEY USING INDEX TABLESPACE ts",
            ],
            &[
                // `ConstraintAttr` is an entry of `ColQualList` in its own
                // right, so it takes no `CONSTRAINT name` prefix.
                "CREATE DOMAIN d AS int CONSTRAINT c NOT DEFERRABLE",
                "CREATE DOMAIN d AS int CONSTRAINT c DEFERRABLE",
                // `CHECK` takes `opt_no_inherit` here, not the full spec.
                "CREATE DOMAIN d AS int CHECK (VALUE > 0) NOT VALID",
                // The generated-column arm takes `ALWAYS` only.
                "CREATE DOMAIN d AS int GENERATED BY DEFAULT AS (1) STORED",
                // `opt_column_storage` and `opt_column_compression` belong to
                // `columnDef`, not to `ColQualList`.
                "CREATE DOMAIN d AS int STORAGE plain",
                "CREATE DOMAIN d AS int COMPRESSION lz4",
                // `SplitColQualList` rejects a second COLLATE.
                "CREATE DOMAIN d AS text COLLATE \"C\" COLLATE \"POSIX\"",
            ],
        );
    }

    // Added in 15: `opt_unique_null_treatment` (research, PostgreSQL 15,
    // "Changes to existing statements"; REL_15_19 gram.y `ColConstraintElem`).
    #[cfg(feature = "since-pg15")]
    #[test]
    fn create_domain_unique_nulls_distinct_from_15() {
        crate::ast::test_support::check_statement_forms(
            &[
                "CREATE DOMAIN d AS int UNIQUE NULLS DISTINCT",
                "CREATE DOMAIN d AS int UNIQUE NULLS NOT DISTINCT",
            ],
            &[],
        );
    }

    // Added in 18: `ColConstraintElem: NOT NULL_P opt_no_inherit`, `ConstraintAttr:
    // ENFORCED | NOT ENFORCED` and `opt_virtual_or_stored`
    // (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
    // items 1, 4 and 7; REL_18_6 gram.y `ColConstraintElem`, `ConstraintAttr`).
    #[cfg(feature = "since-pg18")]
    #[test]
    fn create_domain_qual_list_18_forms() {
        crate::ast::test_support::check_statement_forms(
            &[
                "CREATE DOMAIN d AS int NOT NULL NO INHERIT",
                "CREATE DOMAIN d AS int CONSTRAINT c NOT NULL NO INHERIT",
                "CREATE DOMAIN d AS int ENFORCED",
                "CREATE DOMAIN d AS int NOT ENFORCED",
                "CREATE DOMAIN d AS int CHECK (VALUE > 0) NOT ENFORCED",
                "CREATE DOMAIN d AS int GENERATED ALWAYS AS (1)",
                "CREATE DOMAIN d AS int GENERATED ALWAYS AS (1) VIRTUAL",
            ],
            &[],
        );
    }

    // Added in 18, so rejected before 18: see `create_domain_qual_list_18_forms`.
    #[cfg(not(feature = "since-pg18"))]
    #[test]
    fn create_domain_qual_list_18_forms_are_rejected_before_18() {
        crate::ast::test_support::assert_statements_rejected(&[
            "CREATE DOMAIN d AS int NOT NULL NO INHERIT",
            "CREATE DOMAIN d AS int CONSTRAINT c NOT NULL NO INHERIT",
            "CREATE DOMAIN d AS int ENFORCED",
            "CREATE DOMAIN d AS int NOT ENFORCED",
            "CREATE DOMAIN d AS int CHECK (VALUE > 0) NOT ENFORCED",
            "CREATE DOMAIN d AS int GENERATED ALWAYS AS (1)",
            "CREATE DOMAIN d AS int GENERATED ALWAYS AS (1) VIRTUAL",
        ]);
    }

    // `DomainConstraintElem`'s CHECK arm passes `processCASbits` no
    // `deferrable` and no `initdeferred` pointer, so it rejects `DEFERRABLE`
    // and `INITIALLY DEFERRED`, and from 18 no `is_enforced` pointer either
    // (REL_17_11 gram.y 4286-4288; REL_18_6 gram.y 4384-4386).
    #[cfg(feature = "since-pg17")]
    #[test]
    fn alter_domain_check_takes_only_its_own_attributes() {
        crate::ast::test_support::check_statement_forms(
            &[
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) NOT DEFERRABLE",
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) INITIALLY IMMEDIATE",
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) NOT VALID",
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) NO INHERIT",
            ],
            &[
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) DEFERRABLE",
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) INITIALLY DEFERRED",
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) ENFORCED",
                "ALTER DOMAIN d ADD CHECK (VALUE > 0) NOT ENFORCED",
            ],
        );
    }

    // `DomainConstraintElem`'s NOT NULL arm passes `processCASbits` only a
    // `no_inherit` pointer, so it rejects the deferrable entries other than
    // `NOT DEFERRABLE` and `INITIALLY IMMEDIATE`, and `NOT VALID`
    // (REL_17_11 gram.y 4300-4302).
    #[cfg(feature = "since-pg17")]
    #[test]
    fn alter_domain_not_null_takes_only_its_own_attributes() {
        crate::ast::test_support::check_statement_forms(
            &[
                "ALTER DOMAIN d ADD NOT NULL NOT DEFERRABLE",
                "ALTER DOMAIN d ADD NOT NULL INITIALLY IMMEDIATE",
            ],
            &[
                "ALTER DOMAIN d ADD NOT NULL DEFERRABLE",
                "ALTER DOMAIN d ADD NOT NULL INITIALLY DEFERRED",
                "ALTER DOMAIN d ADD NOT NULL NOT VALID",
                "ALTER DOMAIN d ADD NOT NULL ENFORCED",
                "ALTER DOMAIN d ADD NOT NULL NOT ENFORCED",
            ],
        );
    }

    // Removed in 18: `DomainConstraintElem`'s NOT NULL arm passes
    // `processCASbits` no pointer at all from 18, so `NO INHERIT` joins the
    // rejected entries (REL_17_11 gram.y 4300-4302; REL_18_6 gram.y
    // 4399-4401, commit 14e87ffa5).
    #[cfg(all(feature = "since-pg17", not(feature = "since-pg18")))]
    #[test]
    fn alter_domain_not_null_takes_no_inherit_in_17() {
        crate::ast::test_support::check_statement_forms(
            &["ALTER DOMAIN d ADD NOT NULL NO INHERIT"],
            &[],
        );
    }

    // Removed in 18: see `alter_domain_not_null_takes_no_inherit_in_17`.
    #[cfg(feature = "since-pg18")]
    #[test]
    fn alter_domain_not_null_rejects_no_inherit_from_18() {
        crate::ast::test_support::check_statement_forms(
            &[],
            &["ALTER DOMAIN d ADD NOT NULL NO INHERIT"],
        );
    }
}
