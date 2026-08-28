/// WITH clause (Common Table Expressions) AST.
///
/// Supports `WITH [RECURSIVE] name [(col, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (query)
///   [SEARCH DEPTH|BREADTH FIRST BY col, ... SET col]
///   [CYCLE col, ... SET col [TO val DEFAULT val] USING col]
///   [, ...] SELECT|INSERT|UPDATE|DELETE|MERGE`
use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::shared::expr::Expr;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Materialization option: `MATERIALIZED` or `NOT MATERIALIZED`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum MaterializedOption {
    NotMaterialized((NOT, MATERIALIZED)),
    Materialized(MATERIALIZED),
}

/// SEARCH direction: DEPTH or BREADTH
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SearchDirection {
    Depth(DEPTH),
    Breadth(BREADTH),
}

/// SEARCH clause: `SEARCH DEPTH|BREADTH FIRST BY col, ... SET col`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SearchClause<'input> {
    pub search: SEARCH,
    pub direction: SearchDirection,
    pub first: FIRST,
    pub by: BY,
    pub columns: Seq0<literal::AliasName<'input>, punct::Comma>,
    pub set: SET,
    pub set_column: literal::AliasName<'input>,
}

/// CYCLE clause: `CYCLE col, ... SET col [TO val DEFAULT val] USING col`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CycleClause<'input> {
    pub cycle: CYCLE,
    pub columns: Seq0<literal::AliasName<'input>, punct::Comma>,
    pub set: SET,
    pub set_column: CycleSetColumn<'input>,
    pub using: USING,
    pub using_column: literal::AliasName<'input>,
}

/// SET column with optional TO/DEFAULT values.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CycleSetColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub to_default: Option<CycleToDefault<'input>>,
}

/// TO value DEFAULT value
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CycleToDefault<'input> {
    pub to: TO,
    pub to_value: Expr<'input>,
    pub default: DEFAULT,
    pub default_value: Expr<'input>,
}

/// A single CTE definition: `name [(col, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (query)
///   [SEARCH ...] [CYCLE ...]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CteDefinition<'input> {
    pub name: literal::AliasName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
    pub r#as: AS,
    pub materialized: Option<MaterializedOption>,
    pub query: Surrounded<punct::LParen, Box<crate::ast::Statement<'input>>, punct::RParen>,
    pub search: Option<SearchClause<'input>>,
    pub cycle: Option<CycleClause<'input>>,
}

/// WITH clause: `WITH [RECURSIVE] cte_def, ...`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithClause<'input> {
    pub with_recursive: (WITH, Option<RECURSIVE>),
    pub ctes: Seq1<CteDefinition<'input>, punct::Comma>,
}

/// WITH statement: WITH clause followed by a body statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dql"])]
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
        let mut input = crate::tokens::test_input("WITH q1(x,y) AS (SELECT 1,2) SELECT * FROM q1");
        let stmt = WithStatement::parse(&mut input).unwrap();
        assert_eq!(stmt.with_clause.ctes.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_with_recursive() {
        let mut input = crate::tokens::test_input(
            "WITH RECURSIVE t(n) AS (VALUES (1) UNION ALL SELECT n+1 FROM t WHERE n < 100) SELECT sum(n) FROM t",
        );
        let stmt = WithStatement::parse(&mut input).unwrap();
        assert!(stmt.with_clause.with_recursive.1.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_with_materialized() {
        let mut input = crate::tokens::test_input(
            "WITH x AS MATERIALIZED (SELECT unique1 FROM tenk1) SELECT count(*) FROM tenk1 a WHERE unique1 IN (SELECT * FROM x)",
        );
        let stmt = WithStatement::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.with_clause.ctes.get(0).unwrap().materialized,
            Some(MaterializedOption::Materialized(_))
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_with_multiple_ctes() {
        let mut input = crate::tokens::test_input(
            "WITH RECURSIVE y (id) AS (VALUES (1)), x (id) AS (SELECT * FROM y UNION ALL SELECT id+1 FROM x WHERE id < 5) SELECT * FROM x",
        );
        let stmt = WithStatement::parse(&mut input).unwrap();
        assert_eq!(stmt.with_clause.ctes.len(), 2);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_with_search_depth_first() {
        let sql = "WITH RECURSIVE search_graph(f, t, label) AS (SELECT * FROM graph0 g UNION ALL SELECT g.* FROM graph0 g, search_graph sg WHERE g.f = sg.t) SEARCH DEPTH FIRST BY f, t SET seq SELECT * FROM search_graph";
        let mut input = crate::tokens::test_input(sql);
        let stmt = WithStatement::parse(&mut input).unwrap();
        assert!(stmt.with_clause.ctes.get(0).unwrap().search.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_with_cycle() {
        let sql = "WITH RECURSIVE search_graph(f, t, label) AS (SELECT * FROM graph g UNION ALL SELECT g.* FROM graph g, search_graph sg WHERE g.f = sg.t) CYCLE f, t SET is_cycle USING path SELECT * FROM search_graph";
        let mut input = crate::tokens::test_input(sql);
        let stmt = WithStatement::parse(&mut input).unwrap();
        assert!(stmt.with_clause.ctes.get(0).unwrap().cycle.is_some());
        assert!(input.is_empty());
    }
}
