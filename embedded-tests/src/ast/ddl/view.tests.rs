#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_create_view() {
        let lexed = crate::lex("CREATE VIEW v AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "v");
        assert!(!stmt.or_replace);
        assert!(stmt.temp.is_none());
        assert!(!stmt.recursive);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_temp_view() {
        let lexed = crate::lex("CREATE TEMPORARY VIEW v AS SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.temp.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_recursive_view() {
        let lexed = crate::lex(
            "CREATE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 5",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.recursive);
        assert!(stmt.columns.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_or_replace_recursive_view() {
        let lexed = crate::lex(
            "CREATE OR REPLACE RECURSIVE VIEW nums (n) AS VALUES (1) UNION ALL SELECT n+1 FROM nums WHERE n < 6",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.or_replace);
        assert!(stmt.recursive);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_view() {
        let lexed = crate::lex("DROP VIEW v");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropViewStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(!stmt.if_exists);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_view_if_exists_multi_cascade() {
        let lexed = crate::lex("DROP VIEW IF EXISTS a, b CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropViewStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists);
        assert_eq!(stmt.names.len(), 2);
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
}
