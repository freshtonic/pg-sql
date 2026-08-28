//! ROLE DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::foreign::AlterGenericOptions;
use crate::ast::ddl::table::CreateGenericOptions;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Optional `TEMP` or `TEMPORARY` modifier that can appear between `CREATE`
/// and the object keyword for temporary objects (sequences, tables, views,
/// etc.).
///
/// Variant ordering: `Temporary` (longer) before `Temp` so the longer keyword
/// wins longest-match disambiguation.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TempModifier {
    Temporary(TEMPORARY),
    Temp(TEMP),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DefArg<'input> {
    Default(DEFAULT),
    True(TRUE),
    False(FALSE),
    None(NONE),
    /// Reserved keyword used as a `def_arg` value. The corpus uses this
    /// for `ALTER SUBSCRIPTION ... SET (origin = any)`. Per gram.y
    /// `def_arg` accepts `reserved_keyword` directly; pg-sql only adds
    /// keywords actually exercised by the corpus.
    Any(ANY),
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
    /// `name(typename_list)` — function-style value with type arguments,
    /// used in CREATE AGGREGATE def-lists: `SFUNC = balkifnull(int8, int4)`.
    /// PG accepts this via gram.y `def_arg → func_type → Typename → GenericType
    /// → type_function_name opt_type_modifiers` where `opt_type_modifiers` is
    /// `'(' expr_list ')'`. pg-sql's `CastType` precision only accepts signed
    /// integers (`numeric(3,-6)`), so the function-style type-argument form
    /// is modelled as a dedicated variant. Declared BEFORE `Type` so the
    /// `name(...)` shape is tried first when the value starts with an ident.
    FuncWithArgs(DefArgFuncWithArgs<'input>),
    /// A type name with optional precision and array suffixes. Also covers
    /// the bare-ident case (`internallength = variable`, `PROVIDER = builtin`)
    /// — `CastType` resolves identifiers through `TypeName::Ident`.
    Type(CastType<'input>),
}

/// `name(typename_list)` — function-style def-arg value with type-name
/// arguments, used by CREATE AGGREGATE's `SFUNC = balkifnull(int8, int4)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefArgFuncWithArgs<'input> {
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    pub args: Surrounded<punct::LParen, Seq1<CastType<'input>, punct::Comma>, punct::RParen>,
}

/// `SETOF type` value in a `def_list` (PG: `func_type` form on `def_arg`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefArgSetof<'input> {
    pub setof: SETOF,
    pub type_name: CastType<'input>,
}

/// `value` separator on `def_elem` — Postgres uses `'='`.
///
/// One-variant enum so the AST has a typed node where the literal sits.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefElemValue<'input> {
    pub eq: punct::Eq,
    pub arg: DefArg<'input>,
}

/// One entry in a `def_list` — `name [= value]`.
///
/// The name is `AliasName` so any keyword or identifier is accepted (Postgres
/// `ColLabel` permits every keyword class). The value is optional: some
/// CREATE TYPE base-type forms use bare names like `passedbyvalue`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefElem<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<DefElemValue<'input>>,
}

/// A parenthesised `def_list`: `(name [= value], ...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DefList<'input> {
    pub items: Surrounded<punct::LParen, Seq1<DefElem<'input>, punct::Comma>, punct::RParen>,
}

/// Password value in `PASSWORD { sconst | NULL }` — Postgres'
/// `AlterOptRoleElem` PASSWORD branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PasswordValue<'input> {
    Null(NULL),
    /// Any string-constant form: plain, `E'…'`, `U&'…'`, `B'…'`, `X'…'`.
    String(CopySconst<'input>),
}

/// `[ENCRYPTED] PASSWORD value` — Postgres' role-attribute PASSWORD clause.
/// `ENCRYPTED` is a backward-compat noise word (passwords are always
/// encrypted today). The `UNENCRYPTED PASSWORD` form is also recognised by
/// PG's grammar but raises an immediate error, so we accept it and round-
/// trip it byte-faithfully.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PasswordOption<'input> {
    /// Optional `ENCRYPTED` or `UNENCRYPTED` modifier.
    pub modifier: Option<PasswordModifier>,
    pub password: crate::tokens::soft_keyword::PASSWORD,
    pub value: PasswordValue<'input>,
}

/// `ENCRYPTED | UNENCRYPTED` — the backward-compat password modifier.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PasswordModifier {
    Encrypted(crate::tokens::soft_keyword::ENCRYPTED),
    Unencrypted(crate::tokens::soft_keyword::UNENCRYPTED),
}

/// `CONNECTION LIMIT signed_iconst` — role-attribute connection limit.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ConnectionLimitOption<'input> {
    pub connection: crate::tokens::soft_keyword::CONNECTION,
    pub limit: LIMIT,
    pub value: SignedIconst<'input>,
}

/// `VALID UNTIL sconst` — role-attribute expiry.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ValidUntilOption<'input> {
    pub valid: VALID,
    pub until: crate::tokens::soft_keyword::UNTIL,
    pub value: CopySconst<'input>,
}

/// `IN { ROLE | GROUP } role_list` — role membership target list.
///
/// Variant ordering: keyword kind discriminates first; both are single-token.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum InRoleOrGroup {
    Role(ROLE),
    Group(GROUP),
}

/// `IN { ROLE | GROUP } role [, ...]` — Postgres' `CreateOptRoleElem`
/// `IN_P ROLE`/`IN_P GROUP_P` branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InRoleOption<'input> {
    pub r#in: IN,
    pub kind: InRoleOrGroup,
    pub roles: RoleList<'input>,
}

/// `SYSID iconst` — legacy noise option preserved for backward compat.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SysIdOption<'input> {
    pub sysid: crate::tokens::soft_keyword::SYSID,
    pub value: literal::IntegerLit<'input>,
}

/// `ADMIN role_list` — `CreateOptRoleElem` ADMIN branch (creates role with
/// admin members).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AdminOption<'input> {
    pub admin: crate::tokens::soft_keyword::ADMIN,
    pub roles: RoleList<'input>,
}

/// `ROLE role_list` — `CreateOptRoleElem` ROLE branch (creates role with
/// child members).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RoleMembersOption<'input> {
    pub role: ROLE,
    pub roles: RoleList<'input>,
}

/// `USER role_list` — legacy `CREATE GROUP name [WITH] USER u1, u2`
/// (supported but undocumented for ALTER GROUP); also matches
/// `AlterOptRoleElem`'s undocumented `USER role_list` branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UserMembersOption<'input> {
    pub user: USER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
    Superuser(crate::tokens::soft_keyword::SUPERUSER),
    NoSuperuser(crate::tokens::soft_keyword::NOSUPERUSER),
    CreateDb(crate::tokens::soft_keyword::CREATEDB),
    NoCreateDb(crate::tokens::soft_keyword::NOCREATEDB),
    CreateRole(crate::tokens::soft_keyword::CREATEROLE),
    NoCreateRole(crate::tokens::soft_keyword::NOCREATEROLE),
    Inherit(INHERIT),
    NoInherit(crate::tokens::soft_keyword::NOINHERIT),
    Login(crate::tokens::soft_keyword::LOGIN),
    NoLogin(crate::tokens::soft_keyword::NOLOGIN),
    Replication(crate::tokens::soft_keyword::REPLICATION),
    NoReplication(crate::tokens::soft_keyword::NOREPLICATION),
    BypassRls(crate::tokens::soft_keyword::BYPASSRLS),
    NoBypassRls(crate::tokens::soft_keyword::NOBYPASSRLS),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateGroupStmt<'input> {
    pub create: CREATE,
    pub group: GROUP,
    pub name: crate::tokens::NonReservedWord<'input>,
    pub with: Option<WITH>,
    pub options: Vec<CreateRoleOption<'input>>,
}

/// `DROP GROUP [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropGroupStmt<'input> {
    pub drop: DROP,
    pub group: GROUP,
    pub if_exists: Option<IfExists>,
    pub roles: RoleList<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateRoleStmt<'input> {
    pub create: CREATE,
    pub role: ROLE,
    pub name: crate::tokens::NonReservedWord<'input>,
    pub with: Option<WITH>,
    pub options: Vec<CreateRoleOption<'input>>,
}

/// `DROP ROLE [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropRoleStmt<'input> {
    pub drop: DROP,
    pub role: ROLE,
    pub if_exists: Option<IfExists>,
    pub roles: RoleList<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateUserStmt<'input> {
    pub create: CREATE,
    pub user: USER,
    pub name: crate::tokens::NonReservedWord<'input>,
    pub with: Option<WITH>,
    pub options: Vec<CreateRoleOption<'input>>,
}

/// `DROP USER [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropUserStmt<'input> {
    pub drop: DROP,
    pub user: USER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterRoleOption<'input> {
    // Multi-keyword forms first.
    ConnectionLimit(ConnectionLimitOption<'input>),
    ValidUntil(ValidUntilOption<'input>),
    Password(PasswordOption<'input>),
    User(UserMembersOption<'input>),
    // Single-keyword forms — soft keywords for the role-attribute names.
    Superuser(crate::tokens::soft_keyword::SUPERUSER),
    NoSuperuser(crate::tokens::soft_keyword::NOSUPERUSER),
    CreateDb(crate::tokens::soft_keyword::CREATEDB),
    NoCreateDb(crate::tokens::soft_keyword::NOCREATEDB),
    CreateRole(crate::tokens::soft_keyword::CREATEROLE),
    NoCreateRole(crate::tokens::soft_keyword::NOCREATEROLE),
    Inherit(INHERIT),
    NoInherit(crate::tokens::soft_keyword::NOINHERIT),
    Login(crate::tokens::soft_keyword::LOGIN),
    NoLogin(crate::tokens::soft_keyword::NOLOGIN),
    Replication(crate::tokens::soft_keyword::REPLICATION),
    NoReplication(crate::tokens::soft_keyword::NOREPLICATION),
    BypassRls(crate::tokens::soft_keyword::BYPASSRLS),
    NoBypassRls(crate::tokens::soft_keyword::NOBYPASSRLS),
}

/// The role target on an `ALTER ROLE` / `ALTER USER` statement — either
/// a specific role spec or `ALL` (the latter only legal with `SET`/`RESET`
/// actions, per gram.y's `AlterRoleSetStmt`).
///
/// Variant ordering: `ALL` (hard keyword) is keyword-disjoint from
/// `RoleSpec` (an `Ident` / non-reserved word), so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterRoleTarget<'input> {
    All(ALL),
    Role(RoleSpec<'input>),
}

/// `IN DATABASE name` — Postgres' `opt_in_database` clause on
/// `AlterRoleSetStmt`. Scopes a SET/RESET to a particular database.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InDatabaseClause<'input> {
    pub in_: IN,
    pub database: DATABASE,
    pub name: crate::tokens::ColId<'input>,
}

/// `SET set_rest | VariableResetStmt` — Postgres' `SetResetClause`.
///
/// Reuses the top-level [`crate::ast::session::set_reset::SetStmt`] and
/// [`crate::ast::session::set_reset::ResetStmt`]; both start with their own
/// keyword (`SET` / `RESET`), so the two variants are keyword-disjoint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetResetClause<'input> {
    Set(crate::ast::session::set_reset::SetStmt<'input>),
    Reset(crate::ast::session::set_reset::ResetStmt<'input>),
}

/// `[IN DATABASE name] SetResetClause` — the body of `AlterRoleSetStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterRoleSetReset<'input> {
    pub in_database: Option<InDatabaseClause<'input>>,
    pub clause: SetResetClause<'input>,
}

/// `WITH AlterOptRoleList` — Postgres' `opt_with AlterOptRoleList` form
/// when the explicit `WITH` keyword is present. The option list itself
/// may be empty (gram.y's `AlterOptRoleList` is right-recursive with an
/// `/* EMPTY */` base case).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterRoleWithOptions<'input> {
    pub with: WITH,
    pub options: Vec<AlterRoleOption<'input>>,
}

/// One non-empty `AlterOptRoleList` (no leading `WITH`) — at least one
/// option. The peek of [`AlterRoleOption`] gates this variant so the
/// empty-list case never matches (it falls through to no action at all,
/// which gram.y also allows but the corpus never uses).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterRoleOptionsOnly<'input> {
    pub options: Vec<AlterRoleOption<'input>>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AddDrop {
    Add(ADD),
    Drop(DROP),
}

/// `{ ADD | DROP } USER role_list` — body of Postgres' `AlterGroupStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterGroupUsers<'input> {
    pub add_drop: AddDrop,
    pub user: USER,
    pub roles: RoleList<'input>,
}

/// One action on `ALTER GROUP role_spec action` — covers Postgres'
/// `AlterGroupStmt` (`add_drop USER role_list`) and the `RENAME TO`
/// branch from `RenameStmt`. Both have disjoint first tokens
/// (`ADD`/`DROP` vs `RENAME`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterGroupAction<'input> {
    Rename(RenameTo<'input>),
    AddDropUsers(AlterGroupUsers<'input>),
}

/// `ALTER GROUP role_spec action` — Postgres' `AlterGroupStmt`
/// (`add_drop USER role_list`) plus the `RENAME TO` branch from
/// `RenameStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterGroupStmt<'input> {
    pub alter: ALTER,
    pub group: GROUP,
    pub name: RoleSpec<'input>,
    pub action: AlterGroupAction<'input>,
}

/// `ALTER ROLE { role_spec | ALL } action` — Postgres' `AlterRoleStmt`,
/// `AlterRoleSetStmt`, and the `RENAME TO` branch from `RenameStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterRoleStmt<'input> {
    pub alter: ALTER,
    pub role: ROLE,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum UserMappingFor<'input> {
    User(USER),
    Role(RoleSpec<'input>),
}

/// `CREATE USER MAPPING [IF NOT EXISTS] FOR auth_ident SERVER name
/// [OPTIONS (...)]` — Postgres' `CreateUserMappingStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateUserMappingStmt<'input> {
    pub create: CREATE,
    pub user: USER,
    pub mapping: MAPPING,
    pub if_not_exists: Option<IfNotExists>,
    pub for_: FOR,
    pub auth_ident: UserMappingFor<'input>,
    pub server: SERVER,
    pub server_name: crate::tokens::ColId<'input>,
    pub options: Option<CreateGenericOptions<'input>>,
}

/// `ALTER USER MAPPING FOR auth_ident SERVER name OPTIONS (...)` —
/// Postgres' `AlterUserMappingStmt`. The `OPTIONS` clause is mandatory
/// in gram.y.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterUserMappingStmt<'input> {
    pub alter: ALTER,
    pub user: USER,
    pub mapping: MAPPING,
    pub for_: FOR,
    pub auth_ident: UserMappingFor<'input>,
    pub server: SERVER,
    pub server_name: crate::tokens::ColId<'input>,
    pub options: AlterGenericOptions<'input>,
}

/// `DROP USER MAPPING [IF EXISTS] FOR auth_ident SERVER name` —
/// Postgres' `DropUserMappingStmt`. No CASCADE/RESTRICT (gram.y
/// comment: "XXX you'd think this should have a CASCADE/RESTRICT
/// option, even if it's only pro forma; but the SQL standard doesn't
/// show one.").
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropUserMappingStmt<'input> {
    pub drop: DROP,
    pub user: USER,
    pub mapping: MAPPING,
    pub if_exists: Option<IfExists>,
    pub for_: FOR,
    pub auth_ident: UserMappingFor<'input>,
    pub server: SERVER,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterUserStmt<'input> {
    pub alter: ALTER,
    pub user: USER,
    pub target: AlterRoleTarget<'input>,
    pub action: AlterRoleAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_group() {
        let mut input = crate::tokens::test_input("CREATE GROUP g1");
        let _stmt = CreateGroupStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_group_with_users() {
        let mut input = crate::tokens::test_input("CREATE GROUP g1 WITH USER u1, u2");
        let _stmt = CreateGroupStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_group_add_user() {
        let mut input = crate::tokens::test_input("ALTER GROUP g1 ADD USER u1");
        let _stmt = AlterGroupStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_alter_group_drop_user() {
        let mut input = crate::tokens::test_input("ALTER GROUP g1 DROP USER u1");
        let _stmt = AlterGroupStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_plain() {
        let mut input = crate::tokens::test_input("CREATE ROLE alice");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "alice");
        assert!(stmt.with.is_none());
        assert!(stmt.options.is_empty());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_with_attributes() {
        let mut input = crate::tokens::test_input(
            "CREATE ROLE alice WITH SUPERUSER CREATEDB CREATEROLE NOINHERIT \
             REPLICATION BYPASSRLS LOGIN",
        );
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 7);
        assert!(stmt.with.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_negated_attributes() {
        let mut input = crate::tokens::test_input(
            "CREATE ROLE alice NOSUPERUSER NOCREATEDB NOCREATEROLE NOLOGIN \
             NOREPLICATION NOBYPASSRLS",
        );
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 6);
        assert!(stmt.with.is_none());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_password() {
        let mut input = crate::tokens::test_input("CREATE ROLE alice PASSWORD 'secret'");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::Password(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_encrypted_password_null() {
        let mut input = crate::tokens::test_input("CREATE ROLE alice ENCRYPTED PASSWORD NULL");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_connection_limit() {
        let mut input = crate::tokens::test_input("CREATE ROLE alice CONNECTION LIMIT 5");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.options.len(), 1);
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::ConnectionLimit(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_valid_until() {
        let mut input = crate::tokens::test_input("CREATE ROLE alice VALID UNTIL '2030-01-01'");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::ValidUntil(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_in_role() {
        let mut input = crate::tokens::test_input("CREATE ROLE bob IN ROLE alice, charlie");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::InRole(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_admin() {
        let mut input = crate::tokens::test_input("CREATE ROLE bob ADMIN alice, charlie");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::Admin(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_role_sysid() {
        let mut input = crate::tokens::test_input("CREATE ROLE bob SYSID 12345");
        let stmt = CreateRoleStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::SysId(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_user_with_login() {
        let mut input = crate::tokens::test_input("CREATE USER alice WITH NOLOGIN");
        let stmt = CreateUserStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "alice");
        assert_eq!(stmt.options.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_group_role_members() {
        let mut input = crate::tokens::test_input("CREATE GROUP g1 ROLE alice, bob");
        let stmt = CreateGroupStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::Role(_),
        ));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_group_user_members() {
        let mut input = crate::tokens::test_input("CREATE GROUP g1 WITH USER u1, u2");
        let stmt = CreateGroupStmt::parse(&mut input).unwrap();
        assert!(matches!(
            stmt.options.first().unwrap(),
            CreateRoleOption::User(_),
        ));
        assert!(stmt.with.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_group() {
        let mut input = crate::tokens::test_input("DROP GROUP g1");
        let stmt = DropGroupStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.roles.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_role_if_exists_multi() {
        let mut input = crate::tokens::test_input("DROP ROLE IF EXISTS a, b, c");
        let stmt = DropRoleStmt::parse(&mut input).unwrap();
        assert!(stmt.if_exists.is_some());
        assert_eq!(stmt.roles.len(), 3);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_def_arg_custom_op() {
        // The smallest reproducer: a `def_arg` value that is a 3-char custom
        // operator. Used as the RHS of `COMMUTATOR =` etc.
        let mut input = crate::tokens::test_input("===");
        let _arg = DefArg::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_def_arg_at_eq() {
        let mut input = crate::tokens::test_input("@=");
        let _arg = DefArg::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_def_arg_bang_eq_eq() {
        let mut input = crate::tokens::test_input("!==");
        let _arg = DefArg::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_def_arg_signed_numeric_still_parses_as_numeric() {
        // Regression: `Numeric` and `QualOp` share `+`/`-` as a leading
        // token. `Numeric` is declared first so a signed integer must still
        // win — `+1` and `-2` are common `def_arg` values (`default = -1`,
        // `internallength = +24`) and must not silently demote to QualOp.
        let mut input = crate::tokens::test_input("+1");
        let arg = DefArg::parse(&mut input).unwrap();
        assert!(
            matches!(arg, DefArg::Numeric(_)),
            "expected Numeric for `+1`, got {arg:?}"
        );
        assert!(input.is_empty());

        let mut input = crate::tokens::test_input("-2");
        let arg = DefArg::parse(&mut input).unwrap();
        assert!(
            matches!(arg, DefArg::Numeric(_)),
            "expected Numeric for `-2`, got {arg:?}"
        );
        assert!(input.is_empty());
    }
}
