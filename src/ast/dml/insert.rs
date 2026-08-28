/// INSERT INTO statement AST.
///
/// Supports: `INSERT INTO table [(cols)] source [ON CONFLICT ...] [RETURNING ...]`
/// where source is DEFAULT VALUES, VALUES rows, or SELECT query.
use recursa::seq::Seq0;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::dml::select::WhereClause;
use crate::ast::dml::update::{ReturningClause, SetAssignment};
use crate::ast::dml::values::Subquery;
use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::tokens::punct;

use crate::tokens::keyword::*;

/// `[AS] alias` on INSERT target table, e.g. `INSERT INTO t AS x`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InsertTableAlias<'input> {
    pub r#as: AS,
    pub name: crate::tokens::ColId<'input>,
}

/// `OVERRIDING {SYSTEM | USER} VALUE` clause on an INSERT statement.
///
/// Variant ordering: distinct first tokens (`SYSTEM` vs `USER`), so
/// declaration order is cosmetic.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OverridingClause {
    pub overriding: OVERRIDING,
    pub which: OverridingKind,
    pub value: VALUE,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OverridingKind {
    System(SYSTEM),
    User(USER),
}

/// Multiple value rows: `VALUES (row1), (row2), ...`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InsertValueRows<'input> {
    pub values: VALUES,
    pub rows: Seq0<ValueList<'input>, punct::Comma>,
}

/// Insert source: DEFAULT VALUES, VALUES (row), ..., or SELECT query.
///
/// Variant ordering: Default (`DEFAULT VALUES`) is longer than Rows (`VALUES`),
/// so longest-match-wins picks it when DEFAULT is present.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum InsertSource<'input> {
    Default((DEFAULT, VALUES)),
    Rows(InsertValueRows<'input>),
    Select(Box<Subquery<'input>>),
}

/// DO UPDATE SET ... [WHERE ...] action.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DoUpdateAction<'input> {
    pub r#do: DO,
    pub update: UPDATE,
    pub set: SET,
    pub assignments: Seq0<SetAssignment<'input>, punct::Comma>,
    pub where_clause: Option<WhereClause<'input>>,
}

/// ON CONFLICT action: DO UPDATE SET ... [WHERE ...] or DO NOTHING.
///
/// Variant ordering: DoUpdate (`DO UPDATE SET`) is longer than
/// DoNothing (`DO NOTHING`), but both start with `DO` and diverge
/// at the next keyword, so the regex disambiguates.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ConflictAction<'input> {
    DoUpdate(Box<DoUpdateAction<'input>>),
    DoNothing((DO, NOTHING)),
}

/// One entry in an `ON CONFLICT (...)` target list.
///
/// Matches the index-element grammar: an expression (plain column name,
/// qualified name, parenthesized expression, or function call) optionally
/// followed by a `COLLATE "name"` clause and an optional opclass ident.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ConflictTargetItem<'input> {
    pub expr: Expr<'input>,
    pub collate: Option<crate::ast::ddl::table::CollateClause<'input>>,
    pub opclass: Option<crate::tokens::ColId<'input>>,
}

/// `ON CONSTRAINT name` arbiter form of `opt_conf_expr` — names a unique
/// or exclusion constraint directly instead of inferring from column list.
/// Per gram.y `opt_conf_expr: ON CONSTRAINT name`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OnConflictConstraint<'input> {
    pub on: ON,
    pub constraint: CONSTRAINT,
    pub name: crate::tokens::ColId<'input>,
}

/// Arbiter specification for `ON CONFLICT` — Postgres' `opt_conf_expr`.
///
/// Variant ordering: each variant has a distinct first token (`(` vs `ON`),
/// so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ConflictTarget<'input> {
    /// `( index_params )` — the inferring-by-columns form.
    Index(Surrounded<punct::LParen, Seq0<ConflictTargetItem<'input>, punct::Comma>, punct::RParen>),
    /// `ON CONSTRAINT name` — the named-constraint form.
    Constraint(OnConflictConstraint<'input>),
}

/// ON CONFLICT clause: `ON CONFLICT [(col, ...) | ON CONSTRAINT name]
/// DO UPDATE SET ... | DO NOTHING`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OnConflictClause<'input> {
    pub on: ON,
    pub conflict: CONFLICT,
    pub target: Option<ConflictTarget<'input>>,
    /// `WHERE predicate` after the arbiter target list, restricting the
    /// partial-index arbiter to matching rows. Only valid for the
    /// index-params form (gram.y attaches `where_clause` to that branch
    /// only); attached at the outer struct so the enum stays simple.
    pub where_clause: Option<WhereClause<'input>>,
    pub action: ConflictAction<'input>,
}

/// INSERT INTO statement with optional ON CONFLICT and RETURNING.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dml"])]
#[format_tokens(group(consistent))]
pub struct InsertStmt<'input> {
    pub insert: INSERT,
    pub into: INTO,
    pub table_name: QualifiedName<'input>,
    /// Optional `[AS] alias` after the target table, used to rebind the
    /// target in ON CONFLICT DO UPDATE expressions.
    pub alias: Option<InsertTableAlias<'input>>,
    pub columns: Option<Box<ColumnList<'input>>>,
    /// `OVERRIDING {SYSTEM|USER} VALUE` between the column list and the
    /// source. Controls whether explicit values override GENERATED ALWAYS
    /// identity columns.
    pub overriding: Option<OverridingClause>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub source: Box<InsertSource<'input>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub on_conflict: Option<Box<OnConflictClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub returning: Option<Box<ReturningClause<'input>>>,
}

/// One target column of an INSERT column list: a column name plus an
/// optional indirection chain — `f2[1]`, `f3.if1`, `a[1:5]` (Postgres
/// `insert_column_item: ColId opt_indirection`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InsertColumnItem<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub indirection: Vec<crate::ast::shared::expr::IndirectionEl<'input>>,
}

/// Column list: `(col1, col2[1], col3.field, ...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
#[recursa::parser(rules = SqlRules)]
pub struct ColumnList<'input>(
    #[deref]
    pub  Surrounded<punct::LParen, Seq0<InsertColumnItem<'input>, punct::Comma>, punct::RParen>,
);

/// Value list: `(col1, col2, ...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
#[recursa::parser(rules = SqlRules)]
pub struct ValueList<'input>(
    #[deref] pub Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
);

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::dml::insert::InsertStmt;

    #[test]
    fn parse_insert_qualified_table() {
        let mut input = crate::tokens::test_input("INSERT INTO pg_catalog.foo VALUES (1)");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "foo");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_insert_with_columns() {
        let mut input = crate::tokens::test_input("INSERT INTO BOOLTBL1 (f1) VALUES (bool 't')");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.table_name.object(), "BOOLTBL1");
        assert!(stmt.columns.is_some());
        assert_eq!(stmt.columns.as_ref().unwrap().inner.len(), 1);
        assert!(input.is_empty());
    }

    /// An INSERT column-list target may carry an indirection chain —
    /// `f2[1]`, `f3.if1`, `a[1:5]` (Postgres `insert_column_item`).
    #[test]
    fn parse_insert_column_indirection() {
        for src in [
            "INSERT INTO t (f2[1], f2[2]) VALUES (1, 2)",
            "INSERT INTO t (f3.if1, f3.if2) VALUES (1, '{foo}')",
            "INSERT INTO t (a[1:5], b[1:1][1:2]) VALUES ('{1}', '{2}')",
        ] {
            let mut input = crate::tokens::test_input(src);
            InsertStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_insert_multiple_columns() {
        let mut input =
            crate::tokens::test_input("INSERT INTO BOOLTBL3 (d, b, o) VALUES ('true', true, 1)");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.columns.as_ref().unwrap().inner.len(), 3);
    }

    #[test]
    fn parse_insert_without_columns() {
        let mut input =
            crate::tokens::test_input("INSERT INTO booltbl4 VALUES (false, true, null)");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(stmt.columns.is_none());
    }

    #[test]
    fn parse_insert_default_values_returning() {
        let mut input = crate::tokens::test_input("INSERT INTO t DEFAULT VALUES RETURNING *");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(matches!(*stmt.source, super::InsertSource::Default(_)));
        assert!(stmt.returning.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_insert_select() {
        let mut input = crate::tokens::test_input("INSERT INTO y SELECT generate_series(1, 10)");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(matches!(*stmt.source, super::InsertSource::Select(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_insert_on_conflict_do_nothing() {
        let mut input =
            crate::tokens::test_input("INSERT INTO t VALUES (1) ON CONFLICT (k) DO NOTHING");
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_insert_on_conflict_do_update() {
        let mut input = crate::tokens::test_input(
            "INSERT INTO t VALUES (1) ON CONFLICT (k) DO UPDATE SET v = 'updated'",
        );
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_empty());
    }

    /// `ON CONFLICT ON CONSTRAINT name DO …` — the arbiter-by-constraint form
    /// of `opt_conf_expr` (gram.y `ON CONSTRAINT name`). Distinct from the
    /// `( index_params )` form.
    #[test]
    fn parse_insert_on_conflict_on_constraint_do_nothing() {
        let mut input = crate::tokens::test_input(
            "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING",
        );
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_insert_on_conflict_on_constraint_do_update() {
        let mut input = crate::tokens::test_input(
            "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO UPDATE SET v = 'x'",
        );
        let stmt = InsertStmt::parse(&mut input).unwrap();
        assert!(stmt.on_conflict.is_some());
        assert!(input.is_empty());
    }
}
