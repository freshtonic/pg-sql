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
        let stmt = CreateDomainStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "domaintext");
        assert!(stmt.constraints.is_empty());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_check_default_notnull() {
        let lexed = crate::lex("CREATE DOMAIN dcheck varchar(15) NOT NULL DEFAULT 'a' CHECK (VALUE = 'a')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateDomainStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.constraints.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_domain_named_constraint() {
        let lexed = crate::lex("CREATE DOMAIN testdomain1 AS int CONSTRAINT unsigned CHECK (value > 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateDomainStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateDomainStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.type_name.array_suffixes.len(), 1);
        assert!(input.is_eof());
    }
    #[test]
    fn parse_alter_domain_add_not_null() {
        // `ALTER DOMAIN d ADD NOT NULL` — bare NOT NULL domain constraint
        // (gram.y `AlterDomainStmt: ALTER DOMAIN_P any_name ADD_P TableConstraint`).
        let lexed = crate::lex("alter domain connotnull add not null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = AlterDomainStmt::parse(&mut input).unwrap().into_ast();
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

    #[test]
    fn parse_alter_domain_add_named_not_null() {
        let lexed = crate::lex("alter domain connotnull add constraint constr1 not null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = AlterDomainStmt::parse(&mut input).unwrap().into_ast();
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

    #[test]
    fn parse_alter_domain_add_not_null_with_attrs() {
        // The `ConstraintAttributeSpec` tail is still available on the
        // NOT NULL arm of `DomainConstraintElem`.
        let lexed = crate::lex("alter domain connotnull add not null no inherit");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = AlterDomainStmt::parse(&mut input).unwrap().into_ast();
        let AlterDomainAction::Add(add) = &stmt.action else {
            panic!("expected an ADD action, got {:?}", stmt.action);
        };
        let AlterDomainConstraintElem::NotNull(not_null) = &add.constraint.elem else {
            panic!("expected a NOT NULL constraint element");
        };
        assert_eq!(not_null.attrs.len(), 1);
        assert!(input.is_eof());
    }
}
