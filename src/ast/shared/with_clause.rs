/// WITH clause (Common Table Expressions) AST.
///
/// Supports `WITH [RECURSIVE] name [(col, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (query)
///   [SEARCH DEPTH|BREADTH FIRST BY col, ... SET col]
///   [CYCLE col, ... SET col [TO val DEFAULT val] USING col]
///   [, ...] SELECT|INSERT|UPDATE|DELETE|MERGE`
use recursa::seq::{Seq0, Seq1};

use crate::ast::shared::expr::Expr;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Materialization option: `MATERIALIZED` or `NOT MATERIALIZED`.
#[derive(recursa::Node, Debug, Clone)]
pub enum MaterializedOption {
    #[tok(NOT, MATERIALIZED)] NotMaterialized,
    #[tok(MATERIALIZED)] Materialized,
}

/// SEARCH direction: DEPTH or BREADTH
#[derive(recursa::Node, Debug, Clone)]
pub enum SearchDirection {
    #[tok(DEPTH)] Depth,
    #[tok(BREADTH)] Breadth,
}

/// SEARCH clause: `SEARCH DEPTH|BREADTH FIRST BY col, ... SET col`
#[derive(recursa::Node, Debug, Clone)]
pub struct SearchClause<'input> {
    #[tok(SEARCH, this)]
    pub direction: SearchDirection,
    #[tok(FIRST, BY, this)]
    #[sep(COMMA)]
    pub columns: Vec<literal::AliasName<'input> >,
    #[tok(SET, this)]
    pub set_column: literal::AliasName<'input>,
}

/// CYCLE clause: `CYCLE col, ... SET col [TO val DEFAULT val] USING col`
#[derive(recursa::Node, Debug, Clone)]
pub struct CycleClause<'input> {
    #[tok(CYCLE, this)]
    #[sep(COMMA)]
    pub columns: Vec<literal::AliasName<'input> >,
    #[tok(SET, this)]
    pub set_column: CycleSetColumn<'input>,
    #[tok(USING, this)]
    pub using_column: literal::AliasName<'input>,
}

/// SET column with optional TO/DEFAULT values.
#[derive(recursa::Node, Debug, Clone)]
pub struct CycleSetColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub to_default: Option<CycleToDefault<'input>>,
}

/// TO value DEFAULT value
#[derive(recursa::Node, Debug, Clone)]
pub struct CycleToDefault<'input> {
    #[tok(TO, this)]
    pub to_value: Expr<'input>,
    #[tok(DEFAULT, this)]
    pub default_value: Expr<'input>,
}

/// A single CTE definition: `name [(col, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (query)
///   [SEARCH ...] [CYCLE ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct CteDefinition<'input> {
    pub name: literal::AliasName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<
         Vec<literal::AliasName<'input> > ,
    >,
    #[tok(AS, this)]
    pub materialized: Option<MaterializedOption>,
    #[tok(LPAREN, this, RPAREN)]
    pub query:  Box<crate::ast::Statement<'input>> ,
    pub search: Option<SearchClause<'input>>,
    pub cycle: Option<CycleClause<'input>>,
}

/// WITH clause: `WITH [RECURSIVE] cte_def, ...`
#[derive(recursa::Node, Debug, Clone)]
pub struct WithClause<'input> {
    #[tok(WITH, optional(RECURSIVE), this)]
    #[sep(COMMA)]
    pub ctes: recursa::Vec1<CteDefinition<'input> >,
}

/// WITH statement: WITH clause followed by a body statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithStatement<'input> {
    pub with_clause: WithClause<'input>,
    pub body: Box<crate::ast::Statement<'input>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_simple_with() {
        let lexed = crate::tokens::lex("WITH q1(x,y) AS (SELECT 1,2) SELECT * FROM q1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.with_clause.ctes.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_recursive() {
        let lexed = crate::tokens::lex("WITH RECURSIVE t(n) AS (VALUES (1) UNION ALL SELECT n+1 FROM t WHERE n < 100) SELECT sum(n) FROM t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.with_recursive.1.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_materialized() {
        let lexed = crate::tokens::lex("WITH x AS MATERIALIZED (SELECT unique1 FROM tenk1) SELECT count(*) FROM tenk1 a WHERE unique1 IN (SELECT * FROM x)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            stmt.with_clause.ctes.get(0).unwrap().materialized,
            Some(MaterializedOption::Materialized(_))
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_multiple_ctes() {
        let lexed = crate::tokens::lex("WITH RECURSIVE y (id) AS (VALUES (1)), x (id) AS (SELECT * FROM y UNION ALL SELECT id+1 FROM x WHERE id < 5) SELECT * FROM x");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.with_clause.ctes.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_search_depth_first() {
        let sql = "WITH RECURSIVE search_graph(f, t, label) AS (SELECT * FROM graph0 g UNION ALL SELECT g.* FROM graph0 g, search_graph sg WHERE g.f = sg.t) SEARCH DEPTH FIRST BY f, t SET seq SELECT * FROM search_graph";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.ctes.get(0).unwrap().search.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_with_cycle() {
        let sql = "WITH RECURSIVE search_graph(f, t, label) AS (SELECT * FROM graph g UNION ALL SELECT g.* FROM graph g, search_graph sg WHERE g.f = sg.t) CYCLE f, t SET is_cycle USING path SELECT * FROM search_graph";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = WithStatement::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_clause.ctes.get(0).unwrap().cycle.is_some());
        assert!(input.is_eof());
    }
}
