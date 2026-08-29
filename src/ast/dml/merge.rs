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

use crate::ast::dml::select::{PlainTable, TableRef};
use crate::ast::dml::update::{ReturningClause, SetAssignment};
use crate::ast::shared::expr::Expr;
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `AND cond` qualifier on a WHEN clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct AndCondition<'input> {
    #[tok(AND, this)]
    pub condition: Expr<'input>,
}

/// `BY SOURCE` or `BY TARGET`.
#[derive(recursa::Node, Debug, Clone)]
pub enum NotMatchedBy {
    #[tok(BY, SOURCE)] Source,
    #[tok(BY, TARGET)] Target,
}

/// `UPDATE SET col = expr, ...` action body (the part after THEN).
#[derive(recursa::Node, Debug, Clone)]
pub struct UpdateAction<'input> {
    #[tok(UPDATE, SET, this)]
    #[sep(COMMA)]
    pub assignments: Vec<SetAssignment<'input> >,
}

/// Action allowed after `WHEN MATCHED ... THEN`.
///
/// Variant ordering: `DoNothing` (`DO NOTHING`) and `Update` (`UPDATE`) and
/// `Delete` (`DELETE`) all start with distinct keywords, so order is by
/// declaration only.
#[derive(recursa::Node, Debug, Clone)]
pub enum MatchedAction<'input> {
    Update(UpdateAction<'input>),
    #[tok(DELETE)] Delete,
    #[tok(DO, NOTHING)] DoNothing,
}

/// A single row of values: `(expr, ...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ValueRow<'input>(
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub  Vec<Expr<'input> > ,
);

/// `VALUES (row), (row), ...` body.
#[derive(recursa::Node, Debug, Clone)]
pub struct InsertValuesBody<'input> {
    #[tok(VALUES, this)]
    #[sep(COMMA)]
    pub rows: Vec<ValueRow<'input> >,
}

/// Body of an INSERT inside MERGE: `VALUES ...` or `DEFAULT VALUES`.
///
/// Variant ordering: `Default` (`DEFAULT VALUES`) is matched before
/// `Values` (`VALUES`) since they begin with different keywords.
#[derive(recursa::Node, Debug, Clone)]
pub enum InsertBody<'input> {
    #[tok(DEFAULT, VALUES)] Default,
    Values(InsertValuesBody<'input>),
}

/// Optional `INTO target_name` after `INSERT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct InsertInto<'input> {
    #[tok(INTO, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `INSERT [INTO target] [(cols)] { VALUES ... | DEFAULT VALUES }`
#[derive(recursa::Node, Debug, Clone)]
pub struct InsertAction<'input> {
    #[tok(INSERT, this)]
    pub into: Option<InsertInto<'input>>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Option<
         Vec<literal::AliasName<'input> > ,
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
#[derive(recursa::Node, Debug, Clone)]
pub enum NotMatchedAction<'input> {
    Insert(InsertAction<'input>),
    Update(UpdateAction<'input>),
    #[tok(DELETE)] Delete,
    #[tok(DO, NOTHING)] DoNothing,
}

/// `WHEN NOT MATCHED [BY {SOURCE|TARGET}] [AND cond] THEN action`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WhenNotMatched<'input> {
    #[tok(WHEN, NOT, MATCHED, this)]
    pub by: Option<NotMatchedBy>,
    pub and: Option<AndCondition<'input>>,
    #[tok(THEN, this)]
    pub action: NotMatchedAction<'input>,
}

/// `WHEN MATCHED [AND cond] THEN action`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WhenMatched<'input> {
    #[tok(WHEN, MATCHED, this)]
    pub and: Option<AndCondition<'input>>,
    #[tok(THEN, this)]
    pub action: MatchedAction<'input>,
}

/// A WHEN clause in MERGE.
///
/// Variant ordering: `NotMatched` (`WHEN NOT MATCHED`) is longer than
/// `Matched` (`WHEN MATCHED`); list it first.
#[derive(recursa::Node, Debug, Clone)]
pub enum WhenClause<'input> {
    NotMatched(WhenNotMatched<'input>),
    Matched(WhenMatched<'input>),
}

/// MERGE statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct MergeStmt<'input> {
    #[tok(MERGE, INTO, this)]
    pub target: Box<PlainTable<'input>>,
    #[tok(USING, this)]
    pub source: Box<TableRef<'input>>,
    #[tok(ON, this)]
    pub condition: Box<Expr<'input>>,
    pub when_clauses: Vec<WhenClause<'input>  >,
    pub returning: Option<Box<ReturningClause<'input>>>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_merge_basic() {
        let sql = "MERGE INTO m USING (select 0 k, 'v' v) o ON m.k = o.k WHEN MATCHED THEN UPDATE SET v = 'updated' WHEN NOT MATCHED THEN INSERT VALUES(o.k, o.v)";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.when_clauses.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_target_alias() {
        let sql = "MERGE INTO target t USING source s ON t.tid = s.sid WHEN MATCHED THEN DELETE";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_when_matched_and() {
        let sql =
            "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED AND t.a = 2 THEN UPDATE SET b = s.b";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_not_matched_by_source_default_values() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN NOT MATCHED BY SOURCE THEN INSERT DEFAULT VALUES";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
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
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            MergeStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_merge_do_nothing_both() {
        let sql = "MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DO NOTHING WHEN NOT MATCHED THEN DO NOTHING";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_insert_multi_values() {
        let sql =
            "MERGE INTO t USING s ON t.a = s.a WHEN NOT MATCHED THEN INSERT VALUES (1,1), (2,2)";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_merge_insert_into_default_values() {
        let sql = "MERGE INTO target t USING source s ON t.tid = s.sid WHEN NOT MATCHED THEN INSERT INTO target DEFAULT VALUES";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = MergeStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
