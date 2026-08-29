//! AGGREGATE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

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
    #[tok(LPAREN, STAR, RPAREN)] Star,
    OrderBy(CreateAggregateOrderBy<'input>),
    BothArgs(CreateAggregateBothArgs<'input>),
    Args(
        #[tok(LPAREN, this, RPAREN)]
        #[sep(COMMA)]


            recursa::Vec1<crate::ast::ddl::function::FuncParam<'input> >

        ,
    ),
}

/// `(ORDER BY aggr_args_list)` — ordered-set aggregate with no plain args.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateOrderBy<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub args:  CreateAggregateOrderByInner<'input> ,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateOrderByInner<'input> {
    #[tok(ORDER, BY, this)]
    #[sep(COMMA)]
    pub args: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input> >,
}

/// `(aggr_args_list ORDER BY aggr_args_list)` — ordered-set aggregate with
/// direct args and ordered args.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateBothArgs<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub args:  CreateAggregateBothArgsInner<'input> ,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateAggregateBothArgsInner<'input> {
    #[sep(COMMA)]
    pub direct: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input> >,
    #[tok(ORDER, BY, this)]
    #[sep(COMMA)]
    pub ordered: recursa::Vec1<crate::ast::ddl::function::FuncParam<'input> >,
}

/// Modern-style aggregate signature: `func_name aggr_args`. The argument
/// signature is required.
#[derive(recursa::Node, Debug, Clone)]
pub struct AggregateModernSig<'input> {
    pub name: QualifiedName<'input>,
    pub args: CreateAggregateArgs<'input>,
    pub definition: DefList<'input>,
}

/// Old-style aggregate signature: `func_name (def_list)` with no separate
/// argument list; the argument type is encoded as a `BASETYPE = ...` def-elem.
#[derive(recursa::Node, Debug, Clone)]
pub struct AggregateOldStyleSig<'input> {
    pub name: QualifiedName<'input>,
    pub definition: DefList<'input>,
}

/// `CREATE [OR REPLACE] AGGREGATE func_name { aggr_args (def_list) | (def_list) }`.
///
/// Variant ordering: `Modern` (name + `aggr_args` + def_list) before
/// `OldStyle` (name + def_list). After the `func_name`, the next token
/// decides: `(` followed by either `*` or a type-name signature is `Modern`;
/// `(` followed by an ident/key=value is `OldStyle`. Both start with `(`, so
/// peek-level disambiguation cannot distinguish them — but the longer
/// `Modern` form fully consumes its `aggr_args` `(...)` and then sees another
/// `(` to start the def_list, while `OldStyle` sees only one set of parens.
/// Listing `Modern` first ensures it is tried first.
#[derive(recursa::Node, Debug, Clone)]
pub enum AggregateSig<'input> {
    Modern(AggregateModernSig<'input>),
    OldStyle(AggregateOldStyleSig<'input>),
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
pub struct DropAggregateStmt<'input> {
    #[tok(DROP, AGGREGATE, this)]
    pub if_exists: Option<IfExists>,
    #[sep(COMMA)]
    pub targets: Vec<DropAggregateTarget<'input> >,
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

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    /// `CREATE AGGREGATE name(args) ( ... SFUNC = balkifnull(int8, int4) ... )`
    /// — the aggregates.sql corpus exercises a `def_arg` whose value is a
    /// function-style name with type-name arguments. PG's `def_arg` accepts
    /// this via `func_type → Typename → GenericType → name opt_type_modifiers`
    /// (`opt_type_modifiers` is `'(' expr_list ')'`), but pg-sql's
    /// `TypePrecision` is restricted to signed-integer modifiers, so the
    /// function-style form needs a dedicated `DefArg::FuncWithArgs` variant.
    #[test]
    fn parse_create_aggregate_funcname_def_arg() {
        for src in [
            "CREATE AGGREGATE balk(int4) ( SFUNC = balkifnull(int8, int4), STYPE = int8 )",
            "CREATE AGGREGATE balk(int4) ( SFUNC = balkifnull(int8, int4), STYPE = int8, PARALLEL = SAFE, INITCOND = '0' )",
            "CREATE AGGREGATE balk(int4) ( SFUNC = int4_sum(int8, int4), STYPE = int8, COMBINEFUNC = balkifnull(int8, int8), PARALLEL = SAFE, INITCOND = '0' )",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_drop_aggregate_typed() {
        let lexed = crate::tokens::lex("DROP AGGREGATE myavg(numeric)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.targets.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_aggregate_star() {
        let lexed = crate::tokens::lex("DROP AGGREGATE IF EXISTS test_agg(*)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(matches!(
            stmt.targets.iter().next().unwrap().args,
            AggregateArgs::Star(_)
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_modern() {
        let lexed = crate::tokens::lex("CREATE AGGREGATE sumdouble (float8) (sfunc = float8pl, stype = float8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.signature, AggregateSig::Modern(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_old_style() {
        let lexed = crate::tokens::lex("CREATE AGGREGATE newavg (sfunc = int4_avg_accum, basetype = int4, stype = _int8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.signature, AggregateSig::OldStyle(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_zero_args() {
        let lexed = crate::tokens::lex("CREATE AGGREGATE newcnt (*) (sfunc = int8inc, stype = int8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.signature, AggregateSig::Modern(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_ordered_set() {
        let lexed = crate::tokens::lex("CREATE AGGREGATE my_percentile_disc(float8 ORDER BY anyelement) \
             (stype = internal, sfunc = ordered_set_transition)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.signature, AggregateSig::Modern(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_or_replace() {
        let lexed = crate::tokens::lex("CREATE OR REPLACE AGGREGATE myavg (numeric) (stype = numeric, sfunc = numeric_add)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.or_replace.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn alter_aggregate_rename() {
        let stmt: AlterAggregateStmt =
            parse_stmt("ALTER AGGREGATE alt_agg1(int) RENAME TO alt_agg2");
        assert_eq!(stmt.name.object(), "alt_agg1");
        assert!(matches!(stmt.action, AlterAggregateAction::Rename(_)));
        reparse_stable::<AlterAggregateStmt>("ALTER AGGREGATE alt_agg1(int) RENAME TO alt_agg2");
    }

    #[test]
    fn alter_aggregate_owner() {
        reparse_stable::<AlterAggregateStmt>(
            "ALTER AGGREGATE alt_agg2(int) OWNER TO regress_alter_generic_user3",
        );
    }

    #[test]
    fn alter_aggregate_set_schema() {
        reparse_stable::<AlterAggregateStmt>("ALTER AGGREGATE alt_agg2(int) SET SCHEMA alt_nsp2");
    }

    #[test]
    fn alter_aggregate_star_args() {
        reparse_stable::<AlterAggregateStmt>("ALTER AGGREGATE my_count(*) RENAME TO new_count");
    }

    #[test]
    fn alter_aggregate_order_by_args() {
        // Ordered-set aggregate signature with `ORDER BY`.
        reparse_stable::<AlterAggregateStmt>(
            "ALTER AGGREGATE my_percentile_disc(float8 ORDER BY anyelement) RENAME TO test_percentile_disc",
        );
    }

    #[test]
    fn alter_aggregate_variadic_order_by() {
        reparse_stable::<AlterAggregateStmt>(
            "ALTER AGGREGATE my_rank(VARIADIC \"any\" ORDER BY VARIADIC \"any\") RENAME TO test_rank",
        );
    }
}
