//! STATISTICS DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// A single `stats_param`: a parenthesised expression, a bare column
/// reference, or a window-less function expression.
///
/// Postgres' grammar:
///
/// ```text
/// stats_param: ColId
///            | func_expr_windowless
///            | '(' a_expr ')'
/// ```
///
/// Variant ordering: `Paren` (`(` lookahead) and `Func` (ident + `(`)
/// before `Bare` (bare ident). `Paren` and `Func` both start with `(`/
/// ident; `Func` is detected by the function-call shape via `Expr`. We
/// model `func_expr_windowless` by re-using `Expr` and letting any
/// expression that begins like a function call lex into the `Func` arm.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum StatsParam<'input> {
    Paren(Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>),
    Func(StatsFuncParam<'input>),
    Bare(crate::tokens::NonReservedWord<'input>),
}

/// A bare function call as a `stats_param` — `ident '(' args ')'`. The
/// argument list is captured as a raw `Expr` so any of PG's `func_expr_*`
/// shapes round-trip.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct StatsFuncParam<'input> {
    pub name: QualifiedName<'input>,
    pub args: Surrounded<punct::LParen, Seq0<Box<Expr<'input>>, punct::Comma>, punct::RParen>,
}

/// `ON stats_param (, stats_param)*` clause on `CREATE STATISTICS`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct StatisticsOnClause<'input> {
    pub on: ON,
    pub params: Seq1<StatsParam<'input>, punct::Comma>,
}

/// `FROM table_ref (, table_ref)*` clause on `CREATE STATISTICS`. Re-uses
/// `select::TableRef` so the full `from_list` grammar (JOIN, subquery,
/// TABLESAMPLE, function table, XMLTABLE, JSON_TABLE) is accepted — gram.y's
/// CreateStatsStmt rule explicitly uses `from_list`, even though PG
/// semantically rejects most of these.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct StatisticsFromClause<'input> {
    pub from: FROM,
    pub tables: Seq1<crate::ast::dml::select::TableRef<'input>, punct::Comma>,
}

/// `CREATE STATISTICS [IF NOT EXISTS] [name] [(stat_type, ...)]
/// [ON expr_list] [FROM from_list]`.
///
/// PG's gram.y treats `ON` and `FROM` as mandatory; the corpus deliberately
/// tests partial forms (`CREATE STATISTICS tst;`) which PG rejects. Make all
/// trailers optional so the partial forms round-trip without pg-sql claiming
/// to fix PG-rejected SQL — the differential test verifies PG still rejects
/// the formatted output.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateStatisticsStmt<'input> {
    pub create: CREATE,
    pub statistics: STATISTICS,
    pub if_not_exists: Option<IfNotExists>,
    pub name: Option<QualifiedName<'input>>,
    pub stat_types: Option<
        Surrounded<punct::LParen, Seq1<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
    pub on: Option<StatisticsOnClause<'input>>,
    pub from: Option<StatisticsFromClause<'input>>,
}

/// `DROP STATISTICS [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropStatisticsStmt<'input> {
    pub drop: DROP,
    pub statistics: STATISTICS,
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET STATISTICS set_statistics_value` — Postgres' `AlterStatsStmt`
/// body. The value is either a (signed) integer literal or `DEFAULT`.
///
/// Variant ordering: `DEFAULT` is a hard keyword (distinct first token
/// from a numeric literal), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetStatisticsValue<'input> {
    Default(DEFAULT),
    Value(SignedIconst<'input>),
}

/// `SET STATISTICS value` clause on `ALTER STATISTICS`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterStatisticsSetStatisticsClause<'input> {
    pub set: SET,
    pub statistics: STATISTICS,
    pub value: SetStatisticsValue<'input>,
}

/// One action on `ALTER STATISTICS [IF EXISTS] any_name action` — covers
/// Postgres' `RenameStmt`, `AlterOwnerStmt`, `AlterObjectSchemaStmt`,
/// and `AlterStatsStmt` branches for extended statistics.
///
/// Variant ordering: variants beginning with unique keywords
/// (`RENAME`, `OWNER`) come first. The two `SET ...` variants share the
/// `SET` token; `SET SCHEMA` (followed by a keyword) and `SET
/// STATISTICS` (followed by a keyword) disambiguate on the second
/// token. `SET STATISTICS` is listed before `SET SCHEMA` only for
/// readability — both have a unique two-token prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterStatisticsAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetStatistics(AlterStatisticsSetStatisticsClause<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER STATISTICS [IF EXISTS] any_name action` — Postgres'
/// `AlterStatsStmt` plus the statistics branches of `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
///
/// `IF EXISTS` is permitted by Postgres only on the `SET STATISTICS`
/// branch; pg-sql accepts it on every action (the differential corpus
/// only exercises the SET STATISTICS form) — a strict per-action gate
/// would require either a sub-grammar dispatcher or two separate
/// statement types. The corpus oracle catches any regression here.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterStatisticsStmt<'input> {
    pub alter: ALTER,
    pub statistics: STATISTICS,
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterStatisticsAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_statistics_set_statistics_default() {
        let mut input =
            crate::tokens::test_input("ALTER STATISTICS IF EXISTS ab1_a_b_stats SET STATISTICS 0");
        let _stmt = AlterStatisticsStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_statistics_set_statistics_negative() {
        let mut input =
            crate::tokens::test_input("ALTER STATISTICS ab1_a_b_stats SET STATISTICS -1");
        let _stmt = AlterStatisticsStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_statistics_full() {
        let mut input =
            crate::tokens::test_input("CREATE STATISTICS s ON a, b FROM ext_stats_test");
        let stmt = CreateStatisticsStmt::parse(&mut input).unwrap();
        assert!(stmt.if_not_exists.is_none());
        assert!(stmt.name.is_some());
        assert!(stmt.on.is_some());
        assert!(stmt.from.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_statistics_with_kinds_and_if_not_exists() {
        let mut input = crate::tokens::test_input(
            "CREATE STATISTICS IF NOT EXISTS s (ndistinct, dependencies) ON a, b FROM tab",
        );
        let stmt = CreateStatisticsStmt::parse(&mut input).unwrap();
        assert!(stmt.if_not_exists.is_some());
        assert!(stmt.stat_types.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_statistics_paren_expr() {
        let mut input =
            crate::tokens::test_input("CREATE STATISTICS s ON (a + b), c FROM ext_stats_test");
        let stmt = CreateStatisticsStmt::parse(&mut input).unwrap();
        assert!(stmt.on.is_some());
        assert!(input.is_empty());
    }
}
