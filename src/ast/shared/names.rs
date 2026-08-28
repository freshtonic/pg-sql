/// Name-shaped AST primitives: qualified names, role names, type names,
/// operator names, and the rename/owner/schema action clauses that bundle them.
use recursa::seq::Seq1;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};

/// A comma-separated list of qualified (dotted) names — Postgres'
/// `any_name_list` / `name_list` in DROP-family statements.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NameList<'input> {
    pub names: Seq1<QualifiedName<'input>, punct::Comma>,
}

impl<'input> NameList<'input> {
    /// Number of names in the list.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the list is empty (always false — `Seq1` requires one entry).
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// A single role reference — Postgres' `RoleSpec`.
///
/// Only the `NonReservedWord` form is modelled: every role reference in the
/// differential corpus is a plain (possibly quoted) identifier. The reserved
/// pseudo-roles `CURRENT_ROLE` / `CURRENT_USER` / `SESSION_USER` are not yet
/// modelled — when a corpus statement needs one, add reserved-keyword tokens
/// and extend this enum to a tuple variant per form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RoleSpec<'input> {
    pub name: crate::tokens::NonReservedWord<'input>,
}

/// A comma-separated list of roles — Postgres' `role_list`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RoleList<'input> {
    pub roles: Seq1<RoleSpec<'input>, punct::Comma>,
}

impl<'input> RoleList<'input> {
    /// Number of roles in the list.
    pub fn len(&self) -> usize {
        self.roles.len()
    }

    /// Whether the list is empty (always false — `Seq1` requires one entry).
    pub fn is_empty(&self) -> bool {
        self.roles.is_empty()
    }
}

/// A type-name reference — Postgres' `Typename` as it appears in
/// `DROP TYPE` / `DROP DOMAIN` / `DROP CAST`.
///
/// The corpus only exercises simple (possibly qualified) type names and
/// keyword-spelled built-in types in these positions, so this delegates to
/// the expression-level `TypeName`. Array suffixes and `%TYPE` are not used
/// by any DROP corpus statement.
pub use crate::ast::shared::expr::TypeName;

/// A comma-separated list of type names — Postgres' `type_name_list`.
///
/// Items are `CastType` rather than bare `TypeName` so the array suffix
/// (`int[]`, `text[]`) survives. PG's `type_name_list` is built from
/// `Typename`, which includes the `[]`/`[N]` array suffix(es) — the bare
/// `TypeName` enum in pg-sql models only `SimpleTypename`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TypeNameList<'input> {
    pub types: Seq1<crate::ast::shared::expr::CastType<'input>, punct::Comma>,
}

/// The `(...)` argument signature on `DROP AGGREGATE name(...)`.
///
/// The corpus only exercises `(*)` (zero-argument aggregate) and a plain
/// comma-separated type list. The ordered-set `(... ORDER BY ...)` forms and
/// named/moded `aggr_arg`s are not used by any DROP corpus statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AggregateArgs<'input> {
    /// `(*)` — the zero-argument aggregate (spelled like `COUNT(*)`).
    Star(recursa::surrounded::Surrounded<punct::LParen, punct::Star, punct::RParen>),
    /// `(type, ...)` — explicit argument type list.
    Types(
        recursa::surrounded::Surrounded<
            punct::LParen,
            Seq1<TypeName<'input>, punct::Comma>,
            punct::RParen,
        >,
    ),
}

/// A dotted name: `name`, `schema.name`, or `catalog.schema.name`.
///
/// This is the usual shape for table/view/sequence/type references in SQL.
/// Must NOT collide with `Expr::QualRef` at the Pratt level because
/// `QualifiedName` is only used in non-expression positions (FROM targets,
/// DROP targets, ALTER targets, etc.).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct QualifiedName<'input> {
    pub parts: Seq1<literal::Ident<'input>, punct::Dot>,
}

impl<'input> QualifiedName<'input> {
    /// Returns the final (object) name part.
    pub fn object(&self) -> &str {
        self.parts.last().text()
    }
}

/// Function definition name (CREATE FUNCTION / DROP FUNCTION / DROP ROUTINE).
///
/// PG's `func_name: type_function_name | ColId indirection` admits
/// `unreserved_keyword`s like `set` as legal function names. pg-sql keeps
/// `SET` reserved at the token level (to disambiguate `UPDATE … SET …`
/// from an UPDATE table-alias) but reclaims it explicitly here so PG's
/// `CREATE FUNCTION set(...) ...` and `DROP FUNCTION set(name)` corpus
/// statements parse structurally.
///
/// Variant ordering: keyword variants first so their `SET(`/`SET ` form
/// is matched before the generic `Name(QualifiedName)` fallback.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncDefName<'input> {
    Set(SET),
    Name(QualifiedName<'input>),
}

impl<'input> FuncDefName<'input> {
    /// Returns the final (object) name part as text.
    pub fn object(&self) -> &str {
        match self {
            FuncDefName::Set(_) => "set",
            FuncDefName::Name(q) => q.object(),
        }
    }
}

/// `RENAME TO new_name` — the rename action shared by many ALTER
/// statements. Postgres routes most of these through `RenameStmt`, but
/// pg-sql's dispatcher commits on the leading `ALTER objtype ...`
/// keywords, so each `Alter*Stmt` re-models its own rename branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RenameTo<'input> {
    pub rename: RENAME,
    pub to: TO,
    pub new_name: literal::Ident<'input>,
}

/// `OWNER TO RoleSpec` — the owner-change action shared by many ALTER
/// statements. Postgres routes most of these through `AlterOwnerStmt`,
/// but pg-sql's dispatcher commits on the leading `ALTER objtype ...`
/// keywords, so each `Alter*Stmt` re-models its own owner branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OwnerTo<'input> {
    pub owner: OWNER,
    pub to: TO,
    pub new_owner: RoleSpec<'input>,
}

/// `SET SCHEMA name` — the set-schema action shared by ALTER FOREIGN
/// TABLE, ALTER TABLE, ALTER VIEW, ALTER MATERIALIZED VIEW, etc.
/// Postgres routes most of these through `AlterObjectSchemaStmt`, but
/// pg-sql's dispatcher commits on the leading `ALTER objtype ...`
/// keywords, so each `Alter*Stmt` re-models its own set-schema branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetSchemaClause<'input> {
    pub set: SET,
    pub schema: SCHEMA,
    pub new_schema: literal::Ident<'input>,
}

/// A single (unqualified) operator name — Postgres' `all_Op` rule
/// (`Op | MathOp`).
///
/// `all_Op` is a lexer class in PG that absorbs any operator-character
/// sequence, plus the single-char `MathOp`s (`+ - * / % ^ < > =`) and the
/// 2-char comparisons `<= >= <>`. In recursa's logos token model every
/// distinct multi-char operator gets its own punct token (`Lte`, `Gte`,
/// `Neq`, `TripleEq`, `BangEqEq`, `BangEqMinus`, `LtLtLt`, …), so this enum
/// must enumerate every punct token whose spelling is made of operator
/// characters (`+ - * / % ^ < > = ~ ! @ # & | ?`). Anything else falls into
/// the multi-char catch-all `CustomOp`.
///
/// Variant ordering: peek regexes are exact per-variant (each variant maps
/// to exactly one token kind), so disambiguation is unambiguous regardless
/// of order. Variants are grouped by leading char for readability.
///
/// `FatArrow` (`=>`) is deliberately omitted: PG explicitly rejects `=>` as
/// an operator name, and excluding it lets the few corpus `CREATE OPERATOR
/// =>` lines surface as [`crate::ast::FileItem::ParseError`], matching
/// PG's rejection on both sides of the differential oracle.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OperatorName<'input> {
    // Multi-char tokens whose spelling is purely operator chars. Each is
    // a single logos token kind so their peek regexes are disjoint.
    StarLte(punct::StarLte),
    StarGte(punct::StarGte),
    StarNeq(punct::StarNeq),
    StarLt(punct::StarLt),
    StarGt(punct::StarGt),
    StarEq(punct::StarEq),
    TripleEq(punct::TripleEq),
    BangEqEq(punct::BangEqEq),
    BangEqMinus(punct::BangEqMinus),
    BangEq(punct::BangEq),
    LtLtLt(punct::LtLtLt),
    LtLtEq(punct::LtLtEq),
    LtLtPipe(punct::LtLtPipe),
    LtMinusGt(punct::LtMinusGt),
    LtLt(punct::LtLt),
    LtCaret(punct::LtCaret),
    LtAt(punct::LtAt),
    GtGtGt(punct::GtGtGt),
    GtGtEq(punct::GtGtEq),
    GtGt(punct::GtGt),
    GtCaret(punct::GtCaret),
    HashArrowArrow(punct::HashArrowArrow),
    HashArrow(punct::HashArrow),
    HashHash(punct::HashHash),
    HashMinus(punct::HashMinus),
    ArrowArrow(punct::ArrowArrow),
    Arrow(punct::Arrow),
    MinusPipeMinus(punct::MinusPipeMinus),
    PipeGtGt(punct::PipeGtGt),
    PipeAmpGt(punct::PipeAmpGt),
    PipePipeSlash(punct::PipePipeSlash),
    Concat(punct::Concat),
    PipeSlash(punct::PipeSlash),
    QuestionPipePipe(punct::QuestionPipePipe),
    QuestionDashPipe(punct::QuestionDashPipe),
    QuestionPipe(punct::QuestionPipe),
    QuestionAmp(punct::QuestionAmp),
    QuestionHash(punct::QuestionHash),
    QuestionDash(punct::QuestionDash),
    AtAtAt(punct::AtAtAt),
    AtMinusAt(punct::AtMinusAt),
    AtHashAt(punct::AtHashAt),
    AtPlusAt(punct::AtPlusAt),
    AtAt(punct::AtAt),
    AtQuestion(punct::AtQuestion),
    AtGt(punct::AtGt),
    AmpLtPipe(punct::AmpLtPipe),
    AmpAmp(punct::AmpAmp),
    AmpLt(punct::AmpLt),
    AmpGt(punct::AmpGt),
    TildeLeqTilde(punct::TildeLeqTilde),
    TildeGeqTilde(punct::TildeGeqTilde),
    TildeLtTilde(punct::TildeLtTilde),
    TildeGtTilde(punct::TildeGtTilde),
    BangTildeTildeStar(punct::BangTildeTildeStar),
    TildeTildeStar(punct::TildeTildeStar),
    BangTildeTilde(punct::BangTildeTilde),
    TildeTilde(punct::TildeTilde),
    BangTildeStar(punct::BangTildeStar),
    TildeStar(punct::TildeStar),
    BangTilde(punct::BangTilde),
    TildeEq(punct::TildeEq),
    CaretAt(punct::CaretAt),
    // Single-char punct tokens (the `MathOp` set plus the bare operator
    // characters PG treats as operator chars).
    Lte(punct::Lte),
    Gte(punct::Gte),
    Neq(punct::Neq),
    Plus(punct::Plus),
    Minus(punct::Minus),
    Star(punct::Star),
    Slash(punct::Slash),
    Percent(punct::Percent),
    Caret(punct::Caret),
    Lt(punct::Lt),
    Gt(punct::Gt),
    Eq(punct::Eq),
    Tilde(punct::Tilde),
    At(punct::At),
    Pound(punct::Pound),
    Amp(punct::Amp),
    Pipe(punct::Pipe),
    Question(punct::Question),
    // Multi-char catch-all. Listed last because each of the specific punct
    // tokens above wins at the lexer level (logos longest-match-wins with
    // declaration order tiebreaker); only operator names that don't match
    // any specific token end up as `CustomOp`.
    Custom(literal::CustomOp<'input>),
}

/// A possibly schema-qualified operator name — Postgres' `any_operator`.
///
/// Postgres allows arbitrary prefixes of `ColId.` parts (e.g., `pg_catalog.+`,
/// `schema_op1.#*#`). Modelled as an enum so the peek set covers both the
/// `Ident.` qualified path and every bare-operator first-token from
/// [`OperatorName`].
///
/// Variant ordering: `Qualified` starts with `Ident`, `Plain` starts with a
/// punct/operator token. Their first sets are disjoint, so order is for
/// clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum QualifiedOperatorName<'input> {
    /// `[schema.]op` — at least one `Ident.` segment followed by an
    /// `OperatorName`.
    Qualified(QualifiedOperatorPath<'input>),
    /// Bare operator name with no schema qualifier.
    Plain(OperatorName<'input>),
}

/// A schema-qualified operator name: one or more `Ident.` segments followed
/// by an `OperatorName`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct QualifiedOperatorPath<'input> {
    pub first: QualifiedOperatorPrefix<'input>,
    pub rest: Vec<QualifiedOperatorPrefix<'input>>,
    pub name: OperatorName<'input>,
}

/// One `Ident.` segment of a qualified operator name's schema prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct QualifiedOperatorPrefix<'input> {
    pub name: literal::Ident<'input>,
    pub dot: punct::Dot,
}

/// `(left, right)` argument-type signature on `operator_with_argtypes` —
/// Postgres' `oper_argtypes`.
///
/// Postgres' grammar accepts four shapes, three of which (`(NONE,
/// Typename)` — left unary; `(Typename, NONE)` — right unary; and `(Typename,
/// Typename)` — binary) are still valid. The fourth (`(Typename)`) raises an
/// immediate parse error in PG ("missing argument"), so we don't model it —
/// any input of that shape is PG-rejected and surfaces as
/// [`crate::ast::FileItem::ParseError`].
///
/// Variant ordering: `Binary`'s second slot is a `TypeName`, while
/// `LeftUnary`'s and `RightUnary`'s second slots include `NONE`. The peek
/// regex for each variant covers two-token prefixes (`( ident` vs `( NONE`
/// etc.), so the variants are distinguishable up to whether the SECOND
/// slot is `NONE`. We list `LeftUnary` (begins with `NONE`) before the
/// binary forms so its leading `NONE` is unambiguous; the binary case must
/// then commit on a `Typename` in the first slot.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum OperatorArgtypes<'input> {
    /// `(NONE, Typename)` — left-unary (prefix) operator signature.
    LeftUnary(Surrounded<punct::LParen, OperatorArgtypesLeftUnary<'input>, punct::RParen>),
    /// `(Typename, NONE)` — right-unary (postfix) operator. PostgreSQL no
    /// longer supports postfix operators at runtime, but the grammar still
    /// accepts the spelling; we round-trip it.
    RightUnary(Surrounded<punct::LParen, OperatorArgtypesRightUnary<'input>, punct::RParen>),
    /// `(Typename, Typename)` — binary operator signature.
    Binary(Surrounded<punct::LParen, OperatorArgtypesBinary<'input>, punct::RParen>),
}

/// Inner content of `(NONE, Typename)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OperatorArgtypesLeftUnary<'input> {
    pub left: NONE,
    pub comma: punct::Comma,
    pub right: TypeName<'input>,
}

/// Inner content of `(Typename, NONE)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OperatorArgtypesRightUnary<'input> {
    pub left: TypeName<'input>,
    pub comma: punct::Comma,
    pub right: NONE,
}

/// Inner content of `(Typename, Typename)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OperatorArgtypesBinary<'input> {
    pub left: TypeName<'input>,
    pub comma: punct::Comma,
    pub right: TypeName<'input>,
}

/// `any_operator oper_argtypes` — Postgres' `operator_with_argtypes`. The
/// full reference to a specific operator (including overload signature)
/// used by `DROP OPERATOR`, `ALTER OPERATOR`, `COMMENT ON OPERATOR`,
/// `SECURITY LABEL ON OPERATOR`, etc.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OperatorWithArgtypes<'input> {
    pub name: QualifiedOperatorName<'input>,
    pub args: OperatorArgtypes<'input>,
}
