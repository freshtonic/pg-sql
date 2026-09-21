//! The shape `gram.y`'s precedence declarations give an expression.
//!
//! `Expr` is an ordinary left-recursive enum whose operator conflicts are
//! settled only by the `precedence { ... }` block of `crate::tokens`, which
//! mirrors `gram.y:829-908`, and by the `#[parse(prec = ...)]` overrides that
//! mirror `gram.y`'s `%prec`. The differential compares formatter output, which
//! reproduces the source text whichever way an expression nests, and the
//! embedded tests pin variants rather than nesting; neither pins the levels.
//! These cases do, one per decision the declaration is responsible for.

use pg_sql::ast::shared::expr::Expr;
use pg_sql::lex;

fn parse_shape(source: &str) -> String {
    let lexed = lex(source);
    assert_eq!(lexed.errors().count(), 0, "lex errors in {source}");
    let mut input = lexed.input();
    let parsed = Expr::parse(&mut input).unwrap_or_else(|e| panic!("{source}: {e}"));
    assert!(input.is_eof(), "{source} not fully consumed");
    shape(parsed.ast())
}

fn shape(e: &Expr<'_>) -> String {
    match e {
        Expr::Not(a) => format!("Not({})", shape(a)),
        Expr::Eq(a, b) => format!("Eq({},{})", shape(a), shape(b)),
        Expr::Add(a, b) => format!("Add({},{})", shape(a), shape(b)),
        Expr::Mul(a, b) => format!("Mul({},{})", shape(a), shape(b)),
        Expr::Pow(a, b) => format!("Pow({},{})", shape(a), shape(b)),
        Expr::Neg(a) => format!("Neg({})", shape(a)),
        Expr::Cast(a, _) => format!("Cast({})", shape(a)),
        Expr::And(a, b) => format!("And({},{})", shape(a), shape(b)),
        Expr::Or(a, b) => format!("Or({},{})", shape(a), shape(b)),
        Expr::RegexMatch(a, b) => format!("Tilde({},{})", shape(a), shape(b)),
        Expr::Concat(a, b) => format!("Concat({},{})", shape(a), shape(b)),
        Expr::BetweenExpr(a, b, c) => {
            format!("Between({},{},{})", shape(a), shape(&b.low), shape(c))
        }
        Expr::AtTimeZone(a, b) => format!("AtTz({},{})", shape(a), shape(b)),
        Expr::BoolTest(a, _) => format!("BoolTest({})", shape(a)),
        Expr::Like(a, b, _) => format!("Like({},{})", shape(a), shape(b)),
        Expr::DecoratedInfix(a, _, b) => format!("Decorated({},{})", shape(a), shape(b)),
        Expr::DecoratedPrefix(_, a) => format!("DecoratedPrefix({})", shape(a)),
        Expr::ColumnRef(_) => "c".to_owned(),
        Expr::IntegerLit(_) => "n".to_owned(),
        Expr::Parenthesized(_) => "(...)".to_owned(),
        other => format!("{other:?}").split('(').next().unwrap().to_owned(),
    }
}

#[test]
fn declared_precedence_matches_gram_y() {
    // Each case names the gram.y levels it decides between.
    let cases = [
        // gram.y:834 `%right NOT` below gram.y:836 the comparison level.
        ("NOT a = b", "Not(Eq(c,c))"),
        ("a AND NOT b", "And(c,Not(c))"),
        // gram.y:889 `%left Op`, above the comparison level: every operator
        // spelling PostgreSQL's scanner returns as `Op` sits there.
        ("a = b ~ c", "Eq(c,Tilde(c,c))"),
        ("a ~ b = c", "Eq(Tilde(c,c),c)"),
        ("a || b = c", "Eq(Concat(c,c),c)"),
        // gram.y:896 `%right UMINUS` above gram.y:892 `%left '^'`, which is
        // why PostgreSQL reads `-2^2` as 4.
        ("- a ^ b", "Pow(Neg(c),c)"),
        ("a + b * c", "Add(c,Mul(c,c))"),
        ("a BETWEEN b AND c", "Between(c,c,c)"),
        // gram.y:15076 `%prec BETWEEN`: the rule ends in `AND`, so without the
        // override the whole form would sit at boolean `AND`'s level.
        ("a BETWEEN b AND c AND d", "And(Between(c,c,c),c)"),
        // gram.y:894 `%left AT` above gram.y:890 `%left '+' '-'`.
        ("a + b AT TIME ZONE c", "Add(c,AtTz(c,c))"),
        ("a = b IS NULL", "BoolTest(Eq(c,c))"),
        ("a LIKE b = c", "Eq(Like(c,c),c)"),
        ("a::int + b", "Add(Cast(c),c)"),
        // gram.y:14844 `a_expr qual_Op a_expr %prec Op`: `OPERATOR(...)` sits
        // at `Op` whatever operator it names, so a decorated `=` binds
        // tighter than a bare `=` and a decorated `+` looser than `*`.
        ("1 OPERATOR(pg_catalog.=) 2 = 3", "Eq(Decorated(n,n),n)"),
        ("1 = 2 OPERATOR(pg_catalog.=) 3", "Eq(n,Decorated(n,n))"),
        ("a OPERATOR(pg_catalog.+) b * c", "Decorated(c,Mul(c,c))"),
        ("a * b OPERATOR(pg_catalog.+) c", "Decorated(Mul(c,c),c)"),
        ("a OPERATOR(pg_catalog.*) b + c", "Decorated(c,Add(c,c))"),
        // gram.y:889 `%left Op OPERATOR`.
        (
            "a OPERATOR(pg_catalog.||) b OPERATOR(pg_catalog.||) c",
            "Decorated(Decorated(c,c),c)",
        ),
        (
            "a || b OPERATOR(pg_catalog.||) c",
            "Decorated(Concat(c,c),c)",
        ),
        // gram.y:14846 `qual_Op a_expr %prec Op`.
        ("OPERATOR(pg_catalog.-) a * b", "DecoratedPrefix(Mul(c,c))"),
        ("OPERATOR(pg_catalog.-) a = b", "Eq(DecoratedPrefix(c),c)"),
    ];
    for (source, expected) in cases {
        assert_eq!(parse_shape(source), expected, "for {source}");
    }
}
