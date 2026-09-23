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

recursa::ast_node! {
    /// Optional `TEMP` or `TEMPORARY` modifier that can appear between `CREATE`
    /// and the object keyword for temporary objects (sequences, tables, views,
    /// etc.).
    ///
    /// Variant ordering: `Temporary` (longer) before `Temp` so the longer keyword
    /// wins longest-match disambiguation.
    #[derive(Debug)]
    pub enum TempModifier {
        #[tok(TEMPORARY)]
        Temporary,
        #[tok(TEMP)]
        Temp,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum DefArg {
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
        String(CopySconst),
        Numeric(NumericOnly),
        /// A bare operator name on the RHS of a `def_elem`. Postgres' `def_arg`
        /// accepts `qual_all_Op`; the corpus exercises this on CREATE/ALTER
        /// OPERATOR's `commutator`/`negator` attributes.
        QualOp(crate::ast::shared::names::OperatorName),
        /// `SETOF type` — used in CREATE OPERATOR `leftarg`/`rightarg`
        /// (PG gram.y `def_arg: func_type`, and `func_type: Typename | type_function_name … | SETOF SimpleTypename …`).
        /// Listed before `Type` so the SETOF keyword reliably anchors this variant.
        Setof(DefArgSetof),
        /// A built-in type name with ordinary cast-type suffixes.
        BuiltinType(FunctionBuiltinType),
        /// An identifier-spelled type or `name(typename_list)` function-style
        /// value. The qualified name is parsed once before deciding whether a
        /// parenthesized argument is a typmod list or a type-name list.
        NamedType(DefArgNamedType),
    }
}

recursa::ast_node! {
    /// Parenthesized values following an identifier-spelled def-arg name.
    /// Integer-led values are type modifiers; type-led values are aggregate
    /// support-function argument types.
    #[derive(Debug)]
    pub enum DefArgNamedParameterValues {
        TypeModifiers(#[sep(COMMA)] one_or_many!(TypeModifierArg)),
        FuncArgs(#[sep(COMMA)] one_or_many!(CastType)),
    }
}

recursa::ast_node! {
    /// A factored parenthesized def-arg suffix. Factoring the delimiters lets the
    /// LR state distinguish the first value inside them (`10` versus `int8`).
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct DefArgNamedParameters {
        pub values: DefArgNamedParameterValues,
    }
}

recursa::ast_node! {
    /// Identifier-spelled def-arg type or function-style value.
    #[derive(Debug)]
    pub struct DefArgNamedType {
        pub name: FunctionTypeName,
        #[presence(PRECISION)]
        pub precision_keyword: bool,
        #[presence(VARYING)]
        pub varying: bool,
        pub parameters: Option<DefArgNamedParameters>,
        pub tz: Option<TimeZoneQualifier>,
        pub interval_qualifier: Option<IntervalQualifier>,
        pub array_suffixes: zero_or_many!(ArraySuffix),
        pub array_kw_suffix: Option<ArrayKwSuffix>,
    }
}

recursa::ast_node! {
    /// `SETOF type` value in a `def_list` (PG: `func_type` form on `def_arg`).
    #[derive(Debug)]
    pub struct DefArgSetof {
        #[tok(SETOF, this)]
        pub type_name: CastType,
    }
}

recursa::ast_node! {
    /// `value` separator on `def_elem` — Postgres uses `'='`.
    ///
    /// One-variant enum so the AST has a typed node where the literal sits.
    #[derive(Debug)]
    pub struct DefElemValue {
        #[tok(EQ, this)]
        pub arg: DefArg,
    }
}

recursa::ast_node! {
    /// One entry in a `def_list` — `name [= value]`.
    ///
    /// The name is `AliasName` so any keyword or identifier is accepted (Postgres
    /// `ColLabel` permits every keyword class). The value is optional: some
    /// CREATE TYPE base-type forms use bare names like `passedbyvalue`.
    #[derive(Debug)]
    pub struct DefElem {
        pub name: literal::AliasName,
        pub value: Option<DefElemValue>,
    }
}

recursa::ast_node! {
    /// A parenthesised `def_list`: `(name [= value], ...)`.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct DefList {
        #[sep(COMMA)]
        pub items: one_or_many!(DefElem),
    }
}

recursa::ast_node! {
    /// Password value in `PASSWORD { sconst | NULL }` — Postgres'
    /// `AlterOptRoleElem` PASSWORD branch.
    #[derive(Debug)]
    pub enum PasswordValue {
        #[tok(NULL)]
        Null,
        /// Any string-constant form: plain, `E'…'`, `U&'…'`, `B'…'`, `X'…'`.
        String(CopySconst),
    }
}

recursa::ast_node! {
    /// `[ENCRYPTED] PASSWORD value` — Postgres' role-attribute PASSWORD clause.
    /// `ENCRYPTED` is a backward-compat noise word (passwords are always
    /// encrypted today). The `UNENCRYPTED PASSWORD` form is also recognised by
    /// PG's grammar but raises an immediate error, so we accept it and round-
    /// trip it byte-faithfully.
    #[derive(Debug)]
    pub struct PasswordOption {
        /// Optional `ENCRYPTED` or `UNENCRYPTED` modifier.
        pub modifier: Option<PasswordModifier>,
        #[tok(PASSWORD, this)]
        pub value: PasswordValue,
    }
}

recursa::ast_node! {
    /// `ENCRYPTED | UNENCRYPTED` — the backward-compat password modifier.
    #[derive(Debug)]
    pub enum PasswordModifier {
        #[tok(ENCRYPTED)]
        Encrypted,
        #[tok(UNENCRYPTED)]
        Unencrypted,
    }
}

recursa::ast_node! {
    /// `CONNECTION LIMIT signed_iconst` — role-attribute connection limit.
    #[derive(Debug)]
    pub struct ConnectionLimitOption {
        #[tok(CONNECTION, LIMIT, this)]
        pub value: SignedIconst,
    }
}

recursa::ast_node! {
    /// `VALID UNTIL sconst` — role-attribute expiry.
    #[derive(Debug)]
    pub struct ValidUntilOption {
        #[tok(VALID, UNTIL, this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `IN { ROLE | GROUP } role_list` — role membership target list.
    ///
    /// Variant ordering: keyword kind discriminates first; both are single-token.
    #[derive(Debug)]
    pub enum InRoleOrGroup {
        #[tok(ROLE)]
        Role,
        #[tok(GROUP)]
        Group,
    }
}

recursa::ast_node! {
    /// `IN { ROLE | GROUP } role [, ...]` — Postgres' `CreateOptRoleElem`
    /// `IN_P ROLE`/`IN_P GROUP_P` branch.
    #[derive(Debug)]
    pub struct InRoleOption {
        #[tok(IN, this)]
        pub kind: InRoleOrGroup,
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// `SYSID iconst` — legacy noise option preserved for backward compat.
    #[derive(Debug)]
    pub struct SysIdOption {
        #[tok(SYSID, this)]
        pub value: literal::IntegerLit,
    }
}

recursa::ast_node! {
    /// `ADMIN role_list` — `CreateOptRoleElem` ADMIN branch (creates role with
    /// admin members).
    #[derive(Debug)]
    pub struct AdminOption {
        #[tok(ADMIN, this)]
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// `ROLE role_list` — `CreateOptRoleElem` ROLE branch (creates role with
    /// child members).
    #[derive(Debug)]
    pub struct RoleMembersOption {
        #[tok(ROLE, this)]
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// `USER role_list` — legacy `CREATE GROUP name [WITH] USER u1, u2`
    /// (supported but undocumented for ALTER GROUP); also matches
    /// `AlterOptRoleElem`'s undocumented `USER role_list` branch.
    #[derive(Debug)]
    pub struct UserMembersOption {
        #[tok(USER, this)]
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// A single CREATE ROLE / CREATE USER / CREATE GROUP option — Postgres'
    /// `CreateOptRoleElem`. Options are unordered and repeatable.
    ///
    /// Variant ordering: multi-keyword forms (`IN ROLE`, `CONNECTION LIMIT`,
    /// `VALID UNTIL`, `ENCRYPTED PASSWORD`, `UNENCRYPTED PASSWORD`) before any
    /// single-keyword form they share a first-token with, so longest-match-wins
    /// disambiguates. `ROLE role_list` and `IN ROLE …` both start with `ROLE`/
    /// `IN` so the longer `IN ROLE` form wins on its leading `IN`.
    #[derive(Debug)]
    pub enum CreateRoleOption {
        // Multi-keyword forms first.
        InRole(InRoleOption),
        ConnectionLimit(ConnectionLimitOption),
        ValidUntil(ValidUntilOption),
        Password(PasswordOption),
        SysId(SysIdOption),
        Admin(AdminOption),
        Role(RoleMembersOption),
        User(UserMembersOption),
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
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CreateGroupStmt {
        #[tok(CREATE, GROUP, this)]
        pub name: crate::tokens::NonReservedWord,
        #[tok(optional(WITH), this)]
        pub options: zero_or_many!(CreateRoleOption),
    }
}

recursa::ast_node! {
    /// `DROP GROUP [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
    #[derive(Debug)]
    #[tok(DROP, GROUP, this)]
    pub struct DropGroupStmt {
        pub if_exists: Option<IfExists>,
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CreateRoleStmt {
        #[tok(CREATE, ROLE, this)]
        pub name: crate::tokens::NonReservedWord,
        #[tok(optional(WITH), this)]
        pub options: zero_or_many!(CreateRoleOption),
    }
}

recursa::ast_node! {
    /// `DROP ROLE [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
    #[derive(Debug)]
    #[tok(DROP, ROLE, this)]
    pub struct DropRoleStmt {
        pub if_exists: Option<IfExists>,
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CreateUserStmt {
        #[tok(CREATE, USER, this)]
        pub name: crate::tokens::NonReservedWord,
        #[tok(optional(WITH), this)]
        pub options: zero_or_many!(CreateRoleOption),
    }
}

recursa::ast_node! {
    /// `DROP USER [IF EXISTS] role [, ...]` — no `CASCADE`/`RESTRICT`.
    #[derive(Debug)]
    #[tok(DROP, USER, this)]
    pub struct DropUserStmt {
        pub if_exists: Option<IfExists>,
        pub roles: RoleList,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterRoleOption {
        // Multi-keyword forms first.
        ConnectionLimit(ConnectionLimitOption),
        ValidUntil(ValidUntilOption),
        Password(PasswordOption),
        User(UserMembersOption),
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
}

recursa::ast_node! {
    /// The role target on an `ALTER ROLE` / `ALTER USER` statement — either
    /// a specific role spec or `ALL` (the latter only legal with `SET`/`RESET`
    /// actions, per gram.y's `AlterRoleSetStmt`).
    ///
    /// Variant ordering: `ALL` (hard keyword) is keyword-disjoint from
    /// `RoleSpec` (an `Ident` / non-reserved word), so order is for clarity.
    #[derive(Debug)]
    pub enum AlterRoleTarget {
        #[tok(ALL)]
        All,
        Role(RoleSpec),
    }
}

recursa::ast_node! {
    /// `IN DATABASE name` — Postgres' `opt_in_database` clause on
    /// `AlterRoleSetStmt`. Scopes a SET/RESET to a particular database.
    #[derive(Debug)]
    pub struct InDatabaseClause {
        #[tok(IN, DATABASE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `SET set_rest | VariableResetStmt` — Postgres' `SetResetClause`.
    ///
    /// The `SET` half is the whole of `set_rest`, so every special form of a
    /// top-level unscoped `SET` is legal here: `ALTER ROLE r SET TIME ZONE
    /// 'UTC'` and `ALTER DATABASE d SET TRANSACTION ISOLATION LEVEL
    /// SERIALIZABLE` are accepted. `LOCAL` and `SESSION` are not, because
    /// `SetResetClause` names the literal `SET` itself.
    ///
    /// Reuses [`crate::ast::session::set_reset::SetUnscoped`] (`SET set_rest`)
    /// and [`crate::ast::session::set_reset::ResetStmt`]
    /// (`VariableResetStmt`); both start with their own keyword, so the two
    /// variants are keyword-disjoint.
    #[derive(Debug)]
    pub enum SetResetClause {
        Set(crate::ast::session::set_reset::SetUnscoped),
        Reset(crate::ast::session::set_reset::ResetStmt),
    }
}

recursa::ast_node! {
    /// `[IN DATABASE name] SetResetClause` — the body of `AlterRoleSetStmt`.
    #[derive(Debug)]
    pub struct AlterRoleSetReset {
        pub in_database: Option<InDatabaseClause>,
        pub clause: SetResetClause,
    }
}

recursa::ast_node! {
    /// `WITH AlterOptRoleList` — Postgres' `opt_with AlterOptRoleList` form
    /// when the explicit `WITH` keyword is present. The option list itself
    /// may be empty (gram.y's `AlterOptRoleList` is right-recursive with an
    /// `/* EMPTY */` base case).
    #[derive(Debug)]
    #[tok(WITH, this)]
    pub struct AlterRoleWithOptions {
        pub options: zero_or_many!(AlterRoleOption),
    }
}

recursa::ast_node! {
    /// One non-empty `AlterOptRoleList` (no leading `WITH`) — at least one
    /// option. The peek of [`AlterRoleOption`] gates this variant so the
    /// empty-list case never matches (it falls through to no action at all,
    /// which gram.y also allows but the corpus never uses).
    #[derive(Debug)]
    pub struct AlterRoleOptionsOnly {
        pub options: one_or_many!(AlterRoleOption),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterRoleAction {
        Rename(RenameTo),
        SetReset(AlterRoleSetReset),
        With(AlterRoleWithOptions),
        Options(AlterRoleOptionsOnly),
    }
}

recursa::ast_node! {
    /// `ALTER GROUP role_spec { ADD | DROP } USER role_list` — Postgres'
    /// `AlterGroupStmt` add/drop form.
    ///
    /// Variant ordering: `ADD` and `DROP` are keyword-disjoint.
    #[derive(Debug)]
    pub enum AddDrop {
        #[tok(ADD)]
        Add,
        #[tok(DROP)]
        Drop,
    }
}

recursa::ast_node! {
    /// `{ ADD | DROP } USER role_list` — body of Postgres' `AlterGroupStmt`.
    #[derive(Debug)]
    pub struct AlterGroupUsers {
        pub add_drop: AddDrop,
        #[tok(USER, this)]
        pub roles: RoleList,
    }
}

recursa::ast_node! {
    /// One action on `ALTER GROUP role_spec action` — covers Postgres'
    /// `AlterGroupStmt` (`add_drop USER role_list`) and the `RENAME TO`
    /// branch from `RenameStmt`. Both have disjoint first tokens
    /// (`ADD`/`DROP` vs `RENAME`).
    #[derive(Debug)]
    pub enum AlterGroupAction {
        Rename(RenameTo),
        AddDropUsers(AlterGroupUsers),
    }
}

recursa::ast_node! {
    /// `ALTER GROUP role_spec action` — Postgres' `AlterGroupStmt`
    /// (`add_drop USER role_list`) plus the `RENAME TO` branch from
    /// `RenameStmt`.
    #[derive(Debug)]
    pub struct AlterGroupStmt {
        #[tok(ALTER, GROUP, this)]
        pub name: RoleSpec,
        pub action: AlterGroupAction,
    }
}

recursa::ast_node! {
    /// `ALTER ROLE { role_spec | ALL } action` — Postgres' `AlterRoleStmt`,
    /// `AlterRoleSetStmt`, and the `RENAME TO` branch from `RenameStmt`.
    #[derive(Debug)]
    pub struct AlterRoleStmt {
        #[tok(ALTER, ROLE, this)]
        pub target: AlterRoleTarget,
        pub action: AlterRoleAction,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum UserMappingFor {
        #[tok(USER)]
        User,
        Role(RoleSpec),
    }
}

recursa::ast_node! {
    /// `CREATE USER MAPPING [IF NOT EXISTS] FOR auth_ident SERVER name
    /// [OPTIONS (...)]` — Postgres' `CreateUserMappingStmt`.
    #[derive(Debug)]
    #[tok(CREATE, USER, MAPPING, this)]
    pub struct CreateUserMappingStmt {
        pub if_not_exists: Option<IfNotExists>,
        #[tok(FOR, this)]
        pub auth_ident: UserMappingFor,
        #[tok(SERVER, this)]
        pub server_name: crate::tokens::ColId,
        pub options: Option<CreateGenericOptions>,
    }
}

recursa::ast_node! {
    /// `ALTER USER MAPPING FOR auth_ident SERVER name OPTIONS (...)` —
    /// Postgres' `AlterUserMappingStmt`. The `OPTIONS` clause is mandatory
    /// in gram.y.
    #[derive(Debug)]
    pub struct AlterUserMappingStmt {
        #[tok(ALTER, USER, MAPPING, FOR, this)]
        pub auth_ident: UserMappingFor,
        #[tok(SERVER, this)]
        pub server_name: crate::tokens::ColId,
        pub options: AlterGenericOptions,
    }
}

recursa::ast_node! {
    /// `DROP USER MAPPING [IF EXISTS] FOR auth_ident SERVER name` —
    /// Postgres' `DropUserMappingStmt`. No CASCADE/RESTRICT (gram.y
    /// comment: "XXX you'd think this should have a CASCADE/RESTRICT
    /// option, even if it's only pro forma; but the SQL standard doesn't
    /// show one.").
    #[derive(Debug)]
    #[tok(DROP, USER, MAPPING, this)]
    pub struct DropUserMappingStmt {
        pub if_exists: Option<IfExists>,
        #[tok(FOR, this)]
        pub auth_ident: UserMappingFor,
        #[tok(SERVER, this)]
        pub server_name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `ALTER USER { role_spec | ALL } action` — Postgres' `AlterRoleStmt`
    /// / `AlterRoleSetStmt` (USER alias for ROLE) plus the `RENAME TO`
    /// branch from `RenameStmt`.
    ///
    /// The `ALTER USER MAPPING ...` form lives in its own top-level
    /// [`AlterUserMappingStmt`] / `Statement::AlterUserMapping`; the
    /// `Statement` enum dispatches on the three-keyword `ALTER USER
    /// MAPPING` lead before the bare `ALTER USER` variant.
    #[derive(Debug)]
    pub struct AlterUserStmt {
        #[tok(ALTER, USER, this)]
        pub target: AlterRoleTarget,
        pub action: AlterRoleAction,
    }
}
