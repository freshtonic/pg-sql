//! AGGREGATE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

/// Argument signature for `CREATE AGGREGATE` — Postgres' `aggr_args`:
///
/// ```text
/// '(' '*' ')'
///   | '(' aggr_args_list ')'
///   | '(' ORDER BY aggr_args_list ')'
///   | '(' aggr_args_list ORDER BY aggr_args_list ')'
/// ```
///
/// `aggr_arg` is `func_arg` (re-using `FuncParam` here): a type, optionally
/// preceded by a mode keyword (`VARIADIC` / `IN` / `OUT` / `INOUT`) and an
/// argument name.
///
/// Variant ordering: `Star` first (the literal `*` is unambiguous), then
/// `OrderBy` (leading `ORDER BY`), then `BothArgs` (which has an args list
/// followed by `ORDER BY`), then `Args` (a bare args list). The peek
/// regex disambiguates `OrderBy` from `BothArgs`/`Args` by the leading
/// `ORDER` keyword; `BothArgs` and `Args` are disambiguated at parse time
/// by the presence of a trailing `ORDER BY`.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateAggregateArgs<'input> {
    #[tok(LPAREN, STAR, RPAREN)]
    Star,
    OrderBy(CreateAggregateOrderBy<'input>),
    Args(CreateAggregateArgLists<'input>),
}

/// `(ORDER BY aggr_args_list)` — ordered-set aggregate with no plain args.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateOrderBy<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub args: CreateAggregateOrderByInner<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateOrderByInner<'input> {
    #[tok(ORDER, BY, this)]
    #[sep(COMMA)]
    pub args: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input>>,
}

/// `(aggr_args_list [ORDER BY aggr_args_list])` — regular arguments with an
/// optional ordered-set tail.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct CreateAggregateArgLists<'input> {
    #[sep(COMMA)]
    pub direct: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input>>,
    pub ordered: Option<CreateAggregateOrderedTail<'input>>,
}

/// `ORDER BY aggr_args_list` tail of an ordered-set aggregate signature.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateOrderedTail<'input> {
    #[tok(ORDER, BY, this)]
    #[sep(COMMA)]
    pub ordered: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input>>,
}

/// `CREATE [OR REPLACE] AGGREGATE func_name { aggr_args (def_list) | (def_list) }`.
///
/// Old-style definition entries and modern aggregate arguments have an
/// intrinsically overlapping token shape: for example, `sfunc = int8pl` is
/// both a definition element and a function parameter with an `=` default.
/// Parse that shared first parenthesized group once as aggregate arguments;
/// the presence of a second definition list records the modern form. This
/// avoids making bounded lookahead search for a second `(` beyond an
/// arbitrarily long first group.
#[derive(recursa::Node, Debug, Clone)]
pub struct AggregateSig<'input> {
    pub name: QualifiedName<'input>,
    pub args: CreateAggregateArgs<'input>,
    pub definition: Option<DefList<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateStmt<'input> {
    #[tok(CREATE, this, AGGREGATE)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub signature: AggregateSig<'input>,
}

/// A single `DROP AGGREGATE` target: a qualified name plus its `(...)`
/// argument signature — Postgres' `aggregate_with_argtypes`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropAggregateTarget<'input> {
    pub name: QualifiedName<'input>,
    pub args: AggregateArgs<'input>,
}

/// `DROP AGGREGATE [IF EXISTS] name(args) [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, AGGREGATE, this)]
pub struct DropAggregateStmt<'input> {
    pub if_exists: Option<IfExists>,
    /// Greedy: a leading CASCADE, RESTRICT starts this element instead of ending `DropAggregateStmt` (bison shift preference).
    #[greedy(CASCADE, RESTRICT)]
    #[sep(COMMA)]
    pub targets: Vec<DropAggregateTarget<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// One action on `ALTER AGGREGATE name(args) action` — Postgres'
/// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches
/// for aggregates. Aggregates have no `[NO] DEPENDS ON EXTENSION` and no
/// `alterfunc_opt_list` form in gram.y.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterAggregateAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
}

/// `ALTER AGGREGATE aggregate_with_argtypes { RENAME TO new | OWNER TO
/// role | SET SCHEMA new }` — Postgres' `RenameStmt`, `AlterOwnerStmt`,
/// and `AlterObjectSchemaStmt` branches for aggregates.
///
/// The argument signature uses Postgres' full `aggr_args` shape (covers
/// `(*)`, plain type lists, and the ordered-set `(... ORDER BY ...)`
/// forms) — reuses [`CreateAggregateArgs`] from the CREATE AGGREGATE
/// path.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterAggregateStmt<'input> {
    #[tok(ALTER, AGGREGATE, this)]
    pub name: QualifiedName<'input>,
    pub args: CreateAggregateArgs<'input>,
    pub action: AlterAggregateAction<'input>,
}
