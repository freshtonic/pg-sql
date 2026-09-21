//! RULE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::session::notify::NotifyStmt;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// Rule event — Postgres' `event` rule (a strict subset of trigger events):
    /// `SELECT | INSERT | UPDATE | DELETE`.
    #[derive(Debug)]
    pub enum RuleEvent {
        #[tok(SELECT)]
        Select,
        #[tok(INSERT)]
        Insert,
        #[tok(UPDATE)]
        Update,
        #[tok(DELETE)]
        Delete,
    }
}

recursa::ast_node! {
    /// `INSTEAD | ALSO` — Postgres' `opt_instead`. Either keyword is optional;
    /// when absent the rule fires alongside the original command (`ALSO`).
    #[derive(Debug)]
    pub enum RuleInsteadAlso {
        #[tok(INSTEAD)]
        Instead,
        #[tok(ALSO)]
        Also,
    }
}

recursa::ast_node! {
    /// `WHERE expr` clause on a rule. Postgres allows any `a_expr`.
    #[derive(Debug)]
    pub struct RuleWhereClause {
        #[tok(WHERE, this)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// A single statement that may appear as a rule action — gram.y
    /// `RuleActionStmt`: `SelectStmt`, `InsertStmt`, `UpdateStmt`, `DeleteStmt`
    /// or `NotifyStmt`. gram.y gives the three DML statements an
    /// `opt_with_clause` and `SelectStmt` its `with_clause`; pg-sql factors that
    /// prefix once as `With`, so a `WITH`-led action is decided after the CTE
    /// list.
    ///
    /// Each variant boxes its underlying statement type so the parent
    /// `RuleActions::Single` enum stays small. We reuse the existing statement
    /// AST types directly.
    ///
    /// Query actions deliberately exclude a parenthesized outer query because
    /// parentheses at this level delimit a multi-action rule list.
    #[derive(Debug)]
    pub enum RuleActionStmt {
        Query(boxed!(RuleQuery)),
        With(boxed!(RuleWithAction)),
        Insert(boxed!(crate::ast::dml::insert::InsertStmt)),
        Update(boxed!(crate::ast::dml::update::UpdateStmt)),
        Delete(boxed!(crate::ast::dml::delete::DeleteStmt)),
        Notify(NotifyStmt),
    }
}

recursa::ast_node! {
    /// gram.y `with_clause` followed by the rule action it prefixes: a query
    /// (`select_no_parens`), `insert_rest`, `update` or `delete`. `MERGE` is
    /// not a rule action in gram.y, so this is not `WithStatement`.
    #[derive(Debug)]
    pub struct RuleWithAction {
        pub with_clause: crate::ast::shared::with_clause::WithClause,
        pub body: RuleWithBody,
    }
}

recursa::ast_node! {
    /// The statement after a rule action's `WITH` clause.
    ///
    /// Variant ordering: `Query` leads with `SELECT`, `VALUES` or `TABLE`; the
    /// three DML variants have disjoint leading keywords.
    #[derive(Debug)]
    pub enum RuleWithBody {
        Query(boxed!(RuleQuery)),
        Insert(boxed!(crate::ast::dml::insert::InsertStmt)),
        Update(boxed!(crate::ast::dml::update::UpdateStmt)),
        Delete(boxed!(crate::ast::dml::delete::DeleteStmt)),
    }
}

recursa::ast_node! {
    /// gram.y `select_no_parens` less its `with_clause`: a non-parenthesized
    /// `select_clause` with its `opt_sort_clause`, `select_limit` and
    /// `for_locking_clause` tails.
    #[derive(Debug)]
    pub struct RuleQuery {
        pub clause: RuleSelectClause,
        #[pretty(break_before = soft)]
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
        #[pretty(break_before = soft)]
        pub limit_offset: Option<boxed!(crate::ast::dml::select::LimitOffsetClause)>,
        #[pretty(break_before = soft)]
        pub for_update: Option<boxed!(crate::ast::dml::select::ForUpdateClause)>,
    }
}

recursa::ast_node! {
    /// The `select_clause` of a single rule action: a restricted
    /// [`SelectClause`](crate::ast::dml::values::SelectClause), which builds a
    /// `SelectClause`, so a consumer sees one tree type and the same
    /// left-associative nesting with `INTERSECT` above `UNION` and `EXCEPT`.
    ///
    /// gram.y's `RuleActionList` is `NOTHING | RuleActionStmt | '('
    /// RuleActionMulti ')'`. A `(` after `DO [ALSO | INSTEAD]` opens the action
    /// list, so `Parens` is not admitted and the clause never leads with a
    /// parenthesized query.
    ///
    /// Known limit: a restricted expression restricts its last operand as well
    /// as its first, so `DO ALSO SELECT 1 UNION (SELECT 2)` is rejected, which
    /// PostgreSQL accepts. A separate enum with a full right operand is not
    /// derivable: `Pretty` repairs precedence with the grammar's own
    /// parenthesized atom, and this clause has none (RCA9105). The spelling
    /// `DO ALSO (SELECT 1 UNION (SELECT 2))` parses.
    #[restricts(crate::ast::dml::values::SelectClause)]
    pub enum RuleSelectClause {
        Union,
        Except,
        Intersect,
        Select,
        Values,
        Table,
    }
}

recursa::ast_node! {
    /// Rule actions — Postgres' `RuleActionList`:
    /// `NOTHING | RuleActionStmt | '(' RuleActionMulti ')'`.
    ///
    /// Variant ordering: distinct first tokens (`NOTHING` keyword, `(` punct, or
    /// statement-leading keyword) so disambiguation is unambiguous.
    #[derive(Debug)]
    pub enum RuleActions {
        #[tok(NOTHING)]
        Nothing,
        /// `'(' stmt; stmt; … ')'` — RuleActionMulti, accepting empty statements
        /// between semicolons. We use `Seq0` with `Semi` separator and an
        /// optional trailing separator so `(stmt;)` and `(stmt; stmt;)` both
        /// round-trip.
        Multi(RuleActionList),
        Single(boxed!(RuleActionStmt)),
    }
}

recursa::ast_node! {
    /// Parenthesized, semicolon-separated rule action list.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct RuleActionList {
        #[sep(SEMI, trailing)]
        pub actions: zero_or_many!(RuleActionStmt),
    }
}

recursa::ast_node! {
    /// Required `DO [INSTEAD | ALSO] actions` tail of a `CREATE RULE` statement.
    ///
    /// The `DO` keyword belongs to this required wrapper rather than either child:
    /// attaching it to the optional modifier would make `DO` optional, while
    /// attaching it to `actions` would place it after the modifier.
    #[derive(Debug)]
    #[tok(DO, this)]
    pub struct RuleDoClause {
        pub instead_also: Option<RuleInsteadAlso>,
        pub actions: RuleActions,
    }
}

recursa::ast_node! {
    /// `CREATE [OR REPLACE] RULE name AS ON event TO qualified_name [WHERE expr]
    /// DO [INSTEAD|ALSO] RuleActionList` — Postgres' `RuleStmt`.
    #[derive(Debug)]
    pub struct CreateRuleStmt {
        #[tok(CREATE, this, RULE)]
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        pub name: crate::tokens::ColId,
        #[tok(AS, ON, this)]
        pub event: RuleEvent,
        #[tok(TO, this)]
        pub table: QualifiedName,
        pub where_clause: Option<RuleWhereClause>,
        pub do_clause: RuleDoClause,
    }
}

recursa::ast_node! {
    /// `DROP RULE [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, RULE, this)]
    pub struct DropRuleStmt {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `ALTER RULE name ON qualified_name RENAME TO new` — Postgres'
    /// `RenameStmt` branch for rules. Rules have no OWNER / SET SCHEMA
    /// actions in gram.y, so RENAME is the only branch.
    #[derive(Debug)]
    pub struct AlterRuleStmt {
        #[tok(ALTER, RULE, this)]
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
        pub rename_to: RenameTo,
    }
}
