//! POLICY DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RowSecurityCmd {
    All(ALL),
    Select(SELECT),
    Insert(INSERT),
    Update(UPDATE),
    Delete(DELETE),
}

/// `AS ident` permissive/restrictive selector on CREATE POLICY —
/// Postgres' `RowSecurityDefaultPermissive`.
///
/// gram.y parses the keyword as `IDENT` and validates `"permissive"` /
/// `"restrictive"` via `strcmp`; the bogus `AS UGLY` form in the corpus
/// is intentionally syntactically valid but semantically rejected.
/// Modelling the identifier as `literal::Ident` preserves both cases.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PolicyPermissiveClause<'input> {
    pub r#as: AS,
    pub kind: crate::tokens::NonReservedWord<'input>,
}

/// `FOR row_security_cmd` clause on CREATE/ALTER POLICY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PolicyForClause {
    pub r#for: FOR,
    pub cmd: RowSecurityCmd,
}

/// `TO role_list` clause on CREATE/ALTER POLICY — Postgres'
/// `RowSecurityDefaultToRole`. `PUBLIC`/`CURRENT_USER`/etc. are not
/// keywords in pg-sql; they pass through as `RoleSpec` identifiers.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PolicyToClause<'input> {
    pub to: TO,
    pub roles: RoleList<'input>,
}

/// `USING (a_expr)` clause on CREATE/ALTER POLICY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PolicyUsingClause<'input> {
    pub using: USING,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `WITH CHECK (a_expr)` clause on CREATE/ALTER POLICY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PolicyWithCheckClause<'input> {
    pub with: WITH,
    pub check: CHECK,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `CREATE POLICY name ON table [AS PERMISSIVE|RESTRICTIVE]
/// [FOR cmd] [TO role_list] [USING (expr)] [WITH CHECK (expr)]` —
/// Postgres' `CreatePolicyStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreatePolicyStmt<'input> {
    pub create: CREATE,
    pub policy: POLICY,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
    pub permissive: Option<PolicyPermissiveClause<'input>>,
    pub for_cmd: Option<PolicyForClause>,
    pub to_roles: Option<PolicyToClause<'input>>,
    pub using: Option<PolicyUsingClause<'input>>,
    pub with_check: Option<PolicyWithCheckClause<'input>>,
}

/// `DROP POLICY [IF EXISTS] name ON table [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropPolicyStmt<'input> {
    pub drop: DROP,
    pub policy: POLICY,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterPolicyAction<'input> {
    Rename(RenameTo<'input>),
    Modify(AlterPolicyModify<'input>),
}

/// `[TO role_list] [USING (expr)] [WITH CHECK (expr)]` — the non-rename
/// action on `ALTER POLICY`. All three clauses are optional but at least
/// one must be present at the semantic level; pg-sql accepts the
/// all-empty form too because gram.y's `AlterPolicyStmt` does.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterPolicyModify<'input> {
    pub to_roles: Option<PolicyToClause<'input>>,
    pub using: Option<PolicyUsingClause<'input>>,
    pub with_check: Option<PolicyWithCheckClause<'input>>,
}

/// `ALTER POLICY name ON qualified_name action` — Postgres'
/// `AlterPolicyStmt` plus the `ALTER POLICY ... RENAME TO ...` branch
/// from `RenameStmt`. Both share the same prefix; the action enum
/// dispatches.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterPolicyStmt<'input> {
    pub alter: ALTER,
    pub policy: POLICY,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
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
        let mut input = crate::tokens::test_input("DROP POLICY p1 ON document");
        let stmt = DropPolicyStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "p1");
        assert_eq!(stmt.table.object(), "document");
        assert!(input.is_empty());
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
