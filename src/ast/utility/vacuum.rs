//! VACUUM statement and the shared `VacuumOption(s)` AST nodes used by
//! VACUUM/REINDEX/CLUSTER for their `( option [= value], ... )` lists.

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::shared::names::QualifiedName;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::FREEZE;
use crate::tokens::{literal, punct};

/// A single option inside a VACUUM/REINDEX `( ... )` list: `name [value]`.
///
/// The option name may be any SQL word (including keywords like `FULL`,
/// `FREEZE`, `PARALLEL`) so it uses `AliasName`. The value matches gram.y's
/// `utility_option_arg`: `opt_boolean_or_string | NumericOnly | EMPTY` —
/// i.e. `ON`, `OFF`, `TRUE`, `FALSE`, `DEFAULT`, a numeric (signed or not),
/// a string literal, or an identifier. We model that with `SetValue`, which
/// is the same vocabulary `SET` accepts.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct VacuumOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<crate::ast::session::set_reset::SetValue<'input>>,
}

/// Parenthesized options list: `( opt [= val] [, ...] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct VacuumOptions<'input> {
    pub list: Surrounded<punct::LParen, Seq0<VacuumOption<'input>, punct::Comma>, punct::RParen>,
}

// -----------------------------------------------------------------------
// VACUUM statement itself.
// -----------------------------------------------------------------------

// --- VACUUM ---

/// A single VACUUM/ANALYZE relation target — Postgres' `vacuum_relation`.
///
/// `qualified_name [(column [, ...])]`. The optional column list applies to
/// `VACUUM ANALYZE` to scope the analyze to specific columns.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct VacuumRelation<'input> {
    pub name: QualifiedName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq1<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
    >,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct VacuumStmt<'input> {
    pub vacuum: VACUUM,
    pub options: Option<VacuumOptions<'input>>,
    pub full: Option<FULL>,
    pub freeze: Option<FREEZE>,
    pub verbose: Option<VERBOSE>,
    pub analyze: Option<ANALYZE>,
    pub relations: Option<Seq1<VacuumRelation<'input>, punct::Comma>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_vacuum_full() {
        let mut input = crate::tokens::test_input("VACUUM (FULL) tbl");
        let stmt = VacuumStmt::parse(&mut input).unwrap();
        assert!(stmt.options.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_vacuum_full_freeze() {
        let mut input = crate::tokens::test_input("VACUUM (FULL, FREEZE) tbl");
        let _stmt = VacuumStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_vacuum_parallel_value() {
        let mut input = crate::tokens::test_input("VACUUM (PARALLEL 2) tbl");
        let _stmt = VacuumStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn vacuum_bare_is_modelled() {
        let stmt: VacuumStmt = parse_stmt("VACUUM");
        assert!(stmt.full.is_none());
        assert!(stmt.freeze.is_none());
        assert!(stmt.verbose.is_none());
        assert!(stmt.analyze.is_none());
        assert!(stmt.options.is_none());
        assert!(stmt.relations.is_none());
        reparse_stable::<VacuumStmt>("VACUUM");
    }

    #[test]
    fn vacuum_full_legacy_roundtrips() {
        let stmt: VacuumStmt = parse_stmt("VACUUM FULL vactst");
        assert!(stmt.full.is_some());
        reparse_stable::<VacuumStmt>("VACUUM FULL vactst");
    }

    #[test]
    fn vacuum_full_analyze_roundtrips() {
        let stmt: VacuumStmt = parse_stmt("VACUUM ANALYZE vacparted");
        assert!(stmt.analyze.is_some());
        reparse_stable::<VacuumStmt>("VACUUM ANALYZE vacparted");
    }

    #[test]
    fn vacuum_full_freeze_legacy_roundtrips() {
        reparse_stable::<VacuumStmt>("VACUUM FULL FREEZE vactst");
    }

    #[test]
    fn vacuum_verbose_analyze_roundtrips() {
        reparse_stable::<VacuumStmt>("VACUUM VERBOSE ANALYZE vactst");
    }

    #[test]
    fn vacuum_analyze_columns_roundtrips() {
        let stmt: VacuumStmt = parse_stmt("VACUUM ANALYZE vacparted(a, b, a)");
        assert_eq!(stmt.relations.as_ref().unwrap().len(), 1);
        reparse_stable::<VacuumStmt>("VACUUM ANALYZE vacparted(a, b, a)");
    }

    #[test]
    fn vacuum_multi_targets_roundtrips() {
        let stmt: VacuumStmt = parse_stmt("VACUUM ANALYZE vactst, vacparted (a)");
        assert_eq!(stmt.relations.as_ref().unwrap().len(), 2);
        reparse_stable::<VacuumStmt>("VACUUM ANALYZE vactst, vacparted (a)");
    }

    #[test]
    fn vacuum_options_with_target_roundtrips() {
        let stmt: VacuumStmt = parse_stmt("VACUUM (FULL, FREEZE) vactst");
        assert!(stmt.options.is_some());
        assert!(stmt.relations.is_some());
        reparse_stable::<VacuumStmt>("VACUUM (FULL, FREEZE) vactst");
    }

    #[test]
    fn vacuum_parallel_option_with_value_roundtrips() {
        reparse_stable::<VacuumStmt>("VACUUM (PARALLEL 2) pvactst");
    }
}
