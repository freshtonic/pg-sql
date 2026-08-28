/// MERGE statement AST.
///
/// ```sql
/// MERGE INTO [ONLY] target [[AS] alias]
/// USING source [[AS] alias] ON condition
/// WHEN [NOT] MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN
///     { UPDATE SET ... | DELETE | DO NOTHING
///     | INSERT [INTO target] [(cols)] { VALUES (...) [, (...)] | DEFAULT VALUES } }
/// [RETURNING ...]
/// ```
use recursa::seq::{OptionalTrailing, Seq0};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::dml::select::{PlainTable, TableRef};
use crate::ast::dml::update::{ReturningClause, SetAssignment};
use crate::ast::shared::expr::Expr;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `AND cond` qualifier on a WHEN clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AndCondition<'input> {
    pub and: AND,
    pub condition: Expr<'input>,
}

/// `BY SOURCE` or `BY TARGET`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NotMatchedBy {
    Source((BY, SOURCE)),
    Target((BY, TARGET)),
}

/// `UPDATE SET col = expr, ...` action body (the part after THEN).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UpdateAction<'input> {
    pub update: UPDATE,
    pub set: SET,
    pub assignments: Seq0<SetAssignment<'input>, punct::Comma>,
}

/// Action allowed after `WHEN MATCHED ... THEN`.
///
/// Variant ordering: `DoNothing` (`DO NOTHING`) and `Update` (`UPDATE`) and
/// `Delete` (`DELETE`) all start with distinct keywords, so order is by
/// declaration only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum MatchedAction<'input> {
    Update(UpdateAction<'input>),
    Delete(DELETE),
    DoNothing((DO, NOTHING)),
}

/// A single row of values: `(expr, ...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ValueRow<'input>(
    pub Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
);

/// `VALUES (row), (row), ...` body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InsertValuesBody<'input> {
    pub values: VALUES,
    pub rows: Seq0<ValueRow<'input>, punct::Comma>,
}

/// Body of an INSERT inside MERGE: `VALUES ...` or `DEFAULT VALUES`.
///
/// Variant ordering: `Default` (`DEFAULT VALUES`) is matched before
/// `Values` (`VALUES`) since they begin with different keywords.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum InsertBody<'input> {
    Default((DEFAULT, VALUES)),
    Values(InsertValuesBody<'input>),
}

/// Optional `INTO target_name` after `INSERT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InsertInto<'input> {
    pub into: INTO,
    pub name: crate::tokens::ColId<'input>,
}

/// `INSERT [INTO target] [(cols)] { VALUES ... | DEFAULT VALUES }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InsertAction<'input> {
    pub insert: INSERT,
    pub into: Option<InsertInto<'input>>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
    /// `OVERRIDING {SYSTEM|USER} VALUE` between the columns and the body.
    pub overriding: Option<crate::ast::dml::insert::OverridingClause>,
    pub body: InsertBody<'input>,
}

/// Action allowed after `WHEN NOT MATCHED ... THEN`.
///
/// `WHEN NOT MATCHED [BY TARGET]` takes an `INSERT` (or `DO NOTHING`);
/// `WHEN NOT MATCHED BY SOURCE` takes an `UPDATE` / `DELETE` instead
/// (the target row exists, the source row does not). Which `by` form
/// permits which action is a semantic rule, so all four are accepted
/// grammatically.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NotMatchedAction<'input> {
    Insert(InsertAction<'input>),
    Update(UpdateAction<'input>),
    Delete(DELETE),
    DoNothing((DO, NOTHING)),
}

/// `WHEN NOT MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN action`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WhenNotMatched<'input> {
    pub when: WHEN,
    pub not: NOT,
    pub matched: MATCHED,
    pub by: Option<NotMatchedBy>,
    pub and: Option<AndCondition<'input>>,
    pub then: THEN,
    pub action: NotMatchedAction<'input>,
}

/// `WHEN MATCHED [AND cond] THEN action`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WhenMatched<'input> {
    pub when: WHEN,
    pub matched: MATCHED,
    pub and: Option<AndCondition<'input>>,
    pub then: THEN,
    pub action: MatchedAction<'input>,
}

/// A WHEN clause in MERGE.
///
/// Variant ordering: `NotMatched` (`WHEN NOT MATCHED`) is longer than
/// `Matched` (`WHEN MATCHED`); list it first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WhenClause<'input> {
    NotMatched(WhenNotMatched<'input>),
    Matched(WhenMatched<'input>),
}

/// MERGE statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dml"])]
pub struct MergeStmt<'input> {
    pub merge: MERGE,
    pub into: INTO,
    pub target: Box<PlainTable<'input>>,
    pub using: USING,
    pub source: Box<TableRef<'input>>,
    pub on: ON,
    pub condition: Box<Expr<'input>>,
    pub when_clauses: Seq0<WhenClause<'input>, (), OptionalTrailing>,
    pub returning: Option<Box<ReturningClause<'input>>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_merge_basic() {
        let sql = "MERGE INTO m USING (select 0 k, 'v' v) o ON m.k = o.k WHEN MATCHED THEN UPDATE SET v = 'updated' WHEN NOT MATCHED THEN INSERT VALUES(o.k, o.v)";
        let mut input = crate::tokens::test_input(sql);
        let stmt = MergeStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.when_clauses.len(), 2);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_merge_target_alias() {
        let sql = "MERGE INTO target t USING source s ON t.tid = s.sid WHEN MATCHED THEN DELETE";
        let mut input = crate::tokens::test_input(sql);
        let _stmt = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_merge_when_matched_and() {
        let sql =
            "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED AND t.a = 2 THEN UPDATE SET b = s.b";
        let mut input = crate::tokens::test_input(sql);
        let _stmt = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_merge_not_matched_by_source_default_values() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN NOT MATCHED BY SOURCE THEN INSERT DEFAULT VALUES";
        let mut input = crate::tokens::test_input(sql);
        let _stmt = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// `WHEN NOT MATCHED BY SOURCE` accepts `UPDATE` / `DELETE` (PG17),
    /// and a MERGE may carry a `RETURNING` clause.
    #[test]
    fn parse_merge_not_matched_by_source_update_delete() {
        for src in [
            "MERGE INTO t USING s ON t.a = s.a \
             WHEN NOT MATCHED BY SOURCE THEN DELETE",
            "MERGE INTO t USING s ON t.a = s.a \
             WHEN NOT MATCHED BY SOURCE AND s.b = 1 THEN UPDATE SET b = 0",
            "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DELETE \
             RETURNING merge_action(), t.*",
        ] {
            let mut input = crate::tokens::test_input(src);
            MergeStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_merge_do_nothing_both() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DO NOTHING WHEN NOT MATCHED THEN DO NOTHING";
        let mut input = crate::tokens::test_input(sql);
        let _stmt = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_merge_insert_multi_values() {
        let sql =
            "MERGE INTO t USING s ON t.a = s.a WHEN NOT MATCHED THEN INSERT VALUES (1,1), (2,2)";
        let mut input = crate::tokens::test_input(sql);
        let _stmt = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_merge_insert_into_default_values() {
        let sql = "MERGE INTO target t USING source s ON t.tid = s.sid WHEN NOT MATCHED THEN INSERT INTO target DEFAULT VALUES";
        let mut input = crate::tokens::test_input(sql);
        let _stmt = MergeStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
