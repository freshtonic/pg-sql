//! STATISTICS DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
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
    /// `Paren` starts with `(`. `Func` and `Bare` both begin with an identifier,
    /// and the LR parser distinguishes them when it reaches the function-call
    /// `(`. We
    /// model `func_expr_windowless` by re-using `Expr` and letting any
    /// expression that begins like a function call lex into the `Func` arm.
    #[derive(Debug)]
    pub enum StatsParam {
        Paren(#[tok(LPAREN, this, RPAREN)] boxed!(Expr)),
        Func(StatsFuncParam),
        Bare(crate::tokens::NonReservedWord),
    }
}

recursa::ast_node! {
    /// A bare function call as a `stats_param` — `ident '(' args ')'`. The
    /// argument list is captured as a raw `Expr` so any of PG's `func_expr_*`
    /// shapes round-trip.
    #[derive(Debug)]
    pub struct StatsFuncParam {
        pub name: QualifiedName,
        pub args: StatsFuncArgs,
    }
}

recursa::ast_node! {
    /// Required parentheses around a possibly empty statistics function argument
    /// list. Keeping the delimiters on this owning node prevents the nullable
    /// vector from making the whole suffix disappear.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct StatsFuncArgs {
        #[sep(COMMA)]
        pub args: zero_or_many!(boxed!(Expr)),
    }
}

recursa::ast_node! {
    /// `ON stats_param (, stats_param)*` clause on `CREATE STATISTICS`.
    #[derive(Debug)]
    #[tok(ON, this)]
    pub struct StatisticsOnClause {
        #[sep(COMMA)]
        pub params: one_or_many!(StatsParam),
    }
}

recursa::ast_node! {
    /// `FROM table_ref (, table_ref)*` clause on `CREATE STATISTICS`. Re-uses
    /// `select::TableRef` so the full `from_list` grammar (JOIN, subquery,
    /// TABLESAMPLE, function table, XMLTABLE, JSON_TABLE) is accepted — gram.y's
    /// CreateStatsStmt rule explicitly uses `from_list`, even though PG
    /// semantically rejects most of these.
    #[derive(Debug)]
    #[tok(FROM, this)]
    pub struct StatisticsFromClause {
        #[sep(COMMA)]
        pub tables: one_or_many!(crate::ast::dml::select::TableRef),
    }
}

recursa::ast_node! {
    /// Optional parenthesized extended-statistics kind list.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct StatisticsTypeList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(literal::AliasName),
    );
}

recursa::ast_node! {
    /// `CREATE STATISTICS [IF NOT EXISTS] [name] [(stat_type, ...)]
    /// [ON expr_list] [FROM from_list]`.
    ///
    /// PG's gram.y treats `ON` and `FROM` as mandatory; the corpus deliberately
    /// tests partial forms (`CREATE STATISTICS tst;`) which PG rejects. Make all
    /// trailers optional so the partial forms round-trip without pg-sql claiming
    /// to fix PG-rejected SQL — the differential test verifies PG still rejects
    /// the formatted output.
    #[derive(Debug)]
    #[tok(CREATE, STATISTICS, this)]
    pub struct CreateStatisticsStmt {
        pub if_not_exists: Option<IfNotExists>,
        // The name is optional from 16: research, PostgreSQL 16, "Changes to
        // existing statements" (commit 624aa2a13). REL_15_19 gram.y has
        // `CREATE STATISTICS any_name`.
        #[cfg(feature = "since-pg16")]
        pub name: Option<QualifiedName>,
        #[cfg(not(feature = "since-pg16"))]
        pub name: QualifiedName,
        pub stat_types: Option<StatisticsTypeList>,
        pub on: Option<StatisticsOnClause>,
        pub from: Option<StatisticsFromClause>,
    }
}

recursa::ast_node! {
    /// `DROP STATISTICS [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, STATISTICS, this)]
    pub struct DropStatisticsStmt {
        pub if_exists: Option<IfExists>,
        pub names: NameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `SET STATISTICS set_statistics_value` — Postgres' `AlterStatsStmt`
    /// body. The value is either a (signed) integer literal or `DEFAULT`.
    ///
    /// Variant ordering: `DEFAULT` is a hard keyword (distinct first token
    /// from a numeric literal), so order is for clarity.
    #[derive(Debug)]
    pub enum SetStatisticsValue {
        // Added in 17: research, PostgreSQL 17, "Changes to existing statements"
        // (REL_17_11 gram.y 3093 `set_statistics_value`). REL_16_15 gram.y 2426
        // takes only `SignedIconst`.
        #[cfg(feature = "since-pg17")]
        #[tok(DEFAULT)]
        Default,
        Value(SignedIconst),
    }
}

recursa::ast_node! {
    /// `SET STATISTICS value` clause on `ALTER STATISTICS`.
    #[derive(Debug)]
    pub struct AlterStatisticsSetStatisticsClause {
        #[tok(SET, STATISTICS, this)]
        pub value: SetStatisticsValue,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterStatisticsAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetStatistics(AlterStatisticsSetStatisticsClause),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `ALTER STATISTICS [IF EXISTS] any_name action` — Postgres'
    /// `AlterStatsStmt` plus the statistics branches of `RenameStmt` /
    /// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
    ///
    /// `IF EXISTS` is permitted by Postgres only on the `SET STATISTICS`
    /// branch; pg-sql accepts it on every action (the differential corpus
    /// only exercises the SET STATISTICS form) — a strict per-action gate
    /// would require either a sub-grammar dispatcher or two separate
    /// statement types. The corpus oracle catches any regression here.
    #[derive(Debug)]
    #[tok(ALTER, STATISTICS, this)]
    pub struct AlterStatisticsStmt {
        pub if_exists: Option<IfExists>,
        pub name: QualifiedName,
        pub action: AlterStatisticsAction,
    }
}
