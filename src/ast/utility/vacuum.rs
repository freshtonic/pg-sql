//! VACUUM statement and the shared `VacuumOption(s)` AST nodes used by
//! VACUUM/REINDEX/CLUSTER for their `( option [= value], ... )` lists.

use recursa::seq::{Seq0, Seq1};
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
#[derive(recursa::Node, Debug, Clone)]
pub struct VacuumOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<crate::ast::session::set_reset::SetValue<'input>>,
}

/// Parenthesized options list: `( opt [= val] [, ...] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct VacuumOptions<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub list:  Vec<VacuumOption<'input> > ,
}

// -----------------------------------------------------------------------
// VACUUM statement itself.
// -----------------------------------------------------------------------

// --- VACUUM ---

/// A single VACUUM/ANALYZE relation target — Postgres' `vacuum_relation`.
///
/// `qualified_name [(column [, ...])]`. The optional column list applies to
/// `VACUUM ANALYZE` to scope the analyze to specific columns.
#[derive(recursa::Node, Debug, Clone)]
pub struct VacuumRelation<'input> {
    pub name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<
         recursa::Vec1<crate::tokens::ColId<'input> > ,
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
#[derive(recursa::Node, Debug, Clone)]
pub struct VacuumStmt<'input> {
    #[tok(VACUUM, this)]
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
    pub relations: Option<recursa::Vec1<VacuumRelation<'input> >>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_vacuum_full() {
        let lexed = crate::tokens::lex("VACUUM (FULL) tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = VacuumStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.options.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_vacuum_full_freeze() {
        let lexed = crate::tokens::lex("VACUUM (FULL, FREEZE) tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = VacuumStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_vacuum_parallel_value() {
        let lexed = crate::tokens::lex("VACUUM (PARALLEL 2) tbl");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = VacuumStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
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
