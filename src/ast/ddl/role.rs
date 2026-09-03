//! ROLE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::foreign::AlterGenericOptions;
use crate::ast::ddl::function::{FunctionBuiltinType, FunctionTypeName};
use crate::ast::ddl::table::CreateGenericOptions;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::{literal, punct};

/// Optional `TEMP` or `TEMPORARY` modifier that can appear between `CREATE`
/// and the object keyword for temporary objects (sequences, tables, views,
/// etc.).
///
/// Variant ordering: `Temporary` (longer) before `Temp` so the longer keyword
/// wins longest-match disambiguation.
#[derive(recursa::Node, Debug, Clone)]
pub enum TempModifier {
    #[tok(TEMPORARY)]
    Temporary,
    #[tok(TEMP)]
    Temp,
}

/// A single `def_arg` value — Postgres' grammar:
///
/// ```text
/// def_arg: func_type
///        | reserved_keyword  (e.g. TRUE, FALSE, ANALYZE, ...)
///        | qual_all_Op
///        | NumericOnly
///        | Sconst
///        | NONE
/// ```
///
/// pg-sql models the cases the corpus actually uses:
/// - numeric (with optional sign) — `internallength = 24`, `default = -1`
/// - string literal — `category = 'x'`, `LOCALE = "C"` (the double-quoted
///   form is parsed via the `Type` arm because `func_type`'s ident path
///   accepts quoted identifiers)
/// - `NONE` — `OWNED BY NONE` (used at `def_arg` level by some grammars)
/// - `DEFAULT` keyword as a value — `default_test_row` style
/// - reserved keyword `TRUE` / `FALSE` — `DETERMINISTIC = TRUE`,
///   `preferred = true`
/// - a type-name with optional precision and array suffix — `subtype = int4[]`,
///   `internallength = variable`, `alignment = double`, `PROVIDER = builtin`
/// - an operator name — `commutator = ===`, `negator = !==` on
///   CREATE/ALTER OPERATOR. PG's `def_arg` accepts `qual_all_Op` directly
///   so the right-hand side of a `def_elem` can itself be a bare operator.
///   The corpus only exercises the bare-name form; the `OPERATOR(any_op)`
///   spelling is not modelled until a corpus statement needs it.
///
/// Variant ordering: definite-keyword variants (`Default`, `True`, `False`,
/// `None`, `Any`) come first; `Numeric` next (it can start with `+`/`-`/digit
/// and matches no other variant); `QualOp` after `Numeric` because they
/// share `+`/`-` as a leading token — `Numeric` consumes `+1`/`-2`, and
/// `QualOp` only wins on a lone operator token; `Type` last (it's the
/// broadest, accepting any identifier or built-in type keyword).
#[derive(recursa::Node, Debug, Clone)]
pub enum DefArg<'input> {
    #[tok(DEFAULT)]
    Default,
    #[tok(TRUE)]
    True,
    #[tok(FALSE)]
    False,
    #[tok(NONE)]
    None,
    #[tok(ANY)]
    /// Reserved keyword used as a `def_arg` value. The corpus uses this
    /// for `ALTER SUBSCRIPTION ... SET (origin = any)`. Per gram.y
    /// `def_arg` accepts `reserved_keyword` directly; pg-sql only adds
    /// keywords actually exercised by the corpus.
    Any,
    String(CopySconst<'input>),
    Numeric(NumericOnly<'input>),
    /// A bare operator name on the RHS of a `def_elem`. Postgres' `def_arg`
    /// accepts `qual_all_Op`; the corpus exercises this on CREATE/ALTER
    /// OPERATOR's `commutator`/`negator` attributes.
    QualOp(crate::ast::shared::names::OperatorName<'input>),
    /// `SETOF type` — used in CREATE OPERATOR `leftarg`/`rightarg`
    /// (PG gram.y `def_arg: func_type`, and `func_type: Typename | type_function_name … | SETOF SimpleTypename …`).
    /// Listed before `Type` so the SETOF keyword reliably anchors this variant.
    Setof(DefArgSetof<'input>),
    /// A built-in type name with ordinary cast-type suffixes.
    BuiltinType(FunctionBuiltinType<'input>),
    /// An identifier-spelled type or `name(typename_list)` function-style
    /// value. The qualified name is parsed once before deciding whether a
    /// parenthesized argument is a typmod list or a type-name list.
    NamedType(DefArgNamedType<'input>),
}

/// Parenthesized values following an identifier-spelled def-arg name.
/// Integer-led values are type modifiers; type-led values are aggregate
/// support-function argument types.
#[derive(recursa::Node, Debug, Clone)]
pub enum DefArgNamedParameterValues<'input> {
    TypeModifiers(#[sep(COMMA)] recursa::Vec1<TypeModifierArg<'input>>),
    FuncArgs(#[sep(COMMA)] recursa::Vec1<CastType<'input>>),
}

/// A factored parenthesized def-arg suffix. Factoring the delimiters lets
/// Recursa dispatch on the first value inside them (`10` versus `int8`).
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct DefArgNamedParameters<'input> {
    pub values: DefArgNamedParameterValues<'input>,
}

/// Identifier-spelled def-arg type or function-style value.
#[derive(recursa::Node, Debug, Clone)]
pub struct DefArgNamedType<'input> {
    pub name: FunctionTypeName<'input>,
    #[presence(PRECISION)]
    pub precision_keyword: bool,
    #[presence(VARYING)]
    pub varying: bool,
    pub parameters: Option<DefArgNamedParameters<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub interval_qualifier: Option<IntervalQualifier<'input>>,
    pub array_suffixes: Vec<ArraySuffix<'input>>,
    pub array_kw_suffix: Option<ArrayKwSuffix<'input>>,
}

/// `SETOF type` value in a `def_list` (PG: `func_type` form on `def_arg`).
#[derive(recursa::Node, Debug, Clone)]
pub struct DefArgSetof<'input> {
    #[tok(SETOF, this)]
    pub type_name: CastType<'input>,
}

/// `value` separator on `def_elem` — Postgres uses `'='`.
///
/// One-variant enum so the AST has a typed node where the literal sits.
#[derive(recursa::Node, Debug, Clone)]
pub struct DefElemValue<'input> {
    #[tok(EQ, this)]
    pub arg: DefArg<'input>,
}

/// One entry in a `def_list` — `name [= value]`.
///
/// The name is `AliasName` so any keyword or identifier is accepted (Postgres
/// `ColLabel` permits every keyword class). The value is optional: some
/// CREATE TYPE base-type forms use bare names like `passedbyvalue`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DefElem<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<DefElemValue<'input>>,
}

/// A parenthesised `def_list`: `(name [= value], ...)`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct DefList<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<DefElem<'input>>,
}

/// Password value in `PASSWORD { sconst | NULL }` — Postgres'
/// `AlterOptRoleElem` PASSWORD branch.
#[derive(recursa::Node, Debug, Clone)]
pub enum PasswordValue<'input> {
    #[tok(NULL)]
    Null,
    /// Any string-constant form: plain, `E'…'`, `U&'…'`, `B'…'`, `X'…'`.
    String(CopySconst<'input>),
}

/// `[ENCRYPTED] PASSWORD value` — Postgres' role-attribute PASSWORD clause.
/// `ENCRYPTED` is a backward-compat noise word (passwords are always
/// encrypted today). The `UNENCRYPTED PASSWORD` form is also recognised by
/// PG's grammar but raises an immediate error, so we accept it and round-
/// trip it byte-faithfully.
#[derive(recursa::Node, Debug, Clone)]
pub struct PasswordOption<'input> {
    /// Optional `ENCRYPTED` or `UNENCRYPTED` modifier.
    pub modifier: Option<PasswordModifier>,
    #[tok(PASSWORD, this)]
    pub value: PasswordValue<'input>,
}

/// `ENCRYPTED | UNENCRYPTED` — the backward-compat password modifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum PasswordModifier {
    #[tok(ENCRYPTED)]
    Encrypted,
    #[tok(UNENCRYPTED)]
    Unencrypted,
}

/// `CONNECTION LIMIT signed_iconst` — role-attribute connection limit.
#[derive(recursa::Node, Debug, Clone)]
pub struct ConnectionLimitOption<'input> {
    #[tok(CONNECTION, LIMIT, this)]
    pub value: SignedIconst<'input>,
}

/// `VALID UNTIL sconst` — role-attribute expiry.
#[derive(recursa::Node, Debug, Clone)]
pub struct ValidUntilOption<'input> {
    #[tok(VALID, UNTIL, this)]
    pub value: CopySconst<'input>,
}

/// `IN { ROLE | GROUP } role_list` — role membership target list.
///
/// Variant ordering: keyword kind discriminates first; both are single-token.
#[derive(recursa::Node, Debug, Clone)]
pub enum InRoleOrGroup {
    #[tok(ROLE)]
    Role,
    #[tok(GROUP)]
    Group,
}

/// `IN { ROLE | GROUP } role [, ...]` — Postgres' `CreateOptRoleElem`
/// `IN_P ROLE`/`IN_P GROUP_P` branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct InRoleOption<'input> {
    #[tok(IN, this)]
    pub kind: InRoleOrGroup,
    pub roles: RoleList<'input>,
}

/// `SYSID iconst` — legacy noise option preserved for backward compat.
#[derive(recursa::Node, Debug, Clone)]
pub struct SysIdOption<'input> {
    #[tok(SYSID, this)]
    pub value: literal::IntegerLit<'input>,
}

/// `ADMIN role_list` — `CreateOptRoleElem` ADMIN branch (creates role with
/// admin members).
#[derive(recursa::Node, Debug, Clone)]
pub struct AdminOption<'input> {
    #[tok(ADMIN, this)]
    pub roles: RoleList<'input>,
}

/// `ROLE role_list` — `CreateOptRoleElem` ROLE branch (creates role with
/// child members).
#[derive(recursa::Node, Debug, Clone)]
pub struct RoleMembersOption<'input> {
    #[tok(ROLE, this)]
    pub roles: RoleList<'input>,
}

/// `USER role_list` — legacy `CREATE GROUP name [WITH] USER u1, u2`
/// (supported but undocumented for ALTER GROUP); also matches
/// `AlterOptRoleElem`'s undocumented `USER role_list` branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct UserMembersOption<'input> {
    #[tok(USER, this)]
    pub roles: RoleList<'input>,
}

/// A single CREATE ROLE / CREATE USER / CREATE GROUP option — Postgres'
/// `CreateOptRoleElem`. Options are unordered and repeatable.
///
/// Variant ordering: multi-keyword forms (`IN ROLE`, `CONNECTION LIMIT`,
/// `VALID UNTIL`, `ENCRYPTED PASSWORD`, `UNENCRYPTED PASSWORD`) before any
/// single-keyword form they share a first-token with, so longest-match-wins
/// disambiguates. `ROLE role_list` and `IN ROLE …` both start with `ROLE`/
/// `IN` so the longer `IN ROLE` form wins on its leading `IN`.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateRoleOption<'input> {
    // Multi-keyword forms first.
    InRole(InRoleOption<'input>),
    ConnectionLimit(ConnectionLimitOption<'input>),
    ValidUntil(ValidUntilOption<'input>),
    Password(PasswordOption<'input>),
    SysId(SysIdOption<'input>),
    Admin(AdminOption<'input>),
    Role(RoleMembersOption<'input>),
    User(UserMembersOption<'input>),
    // Single-keyword forms — soft keywords for the role-attribute names.
    #[tok(SUPERUSER)]
    Superuser,
    #[tok(NOSUPERUSER)]
    NoSuperuser,
    #[tok(CREATEDB)]
    CreateDb,
    #[tok(NOCREATEDB)]
    NoCreateDb,
    #[tok(CREATEROLE)]
    CreateRole,
    #[tok(NOCREATEROLE)]
    NoCreateRole,
    #[tok(INHERIT)]
    Inherit,
    #[tok(NOINHERIT)]
    NoInherit,
    #[tok(LOGIN)]
    Login,
    #[tok(NOLOGIN)]
    NoLogin,
    #[tok(REPLICATION)]
    Replication,
    #[tok(NOREPLICATION)]
    NoReplication,
    #[tok(BYPASSRLS)]
    BypassRls,
    #[tok(NOBYPASSRLS)]
    NoBypassRls,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateGroupStmt<'input> {
    #[tok(CREATE, GROUP, this)]
    pub name: crate::tokens::NonReservedWord<'input>,
    #[tok(optional(WITH), this)]
    pub options: Vec<CreateRoleOption<'input>>,
}

/// `DROP GROUP [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, GROUP, this)]
pub struct DropGroupStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub roles: RoleList<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateRoleStmt<'input> {
    #[tok(CREATE, ROLE, this)]
    pub name: crate::tokens::NonReservedWord<'input>,
    #[tok(optional(WITH), this)]
    pub options: Vec<CreateRoleOption<'input>>,
}

/// `DROP ROLE [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, ROLE, this)]
pub struct DropRoleStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub roles: RoleList<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateUserStmt<'input> {
    #[tok(CREATE, USER, this)]
    pub name: crate::tokens::NonReservedWord<'input>,
    #[tok(optional(WITH), this)]
    pub options: Vec<CreateRoleOption<'input>>,
}

/// `DROP USER [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, USER, this)]
pub struct DropUserStmt<'input> {
    pub if_exists: Option<IfExists>,
    pub roles: RoleList<'input>,
}

/// A single option in an `AlterOptRoleList` — Postgres' `AlterOptRoleElem`.
///
/// This is a strict subset of [`CreateRoleOption`] / `CreateOptRoleElem`:
/// the create-only options (`SYSID iconst`, `ADMIN role_list`, `ROLE
/// role_list`, `IN ROLE role_list`, `IN GROUP role_list`) are excluded by
/// gram.y. `USER role_list` is allowed (officially undocumented; supported
/// "for use by ALTER GROUP", per the gram.y comment).
///
/// Variant ordering: multi-keyword forms (`CONNECTION LIMIT`, `VALID
/// UNTIL`, `[ENCRYPTED|UNENCRYPTED] PASSWORD`) before any single-keyword
/// form they share a first-token with — though here they have disjoint
/// first tokens, order is for clarity. The bare attribute keywords
/// (`SUPERUSER` etc.) are all distinct soft keywords.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterRoleOption<'input> {
    // Multi-keyword forms first.
    ConnectionLimit(ConnectionLimitOption<'input>),
    ValidUntil(ValidUntilOption<'input>),
    Password(PasswordOption<'input>),
    User(UserMembersOption<'input>),
    // Single-keyword forms — soft keywords for the role-attribute names.
    #[tok(SUPERUSER)]
    Superuser,
    #[tok(NOSUPERUSER)]
    NoSuperuser,
    #[tok(CREATEDB)]
    CreateDb,
    #[tok(NOCREATEDB)]
    NoCreateDb,
    #[tok(CREATEROLE)]
    CreateRole,
    #[tok(NOCREATEROLE)]
    NoCreateRole,
    #[tok(INHERIT)]
    Inherit,
    #[tok(NOINHERIT)]
    NoInherit,
    #[tok(LOGIN)]
    Login,
    #[tok(NOLOGIN)]
    NoLogin,
    #[tok(REPLICATION)]
    Replication,
    #[tok(NOREPLICATION)]
    NoReplication,
    #[tok(BYPASSRLS)]
    BypassRls,
    #[tok(NOBYPASSRLS)]
    NoBypassRls,
}

/// The role target on an `ALTER ROLE` / `ALTER USER` statement — either
/// a specific role spec or `ALL` (the latter only legal with `SET`/`RESET`
/// actions, per gram.y's `AlterRoleSetStmt`).
///
/// Variant ordering: `ALL` (hard keyword) is keyword-disjoint from
/// `RoleSpec` (an `Ident` / non-reserved word), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterRoleTarget<'input> {
    #[tok(ALL)]
    All,
    Role(RoleSpec<'input>),
}

/// `IN DATABASE name` — Postgres' `opt_in_database` clause on
/// `AlterRoleSetStmt`. Scopes a SET/RESET to a particular database.
#[derive(recursa::Node, Debug, Clone)]
pub struct InDatabaseClause<'input> {
    #[tok(IN, DATABASE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `SET set_rest | VariableResetStmt` — Postgres' `SetResetClause`.
///
/// Reuses the top-level [`crate::ast::session::set_reset::SetStmt`] and
/// [`crate::ast::session::set_reset::ResetStmt`]; both start with their own
/// keyword (`SET` / `RESET`), so the two variants are keyword-disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetResetClause<'input> {
    Set(crate::ast::session::set_reset::SetStmt<'input>),
    Reset(crate::ast::session::set_reset::ResetStmt<'input>),
}

/// `[IN DATABASE name] SetResetClause` — the body of `AlterRoleSetStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterRoleSetReset<'input> {
    pub in_database: Option<InDatabaseClause<'input>>,
    pub clause: SetResetClause<'input>,
}

/// `WITH AlterOptRoleList` — Postgres' `opt_with AlterOptRoleList` form
/// when the explicit `WITH` keyword is present. The option list itself
/// may be empty (gram.y's `AlterOptRoleList` is right-recursive with an
/// `/* EMPTY */` base case).
#[derive(recursa::Node, Debug, Clone)]
#[tok(WITH, this)]
pub struct AlterRoleWithOptions<'input> {
    pub options: Vec<AlterRoleOption<'input>>,
}

/// One non-empty `AlterOptRoleList` (no leading `WITH`) — at least one
/// option. The peek of [`AlterRoleOption`] gates this variant so the
/// empty-list case never matches (it falls through to no action at all,
/// which gram.y also allows but the corpus never uses).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterRoleOptionsOnly<'input> {
    pub options: recursa::Vec1<AlterRoleOption<'input>>,
}

/// One action on `ALTER ROLE`/`ALTER USER` — covers Postgres'
/// `AlterRoleStmt` (`[WITH] AlterOptRoleList`), `AlterRoleSetStmt`
/// (`[IN DATABASE name] SetResetClause`), and the `RENAME TO` branch
/// from `RenameStmt`.
///
/// Variant ordering: keyword-distinct branches first (`Rename` on
/// `RENAME`, `SetReset` on `IN`/`SET`/`RESET`, `With` on `WITH`). The
/// catch-all `Options` is last; its peek is the union of all
/// [`AlterRoleOption`] first tokens (the soft attribute keywords and
/// `PASSWORD`/`CONNECTION`/`VALID`/`ENCRYPTED`/`UNENCRYPTED`/`USER`),
/// none of which collide with `RENAME`/`IN`/`SET`/`RESET`/`WITH`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterRoleAction<'input> {
    Rename(RenameTo<'input>),
    SetReset(AlterRoleSetReset<'input>),
    With(AlterRoleWithOptions<'input>),
    Options(AlterRoleOptionsOnly<'input>),
}

/// `ALTER GROUP role_spec { ADD | DROP } USER role_list` — Postgres'
/// `AlterGroupStmt` add/drop form.
///
/// Variant ordering: `ADD` and `DROP` are keyword-disjoint.
#[derive(recursa::Node, Debug, Clone)]
pub enum AddDrop {
    #[tok(ADD)]
    Add,
    #[tok(DROP)]
    Drop,
}

/// `{ ADD | DROP } USER role_list` — body of Postgres' `AlterGroupStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterGroupUsers<'input> {
    pub add_drop: AddDrop,
    #[tok(USER, this)]
    pub roles: RoleList<'input>,
}

/// One action on `ALTER GROUP role_spec action` — covers Postgres'
/// `AlterGroupStmt` (`add_drop USER role_list`) and the `RENAME TO`
/// branch from `RenameStmt`. Both have disjoint first tokens
/// (`ADD`/`DROP` vs `RENAME`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterGroupAction<'input> {
    Rename(RenameTo<'input>),
    AddDropUsers(AlterGroupUsers<'input>),
}

/// `ALTER GROUP role_spec action` — Postgres' `AlterGroupStmt`
/// (`add_drop USER role_list`) plus the `RENAME TO` branch from
/// `RenameStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterGroupStmt<'input> {
    #[tok(ALTER, GROUP, this)]
    pub name: RoleSpec<'input>,
    pub action: AlterGroupAction<'input>,
}

/// `ALTER ROLE { role_spec | ALL } action` — Postgres' `AlterRoleStmt`,
/// `AlterRoleSetStmt`, and the `RENAME TO` branch from `RenameStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterRoleStmt<'input> {
    #[tok(ALTER, ROLE, this)]
    pub target: AlterRoleTarget<'input>,
    pub action: AlterRoleAction<'input>,
}

/// Postgres' `auth_ident` — the authorization identifier on
/// `CREATE/ALTER/DROP USER MAPPING FOR ...`.
///
/// gram.y: `auth_ident: RoleSpec | USER` — either a role spec (plain
/// identifier; the reserved-word pseudo-roles `current_user`,
/// `current_role`, `session_user`, `public` are not modelled as keywords
/// in pg-sql's lexer and thus arrive here as plain `Ident`s) or the
/// literal `USER` keyword meaning the current user.
///
/// Variant ordering: `User` is the `USER` hard keyword, `Role` is a
/// plain `Ident` — keyword-disjoint, so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum UserMappingFor<'input> {
    #[tok(USER)]
    User,
    Role(RoleSpec<'input>),
}

/// `CREATE USER MAPPING [IF NOT EXISTS] FOR auth_ident SERVER name
/// [OPTIONS (...)]` — Postgres' `CreateUserMappingStmt`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(CREATE, USER, MAPPING, this)]
pub struct CreateUserMappingStmt<'input> {
    pub if_not_exists: Option<IfNotExists>,
    #[tok(FOR, this)]
    pub auth_ident: UserMappingFor<'input>,
    #[tok(SERVER, this)]
    pub server_name: crate::tokens::ColId<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// `ALTER USER MAPPING FOR auth_ident SERVER name OPTIONS (...)` —
/// Postgres' `AlterUserMappingStmt`. The `OPTIONS` clause is mandatory
/// in gram.y.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterUserMappingStmt<'input> {
    #[tok(ALTER, USER, MAPPING, FOR, this)]
    pub auth_ident: UserMappingFor<'input>,
    #[tok(SERVER, this)]
    pub server_name: crate::tokens::ColId<'input>,
    pub options: AlterGenericOptions<'input>,
}

/// `DROP USER MAPPING [IF EXISTS] FOR auth_ident SERVER name` —
/// Postgres' `DropUserMappingStmt`. No CASCADE/RESTRICT (gram.y
/// comment: "XXX you'd think this should have a CASCADE/RESTRICT
/// option, even if it's only pro forma; but the SQL standard doesn't
/// show one.").
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, USER, MAPPING, this)]
pub struct DropUserMappingStmt<'input> {
    pub if_exists: Option<IfExists>,
    #[tok(FOR, this)]
    pub auth_ident: UserMappingFor<'input>,
    #[tok(SERVER, this)]
    pub server_name: crate::tokens::ColId<'input>,
}

/// `ALTER USER { role_spec | ALL } action` — Postgres' `AlterRoleStmt`
/// / `AlterRoleSetStmt` (USER alias for ROLE) plus the `RENAME TO`
/// branch from `RenameStmt`.
///
/// The `ALTER USER MAPPING ...` form lives in its own top-level
/// [`AlterUserMappingStmt`] / `Statement::AlterUserMapping`; the
/// `Statement` enum dispatches on the three-keyword `ALTER USER
/// MAPPING` lead before the bare `ALTER USER` variant.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterUserStmt<'input> {
    #[tok(ALTER, USER, this)]
    pub target: AlterRoleTarget<'input>,
    pub action: AlterRoleAction<'input>,
}
