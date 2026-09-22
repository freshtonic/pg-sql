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
        assert!(stmt.constraints.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_check_default_notnull() {
        let lexed = crate::lex("CREATE DOMAIN dcheck varchar(15) NOT NULL DEFAULT 'a' CHECK (VALUE = 'a')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.constraints.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_named_constraint() {
        let lexed = crate::lex("CREATE DOMAIN testdomain1 AS int CONSTRAINT unsigned CHECK (value > 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateDomainStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.constraints.len(), 1);
        assert!(stmt.constraints[0].name.is_some());
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
        // NOT NULL arm of `DomainConstraintElem`.
        let lexed = crate::lex("alter domain connotnull add not null no inherit");
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
}
