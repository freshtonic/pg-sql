//! VACUUM statement and the shared `VacuumOption(s)` AST nodes used by
//! VACUUM/REINDEX/CLUSTER for their `( option [= value], ... )` lists.

use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

/// A single option inside a VACUUM/REINDEX `( ... )` list: `name [value]`.
///
/// The option name may be any SQL word (including keywords like `FULL`,
/// `FREEZE`, `PARALLEL`) so it uses `AliasName`. The value matches gram.y's
/// `utility_option_arg`: `opt_boolean_or_string | NumericOnly | EMPTY` —
/// i.e. `ON`, `OFF`, `TRUE`, `FALSE`, `DEFAULT`, a numeric (signed or not),
/// a string literal, or an identifier. We model that with `SetValue`, which
/// is the same vocabulary `SET` accepts.
#[derive(recursa::Node, Debug, Clone)]
pub struct VacuumOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<crate::ast::session::set_reset::SetValue<'input>>,
}

/// Parenthesized options list: `( opt [= val] [, ...] )`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct VacuumOptions<'input> {
    #[sep(COMMA)]
    pub list: Vec<VacuumOption<'input>>,
}

// -----------------------------------------------------------------------
// VACUUM statement itself.
// -----------------------------------------------------------------------

// --- VACUUM ---

/// A single VACUUM/ANALYZE relation target — Postgres' `vacuum_relation`.
///
/// `qualified_name [(column [, ...])]`. The optional column list applies to
/// `VACUUM ANALYZE` to scope the analyze to specific columns.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct VacuumColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<crate::tokens::ColId<'input>>,
);

#[derive(recursa::Node, Debug, Clone)]
pub struct VacuumRelation<'input> {
    pub name: QualifiedName<'input>,
    pub columns: Option<VacuumColumnList<'input>>,
}

/// `VACUUM` statement supporting both forms in Postgres' `gram.y`:
///
/// ```sql
/// VACUUM [FULL] [FREEZE] [VERBOSE] [ANALYZE] [vacuum_relation [, ...]]
/// VACUUM '(' option [, ...] ')' [vacuum_relation [, ...]]
/// ```
///
/// In the parenthesised form, `options` is `Some` and the four legacy
/// keyword fields are all `None`. In the legacy form, `options` is `None`
/// and any combination of `FULL` / `FREEZE` / `VERBOSE` / `ANALYZE` is
/// permitted in that fixed declaration order. Both forms share the optional
/// trailing relation list.
#[derive(recursa::Node, Debug, Clone)]
#[tok(VACUUM, this)]
pub struct VacuumStmt<'input> {
    pub options: Option<VacuumOptions<'input>>,
    #[presence(FULL)]
    pub full: bool,
    #[presence(FREEZE)]
    pub freeze: bool,
    #[presence(VERBOSE)]
    pub verbose: bool,
    #[presence(ANALYZE)]
    pub analyze: bool,
    #[sep(COMMA)]
    pub relations: Option<recursa::Vec1<VacuumRelation<'input>>>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/utility/vacuum.tests.rs"
));
