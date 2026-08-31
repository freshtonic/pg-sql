//! STATISTICS DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

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
#[derive(recursa::Node, Debug, Clone)]
pub enum StatsParam<'input> {
    Paren(#[tok(LPAREN, this, RPAREN)] Box<Expr<'input>>),
    Func(StatsFuncParam<'input>),
    Bare(crate::tokens::NonReservedWord<'input>),
}

/// A bare function call as a `stats_param` — `ident '(' args ')'`. The
/// argument list is captured as a raw `Expr` so any of PG's `func_expr_*`
/// shapes round-trip.
#[derive(recursa::Node, Debug, Clone)]
pub struct StatsFuncParam<'input> {
    pub name: QualifiedName<'input>,
    pub args: StatsFuncArgs<'input>,
}

/// Required parentheses around a possibly empty statistics function argument
/// list. Keeping the delimiters on this owning node prevents the nullable
/// vector from making the whole suffix disappear.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct StatsFuncArgs<'input> {
    #[sep(COMMA)]
    pub args: Vec<Box<Expr<'input>>>,
}

/// `ON stats_param (, stats_param)*` clause on `CREATE STATISTICS`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ON, this)]
pub struct StatisticsOnClause<'input> {
    #[sep(COMMA)]
    pub params: recursa::Vec1<StatsParam<'input>>,
}

/// `FROM table_ref (, table_ref)*` clause on `CREATE STATISTICS`. Re-uses
/// `select::TableRef` so the full `from_list` grammar (JOIN, subquery,
/// TABLESAMPLE, function table, XMLTABLE, JSON_TABLE) is accepted — gram.y's
/// CreateStatsStmt rule explicitly uses `from_list`, even though PG
/// semantically rejects most of these.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FROM, this)]
pub struct StatisticsFromClause<'input> {
    #[sep(COMMA)]
    pub tables: recursa::Vec1<crate::ast::dml::select::TableRef<'input>>,
}

/// Optional parenthesized extended-statistics kind list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct StatisticsTypeList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<literal::AliasName<'input>>,
);

/// `CREATE STATISTICS [IF NOT EXISTS] [name] [(stat_type, ...)]
/// [ON expr_list] [FROM from_list]`.
///
/// PG's gram.y treats `ON` and `FROM` as mandatory; the corpus deliberately
/// tests partial forms (`CREATE STATISTICS tst;`) which PG rejects. Make all
/// trailers optional so the partial forms round-trip without pg-sql claiming
/// to fix PG-rejected SQL — the differential test verifies PG still rejects
/// the formatted output.
#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, STATISTICS, this)]
pub struct CreateStatisticsStmt<'input> {
    pub if_not_exists: Option<IfNotExists>,
    pub name: Option<QualifiedName<'input>>,
    pub stat_types: Option<StatisticsTypeList<'input>>,
    pub on: Option<StatisticsOnClause<'input>>,
    pub from: Option<StatisticsFromClause<'input>>,
}

/// `DROP STATISTICS [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, STATISTICS, this)]
pub struct DropStatisticsStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `SET STATISTICS set_statistics_value` — Postgres' `AlterStatsStmt`
/// body. The value is either a (signed) integer literal or `DEFAULT`.
///
/// Variant ordering: `DEFAULT` is a hard keyword (distinct first token
/// from a numeric literal), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetStatisticsValue<'input> {
    #[tok(DEFAULT)]
    Default,
    Value(SignedIconst<'input>),
}

/// `SET STATISTICS value` clause on `ALTER STATISTICS`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterStatisticsSetStatisticsClause<'input> {
    #[tok(SET, STATISTICS, this)]
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
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
#[tok(ALTER, STATISTICS, this)]
pub struct AlterStatisticsStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterStatisticsAction<'input>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/statistics.tests.rs"
));
