//! GRANT, REVOKE, and ALTER DEFAULT PRIVILEGES.
//!
//! These statements share the privilege/grantee/target/role vocabulary;
//! ALTER DEFAULT PRIVILEGES wraps a grant or revoke body and reuses the
//! same `Privileges`/`GrantOption`/`RevokeGrantOptionFor` machinery, so
//! all three live in this file.

use recursa::seq::Seq1;
use recursa_diagram::railroad;

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::{NameList, QualifiedName, RoleList, RoleSpec};
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};

// --- GRANT / REVOKE / ALTER DEFAULT PRIVILEGES ---
//
// PostgreSQL has two related grammars sharing the GRANT/REVOKE keywords:
//
//   * Privilege grants:    `GRANT privileges ON target TO grantees …`
//                          `REVOKE [GRANT OPTION FOR] privileges ON target FROM …`
//   * Role-membership:     `GRANT roles TO roles [WITH …]`
//                          `REVOKE [{ADMIN|INHERIT|SET} OPTION FOR] roles FROM …`
//
// Both share `GRANT privileges` as the leading shape because `privilege_list`
// in the grammar accepts arbitrary ColIds (so role names parse as privileges).
// The disambiguator is the keyword after the leading list: `ON` →
// privilege-grant, `TO`/`FROM` → role-grant. Modeled by a common `Privileges`
// head followed by an alt body that peeks `ON` vs `TO`/`FROM`.
//
// ALTER DEFAULT PRIVILEGES embeds a privilege-only GRANT/REVOKE (no role
// membership form). The target is one of TABLES/SEQUENCES/FUNCTIONS/
// PROCEDURES/ROUTINES/SCHEMAS/TYPES (Postgres' `defacl_privilege_target`)
// and there are no `objects` — only the object-kind keyword.

/// `'(' column [, …] ')'` — the optional column list on `SELECT (a, b)` and
/// related column-level privileges (Postgres' `opt_column_list` / `columnList`).
#[derive(recursa::Node, Debug, Clone)]
pub struct PrivColumnList<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub cols:
         recursa::Vec1<crate::tokens::ColId<'input> > ,
}

/// `ALTER SYSTEM` privilege keyword — Postgres' `privilege: ALTER SYSTEM_P`.
///
/// Modeled as its own struct so the multi-keyword form sorts before the
/// `Named` variant in `Privilege` (longest-match-wins).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterSystemPriv { #[tok(ALTER, SYSTEM)] Value, }

/// `SELECT [(columns)]` privilege.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectPriv<'input> {
    #[tok(SELECT, this)]
    pub cols: Option<PrivColumnList<'input>>,
}

/// `REFERENCES [(columns)]` privilege.
#[derive(recursa::Node, Debug, Clone)]
pub struct ReferencesPriv<'input> {
    #[tok(REFERENCES, this)]
    pub cols: Option<PrivColumnList<'input>>,
}

/// `CREATE [(columns)]` privilege.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreatePriv<'input> {
    #[tok(CREATE, this)]
    pub cols: Option<PrivColumnList<'input>>,
}

/// `name [(columns)]` privilege — Postgres' `privilege: ColId opt_column_list`.
///
/// Covers INSERT/UPDATE/DELETE/TRUNCATE/USAGE/EXECUTE/CONNECT/TEMPORARY/TEMP/
/// MAINTAIN/TRIGGER and any role-name-as-privilege in the role-membership form.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedPriv<'input> {
    pub name: crate::tokens::NonReservedWord<'input>,
    pub cols: Option<PrivColumnList<'input>>,
}

/// A single privilege — Postgres' `privilege` rule.
///
/// Variant ordering: `AlterSystem` (two-keyword) before any single-token
/// form so the longest match wins. SELECT/REFERENCES/CREATE come before
/// `Named` because they are reserved keywords (so `Named`'s `Ident` won't
/// accept them anyway) but listing them first makes the disambiguation
/// explicit.
#[derive(recursa::Node, Debug, Clone)]
pub enum Privilege<'input> {
    AlterSystem(AlterSystemPriv),
    Select(SelectPriv<'input>),
    References(ReferencesPriv<'input>),
    Create(CreatePriv<'input>),
    Named(NamedPriv<'input>),
}

/// `ALL` with no follow-up keyword or column list.
#[derive(recursa::Node, Debug, Clone)]
pub enum AllBarePrivs { #[tok(ALL)] Value, }

/// `ALL PRIVILEGES`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AllPrivilegesPrivs { #[tok(ALL, PRIVILEGES)] Value, }

/// `ALL (columns)` — column-scoped variant of `ALL`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllColsPrivs<'input> {
    #[tok(ALL, this)]
    pub cols: PrivColumnList<'input>,
}

/// `ALL PRIVILEGES (columns)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllPrivilegesColsPrivs<'input> {
    #[tok(ALL, PRIVILEGES, this)]
    pub cols: PrivColumnList<'input>,
}

/// `privilege [, …]` — non-`ALL` privilege list.
#[derive(recursa::Node, Debug, Clone)]
pub struct PrivilegeList<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<Privilege<'input> >,
}

/// The leading privileges/role list of a GRANT/REVOKE — Postgres' `privileges`.
///
/// Variant ordering: longest `ALL …` forms first; bare `ALL` last among the
/// ALL-prefixed forms. `List` is the catch-all and must come after every
/// `ALL`-prefixed variant because `ALL` is a hard keyword that `Privilege`'s
/// `Named` won't accept anyway.
#[derive(recursa::Node, Debug, Clone)]
pub enum Privileges<'input> {
    AllPrivilegesCols(AllPrivilegesColsPrivs<'input>),
    AllPrivileges(AllPrivilegesPrivs),
    AllCols(AllColsPrivs<'input>),
    All(AllBarePrivs),
    List(PrivilegeList<'input>),
}

/// `TABLE name [, …]` — explicit-keyword table target.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableTarget<'input> {
    #[tok(TABLE, this)]
    #[sep(COMMA)]
    pub names: recursa::Vec1<QualifiedName<'input> >,
}

/// `SEQUENCE name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SequenceTarget<'input> {
    #[tok(SEQUENCE, this)]
    #[sep(COMMA)]
    pub names: recursa::Vec1<QualifiedName<'input> >,
}

/// `FOREIGN DATA WRAPPER name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForeignDataWrapperTarget<'input> {
    #[tok(FOREIGN, DATA, WRAPPER, this)]
    pub names: NameList<'input>,
}

/// `FOREIGN SERVER name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForeignServerTarget<'input> {
    #[tok(FOREIGN, SERVER, this)]
    pub names: NameList<'input>,
}

/// `FUNCTION sig [, …]` — uses the same `(name(args))` shape as `DROP FUNCTION`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionTarget<'input> {
    #[tok(FUNCTION, this)]
    #[sep(COMMA)]
    pub sigs: recursa::Vec1<crate::ast::ddl::function::DropFunctionTarget<'input> >,
}

/// `PROCEDURE sig [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ProcedureTarget<'input> {
    #[tok(PROCEDURE, this)]
    #[sep(COMMA)]
    pub sigs: recursa::Vec1<crate::ast::ddl::function::DropFunctionTarget<'input> >,
}

/// `ROUTINE sig [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RoutineTarget<'input> {
    #[tok(ROUTINE, this)]
    #[sep(COMMA)]
    pub sigs: recursa::Vec1<crate::ast::ddl::function::DropFunctionTarget<'input> >,
}

/// `DATABASE name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DatabaseTarget<'input> {
    #[tok(DATABASE, this)]
    pub names: NameList<'input>,
}

/// `DOMAIN any_name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DomainTarget<'input> {
    #[tok(DOMAIN, this)]
    pub names: NameList<'input>,
}

/// `LANGUAGE name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageTarget<'input> {
    #[tok(LANGUAGE, this)]
    pub names: NameList<'input>,
}

/// `LARGE OBJECT oid [, …]` — `NumericOnly_list`. Corpus uses only positive
/// `IntegerLit`s; signed and floating-point forms are not exercised.
#[derive(recursa::Node, Debug, Clone)]
pub struct LargeObjectTarget<'input> {
    #[tok(LARGE, OBJECT, this)]
    #[sep(COMMA)]
    pub oids: recursa::Vec1<literal::IntegerLit<'input> >,
}

/// `SCHEMA name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SchemaTarget<'input> {
    #[tok(SCHEMA, this)]
    pub names: NameList<'input>,
}

/// `TABLESPACE name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TablespaceTarget<'input> {
    #[tok(TABLESPACE, this)]
    pub names: NameList<'input>,
}

/// `TYPE any_name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TypeTarget<'input> {
    #[tok(TYPE, this)]
    pub names: NameList<'input>,
}

/// `ALL TABLES IN SCHEMA name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllTablesInSchemaTarget<'input> {
    #[tok(ALL, TABLES, IN, SCHEMA, this)]
    pub names: NameList<'input>,
}

/// `ALL SEQUENCES IN SCHEMA name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllSequencesInSchemaTarget<'input> {
    #[tok(ALL, SEQUENCES, IN, SCHEMA, this)]
    pub names: NameList<'input>,
}

/// `ALL FUNCTIONS IN SCHEMA name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllFunctionsInSchemaTarget<'input> {
    #[tok(ALL, FUNCTIONS, IN, SCHEMA, this)]
    pub names: NameList<'input>,
}

/// `ALL PROCEDURES IN SCHEMA name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllProceduresInSchemaTarget<'input> {
    #[tok(ALL, PROCEDURES, IN, SCHEMA, this)]
    pub names: NameList<'input>,
}

/// `ALL ROUTINES IN SCHEMA name [, …]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllRoutinesInSchemaTarget<'input> {
    #[tok(ALL, ROUTINES, IN, SCHEMA, this)]
    pub names: NameList<'input>,
}

/// `qualified_name [, …]` — the bare-table form (no `TABLE` keyword).
///
/// Postgres' grammar accepts this as `privilege_target: qualified_name_list`
/// (OBJECT_TABLE). It must be the last `PrivilegeTarget` variant because the
/// first token is an identifier — anything earlier whose first set is an
/// identifier (none exist here, all targets start with keywords) would
/// otherwise win.
#[derive(recursa::Node, Debug, Clone)]
pub struct BareTablesTarget<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<QualifiedName<'input> >,
}

/// `privilege_target` — what comes between `ON` and `TO`/`FROM` in
/// GRANT/REVOKE.
///
/// Variant ordering: every `ALL X IN SCHEMA` form (multi-token) is first; then
/// the keyword-prefixed object kinds in arbitrary order (each starts with a
/// distinct keyword); finally `Bare` because its first set is an identifier,
/// so it must not eat a keyword-led variant. `FOREIGN DATA WRAPPER` is listed
/// before `FOREIGN SERVER` so the longer match wins (both start with
/// `FOREIGN`).
#[derive(recursa::Node, Debug, Clone)]
pub enum PrivilegeTarget<'input> {
    AllTablesInSchema(AllTablesInSchemaTarget<'input>),
    AllSequencesInSchema(AllSequencesInSchemaTarget<'input>),
    AllFunctionsInSchema(AllFunctionsInSchemaTarget<'input>),
    AllProceduresInSchema(AllProceduresInSchemaTarget<'input>),
    AllRoutinesInSchema(AllRoutinesInSchemaTarget<'input>),
    Table(TableTarget<'input>),
    Sequence(SequenceTarget<'input>),
    ForeignDataWrapper(ForeignDataWrapperTarget<'input>),
    ForeignServer(ForeignServerTarget<'input>),
    Function(FunctionTarget<'input>),
    Procedure(ProcedureTarget<'input>),
    Routine(RoutineTarget<'input>),
    Database(DatabaseTarget<'input>),
    Domain(DomainTarget<'input>),
    Language(LanguageTarget<'input>),
    LargeObject(LargeObjectTarget<'input>),
    Schema(SchemaTarget<'input>),
    Tablespace(TablespaceTarget<'input>),
    Type(TypeTarget<'input>),
    Bare(BareTablesTarget<'input>),
}

/// `GROUP role` — a grantee with explicit `GROUP` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub struct GroupGrantee<'input> {
    #[tok(GROUP, this)]
    pub role: RoleSpec<'input>,
}

/// `grantee` — Postgres' grammar accepts either `[GROUP] RoleSpec` or `PUBLIC`.
///
/// `PUBLIC` and the pseudo-roles `CURRENT_USER`/`CURRENT_ROLE`/`SESSION_USER`
/// are scanned as plain `Ident`s by pg-sql's lexer (none are tokens here), so
/// they round-trip through `RoleSpec` byte-faithfully. PostgreSQL's own
/// `RoleSpec` rule recognises them by string match at parse time, giving an
/// equivalent tree.
///
/// Variant ordering: `Group` (two-token) before `Role` so longest match wins.
#[derive(recursa::Node, Debug, Clone)]
pub enum Grantee<'input> {
    Group(GroupGrantee<'input>),
    Role(RoleSpec<'input>),
}

/// `grantee [, …]` — comma-separated grantees.
#[derive(recursa::Node, Debug, Clone)]
pub struct GranteeList<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<Grantee<'input> >,
}

/// `WITH GRANT OPTION` — privilege-grant trailing clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithGrantOption { #[tok(WITH, GRANT, OPTION)] Value, }

/// `GRANTED BY role` — optional grantor reference.
#[derive(recursa::Node, Debug, Clone)]
pub struct GrantedBy<'input> {
    #[tok(GRANTED, BY, this)]
    pub role: RoleSpec<'input>,
}

/// `{ADMIN|INHERIT|SET}` — the keyword on a role-grant `WITH` option.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithRoleOptKind {
    #[tok(ADMIN)] Admin,
    #[tok(INHERIT)] Inherit,
    #[tok(SET)] Set,
}

/// `{OPTION|TRUE|FALSE}` — the value of a role-grant `WITH` option.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithRoleOptValue {
    #[tok(OPTION)] Option,
    #[tok(TRUE)] True,
    #[tok(FALSE)] False,
}

/// `kind value` pair — Postgres' `grant_role_opt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithRoleOpt {
    pub kind: WithRoleOptKind,
    pub value: WithRoleOptValue,
}

/// `WITH opt [, …]` — role-grant trailing options block.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithRoleOpts {
    #[tok(WITH, this)]
    #[sep(COMMA)]
    pub opts: recursa::Vec1<WithRoleOpt >,
}

/// `ON target TO grantees …` — the privilege-grant body of GRANT.
#[derive(recursa::Node, Debug, Clone)]
pub struct GrantPrivilegeBody<'input> {
    #[tok(ON, this)]
    pub target: PrivilegeTarget<'input>,
    #[tok(TO, this)]
    pub grantees: GranteeList<'input>,
    pub grant_option: Option<WithGrantOption>,
    pub granted_by: Option<GrantedBy<'input>>,
}

/// `TO roles [WITH …] [GRANTED BY …]` — the role-membership body of GRANT.
#[derive(recursa::Node, Debug, Clone)]
pub struct GrantRoleBody<'input> {
    #[tok(TO, this)]
    pub roles: RoleList<'input>,
    pub with: Option<WithRoleOpts>,
    pub granted_by: Option<GrantedBy<'input>>,
}

/// The body of a `GRANT` after the leading privilege/role list — either the
/// privilege-grant `ON … TO …` or the role-membership `TO …`.
///
/// Variant ordering: `Privilege` first because its leading `ON` is a single
/// keyword that the `Role` variant's leading `TO` can't shadow; both have
/// disjoint first sets, so ordering is mostly cosmetic.
#[derive(recursa::Node, Debug, Clone)]
pub enum GrantBody<'input> {
    Privilege(GrantPrivilegeBody<'input>),
    Role(GrantRoleBody<'input>),
}

/// `GRANT privileges (ON target TO …) | (TO roles …)` — Postgres'
/// `GrantStmt`/`GrantRoleStmt` unified.
#[derive(recursa::Node, Debug, Clone)]
pub struct GrantStmt<'input> {
    #[tok(GRANT, this)]
    pub privileges: Privileges<'input>,
    pub body: GrantBody<'input>,
}

/// `ON target FROM grantees …` — the privilege-revoke body.
#[derive(recursa::Node, Debug, Clone)]
pub struct RevokePrivilegeBody<'input> {
    #[tok(ON, this)]
    pub target: PrivilegeTarget<'input>,
    #[tok(FROM, this)]
    pub grantees: GranteeList<'input>,
    pub granted_by: Option<GrantedBy<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// `FROM roles …` — the role-membership revoke body.
#[derive(recursa::Node, Debug, Clone)]
pub struct RevokeRoleBody<'input> {
    #[tok(FROM, this)]
    pub roles: RoleList<'input>,
    pub granted_by: Option<GrantedBy<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// The body of a `REVOKE` after the leading privilege/role list.
#[derive(recursa::Node, Debug, Clone)]
pub enum RevokeBody<'input> {
    Privilege(RevokePrivilegeBody<'input>),
    Role(RevokeRoleBody<'input>),
}

/// `GRANT OPTION FOR` — the leading "revoke only the grant option" prefix on
/// the privilege-revoke form.
#[derive(recursa::Node, Debug, Clone)]
pub enum RevokeGrantOptionFor { #[tok(GRANT, OPTION, FOR)] Value, }

/// `ADMIN OPTION FOR` — the role-revoke counterpart that strips just the
/// ADMIN option from an existing role grant.
#[derive(recursa::Node, Debug, Clone)]
pub enum RevokeAdminOptionFor { #[tok(ADMIN, OPTION, FOR)] Value, }

/// `INHERIT OPTION FOR` — strips just INHERIT.
#[derive(recursa::Node, Debug, Clone)]
pub enum RevokeInheritOptionFor { #[tok(INHERIT, OPTION, FOR)] Value, }

/// `SET OPTION FOR` — strips just SET.
#[derive(recursa::Node, Debug, Clone)]
pub enum RevokeSetOptionFor { #[tok(SET, OPTION, FOR)] Value, }

/// Optional `… OPTION FOR` prefix on `REVOKE`. PG distinguishes `GRANT OPTION
/// FOR` (privilege form) from `{ADMIN|INHERIT|SET} OPTION FOR` (role form);
/// the body following the privileges decides which it actually is.
#[derive(recursa::Node, Debug, Clone)]
pub enum RevokeOptionFor {
    GrantOption(RevokeGrantOptionFor),
    AdminOption(RevokeAdminOptionFor),
    InheritOption(RevokeInheritOptionFor),
    SetOption(RevokeSetOptionFor),
}

/// `REVOKE [… OPTION FOR] privileges (ON target FROM …) | (FROM roles …)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct RevokeStmt<'input> {
    #[tok(REVOKE, this)]
    pub option_for: Option<RevokeOptionFor>,
    pub privileges: Privileges<'input>,
    pub body: RevokeBody<'input>,
}

// -----------------------------------------------------------------------
// ALTER DEFAULT PRIVILEGES.
// -----------------------------------------------------------------------

/// `defacl_privilege_target` — the object-kind keyword inside ADP. Postgres
/// only allows one of TABLES / SEQUENCES / FUNCTIONS / PROCEDURES / ROUTINES
/// / SCHEMAS / TYPES.
#[derive(recursa::Node, Debug, Clone)]
pub enum DefAclTarget {
    #[tok(TABLES)] Tables,
    #[tok(SEQUENCES)] Sequences,
    #[tok(FUNCTIONS)] Functions,
    #[tok(PROCEDURES)] Procedures,
    #[tok(ROUTINES)] Routines,
    #[tok(SCHEMAS)] Schemas,
    #[tok(TYPES)] Types,
}

/// `GRANT privileges ON defacl_target TO grantees [WITH GRANT OPTION]` — the
/// inner GRANT of ALTER DEFAULT PRIVILEGES.
#[derive(recursa::Node, Debug, Clone)]
pub struct DefAclGrant<'input> {
    #[tok(GRANT, this)]
    pub privileges: Privileges<'input>,
    #[tok(ON, this)]
    pub target: DefAclTarget,
    #[tok(TO, this)]
    pub grantees: GranteeList<'input>,
    pub grant_option: Option<WithGrantOption>,
}

/// `REVOKE [GRANT OPTION FOR] privileges ON defacl_target FROM grantees
/// [CASCADE|RESTRICT]` — the inner REVOKE of ALTER DEFAULT PRIVILEGES. Note:
/// no `GRANTED BY` in ADP's revoke per `gram.y`'s `DefACLAction`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DefAclRevoke<'input> {
    #[tok(REVOKE, this)]
    pub grant_option_for: Option<RevokeGrantOptionFor>,
    pub privileges: Privileges<'input>,
    #[tok(ON, this)]
    pub target: DefAclTarget,
    #[tok(FROM, this)]
    pub grantees: GranteeList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// The inner action of `ALTER DEFAULT PRIVILEGES`.
#[derive(recursa::Node, Debug, Clone)]
pub enum DefAclAction<'input> {
    Grant(DefAclGrant<'input>),
    Revoke(DefAclRevoke<'input>),
}

/// `FOR { ROLE | USER }` — Postgres' `FOR ROLE` and its `FOR USER` synonym.
#[derive(recursa::Node, Debug, Clone)]
pub enum ForRoleOrUser {
    #[tok(ROLE)] Role,
    #[tok(USER)] User,
}

/// `FOR { ROLE | USER } role [, …]` — restricts the default privileges to
/// the listed role(s).
#[derive(recursa::Node, Debug, Clone)]
pub struct DefAclForRoleOption<'input> {
    #[tok(FOR, this)]
    pub role_or_user: ForRoleOrUser,
    pub roles: RoleList<'input>,
}

/// `IN SCHEMA name [, …]` — restricts the default privileges to listed
/// schema(s).
#[derive(recursa::Node, Debug, Clone)]
pub struct DefAclInSchemaOption<'input> {
    #[tok(IN, SCHEMA, this)]
    pub names: NameList<'input>,
}

/// A single `DefACLOption` — either `FOR ROLE …` or `IN SCHEMA …`. The
/// grammar allows them to repeat in arbitrary order, so an unordered list of
/// these covers every legal form.
#[derive(recursa::Node, Debug, Clone)]
pub enum DefAclOption<'input> {
    ForRole(DefAclForRoleOption<'input>),
    InSchema(DefAclInSchemaOption<'input>),
}

/// `ALTER DEFAULT PRIVILEGES [DefACLOption …] (GRANT … | REVOKE …)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterDefaultPrivilegesStmt<'input> {
    #[tok(ALTER, DEFAULT, PRIVILEGES, this)]
    pub options: Vec<DefAclOption<'input>>,
    pub action: DefAclAction<'input>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn grant_single_privilege_on_bare_table_to_role() {
        let stmt: GrantStmt = parse_stmt("GRANT SELECT ON tbl1 TO u1");
        assert!(matches!(stmt.privileges, Privileges::List(_)));
        assert!(matches!(stmt.body, GrantBody::Privilege(_)));
        reparse_stable::<GrantStmt>("GRANT SELECT ON tbl1 TO u1");
    }

    #[test]
    fn grant_multiple_privileges_on_table_to_list() {
        reparse_stable::<GrantStmt>("GRANT SELECT, INSERT, UPDATE ON tbl1 TO u1, u2");
    }

    #[test]
    fn grant_all_on_table_to_role() {
        let stmt: GrantStmt = parse_stmt("GRANT ALL ON tbl1 TO u1");
        assert!(matches!(stmt.privileges, Privileges::All(_)));
        reparse_stable::<GrantStmt>("GRANT ALL ON tbl1 TO u1");
    }

    #[test]
    fn grant_all_privileges_on_table_to_role() {
        let stmt: GrantStmt = parse_stmt("GRANT ALL PRIVILEGES ON tbl1 TO u1");
        assert!(matches!(stmt.privileges, Privileges::AllPrivileges(_)));
        reparse_stable::<GrantStmt>("GRANT ALL PRIVILEGES ON tbl1 TO u1");
    }

    #[test]
    fn grant_all_with_column_list() {
        let stmt: GrantStmt = parse_stmt("GRANT ALL (a) ON tbl1 TO u1");
        assert!(matches!(stmt.privileges, Privileges::AllCols(_)));
        reparse_stable::<GrantStmt>("GRANT ALL (a) ON tbl1 TO u1");
    }

    #[test]
    fn grant_all_privileges_with_column_list() {
        let stmt: GrantStmt = parse_stmt("GRANT ALL PRIVILEGES (a, b) ON tbl1 TO u1");
        assert!(matches!(stmt.privileges, Privileges::AllPrivilegesCols(_)));
        reparse_stable::<GrantStmt>("GRANT ALL PRIVILEGES (a, b) ON tbl1 TO u1");
    }

    #[test]
    fn grant_column_level_select_to_role() {
        reparse_stable::<GrantStmt>("GRANT SELECT (a, b) ON tbl1 TO u1");
    }

    #[test]
    fn grant_explicit_table_keyword() {
        reparse_stable::<GrantStmt>("GRANT SELECT ON TABLE tbl1 TO u1");
    }

    #[test]
    fn grant_all_tables_in_schema() {
        reparse_stable::<GrantStmt>("GRANT ALL ON ALL TABLES IN SCHEMA testns TO u1");
    }

    #[test]
    fn grant_usage_on_schema() {
        reparse_stable::<GrantStmt>("GRANT USAGE ON SCHEMA s TO u1");
    }

    #[test]
    fn grant_with_grant_option() {
        let stmt: GrantStmt = parse_stmt("GRANT CREATE ON DATABASE d TO u1 WITH GRANT OPTION");
        if let GrantBody::Privilege(body) = &stmt.body {
            assert!(body.grant_option.is_some());
        } else {
            panic!("expected privilege body");
        }
        reparse_stable::<GrantStmt>("GRANT CREATE ON DATABASE d TO u1 WITH GRANT OPTION");
    }

    #[test]
    fn grant_granted_by() {
        reparse_stable::<GrantStmt>("GRANT INSERT ON atest2 TO u4 GRANTED BY CURRENT_USER");
    }

    #[test]
    fn grant_function_signature() {
        reparse_stable::<GrantStmt>("GRANT EXECUTE ON FUNCTION f(int) TO u2");
    }

    #[test]
    fn grant_large_object_to_public() {
        reparse_stable::<GrantStmt>("GRANT ALL ON LARGE OBJECT 1001 TO PUBLIC");
    }

    #[test]
    fn grant_group_grantee() {
        reparse_stable::<GrantStmt>("GRANT DELETE ON atest3 TO GROUP regress_priv_group2");
    }

    #[test]
    fn grant_role_membership_simple() {
        let stmt: GrantStmt = parse_stmt("GRANT role1 TO role2");
        assert!(matches!(stmt.body, GrantBody::Role(_)));
        reparse_stable::<GrantStmt>("GRANT role1 TO role2");
    }

    #[test]
    fn grant_role_membership_with_admin_option() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH ADMIN OPTION");
    }

    #[test]
    fn grant_role_membership_with_inherit_false() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH INHERIT FALSE");
    }

    #[test]
    fn grant_role_membership_with_set_true() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH SET TRUE");
    }

    #[test]
    fn grant_role_membership_with_admin_option_granted_by() {
        reparse_stable::<GrantStmt>("GRANT role1 TO role2 WITH ADMIN OPTION GRANTED BY role3");
    }

    #[test]
    fn revoke_simple_privilege() {
        let stmt: RevokeStmt = parse_stmt("REVOKE SELECT ON tbl1 FROM u1");
        assert!(stmt.option_for.is_none());
        assert!(matches!(stmt.body, RevokeBody::Privilege(_)));
        reparse_stable::<RevokeStmt>("REVOKE SELECT ON tbl1 FROM u1");
    }

    #[test]
    fn revoke_grant_option_for_cascade() {
        let stmt: RevokeStmt = parse_stmt("REVOKE GRANT OPTION FOR SELECT ON tbl1 FROM u1 CASCADE");
        assert!(matches!(
            stmt.option_for,
            Some(RevokeOptionFor::GrantOption(_))
        ));
        reparse_stable::<RevokeStmt>("REVOKE GRANT OPTION FOR SELECT ON tbl1 FROM u1 CASCADE");
    }

    #[test]
    fn revoke_role_membership_cascade() {
        let stmt: RevokeStmt = parse_stmt("REVOKE role1 FROM u1 CASCADE");
        assert!(matches!(stmt.body, RevokeBody::Role(_)));
        reparse_stable::<RevokeStmt>("REVOKE role1 FROM u1 CASCADE");
    }

    #[test]
    fn revoke_admin_option_for_role() {
        let stmt: RevokeStmt = parse_stmt("REVOKE ADMIN OPTION FOR role1 FROM u1");
        assert!(matches!(
            stmt.option_for,
            Some(RevokeOptionFor::AdminOption(_))
        ));
        reparse_stable::<RevokeStmt>("REVOKE ADMIN OPTION FOR role1 FROM u1");
    }

    #[test]
    fn alter_default_privileges_in_schema_grant_tables() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES IN SCHEMA s GRANT SELECT ON TABLES TO u1",
        );
    }

    #[test]
    fn alter_default_privileges_for_role_revoke_functions() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES FOR ROLE r REVOKE EXECUTE ON FUNCTIONS FROM public",
        );
    }

    #[test]
    fn alter_default_privileges_grant_schemas() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES GRANT USAGE ON SCHEMAS TO u2",
        );
    }

    #[test]
    fn alter_default_privileges_for_role_in_schema_grant() {
        reparse_stable::<AlterDefaultPrivilegesStmt>(
            "ALTER DEFAULT PRIVILEGES FOR ROLE r IN SCHEMA s GRANT ALL ON TABLES TO u2",
        );
    }
}
