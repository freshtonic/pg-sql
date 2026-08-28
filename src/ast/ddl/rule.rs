//! RULE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::session::notify::NotifyStmt;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Rule event — Postgres' `event` rule (a strict subset of trigger events):
/// `SELECT | INSERT | UPDATE | DELETE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RuleEvent {
    Select(SELECT),
    Insert(INSERT),
    Update(UPDATE),
    Delete(DELETE),
}

/// `INSTEAD | ALSO` — Postgres' `opt_instead`. Either keyword is optional;
/// when absent the rule fires alongside the original command (`ALSO`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RuleInsteadAlso {
    Instead(INSTEAD),
    Also(ALSO),
}

/// `WHERE expr` clause on a rule. Postgres allows any `a_expr`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RuleWhereClause<'input> {
    pub where_: WHERE,
    pub expr: Box<Expr<'input>>,
}

/// A single statement that may appear as a rule action — Postgres'
/// `RuleActionStmt`: SELECT, INSERT, UPDATE, DELETE, or NOTIFY.
///
/// Each variant boxes its underlying statement type so the parent
/// `RuleActions::Single` enum stays small. We reuse the existing statement
/// AST types directly.
///
/// PG accepts a few additional forms in a rule body that gram.y models
/// elsewhere but that the rules.sql / with.sql regression corpora exercise:
/// - `WITH cte AS (...) {SELECT|INSERT|UPDATE|DELETE} ...` — a
///   CTE-prefixed statement (gram.y models the CTE as part of each DML
///   statement's `opt_with_clause`; in pg-sql the `WithStatement` is a
///   top-level `Statement` variant, so we reuse it here directly).
/// - `VALUES (row), (row), ...` — a bare values_clause, the same shape
///   used by `Statement::Values`.
///
/// Variant ordering: `With` and `Values` lead with their own keyword and
/// don't collide with the DML statement leads.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RuleActionStmt<'input> {
    With(Box<crate::ast::shared::with_clause::WithStatement<'input>>),
    Values(Box<crate::ast::dml::values::Subquery<'input>>),
    Select(Box<crate::ast::dml::select::SelectStmt<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Notify(NotifyStmt<'input>),
}

/// Rule actions — Postgres' `RuleActionList`:
/// `NOTHING | RuleActionStmt | '(' RuleActionMulti ')'`.
///
/// Variant ordering: distinct first tokens (`NOTHING` keyword, `(` punct, or
/// statement-leading keyword) so disambiguation is unambiguous.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RuleActions<'input> {
    Nothing(NOTHING),
    /// `'(' stmt; stmt; … ')'` — RuleActionMulti, accepting empty statements
    /// between semicolons. We use `Seq0` with `Semi` separator and an
    /// optional trailing separator so `(stmt;)` and `(stmt; stmt;)` both
    /// round-trip.
    Multi(
        Surrounded<
            punct::LParen,
            Seq0<RuleActionStmt<'input>, punct::Semi, recursa::seq::OptionalTrailing>,
            punct::RParen,
        >,
    ),
    Single(Box<RuleActionStmt<'input>>),
}

/// `CREATE [OR REPLACE] RULE name AS ON event TO qualified_name [WHERE expr]
/// DO [INSTEAD|ALSO] RuleActionList` — Postgres' `RuleStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateRuleStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub rule: RULE,
    pub name: crate::tokens::ColId<'input>,
    pub r#as: AS,
    pub on: ON,
    pub event: RuleEvent,
    pub to: TO,
    pub table: QualifiedName<'input>,
    pub where_clause: Option<RuleWhereClause<'input>>,
    pub r#do: DO,
    pub instead_also: Option<RuleInsteadAlso>,
    pub actions: RuleActions<'input>,
}

/// `DROP RULE [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropRuleStmt<'input> {
    pub drop: DROP,
    pub rule: RULE,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `ALTER RULE name ON qualified_name RENAME TO new` — Postgres'
/// `RenameStmt` branch for rules. Rules have no OWNER / SET SCHEMA
/// actions in gram.y, so RENAME is the only branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterRuleStmt<'input> {
    pub alter: ALTER,
    pub rule: RULE,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
    pub rename_to: RenameTo<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_rule_rename() {
        let mut input =
            crate::tokens::test_input("ALTER RULE InsertRule ON rule_v1 RENAME TO NewInsertRule");
        let _stmt = AlterRuleStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_rule_nothing() {
        let stmt: CreateRuleStmt =
            parse_stmt("CREATE RULE r AS ON INSERT TO tbl DO INSTEAD NOTHING");
        assert_eq!(stmt.name.text(), "r");
        assert_eq!(stmt.table.object(), "tbl");
        assert!(stmt.or_replace.is_none());
        assert!(matches!(stmt.event, RuleEvent::Insert(_)));
        assert!(matches!(stmt.actions, RuleActions::Nothing(_)));
    }

    #[test]
    fn create_or_replace_rule_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE OR REPLACE RULE r AS ON INSERT TO tbl DO INSTEAD INSERT INTO other VALUES (1)",
        );
    }

    #[test]
    fn create_rule_with_where_clause_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE RULE r AS ON UPDATE TO tbl WHERE NEW.a <> OLD.a DO INSERT INTO log VALUES (NEW.a)",
        );
    }

    #[test]
    fn create_rule_select_event_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            r#"CREATE RULE "_RETURN" AS ON SELECT TO v DO INSTEAD SELECT 1"#,
        );
    }

    #[test]
    fn create_rule_do_also_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE RULE r AS ON DELETE TO tbl DO ALSO DELETE FROM other WHERE a = OLD.a",
        );
    }

    #[test]
    fn create_rule_multi_action_paren_roundtrips() {
        reparse_stable::<CreateRuleStmt>(
            "CREATE RULE r AS ON UPDATE TO tbl DO ALSO (UPDATE other SET a = NEW.a; DELETE FROM log WHERE a = OLD.a)",
        );
    }
}
