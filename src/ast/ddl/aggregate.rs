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

/// `ORDER BY aggr_args_list` — the `ORDER BY` leads the whole list, so it is
/// declared on the struct rather than on the repeated field.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ORDER, BY, this)]
pub struct CreateAggregateOrderByInner<'input> {
    #[sep(COMMA)]
    pub args: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input>>,
}

/// `= def_arg` — the value half of an old-style aggregate definition entry.
///
/// `aggr_arg` is gram.y's `func_arg`, which — unlike `func_arg_with_default`
/// — carries no default, so an `=` in this group is always an
/// `old_aggr_elem` value and never a parameter default. That makes
/// `def_arg`'s wider value language available here: an array type
/// (`STYPE = int[]`), a bare operator, a string, a signed number.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateDefValue<'input> {
    #[tok(EQ, this)]
    pub value: crate::ast::ddl::role::DefArg<'input>,
}

/// One entry of `CREATE AGGREGATE`'s first parenthesized group — an
/// `aggr_arg`, or an `old_aggr_elem` when it carries a `= value` tail.
///
/// gram.y keeps the two apart in separate productions (`DefineStmt: CREATE
/// AGGREGATE func_name aggr_args definition` versus `... func_name
/// old_aggr_definition`), but on the surface both are one comma-separated
/// group directly after the aggregate name, and they are told apart only by
/// the `=` that follows the first name. Parse the shared `[mode] [name]
/// type` prefix once and let the optional tail decide, rather than asking
/// bounded lookahead to choose between two arbitrarily long alternatives.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateArg<'input> {
    pub mode: Option<crate::ast::ddl::function::ArgMode>,
    pub first: crate::ast::ddl::function::FuncArgType<'input>,
    /// `name mode type` — gram.y's `func_arg` also admits the mode after
    /// the parameter name.
    pub name_mode: Option<crate::ast::ddl::function::ArgMode>,
    pub named_type: Option<crate::ast::ddl::function::FuncArgType<'input>>,
    pub value: Option<CreateAggregateDefValue<'input>>,
}

/// `(aggr_args_list [ORDER BY aggr_args_list])` — regular arguments with an
/// optional ordered-set tail, or the old-style `(name = value, ...)`
/// definition list, which occupies the same position.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct CreateAggregateArgLists<'input> {
    #[sep(COMMA)]
    pub direct: recursa::Vec1<CreateAggregateArg<'input>>,
    pub ordered: Option<CreateAggregateOrderedTail<'input>>,
}

/// `ORDER BY aggr_args_list` tail of an ordered-set aggregate signature.
///
/// `ORDER BY` leads the whole list, so it is declared on the struct.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ORDER, BY, this)]
pub struct CreateAggregateOrderedTail<'input> {
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
