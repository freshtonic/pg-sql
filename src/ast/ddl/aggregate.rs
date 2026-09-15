//! AGGREGATE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum CreateAggregateArgs {
        #[tok(LPAREN, STAR, RPAREN)]
        Star,
        OrderBy(CreateAggregateOrderBy),
        /// gram.y `old_aggr_definition`: `(name = value, ...)`. Listed before
        /// `Args`: both open with `(`, and `name =` decides for this form (an
        /// `aggr_arg` takes no default, so `=` never follows one).
        Old(OldAggregateDefinition),
        Args(CreateAggregateArgLists),
    }
}

recursa::ast_node! {
    /// gram.y `old_aggr_definition: '(' old_aggr_list ')'`.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct OldAggregateDefinition {
        #[sep(COMMA)]
        pub elems: one_or_many!(OldAggregateElem),
    }
}

recursa::ast_node! {
    /// gram.y `old_aggr_elem: IDENT '=' def_arg`.
    #[derive(Debug)]
    pub struct OldAggregateElem {
        pub name: crate::tokens::literal::IdentOnly,
        #[tok(EQ, this)]
        pub value: crate::ast::ddl::role::DefArg,
    }
}

recursa::ast_node! {
    /// `(ORDER BY aggr_args_list)` — ordered-set aggregate with no plain args.
    #[derive(Debug)]
    pub struct CreateAggregateOrderBy {
        #[tok(LPAREN, this, RPAREN)]
        pub args: CreateAggregateOrderByInner,
    }
}

recursa::ast_node! {
    /// `ORDER BY aggr_args_list` — the `ORDER BY` leads the whole list, so it is
    /// declared on the struct rather than on the repeated field.
    #[derive(Debug)]
    #[tok(ORDER, BY, this)]
    pub struct CreateAggregateOrderByInner {
        #[sep(COMMA)]
        pub args: one_or_many!(crate::ast::ddl::function::FunctionArg),
    }
}

recursa::ast_node! {
    /// One entry of `CREATE AGGREGATE`'s first parenthesized group — an
    /// `aggr_arg`, or an `old_aggr_elem` when it carries a `= value` tail.
    ///
    /// gram.y keeps the two apart in separate productions (`DefineStmt: CREATE
    /// AGGREGATE func_name aggr_args definition` versus `... func_name
    /// old_aggr_definition`), but on the surface both are one comma-separated
    /// group directly after the aggregate name, and they are told apart only by
    /// the `=` that follows the first name. Parse the shared `[mode] [name]
    /// type` prefix once and let the optional tail decide, avoiding two LR
    /// productions with an arbitrarily long shared prefix.
    #[derive(Debug)]
    pub struct CreateAggregateArg {
        /// gram.y `aggr_arg: func_arg`.
        pub arg: crate::ast::ddl::function::FunctionArg,
    }
}

recursa::ast_node! {
    /// `(aggr_args_list [ORDER BY aggr_args_list])` — regular arguments with an
    /// optional ordered-set tail, or the old-style `(name = value, ...)`
    /// definition list, which occupies the same position.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CreateAggregateArgLists {
        #[sep(COMMA)]
        pub direct: one_or_many!(CreateAggregateArg),
        pub ordered: Option<CreateAggregateOrderedTail>,
    }
}

recursa::ast_node! {
    /// `ORDER BY aggr_args_list` tail of an ordered-set aggregate signature.
    ///
    /// `ORDER BY` leads the whole list, so it is declared on the struct.
    #[derive(Debug)]
    #[tok(ORDER, BY, this)]
    pub struct CreateAggregateOrderedTail {
        #[sep(COMMA)]
        pub ordered: one_or_many!(crate::ast::ddl::function::FunctionArg),
    }
}

recursa::ast_node! {
    /// `CREATE [OR REPLACE] AGGREGATE func_name { aggr_args (def_list) | (def_list) }`.
    ///
    /// Old-style definition entries and modern aggregate arguments have an
    /// intrinsically overlapping token shape: for example, `sfunc = int8pl` is
    /// both a definition element and a function parameter with an `=` default.
    /// Parse that shared first parenthesized group once as aggregate arguments;
    /// the presence of a second definition list records the modern form. This
    /// lets the LR parser decide on the `(` after the completed first group.
    #[derive(Debug)]
    pub struct AggregateSig {
        pub name: QualifiedName,
        pub args: CreateAggregateArgs,
        pub definition: Option<DefList>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CreateAggregateStmt {
        #[tok(CREATE, this, AGGREGATE)]
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        pub signature: AggregateSig,
    }
}

recursa::ast_node! {
    /// A single `DROP AGGREGATE` target: a qualified name plus its `(...)`
    /// argument signature — Postgres' `aggregate_with_argtypes`.
    #[derive(Debug)]
    pub struct DropAggregateTarget {
        pub name: QualifiedName,
        pub args: AggregateArgs,
    }
}

recursa::ast_node! {
    /// `DROP AGGREGATE [IF EXISTS] name(args) [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, AGGREGATE, this)]
    pub struct DropAggregateStmt {
        pub if_exists: Option<IfExists>,
        #[sep(COMMA)]
        pub targets: zero_or_many!(DropAggregateTarget),
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One action on `ALTER AGGREGATE name(args) action` — Postgres'
    /// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches
    /// for aggregates. Aggregates have no `[NO] DEPENDS ON EXTENSION` and no
    /// `alterfunc_opt_list` form in gram.y.
    ///
    /// Variant ordering: each variant has a distinct leading keyword
    /// (`RENAME`, `OWNER`, `SET`), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterAggregateAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
    }
}

recursa::ast_node! {
    /// `ALTER AGGREGATE aggregate_with_argtypes { RENAME TO new | OWNER TO
    /// role | SET SCHEMA new }` — Postgres' `RenameStmt`, `AlterOwnerStmt`,
    /// and `AlterObjectSchemaStmt` branches for aggregates.
    ///
    /// The argument signature uses Postgres' full `aggr_args` shape (covers
    /// `(*)`, plain type lists, and the ordered-set `(... ORDER BY ...)`
    /// forms) — reuses [`CreateAggregateArgs`] from the CREATE AGGREGATE
    /// path.
    #[derive(Debug)]
    pub struct AlterAggregateStmt {
        #[tok(ALTER, AGGREGATE, this)]
        pub name: QualifiedName,
        pub args: CreateAggregateArgs,
        pub action: AlterAggregateAction,
    }
}
