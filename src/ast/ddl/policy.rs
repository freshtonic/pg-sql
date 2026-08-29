//! POLICY DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `row_security_cmd`: the command kind in a `FOR` clause on CREATE/ALTER
/// POLICY — Postgres' `row_security_cmd` rule. All five forms are bare
/// keywords. `ALL` and `SELECT` are reserved; `INSERT`/`UPDATE`/`DELETE`
/// are soft.
///
/// Variant ordering: all distinct first tokens, so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum RowSecurityCmd {
    #[tok(ALL)] All,
    #[tok(SELECT)] Select,
    #[tok(INSERT)] Insert,
    #[tok(UPDATE)] Update,
    #[tok(DELETE)] Delete,
}

/// `AS ident` permissive/restrictive selector on CREATE POLICY —
/// Postgres' `RowSecurityDefaultPermissive`.
///
/// gram.y parses the keyword as `IDENT` and validates `"permissive"` /
/// `"restrictive"` via `strcmp`; the bogus `AS UGLY` form in the corpus
/// is intentionally syntactically valid but semantically rejected.
/// Modelling the identifier as `literal::Ident` preserves both cases.
#[derive(recursa::Node, Debug, Clone)]
pub struct PolicyPermissiveClause<'input> {
    #[tok(AS, this)]
    pub kind: crate::tokens::NonReservedWord<'input>,
}

/// `FOR row_security_cmd` clause on CREATE/ALTER POLICY.
#[derive(recursa::Node, Debug, Clone)]
pub struct PolicyForClause {
    #[tok(FOR, this)]
    pub cmd: RowSecurityCmd,
}

/// `TO role_list` clause on CREATE/ALTER POLICY — Postgres'
/// `RowSecurityDefaultToRole`. `PUBLIC`/`CURRENT_USER`/etc. are not
/// keywords in pg-sql; they pass through as `RoleSpec` identifiers.
#[derive(recursa::Node, Debug, Clone)]
pub struct PolicyToClause<'input> {
    #[tok(TO, this)]
    pub roles: RoleList<'input>,
}

/// `USING (a_expr)` clause on CREATE/ALTER POLICY.
#[derive(recursa::Node, Debug, Clone)]
pub struct PolicyUsingClause<'input> {
    #[tok(USING, LPAREN, this, RPAREN)]
    pub expr:  Box<Expr<'input>> ,
}

/// `WITH CHECK (a_expr)` clause on CREATE/ALTER POLICY.
#[derive(recursa::Node, Debug, Clone)]
pub struct PolicyWithCheckClause<'input> {
    #[tok(WITH, CHECK, LPAREN, this, RPAREN)]
    pub expr:  Box<Expr<'input>> ,
}

/// `CREATE POLICY name ON table [AS PERMISSIVE|RESTRICTIVE]
/// [FOR cmd] [TO role_list] [USING (expr)] [WITH CHECK (expr)]` —
/// Postgres' `CreatePolicyStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreatePolicyStmt<'input> {
    #[tok(CREATE, POLICY, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub permissive: Option<PolicyPermissiveClause<'input>>,
    pub for_cmd: Option<PolicyForClause>,
    pub to_roles: Option<PolicyToClause<'input>>,
    pub using: Option<PolicyUsingClause<'input>>,
    pub with_check: Option<PolicyWithCheckClause<'input>>,
}

/// `DROP POLICY [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropPolicyStmt<'input> {
    #[tok(DROP, POLICY, this)]
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub behavior: Option<DropBehavior>,
}

/// The modification action on `ALTER POLICY` — either `RENAME TO new`
/// (Postgres' `RenameStmt` branch) or the standard
/// `[TO role_list] [USING (expr)] [WITH CHECK (expr)]` action
/// (Postgres' `AlterPolicyStmt`). Both share the `ALTER POLICY name ON
/// qualified_name` prefix; the action discriminates between them.
///
/// Variant ordering: `Rename` (single-keyword `RENAME`) is listed before
/// `Modify` (which can start with `TO`, `USING`, `WITH`, or be empty);
/// the two have disjoint first-token sets.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterPolicyAction<'input> {
    Rename(RenameTo<'input>),
    Modify(AlterPolicyModify<'input>),
}

/// `[TO role_list] [USING (expr)] [WITH CHECK (expr)]` — the non-rename
/// action on `ALTER POLICY`. All three clauses are optional but at least
/// one must be present at the semantic level; pg-sql accepts the
/// all-empty form too because gram.y's `AlterPolicyStmt` does.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPolicyModify<'input> {
    pub to_roles: Option<PolicyToClause<'input>>,
    pub using: Option<PolicyUsingClause<'input>>,
    pub with_check: Option<PolicyWithCheckClause<'input>>,
}

/// `ALTER POLICY name ON qualified_name action` — Postgres'
/// `AlterPolicyStmt` plus the `ALTER POLICY ... RENAME TO ...` branch
/// from `RenameStmt`. Both share the same prefix; the action enum
/// dispatches.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterPolicyStmt<'input> {
    #[tok(ALTER, POLICY, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
    pub action: AlterPolicyAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_policy_on_table() {
        let lexed = crate::tokens::lex("DROP POLICY p1 ON document");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropPolicyStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "p1");
        assert_eq!(stmt.table.object(), "document");
        assert!(input.is_eof());
    }

    #[test]
    fn create_policy_minimal_roundtrips() {
        let stmt: CreatePolicyStmt = parse_stmt("CREATE POLICY p1 ON document");
        assert_eq!(stmt.name.text(), "p1");
        assert_eq!(stmt.table.object(), "document");
        assert!(stmt.permissive.is_none());
        assert!(stmt.for_cmd.is_none());
        assert!(stmt.to_roles.is_none());
        assert!(stmt.using.is_none());
        assert!(stmt.with_check.is_none());
        reparse_stable::<CreatePolicyStmt>("CREATE POLICY p1 ON document");
    }

    #[test]
    fn create_policy_as_permissive_using_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p1 ON document AS PERMISSIVE USING (true)",
        );
    }

    #[test]
    fn create_policy_as_restrictive_to_role_using_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p1r ON document AS RESTRICTIVE TO regress_rls_dave USING (cid <> 44)",
        );
    }

    #[test]
    fn create_policy_for_insert_with_check_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p ON document FOR INSERT WITH CHECK (dauthor = current_user)",
        );
    }

    #[test]
    fn create_policy_for_all_to_public_using_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p ON t FOR ALL TO PUBLIC USING (a % 2 = 0)",
        );
    }

    #[test]
    fn create_policy_for_update_using_with_check_roundtrips() {
        reparse_stable::<CreatePolicyStmt>(
            "CREATE POLICY p3 ON document FOR UPDATE USING (true) WITH CHECK (true)",
        );
    }
}
