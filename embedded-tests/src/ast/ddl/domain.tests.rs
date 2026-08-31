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
}
