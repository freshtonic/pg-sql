#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_with() {
        let lexed = crate::lex("WITH q1(x,y) AS (SELECT 1,2) SELECT * FROM q1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(!stmt.with_clause.recursive);
        assert_eq!(stmt.with_clause.ctes.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_recursive() {
        let lexed = crate::lex(
            "WITH RECURSIVE t(n) AS (VALUES (1) UNION ALL SELECT n+1 FROM t WHERE n < 100) SELECT sum(n) FROM t",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.recursive);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_recursive_cte_named_recursive() {
        let lexed =
            crate::lex("WITH RECURSIVE recursive AS (SELECT 1) SELECT * FROM recursive");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.recursive);
        assert_eq!(stmt.with_clause.ctes.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_later_cte_named_recursive() {
        let lexed = crate::lex(
            "WITH x AS (SELECT 1), recursive AS (SELECT 2) SELECT * FROM recursive",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(!stmt.with_clause.recursive);
        assert_eq!(stmt.with_clause.ctes.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn immediate_recursive_is_the_clause_modifier() {
        let lexed = crate::lex("WITH recursive AS (SELECT 1) SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(WithStatement::parse(&mut input).is_err());
    }

    #[test]
    fn parse_with_materialized() {
        let lexed = crate::lex(
            "WITH x AS MATERIALIZED (SELECT unique1 FROM tenk1) SELECT count(*) FROM tenk1 a WHERE unique1 IN (SELECT * FROM x)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.with_clause.ctes.first().materialized,
            Some(MaterializedOption::Materialized)
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_multiple_ctes() {
        let lexed = crate::lex(
            "WITH RECURSIVE y (id) AS (VALUES (1)), x (id) AS (SELECT * FROM y UNION ALL SELECT id+1 FROM x WHERE id < 5) SELECT * FROM x",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.with_clause.ctes.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_search_depth_first() {
        let sql = "WITH RECURSIVE search_graph(f, t, label) AS (SELECT * FROM graph0 g UNION ALL SELECT g.* FROM graph0 g, search_graph sg WHERE g.f = sg.t) SEARCH DEPTH FIRST BY f, t SET seq SELECT * FROM search_graph";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.ctes.first().search.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_cycle() {
        let sql = "WITH RECURSIVE search_graph(f, t, label) AS (SELECT * FROM graph g UNION ALL SELECT g.* FROM graph g, search_graph sg WHERE g.f = sg.t) CYCLE f, t SET is_cycle USING path SELECT * FROM search_graph";
        let lexed = crate::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.ctes.first().cycle.is_some());
        assert!(input.is_eof());
    }
}
