//! GRANT, REVOKE, and ALTER DEFAULT PRIVILEGES.
//!
//! These statements share the privilege/grantee/target/role vocabulary;
//! ALTER DEFAULT PRIVILEGES wraps a grant or revoke body and reuses the
//! same `Privileges`/`GrantOption`/`RevokeGrantOptionFor` machinery, so
//! all three live in this file.

use crate::ast::shared::flags::DropBehavior;
use crate::ast::shared::names::{NameList, QualifiedName, RoleList, RoleSpec};
use crate::tokens::literal;

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

recursa::ast_node! {
    /// `'(' column [, …] ')'` — the optional column list on `SELECT (a, b)` and
    /// related column-level privileges (Postgres' `opt_column_list` / `columnList`).
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct PrivColumnList {
        #[sep(COMMA)]
        pub cols: one_or_many!(crate::tokens::ColId),
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `privilege: ALTER SYSTEM_P`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `ALTER SYSTEM` privilege keyword — Postgres' `privilege: ALTER SYSTEM_P`.
    ///
    /// Modeled as its own struct so the multi-keyword form sorts before the
    /// `Named` variant in `Privilege` (longest-match-wins).
    #[derive(Debug)]
    pub enum AlterSystemPriv {
        #[tok(ALTER, SYSTEM)]
        Value,
    }
}

recursa::ast_node! {
    /// `SELECT [(columns)]` privilege.
    #[derive(Debug)]
    #[tok(SELECT, this)]
    pub struct SelectPriv {
        pub cols: Option<PrivColumnList>,
    }
}

recursa::ast_node! {
    /// `REFERENCES [(columns)]` privilege.
    #[derive(Debug)]
    #[tok(REFERENCES, this)]
    pub struct ReferencesPriv {
        pub cols: Option<PrivColumnList>,
    }
}

recursa::ast_node! {
    /// `CREATE [(columns)]` privilege.
    #[derive(Debug)]
    #[tok(CREATE, this)]
    pub struct CreatePriv {
        pub cols: Option<PrivColumnList>,
    }
}

recursa::ast_node! {
    /// `name [(columns)]` privilege — Postgres' `privilege: ColId opt_column_list`.
    ///
    /// Covers INSERT/UPDATE/DELETE/TRUNCATE/USAGE/EXECUTE/CONNECT/TEMPORARY/TEMP/
    /// MAINTAIN/TRIGGER and any role-name-as-privilege in the role-membership form.
    #[derive(Debug)]
    pub struct NamedPriv {
        pub name: crate::tokens::NonReservedWord,
        pub cols: Option<PrivColumnList>,
    }
}

recursa::ast_node! {
    /// A single privilege — Postgres' `privilege` rule.
    ///
    /// Variant ordering: `AlterSystem` (two-keyword) before any single-token
    /// form so the longest match wins. SELECT/REFERENCES/CREATE come before
    /// `Named` because they are reserved keywords (so `Named`'s `Ident` won't
    /// accept them anyway) but listing them first makes the disambiguation
    /// explicit.
    #[derive(Debug)]
    pub enum Privilege {
        /// Added in 15: research, PostgreSQL 15, "Changes to existing statements".
        #[config(since = pg15)]
        AlterSystem(AlterSystemPriv),
        Select(SelectPriv),
        References(ReferencesPriv),
        Create(CreatePriv),
        Named(NamedPriv),
    }
}

recursa::ast_node! {
    /// `ALL` with no follow-up keyword or column list.
    #[derive(Debug)]
    pub enum AllBarePrivs {
        #[tok(ALL)]
        Value,
    }
}

recursa::ast_node! {
    /// `ALL PRIVILEGES`.
    #[derive(Debug)]
    pub enum AllPrivilegesPrivs {
        #[tok(ALL, PRIVILEGES)]
        Value,
    }
}

recursa::ast_node! {
    /// `ALL (columns)` — column-scoped variant of `ALL`.
    #[derive(Debug)]
    pub struct AllColsPrivs {
        #[tok(ALL, this)]
        pub cols: PrivColumnList,
    }
}

recursa::ast_node! {
    /// `ALL PRIVILEGES (columns)`.
    #[derive(Debug)]
    pub struct AllPrivilegesColsPrivs {
        #[tok(ALL, PRIVILEGES, this)]
        pub cols: PrivColumnList,
    }
}

recursa::ast_node! {
    /// `privilege [, …]` — non-`ALL` privilege list.
    #[derive(Debug)]
    pub struct PrivilegeList {
        #[sep(COMMA)]
        pub items: one_or_many!(Privilege),
    }
}

recursa::ast_node! {
    /// The leading privileges/role list of a GRANT/REVOKE — Postgres' `privileges`.
    ///
    /// Variant ordering: longest `ALL …` forms first; bare `ALL` last among the
    /// ALL-prefixed forms. `List` is the catch-all and must come after every
    /// `ALL`-prefixed variant because `ALL` is a hard keyword that `Privilege`'s
    /// `Named` won't accept anyway.
    #[derive(Debug)]
    pub enum Privileges {
        AllPrivilegesCols(AllPrivilegesColsPrivs),
        AllPrivileges(AllPrivilegesPrivs),
        AllCols(AllColsPrivs),
        All(AllBarePrivs),
        List(PrivilegeList),
    }
}

recursa::ast_node! {
    /// `TABLE name [, …]` — explicit-keyword table target.
    #[derive(Debug)]
    #[tok(TABLE, this)]
    pub struct TableTarget {
        #[sep(COMMA)]
        pub names: one_or_many!(QualifiedName),
    }
}

recursa::ast_node! {
    /// `SEQUENCE name [, …]`.
    #[derive(Debug)]
    #[tok(SEQUENCE, this)]
    pub struct SequenceTarget {
        #[sep(COMMA)]
        pub names: one_or_many!(QualifiedName),
    }
}

recursa::ast_node! {
    /// `FOREIGN DATA WRAPPER name [, …]`.
    #[derive(Debug)]
    pub struct ForeignDataWrapperTarget {
        #[tok(FOREIGN, DATA, WRAPPER, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `FOREIGN SERVER name [, …]`.
    #[derive(Debug)]
    pub struct ForeignServerTarget {
        #[tok(FOREIGN, SERVER, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `FUNCTION sig [, …]` — uses the same `(name(args))` shape as `DROP FUNCTION`.
    #[derive(Debug)]
    #[tok(FUNCTION, this)]
    pub struct FunctionTarget {
        #[sep(COMMA)]
        pub sigs: one_or_many!(crate::ast::ddl::function::DropFunctionTarget),
    }
}

recursa::ast_node! {
    /// `PROCEDURE sig [, …]`.
    #[derive(Debug)]
    #[tok(PROCEDURE, this)]
    pub struct ProcedureTarget {
        #[sep(COMMA)]
        pub sigs: one_or_many!(crate::ast::ddl::function::DropFunctionTarget),
    }
}

recursa::ast_node! {
    /// `ROUTINE sig [, …]`.
    #[derive(Debug)]
    #[tok(ROUTINE, this)]
    pub struct RoutineTarget {
        #[sep(COMMA)]
        pub sigs: one_or_many!(crate::ast::ddl::function::DropFunctionTarget),
    }
}

recursa::ast_node! {
    /// `DATABASE name [, …]`.
    #[derive(Debug)]
    pub struct DatabaseTarget {
        #[tok(DATABASE, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `DOMAIN any_name [, …]`.
    #[derive(Debug)]
    pub struct DomainTarget {
        #[tok(DOMAIN, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `LANGUAGE name [, …]`.
    #[derive(Debug)]
    pub struct LanguageTarget {
        #[tok(LANGUAGE, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `LARGE OBJECT oid [, …]` — `NumericOnly_list`. Corpus uses only positive
    /// `IntegerLit`s; signed and floating-point forms are not exercised.
    #[derive(Debug)]
    #[tok(LARGE, OBJECT, this)]
    pub struct LargeObjectTarget {
        #[sep(COMMA)]
        pub oids: one_or_many!(literal::IntegerLit),
    }
}

recursa::ast_node! {
    /// `SCHEMA name [, …]`.
    #[derive(Debug)]
    pub struct SchemaTarget {
        #[tok(SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `TABLESPACE name [, …]`.
    #[derive(Debug)]
    pub struct TablespaceTarget {
        #[tok(TABLESPACE, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `TYPE any_name [, …]`.
    #[derive(Debug)]
    pub struct TypeTarget {
        #[tok(TYPE, this)]
        pub names: NameList,
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `parameter_name: ColId | parameter_name '.' ColId`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `. ColId` — one more part of a dotted parameter name.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(DOT, this)]
    pub struct ParameterNamePart(#[deref] pub crate::tokens::ColId);
}

#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// gram.y `parameter_name`: `ColId [. ColId ...]`.
    #[derive(Debug)]
    pub struct ParameterName {
        pub first: crate::tokens::ColId,
        pub rest: zero_or_many!(ParameterNamePart),
    }
}

// Added in 15: research, PostgreSQL 15, "Changes to existing statements"
// (REL_15_19 gram.y `privilege_target: PARAMETER parameter_name_list`).
#[cfg(feature = "since-pg15")]
recursa::ast_node! {
    /// `PARAMETER name [, …]`.
    #[derive(Debug)]
    #[tok(PARAMETER, this)]
    pub struct ParameterTarget {
        #[sep(COMMA)]
        pub names: one_or_many!(ParameterName),
    }
}

recursa::ast_node! {
    /// `ALL TABLES IN SCHEMA name [, …]`.
    #[derive(Debug)]
    pub struct AllTablesInSchemaTarget {
        #[tok(ALL, TABLES, IN, SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `ALL SEQUENCES IN SCHEMA name [, …]`.
    #[derive(Debug)]
    pub struct AllSequencesInSchemaTarget {
        #[tok(ALL, SEQUENCES, IN, SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `ALL FUNCTIONS IN SCHEMA name [, …]`.
    #[derive(Debug)]
    pub struct AllFunctionsInSchemaTarget {
        #[tok(ALL, FUNCTIONS, IN, SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `ALL PROCEDURES IN SCHEMA name [, …]`.
    #[derive(Debug)]
    pub struct AllProceduresInSchemaTarget {
        #[tok(ALL, PROCEDURES, IN, SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `ALL ROUTINES IN SCHEMA name [, …]`.
    #[derive(Debug)]
    pub struct AllRoutinesInSchemaTarget {
        #[tok(ALL, ROUTINES, IN, SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// `qualified_name [, …]` — the bare-table form (no `TABLE` keyword).
    ///
    /// Postgres' grammar accepts this as `privilege_target: qualified_name_list`
    /// (OBJECT_TABLE). It must be the last `PrivilegeTarget` variant because the
    /// first token is an identifier — anything earlier whose first set is an
    /// identifier (none exist here, all targets start with keywords) would
    /// otherwise win.
    #[derive(Debug)]
    pub struct BareTablesTarget {
        #[sep(COMMA)]
        pub names: one_or_many!(QualifiedName),
    }
}

recursa::ast_node! {
    /// `privilege_target` — what comes between `ON` and `TO`/`FROM` in
    /// GRANT/REVOKE.
    ///
    /// Variant ordering: every `ALL X IN SCHEMA` form (multi-token) is first; then
    /// the keyword-prefixed object kinds in arbitrary order (each starts with a
    /// distinct keyword); finally `Bare` because its first set is an identifier,
    /// so it must not eat a keyword-led variant. `FOREIGN DATA WRAPPER` is listed
    /// before `FOREIGN SERVER` so the longer match wins (both start with
    /// `FOREIGN`).
    #[derive(Debug)]
    pub enum PrivilegeTarget {
        AllTablesInSchema(AllTablesInSchemaTarget),
        AllSequencesInSchema(AllSequencesInSchemaTarget),
        AllFunctionsInSchema(AllFunctionsInSchemaTarget),
        AllProceduresInSchema(AllProceduresInSchemaTarget),
        AllRoutinesInSchema(AllRoutinesInSchemaTarget),
        Table(TableTarget),
        Sequence(SequenceTarget),
        ForeignDataWrapper(ForeignDataWrapperTarget),
        ForeignServer(ForeignServerTarget),
        Function(FunctionTarget),
        Procedure(ProcedureTarget),
        Routine(RoutineTarget),
        Database(DatabaseTarget),
        Domain(DomainTarget),
        Language(LanguageTarget),
        LargeObject(LargeObjectTarget),
        Schema(SchemaTarget),
        Tablespace(TablespaceTarget),
        Type(TypeTarget),
        /// Added in 15: research, PostgreSQL 15, "Changes to existing statements".
        #[config(since = pg15)]
        Parameter(ParameterTarget),
        Bare(BareTablesTarget),
    }
}

recursa::ast_node! {
    /// `GROUP role` — a grantee with explicit `GROUP` prefix.
    #[derive(Debug)]
    pub struct GroupGrantee {
        #[tok(GROUP, this)]
        pub role: RoleSpec,
    }
}

recursa::ast_node! {
    /// `grantee` — Postgres' grammar accepts either `[GROUP] RoleSpec` or `PUBLIC`.
    ///
    /// `PUBLIC` and the pseudo-roles `CURRENT_USER`/`CURRENT_ROLE`/`SESSION_USER`
    /// are scanned as plain `Ident`s by pg-sql's lexer (none are tokens here), so
    /// they round-trip through `RoleSpec` byte-faithfully. PostgreSQL's own
    /// `RoleSpec` rule recognises them by string match at parse time, giving an
    /// equivalent tree.
    ///
    /// Variant ordering: `Group` (two-token) before `Role` so longest match wins.
    #[derive(Debug)]
    pub enum Grantee {
        Group(GroupGrantee),
        Role(RoleSpec),
    }
}

recursa::ast_node! {
    /// `grantee [, …]` — comma-separated grantees.
    #[derive(Debug)]
    pub struct GranteeList {
        #[sep(COMMA)]
        pub items: one_or_many!(Grantee),
    }
}

recursa::ast_node! {
    /// `WITH GRANT OPTION` — privilege-grant trailing clause.
    #[derive(Debug)]
    pub enum WithGrantOption {
        #[tok(WITH, GRANT, OPTION)]
        Value,
    }
}

recursa::ast_node! {
    /// `GRANTED BY role` — optional grantor reference.
    #[derive(Debug)]
    pub struct GrantedBy {
        #[tok(GRANTED, BY, this)]
        pub role: RoleSpec,
    }
}

// Added in 16: research, PostgreSQL 16, "Changes to existing statements"
// (REL_16_15 gram.y `grant_role_opt_list`, commits e3ce2de09, 3d14e171e).
#[cfg(feature = "since-pg16")]
recursa::ast_node! {
    /// `{OPTION|TRUE|FALSE}` — the value of a role-grant `WITH` option.
    #[derive(Debug)]
    pub enum WithRoleOptValue {
        // `OPTION` alone is the 14 shape `WITH ADMIN OPTION`, so it records
        // nothing. `TRUE` and `FALSE` are the shapes that 16 adds (REL_15_19
        // gram.y `opt_grant_admin_option: WITH ADMIN OPTION`; REL_16_15
        // `grant_role_opt_value`, commit e3ce2de09).
        #[tok(OPTION)]
        Option,
        #[config(since = pg16)]
        #[tok(TRUE)]
        True,
        #[config(since = pg16)]
        #[tok(FALSE)]
        False,
    }
}

// Added in 16: research, PostgreSQL 16, "Changes to existing statements"
// (REL_16_15 gram.y `grant_role_opt_list`, commits e3ce2de09, 3d14e171e).
#[cfg(feature = "since-pg16")]
recursa::ast_node! {
    /// `name value` pair — gram.y `grant_role_opt: ColLabel
    /// grant_role_opt_value` (REL_17_11 7970). The grammar takes any
    /// `ColLabel`; only `user.c` rejects a name other than `ADMIN`, `INHERIT`
    /// or `SET`, and parse analysis is not part of version parity
    /// (principle 9).
    #[derive(Debug)]
    pub struct WithRoleOpt {
        pub name: crate::tokens::ColLabel,
        pub value: WithRoleOptValue,
    }
}

// Added in 16: research, PostgreSQL 16, "Changes to existing statements"
// (REL_16_15 gram.y `grant_role_opt_list`, commits e3ce2de09, 3d14e171e).
#[cfg(feature = "since-pg16")]
recursa::ast_node! {
    /// `WITH opt [, …]` — role-grant trailing options block.
    #[derive(Debug)]
    #[tok(WITH, this)]
    pub struct WithRoleOpts {
        #[sep(COMMA)]
        pub opts: one_or_many!(WithRoleOpt),
    }
}

// Removed in 16: REL_15_19 gram.y `opt_grant_admin_option: WITH ADMIN OPTION`.
// 16 replaces it with `WITH grant_role_opt_list` (research, PostgreSQL 16,
// "Changes to existing statements").
#[cfg(not(feature = "since-pg16"))]
recursa::ast_node! {
    /// `WITH ADMIN OPTION` — the only role-grant option before 16.
    #[derive(Debug)]
    pub enum WithAdminOption {
        #[tok(WITH, ADMIN, OPTION)]
        Value,
    }
}

recursa::ast_node! {
    /// `ON target TO grantees …` — the privilege-grant body of GRANT.
    #[derive(Debug)]
    pub struct GrantPrivilegeBody {
        #[tok(ON, this)]
        pub target: PrivilegeTarget,
        #[tok(TO, this)]
        pub grantees: GranteeList,
        pub grant_option: Option<WithGrantOption>,
        pub granted_by: Option<GrantedBy>,
    }
}

recursa::ast_node! {
    /// `TO roles [WITH …] [GRANTED BY …]` — the role-membership body of GRANT.
    #[derive(Debug)]
    pub struct GrantRoleBody {
        #[tok(TO, this)]
        pub roles: RoleList,
        // Added in 16: see `WithRoleOpts`.
        // A widening, not an addition (ADR 0010, `docs/minimum-version.md`):
        // the newer type accepts everything the older one does, so both arms
        // only remove and neither records. The gate for what 16 adds belongs
        // to those shapes.
        #[cfg(feature = "since-pg16")]
        pub with: Option<WithRoleOpts>,
        // Removed in 16: see `WithAdminOption`.
        #[cfg(not(feature = "since-pg16"))]
        pub with: Option<WithAdminOption>,
        pub granted_by: Option<GrantedBy>,
    }
}

recursa::ast_node! {
    /// The body of a `GRANT` after the leading privilege/role list — either the
    /// privilege-grant `ON … TO …` or the role-membership `TO …`.
    ///
    /// Variant ordering: `Privilege` first because its leading `ON` is a single
    /// keyword that the `Role` variant's leading `TO` can't shadow; both have
    /// disjoint first sets, so ordering is mostly cosmetic.
    #[derive(Debug)]
    pub enum GrantBody {
        Privilege(GrantPrivilegeBody),
        Role(GrantRoleBody),
    }
}

recursa::ast_node! {
    /// `GRANT privileges (ON target TO …) | (TO roles …)` — Postgres'
    /// `GrantStmt`/`GrantRoleStmt` unified.
    #[derive(Debug)]
    pub struct GrantStmt {
        #[tok(GRANT, this)]
        pub privileges: Privileges,
        pub body: GrantBody,
    }
}

recursa::ast_node! {
    /// `ON target FROM grantees …` — the privilege-revoke body.
    #[derive(Debug)]
    pub struct RevokePrivilegeBody {
        #[tok(ON, this)]
        pub target: PrivilegeTarget,
        #[tok(FROM, this)]
        pub grantees: GranteeList,
        pub granted_by: Option<GrantedBy>,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `FROM roles …` — the role-membership revoke body.
    #[derive(Debug)]
    pub struct RevokeRoleBody {
        #[tok(FROM, this)]
        pub roles: RoleList,
        pub granted_by: Option<GrantedBy>,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// The body of a `REVOKE` after the leading privilege/role list.
    #[derive(Debug)]
    pub enum RevokeBody {
        Privilege(RevokePrivilegeBody),
        Role(RevokeRoleBody),
    }
}

recursa::ast_node! {
    /// `GRANT OPTION FOR` — the leading "revoke only the grant option" prefix on
    /// the privilege-revoke form.
    #[derive(Debug)]
    pub enum RevokeGrantOptionFor {
        #[tok(GRANT, OPTION, FOR)]
        Value,
    }
}

// Changed in 16: research, PostgreSQL 16, "Changes to existing statements"
// (REL_16_15 gram.y `RevokeRoleStmt: REVOKE ColId OPTION FOR …`, commit
// e3ce2de09). REL_15_19 gram.y has only `REVOKE ADMIN OPTION FOR`.
#[cfg(feature = "since-pg16")]
recursa::ast_node! {
    /// `ColId OPTION FOR` — the role-revoke prefix that strips one option of
    /// an existing role grant. gram.y takes any `ColId` here and the catalog
    /// rejects a name other than `ADMIN`, `INHERIT` or `SET`.
    #[derive(Debug)]
    pub struct RevokeRoleOptionFor {
        #[tok(this, OPTION, FOR)]
        pub name: crate::tokens::ColId,
    }
}

// Removed in 16: see `RevokeRoleOptionFor`.
#[cfg(not(feature = "since-pg16"))]
recursa::ast_node! {
    /// `ADMIN OPTION FOR` — the only role-revoke option prefix before 16.
    #[derive(Debug)]
    pub enum RevokeRoleOptionFor {
        #[tok(ADMIN, OPTION, FOR)]
        Value,
    }
}

recursa::ast_node! {
    /// `REVOKE GRANT OPTION FOR privileges ON target FROM …` — gram.y
    /// `RevokeStmt`'s second arm. `GRANT` is a reserved keyword, so the role
    /// arm's `ColId` never spells it, and the prefix reaches only the
    /// privilege body.
    #[derive(Debug)]
    pub struct RevokeGrantOptionForm {
        #[tok(GRANT, OPTION, FOR, this)]
        pub privileges: Privileges,
        pub body: RevokePrivilegeBody,
    }
}

recursa::ast_node! {
    /// `REVOKE ColId OPTION FOR roles FROM …` — gram.y `RevokeRoleStmt`'s
    /// second arm. It has no `ON target`, so the prefix reaches only the role
    /// body.
    #[derive(Debug)]
    pub struct RevokeRoleOptionForm {
        pub option_for: RevokeRoleOptionFor,
        pub privileges: Privileges,
        pub body: RevokeRoleBody,
    }
}

recursa::ast_node! {
    /// `REVOKE privileges (ON target FROM …) | (FROM roles …)` — the first arm
    /// of gram.y `RevokeStmt` and of `RevokeRoleStmt`.
    #[derive(Debug)]
    pub struct RevokePlainForm {
        pub privileges: Privileges,
        pub body: RevokeBody,
    }
}

recursa::ast_node! {
    /// The three `REVOKE` forms. gram.y pairs each `OPTION FOR` prefix with
    /// one body: `GRANT OPTION FOR` belongs to `RevokeStmt`, which needs `ON
    /// target`, and `ColId OPTION FOR` belongs to `RevokeRoleStmt`, which has
    /// no `ON` (REL_17_11 gram.y `RevokeStmt` 7556, `RevokeRoleStmt` 7935).
    ///
    /// Variant ordering: the two prefixed forms first; `Plain` is the
    /// catch-all, and its privileges can begin with the same `ColId`.
    #[derive(Debug)]
    pub enum RevokeForm {
        GrantOption(RevokeGrantOptionForm),
        RoleOption(RevokeRoleOptionForm),
        Plain(RevokePlainForm),
    }
}

recursa::ast_node! {
    /// `REVOKE …` — Postgres' `RevokeStmt` and `RevokeRoleStmt` unified on the
    /// `REVOKE` keyword they share.
    #[derive(Debug)]
    #[tok(REVOKE, this)]
    pub struct RevokeStmt {
        pub form: RevokeForm,
    }
}

// -----------------------------------------------------------------------
// ALTER DEFAULT PRIVILEGES.
// -----------------------------------------------------------------------

recursa::ast_node! {
    /// `defacl_privilege_target` — the object-kind keyword inside ADP. Postgres
    /// only allows one of TABLES / SEQUENCES / FUNCTIONS / PROCEDURES / ROUTINES
    /// / SCHEMAS / TYPES.
    #[derive(Debug)]
    pub enum DefAclTarget {
        #[tok(TABLES)]
        Tables,
        #[tok(SEQUENCES)]
        Sequences,
        #[tok(FUNCTIONS)]
        Functions,
        #[tok(PROCEDURES)]
        Procedures,
        #[tok(ROUTINES)]
        Routines,
        #[tok(SCHEMAS)]
        Schemas,
        #[tok(TYPES)]
        Types,
        /// Added in 18: gram.y `defacl_privilege_target: LARGE_P OBJECTS_P`
        /// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
        /// item 8; REL_18_6 gram.y `defacl_privilege_target`).
        #[config(since = pg18)]
        #[tok(LARGE, OBJECTS)]
        LargeObjects,
    }
}

recursa::ast_node! {
    /// `GRANT privileges ON defacl_target TO grantees [WITH GRANT OPTION]` — the
    /// inner GRANT of ALTER DEFAULT PRIVILEGES.
    #[derive(Debug)]
    pub struct DefAclGrant {
        #[tok(GRANT, this)]
        pub privileges: Privileges,
        #[tok(ON, this)]
        pub target: DefAclTarget,
        #[tok(TO, this)]
        pub grantees: GranteeList,
        pub grant_option: Option<WithGrantOption>,
    }
}

recursa::ast_node! {
    /// `REVOKE [GRANT OPTION FOR] privileges ON defacl_target FROM grantees
    /// [CASCADE|RESTRICT]` — the inner REVOKE of ALTER DEFAULT PRIVILEGES. Note:
    /// no `GRANTED BY` in ADP's revoke per `gram.y`'s `DefACLAction`.
    #[derive(Debug)]
    #[tok(REVOKE, this)]
    pub struct DefAclRevoke {
        pub grant_option_for: Option<RevokeGrantOptionFor>,
        pub privileges: Privileges,
        #[tok(ON, this)]
        pub target: DefAclTarget,
        #[tok(FROM, this)]
        pub grantees: GranteeList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// The inner action of `ALTER DEFAULT PRIVILEGES`.
    #[derive(Debug)]
    pub enum DefAclAction {
        Grant(DefAclGrant),
        Revoke(DefAclRevoke),
    }
}

recursa::ast_node! {
    /// `FOR { ROLE | USER }` — Postgres' `FOR ROLE` and its `FOR USER` synonym.
    #[derive(Debug)]
    pub enum ForRoleOrUser {
        #[tok(ROLE)]
        Role,
        #[tok(USER)]
        User,
    }
}

recursa::ast_node! {
    /// `FOR { ROLE | USER } role [, …]` — restricts the default privileges to
    /// the listed role(s).
    #[derive(Debug)]
    pub struct DefAclForRoleOption {
        #[tok(FOR, this)]
        pub role_or_user: ForRoleOrUser,
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// `IN SCHEMA name [, …]` — restricts the default privileges to listed
    /// schema(s).
    #[derive(Debug)]
    pub struct DefAclInSchemaOption {
        #[tok(IN, SCHEMA, this)]
        pub names: NameList,
    }
}

recursa::ast_node! {
    /// A single `DefACLOption` — either `FOR ROLE …` or `IN SCHEMA …`. The
    /// grammar allows them to repeat in arbitrary order, so an unordered list of
    /// these covers every legal form.
    #[derive(Debug)]
    pub enum DefAclOption {
        ForRole(DefAclForRoleOption),
        InSchema(DefAclInSchemaOption),
    }
}

recursa::ast_node! {
    /// `ALTER DEFAULT PRIVILEGES [DefACLOption …] (GRANT … | REVOKE …)`.
    #[derive(Debug)]
    #[tok(ALTER, DEFAULT, PRIVILEGES, this)]
    pub struct AlterDefaultPrivilegesStmt {
        pub options: zero_or_many!(DefAclOption),
        pub action: DefAclAction,
    }
}
