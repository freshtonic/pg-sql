//! GRANT, REVOKE, and ALTER DEFAULT PRIVILEGES.
//!
//! These statements share the privilege/grantee/target/role vocabulary;
//! ALTER DEFAULT PRIVILEGES wraps a grant or revoke body and reuses the
//! same `Privileges`/`GrantOption`/`RevokeGrantOptionFor` machinery, so
//! all three live in this file.

use recursa::seq::Seq1;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PrivColumnList<'input> {
    pub cols:
        Surrounded<punct::LParen, Seq1<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
}

/// `ALTER SYSTEM` privilege keyword — Postgres' `privilege: ALTER SYSTEM_P`.
///
/// Modeled as its own struct so the multi-keyword form sorts before the
/// `Named` variant in `Privilege` (longest-match-wins).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterSystemPriv {
    pub alter: ALTER,
    pub system: SYSTEM,
}

/// `SELECT [(columns)]` privilege.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SelectPriv<'input> {
    pub select: SELECT,
    pub cols: Option<PrivColumnList<'input>>,
}

/// `REFERENCES [(columns)]` privilege.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReferencesPriv<'input> {
    pub references: REFERENCES,
    pub cols: Option<PrivColumnList<'input>>,
}

/// `CREATE [(columns)]` privilege.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CreatePriv<'input> {
    pub create: CREATE,
    pub cols: Option<PrivColumnList<'input>>,
}

/// `name [(columns)]` privilege — Postgres' `privilege: ColId opt_column_list`.
///
/// Covers INSERT/UPDATE/DELETE/TRUNCATE/USAGE/EXECUTE/CONNECT/TEMPORARY/TEMP/
/// MAINTAIN/TRIGGER and any role-name-as-privilege in the role-membership form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum Privilege<'input> {
    AlterSystem(AlterSystemPriv),
    Select(SelectPriv<'input>),
    References(ReferencesPriv<'input>),
    Create(CreatePriv<'input>),
    Named(NamedPriv<'input>),
}

/// `ALL` with no follow-up keyword or column list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllBarePrivs {
    pub all: ALL,
}

/// `ALL PRIVILEGES`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllPrivilegesPrivs {
    pub all: ALL,
    pub privileges: PRIVILEGES,
}

/// `ALL (columns)` — column-scoped variant of `ALL`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllColsPrivs<'input> {
    pub all: ALL,
    pub cols: PrivColumnList<'input>,
}

/// `ALL PRIVILEGES (columns)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllPrivilegesColsPrivs<'input> {
    pub all: ALL,
    pub privileges: PRIVILEGES,
    pub cols: PrivColumnList<'input>,
}

/// `privilege [, …]` — non-`ALL` privilege list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PrivilegeList<'input> {
    pub items: Seq1<Privilege<'input>, punct::Comma>,
}

/// The leading privileges/role list of a GRANT/REVOKE — Postgres' `privileges`.
///
/// Variant ordering: longest `ALL …` forms first; bare `ALL` last among the
/// ALL-prefixed forms. `List` is the catch-all and must come after every
/// `ALL`-prefixed variant because `ALL` is a hard keyword that `Privilege`'s
/// `Named` won't accept anyway.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum Privileges<'input> {
    AllPrivilegesCols(AllPrivilegesColsPrivs<'input>),
    AllPrivileges(AllPrivilegesPrivs),
    AllCols(AllColsPrivs<'input>),
    All(AllBarePrivs),
    List(PrivilegeList<'input>),
}

/// `TABLE name [, …]` — explicit-keyword table target.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableTarget<'input> {
    pub table: TABLE,
    pub names: Seq1<QualifiedName<'input>, punct::Comma>,
}

/// `SEQUENCE name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SequenceTarget<'input> {
    pub sequence: SEQUENCE,
    pub names: Seq1<QualifiedName<'input>, punct::Comma>,
}

/// `FOREIGN DATA WRAPPER name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForeignDataWrapperTarget<'input> {
    pub foreign: FOREIGN,
    pub data: DATA,
    pub wrapper: WRAPPER,
    pub names: NameList<'input>,
}

/// `FOREIGN SERVER name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForeignServerTarget<'input> {
    pub foreign: FOREIGN,
    pub server: SERVER,
    pub names: NameList<'input>,
}

/// `FUNCTION sig [, …]` — uses the same `(name(args))` shape as `DROP FUNCTION`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FunctionTarget<'input> {
    pub function: FUNCTION,
    pub sigs: Seq1<crate::ast::ddl::function::DropFunctionTarget<'input>, punct::Comma>,
}

/// `PROCEDURE sig [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ProcedureTarget<'input> {
    pub procedure: PROCEDURE,
    pub sigs: Seq1<crate::ast::ddl::function::DropFunctionTarget<'input>, punct::Comma>,
}

/// `ROUTINE sig [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RoutineTarget<'input> {
    pub routine: ROUTINE,
    pub sigs: Seq1<crate::ast::ddl::function::DropFunctionTarget<'input>, punct::Comma>,
}

/// `DATABASE name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DatabaseTarget<'input> {
    pub database: DATABASE,
    pub names: NameList<'input>,
}

/// `DOMAIN any_name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainTarget<'input> {
    pub domain: DOMAIN,
    pub names: NameList<'input>,
}

/// `LANGUAGE name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LanguageTarget<'input> {
    pub language: LANGUAGE,
    pub names: NameList<'input>,
}

/// `LARGE OBJECT oid [, …]` — `NumericOnly_list`. Corpus uses only positive
/// `IntegerLit`s; signed and floating-point forms are not exercised.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LargeObjectTarget<'input> {
    pub large: LARGE,
    pub object: crate::tokens::soft_keyword::OBJECT,
    pub oids: Seq1<literal::IntegerLit<'input>, punct::Comma>,
}

/// `SCHEMA name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SchemaTarget<'input> {
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// `TABLESPACE name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TablespaceTarget<'input> {
    pub tablespace: TABLESPACE,
    pub names: NameList<'input>,
}

/// `TYPE any_name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TypeTarget<'input> {
    pub r#type: TYPE,
    pub names: NameList<'input>,
}

/// `ALL TABLES IN SCHEMA name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllTablesInSchemaTarget<'input> {
    pub all: ALL,
    pub tables: crate::tokens::soft_keyword::TABLES,
    pub r#in: IN,
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// `ALL SEQUENCES IN SCHEMA name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllSequencesInSchemaTarget<'input> {
    pub all: ALL,
    pub sequences: SEQUENCES,
    pub r#in: IN,
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// `ALL FUNCTIONS IN SCHEMA name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllFunctionsInSchemaTarget<'input> {
    pub all: ALL,
    pub functions: crate::tokens::soft_keyword::FUNCTIONS,
    pub r#in: IN,
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// `ALL PROCEDURES IN SCHEMA name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllProceduresInSchemaTarget<'input> {
    pub all: ALL,
    pub procedures: crate::tokens::soft_keyword::PROCEDURES,
    pub r#in: IN,
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// `ALL ROUTINES IN SCHEMA name [, …]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AllRoutinesInSchemaTarget<'input> {
    pub all: ALL,
    pub routines: crate::tokens::soft_keyword::ROUTINES,
    pub r#in: IN,
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// `qualified_name [, …]` — the bare-table form (no `TABLE` keyword).
///
/// Postgres' grammar accepts this as `privilege_target: qualified_name_list`
/// (OBJECT_TABLE). It must be the last `PrivilegeTarget` variant because the
/// first token is an identifier — anything earlier whose first set is an
/// identifier (none exist here, all targets start with keywords) would
/// otherwise win.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct BareTablesTarget<'input> {
    pub names: Seq1<QualifiedName<'input>, punct::Comma>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GroupGrantee<'input> {
    pub group: GROUP,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum Grantee<'input> {
    Group(GroupGrantee<'input>),
    Role(RoleSpec<'input>),
}

/// `grantee [, …]` — comma-separated grantees.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GranteeList<'input> {
    pub items: Seq1<Grantee<'input>, punct::Comma>,
}

/// `WITH GRANT OPTION` — privilege-grant trailing clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithGrantOption {
    pub with: WITH,
    pub grant: GRANT,
    pub option: OPTION,
}

/// `GRANTED BY role` — optional grantor reference.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GrantedBy<'input> {
    pub granted: crate::tokens::soft_keyword::GRANTED,
    pub by: BY,
    pub role: RoleSpec<'input>,
}

/// `{ADMIN|INHERIT|SET}` — the keyword on a role-grant `WITH` option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WithRoleOptKind {
    Admin(crate::tokens::soft_keyword::ADMIN),
    Inherit(INHERIT),
    Set(SET),
}

/// `{OPTION|TRUE|FALSE}` — the value of a role-grant `WITH` option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WithRoleOptValue {
    Option(OPTION),
    True(TRUE),
    False(FALSE),
}

/// `kind value` pair — Postgres' `grant_role_opt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithRoleOpt {
    pub kind: WithRoleOptKind,
    pub value: WithRoleOptValue,
}

/// `WITH opt [, …]` — role-grant trailing options block.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithRoleOpts {
    pub with: WITH,
    pub opts: Seq1<WithRoleOpt, punct::Comma>,
}

/// `ON target TO grantees …` — the privilege-grant body of GRANT.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GrantPrivilegeBody<'input> {
    pub on: ON,
    pub target: PrivilegeTarget<'input>,
    pub to: TO,
    pub grantees: GranteeList<'input>,
    pub grant_option: Option<WithGrantOption>,
    pub granted_by: Option<GrantedBy<'input>>,
}

/// `TO roles [WITH …] [GRANTED BY …]` — the role-membership body of GRANT.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GrantRoleBody<'input> {
    pub to: TO,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum GrantBody<'input> {
    Privilege(GrantPrivilegeBody<'input>),
    Role(GrantRoleBody<'input>),
}

/// `GRANT privileges (ON target TO …) | (TO roles …)` — Postgres'
/// `GrantStmt`/`GrantRoleStmt` unified.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dcl"])]
pub struct GrantStmt<'input> {
    pub grant: GRANT,
    pub privileges: Privileges<'input>,
    pub body: GrantBody<'input>,
}

/// `ON target FROM grantees …` — the privilege-revoke body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RevokePrivilegeBody<'input> {
    pub on: ON,
    pub target: PrivilegeTarget<'input>,
    pub from: FROM,
    pub grantees: GranteeList<'input>,
    pub granted_by: Option<GrantedBy<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// `FROM roles …` — the role-membership revoke body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RevokeRoleBody<'input> {
    pub from: FROM,
    pub roles: RoleList<'input>,
    pub granted_by: Option<GrantedBy<'input>>,
    pub behavior: Option<DropBehavior>,
}

/// The body of a `REVOKE` after the leading privilege/role list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RevokeBody<'input> {
    Privilege(RevokePrivilegeBody<'input>),
    Role(RevokeRoleBody<'input>),
}

/// `GRANT OPTION FOR` — the leading "revoke only the grant option" prefix on
/// the privilege-revoke form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RevokeGrantOptionFor {
    pub grant: GRANT,
    pub option: OPTION,
    pub r#for: FOR,
}

/// `ADMIN OPTION FOR` — the role-revoke counterpart that strips just the
/// ADMIN option from an existing role grant.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RevokeAdminOptionFor {
    pub admin: crate::tokens::soft_keyword::ADMIN,
    pub option: OPTION,
    pub r#for: FOR,
}

/// `INHERIT OPTION FOR` — strips just INHERIT.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RevokeInheritOptionFor {
    pub inherit: INHERIT,
    pub option: OPTION,
    pub r#for: FOR,
}

/// `SET OPTION FOR` — strips just SET.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RevokeSetOptionFor {
    pub set: SET,
    pub option: OPTION,
    pub r#for: FOR,
}

/// Optional `… OPTION FOR` prefix on `REVOKE`. PG distinguishes `GRANT OPTION
/// FOR` (privilege form) from `{ADMIN|INHERIT|SET} OPTION FOR` (role form);
/// the body following the privileges decides which it actually is.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum RevokeOptionFor {
    GrantOption(RevokeGrantOptionFor),
    AdminOption(RevokeAdminOptionFor),
    InheritOption(RevokeInheritOptionFor),
    SetOption(RevokeSetOptionFor),
}

/// `REVOKE [… OPTION FOR] privileges (ON target FROM …) | (FROM roles …)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dcl"])]
pub struct RevokeStmt<'input> {
    pub revoke: REVOKE,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DefAclTarget {
    Tables(crate::tokens::soft_keyword::TABLES),
    Sequences(SEQUENCES),
    Functions(crate::tokens::soft_keyword::FUNCTIONS),
    Procedures(crate::tokens::soft_keyword::PROCEDURES),
    Routines(crate::tokens::soft_keyword::ROUTINES),
    Schemas(crate::tokens::soft_keyword::SCHEMAS),
    Types(crate::tokens::soft_keyword::TYPES),
}

/// `GRANT privileges ON defacl_target TO grantees [WITH GRANT OPTION]` — the
/// inner GRANT of ALTER DEFAULT PRIVILEGES.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefAclGrant<'input> {
    pub grant: GRANT,
    pub privileges: Privileges<'input>,
    pub on: ON,
    pub target: DefAclTarget,
    pub to: TO,
    pub grantees: GranteeList<'input>,
    pub grant_option: Option<WithGrantOption>,
}

/// `REVOKE [GRANT OPTION FOR] privileges ON defacl_target FROM grantees
/// [CASCADE|RESTRICT]` — the inner REVOKE of ALTER DEFAULT PRIVILEGES. Note:
/// no `GRANTED BY` in ADP's revoke per `gram.y`'s `DefACLAction`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefAclRevoke<'input> {
    pub revoke: REVOKE,
    pub grant_option_for: Option<RevokeGrantOptionFor>,
    pub privileges: Privileges<'input>,
    pub on: ON,
    pub target: DefAclTarget,
    pub from: FROM,
    pub grantees: GranteeList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// The inner action of `ALTER DEFAULT PRIVILEGES`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DefAclAction<'input> {
    Grant(DefAclGrant<'input>),
    Revoke(DefAclRevoke<'input>),
}

/// `FOR { ROLE | USER }` — Postgres' `FOR ROLE` and its `FOR USER` synonym.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ForRoleOrUser {
    Role(ROLE),
    User(USER),
}

/// `FOR { ROLE | USER } role [, …]` — restricts the default privileges to
/// the listed role(s).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefAclForRoleOption<'input> {
    pub r#for: FOR,
    pub role_or_user: ForRoleOrUser,
    pub roles: RoleList<'input>,
}

/// `IN SCHEMA name [, …]` — restricts the default privileges to listed
/// schema(s).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefAclInSchemaOption<'input> {
    pub r#in: IN,
    pub schema: SCHEMA,
    pub names: NameList<'input>,
}

/// A single `DefACLOption` — either `FOR ROLE …` or `IN SCHEMA …`. The
/// grammar allows them to repeat in arbitrary order, so an unordered list of
/// these covers every legal form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DefAclOption<'input> {
    ForRole(DefAclForRoleOption<'input>),
    InSchema(DefAclInSchemaOption<'input>),
}

/// `ALTER DEFAULT PRIVILEGES [DefACLOption …] (GRANT … | REVOKE …)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterDefaultPrivilegesStmt<'input> {
    pub alter: ALTER,
    pub default: DEFAULT,
    pub privileges: PRIVILEGES,
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
