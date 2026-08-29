//! RULE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

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
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleEvent {
    #[tok(SELECT)] Select,
    #[tok(INSERT)] Insert,
    #[tok(UPDATE)] Update,
    #[tok(DELETE)] Delete,
}

/// `INSTEAD | ALSO` — Postgres' `opt_instead`. Either keyword is optional;
/// when absent the rule fires alongside the original command (`ALSO`).
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleInsteadAlso {
    #[tok(INSTEAD)] Instead,
    #[tok(ALSO)] Also,
}

/// `WHERE expr` clause on a rule. Postgres allows any `a_expr`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RuleWhereClause<'input> {
    #[tok(WHERE, this)]
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
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
pub enum RuleActions<'input> {
    #[tok(NOTHING)] Nothing,
    /// `'(' stmt; stmt; … ')'` — RuleActionMulti, accepting empty statements
    /// between semicolons. We use `Seq0` with `Semi` separator and an
    /// optional trailing separator so `(stmt;)` and `(stmt; stmt;)` both
    /// round-trip.
    Multi(
        #[tok(LPAREN, this, RPAREN)]
        #[sep(SEMI, trailing)]


            Vec<RuleActionStmt<'input>  >

        ,
    ),
    Single(Box<RuleActionStmt<'input>>),
}

/// `CREATE [OR REPLACE] RULE name AS ON event TO qualified_name [WHERE expr]
/// DO [INSTEAD|ALSO] RuleActionList` — Postgres' `RuleStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateRuleStmt<'input> {
    #[tok(CREATE, this, RULE)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub name: crate::tokens::ColId<'input>,
    #[tok(AS, ON, this)]
    pub event: RuleEvent,
    #[tok(TO, this)]
    pub table: QualifiedName<'input>,
    pub where_clause: Option<RuleWhereClause<'input>>,
    #[tok(DO, this)]
    pub instead_also: Option<RuleInsteadAlso>,
    pub actions: RuleActions<'input>,
}

/// `DROP RULE [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropRuleStmt<'input> {
    #[tok(DROP, RULE, this)]
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `ALTER RULE name ON qualified_name RENAME TO new` — Postgres'
/// `RenameStmt` branch for rules. Rules have no OWNER / SET SCHEMA
/// actions in gram.y, so RENAME is the only branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterRuleStmt<'input> {
    #[tok(ALTER, RULE, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
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
        let lexed = crate::tokens::lex("ALTER RULE InsertRule ON rule_v1 RENAME TO NewInsertRule");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterRuleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
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
