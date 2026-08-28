/// CREATE FUNCTION / DROP FUNCTION statement AST.
use recursa::seq::{OptionalTrailing, Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::shared::expr::{CastType, Expr, TypeName};
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;
// ---------------------------------------------------------------------------
// Additional imports for the ALTER/DROP types appended to this file as part
// of the DDL physical-extraction migration. Glob imports keep cross-batch
// type references resolvable regardless of migration order; a polish pass
// will tighten these once the migration completes.
use crate::ast::ddl::trigger::DependsOnExtension;
#[allow(unused_imports)]
use crate::ast::shared::expr::*;
#[allow(unused_imports)]
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
#[allow(unused_imports)]
use crate::tokens::soft_keyword::*;
// ---------------------------------------------------------------------------

/// SETOF type: `SETOF typename`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetofReturn<'input> {
    pub setof: SETOF,
    pub type_name: TypeName<'input>,
}

/// Function return type: `SETOF type` or plain `type`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ReturnType<'input> {
    Setof(SetofReturn<'input>),
    Plain(TypeName<'input>),
}

/// LANGUAGE clause: `LANGUAGE name` or `LANGUAGE 'name'`. Postgres accepts
/// the language name as an identifier or as a single-quoted string literal.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LanguageName<'input> {
    Ident(literal::AliasName<'input>),
    String(literal::StringLit<'input>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LanguageOption<'input> {
    pub language: LANGUAGE,
    pub name: LanguageName<'input>,
}

/// Function body: either single-quoted string, dollar-quoted string, or a
/// psql client variable substitution (e.g., `AS :'regresslib'` for C-language
/// shared libraries passed in via psql `\set`).
///
/// Variant ordering: dollar-quoted before single-quoted before psql var
/// (different first chars).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(::recursa::arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncBodyPart<'input> {
    Dollar(literal::DollarStringLit<'input>),
    String(literal::StringLit<'input>),
    PsqlVar(literal::PsqlVar<'input>),
}

/// Full function body — `AS body [, symbol]`. The second comma-separated
/// form is used for C-language functions where the first part names the
/// shared object file and the second names the exported C symbol.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncBody<'input> {
    pub obj_file: FuncBodyPart<'input>,
    pub symbol: Option<(punct::Comma, FuncBodyPart<'input>)>,
}

/// Function return type name -- extends TypeName with additional types
/// that are valid as function return types (e.g., `trigger`), and allows
/// array suffixes via `CastType`. Also accepts the `qualified%TYPE`
/// reference-type form (PG `func_type: type_function_name attrs '%' TYPE_P`).
///
/// Variant ordering: `PctType` first so its longer prefix wins on
/// `name.attr%TYPE` over the bare `Base` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncReturnTypeName<'input> {
    Trigger(TRIGGER),
    PctType(PctTypeRef<'input>),
    Base(CastType<'input>),
}

/// RETURNS clause for functions: `RETURNS [SETOF] type`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncReturnsClause<'input> {
    pub returns: RETURNS,
    pub return_type: FuncReturnType<'input>,
}

/// A single column in `RETURNS TABLE(col type, ...)`: `name type`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableColumn<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub type_name: CastType<'input>,
}

/// `TABLE(col type, ...)` — tabular function return type.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncReturnsTable<'input> {
    pub table: TABLE,
    pub columns: Surrounded<punct::LParen, Seq1<TableColumn<'input>, punct::Comma>, punct::RParen>,
}

/// Function return type: TABLE(...), SETOF type, or plain type.
///
/// `Table` before `Setof` and `Plain` — `TABLE` is a keyword that won't
/// match as an identifier-based type.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncReturnType<'input> {
    Table(FuncReturnsTable<'input>),
    Setof(FuncSetofReturn<'input>),
    Plain(FuncReturnTypeName<'input>),
}

/// SETOF type for function returns.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncSetofReturn<'input> {
    pub setof: SETOF,
    pub type_name: FuncReturnTypeName<'input>,
}

// --- Function parameters ---

/// Argument mode prefix: `IN | OUT | INOUT | VARIADIC`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ArgMode {
    In(IN),
    Inout(INOUT),
    Out(OUT),
    Variadic(VARIADIC),
}

/// `qualified_name%TYPE` — the PG-specific reference-type form used in
/// function parameter and return types. `gram.y::func_type` is
/// `type_function_name attrs '%' TYPE_P` (a qualified name with at least
/// one `.attr` segment). We accept a plain qualified name with one or
/// more parts so simple `name.col%TYPE` and longer chains
/// (`hobbies_r.person.name%TYPE`) both round-trip.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PctTypeRef<'input> {
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    pub percent: punct::Percent,
    pub type_kw: TYPE,
}

/// A function parameter / return-type slot. Either a regular type (with
/// optional precision / array suffix) or the `qualified_name%TYPE`
/// reference-type form. The `%TYPE` variant is listed first so its longer
/// match wins via declaration-order tiebreak.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncArgType<'input> {
    PctType(PctTypeRef<'input>),
    Cast(CastType<'input>),
}

/// `[mode] name type [default]` -- a named function parameter with mode first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NamedFuncParam<'input> {
    pub mode: Option<ArgMode>,
    pub name: crate::tokens::type_function_name<'input>,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// `name mode type [default]` -- a named function parameter with mode after name.
///
/// Postgres allows `f2 OUT anyelement` where the mode follows the name.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NameModeParam<'input> {
    pub name: crate::tokens::type_function_name<'input>,
    pub mode: ArgMode,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// `[mode] type [default]` -- an unnamed function parameter.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UnnamedFuncParam<'input> {
    pub mode: Option<ArgMode>,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// Default value separator: `DEFAULT` or `=`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ParamDefaultSep {
    Default(DEFAULT),
    Eq(punct::Eq),
}

/// `DEFAULT expr` or `= expr` trailing default on a function parameter.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ParamDefault<'input> {
    pub sep: ParamDefaultSep,
    pub value: Expr<'input>,
}

/// A single function parameter.
///
/// Variant ordering:
/// - `NameMode` (`ident mode type`) — longest, has ident then mode keyword
/// - `Named` (`[mode] ident type`) — has mode then ident then type
/// - `Unnamed` (`[mode] type`) — shortest, just optional mode + type
///
/// `NameMode` must come first because `name mode type` would otherwise
/// be parsed by `Named` as name=ident, type=mode_keyword (wrong).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncParam<'input> {
    NameMode(NameModeParam<'input>),
    Named(NamedFuncParam<'input>),
    Unnamed(UnnamedFuncParam<'input>),
}

// --- Function options (unordered list) ---

/// `IMMUTABLE` / `STABLE` / `VOLATILE` volatility.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum VolatilityOption {
    Immutable(IMMUTABLE),
    Stable(STABLE),
    Volatile(VOLATILE),
}

/// `PARALLEL SAFE` / `PARALLEL RESTRICTED` / `PARALLEL UNSAFE` parallelism
/// declaration.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ParallelMode {
    Safe(SAFE),
    Restricted(RESTRICTED),
    Unsafe(UNSAFE),
}

/// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` function option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ParallelOption {
    pub parallel: PARALLEL,
    pub mode: ParallelMode,
}

/// Separator between a SET config parameter name and its value — either
/// `=` or `TO`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetAssignSep {
    Eq(punct::Eq),
    To(TO),
}

/// `SET config_param { = | TO } var_list` function option — per-function GUC
/// override applied when the function runs.
///
/// Postgres `set_rest_more: ColId TO var_list | ColId '=' var_list` admits a
/// comma-separated `var_list`, so values like `SET datestyle to iso, mdy`
/// (rules.sql) parse cleanly.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SetFuncOption<'input> {
    pub set: SET,
    pub name: literal::AliasName<'input>,
    pub sep: SetAssignSep,
    pub values: Seq1<crate::ast::session::set_reset::SetValue<'input>, punct::Comma>,
}

/// `STRICT` / `CALLED ON NULL INPUT` / `RETURNS NULL ON NULL INPUT`.
///
/// Variant ordering: longer (multi-keyword) forms before `Strict`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum StrictnessOption {
    CalledOnNullInput((CALLED, ON, NULL, INPUT)),
    ReturnsNullOnNullInput((RETURNS, NULL, ON, NULL, INPUT)),
    Strict(STRICT),
}

/// `AS body` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AsOption<'input> {
    pub r#as: AS,
    pub body: FuncBody<'input>,
}

/// A single function option clause.
///
/// Variant ordering: multi-token options listed before single-keyword
/// options, and `StrictnessOption` (which itself has multi-keyword variants)
/// listed before plain `VolatilityOption`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncOption<'input> {
    Strictness(StrictnessOption),
    Volatility(VolatilityOption),
    Parallel(ParallelOption),
    Set(SetFuncOption<'input>),
    Language(LanguageOption<'input>),
    /// `SECURITY DEFINER` / `SECURITY INVOKER`.
    Security(SecurityOption),
    /// `EXTERNAL SECURITY DEFINER` / `EXTERNAL SECURITY INVOKER` — older
    /// SQL standard spelling, still accepted.
    ExternalSecurity(ExternalSecurityOption),
    /// `LEAKPROOF` / `NOT LEAKPROOF`.
    Leakproof(LeakproofOption),
    /// `WINDOW` — declares the function as a window function.
    Window(WINDOW),
    /// `COST numeric`.
    Cost(CostOption<'input>),
    /// `ROWS numeric`.
    Rows(RowsOption<'input>),
    /// `SUPPORT qualified_name` — planner support function.
    Support(SupportOption<'input>),
    /// `TRANSFORM FOR TYPE typ [, ...]`.
    Transform(TransformOption<'input>),
    As(AsOption<'input>),
    /// `RETURN expr` — SQL-standard single-expression function body.
    Return(ReturnOption<'input>),
    /// `BEGIN ATOMIC ... END` — SQL-standard inline routine body
    /// (gram.y `createfunc_opt_item: BEGIN_P ATOMIC routine_body_stmt_list END_P`).
    /// Only the empty-body shape is modeled here; populating the body
    /// would require a peek-time predicate on the inner statement list to
    /// stop before the closing `END` keyword. The corpus only exercises
    /// the empty form (`CREATE PROCEDURE ptest8(x text) BEGIN ATOMIC
    /// END`); non-empty bodies surface as
    /// [`crate::ast::FileItem::ParseError`] until a peek-postcondition
    /// lands on `BeginAtomicStmt`.
    BeginAtomicEmpty(BeginAtomicEmpty),
}

/// Empty `BEGIN ATOMIC END` body. Non-empty bodies are not yet modelled —
/// see `FuncOption::BeginAtomicEmpty` for the rationale.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct BeginAtomicEmpty {
    pub begin: BEGIN,
    pub atomic: ATOMIC,
    pub end: END,
}

/// `RETURN expr` option on CREATE FUNCTION (SQL-standard body form).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ReturnOption<'input> {
    pub r#return: RETURN,
    pub expr: Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SecurityMode {
    Definer(DEFINER),
    Invoker(INVOKER),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SecurityOption {
    pub security: SECURITY,
    pub mode: SecurityMode,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExternalSecurityOption {
    pub external: EXTERNAL,
    pub inner: SecurityOption,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LeakproofOption {
    NotLeakproof((NOT, LEAKPROOF)),
    Leakproof(LEAKPROOF),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CostOption<'input> {
    pub cost: COST,
    pub value: Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RowsOption<'input> {
    pub rows: ROWS,
    pub value: Expr<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SupportOption<'input> {
    pub support: SUPPORT,
    pub name: crate::ast::shared::names::QualifiedName<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TransformOption<'input> {
    pub transform: TRANSFORM,
    pub items: Seq0<TransformForType<'input>, punct::Comma>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TransformForType<'input> {
    pub r#for: FOR,
    pub r#type: TYPE,
    pub type_name: CastType<'input>,
}

/// Extracted function body: the language name and the raw source text.
///
/// This is a convenience view assembled post-parse from the unordered
/// option list. The body text has its delimiters (`$$`, `'`) stripped.
#[derive(Debug, Clone)]
pub struct ExtractedFuncBody<'a> {
    pub lang: &'a str,
    pub body: &'a str,
}

/// CREATE [OR REPLACE] FUNCTION statement.
///
/// Function options after the signature/RETURNS may appear in any order.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateFunctionStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub function: FUNCTION,
    pub name: crate::ast::shared::names::FuncDefName<'input>,
    pub args: Surrounded<punct::LParen, Seq0<FuncParam<'input>, punct::Comma>, punct::RParen>,
    pub returns: Option<FuncReturnsClause<'input>>,
    pub options: Seq0<FuncOption<'input>, (), OptionalTrailing>,
}

impl<'input> CreateFunctionStmt<'input> {
    /// Extract the function body and language from the unordered option list.
    ///
    /// Scans for `AS body` and `LANGUAGE name` options; returns `None` if
    /// either is missing (e.g. a `RETURN expr` form has no AS clause).
    pub fn func_body(&self) -> Option<ExtractedFuncBody<'_>> {
        let lang = self.options.iter().find_map(|opt| match opt {
            FuncOption::Language(l) => Some(match &l.name {
                LanguageName::Ident(id) => id.text(),
                LanguageName::String(s) => strip_quotes(&s.0),
            }),
            _ => None,
        })?;
        let body = self.options.iter().find_map(|opt| match opt {
            FuncOption::As(a) => Some(strip_body_delimiters(&a.body.obj_file)),
            _ => None,
        })?;
        Some(ExtractedFuncBody { lang, body })
    }
}

fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(s)
}

fn strip_body_delimiters<'a>(part: &'a FuncBodyPart<'a>) -> &'a str {
    match part {
        FuncBodyPart::Dollar(d) => strip_dollar_quotes(&d.0),
        FuncBodyPart::String(s) => strip_quotes(&s.0),
        FuncBodyPart::PsqlVar(v) => &v.0,
    }
}

fn strip_dollar_quotes(s: &str) -> &str {
    if let Some(end_of_open) = s
        .find('$')
        .and_then(|i| s[i + 1..].find('$').map(|j| i + 1 + j + 1))
    {
        let inner = &s[end_of_open..];
        if let Some(close_start) = inner.rfind('$') {
            let before_close = &inner[..close_start];
            if let Some(tag_start) = before_close.rfind('$') {
                return &inner[..tag_start];
            }
        }
    }
    s
}

/// A single entry in a `DROP FUNCTION` target list: optional qualified name
/// plus an optional parenthesized signature.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropFunctionTarget<'input> {
    pub name: crate::ast::shared::names::FuncDefName<'input>,
    pub args:
        Option<Surrounded<punct::LParen, Seq0<FuncParam<'input>, punct::Comma>, punct::RParen>>,
}

/// DROP FUNCTION statement: `DROP FUNCTION name[(args)] [, name[(args)] ...]`.
///
/// The argument list on each target is optional: when the function name is
/// unambiguous in the current schema, Postgres allows omitting the signature.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropFunctionStmt<'input> {
    pub drop: DROP,
    pub function: FUNCTION,
    pub if_exists: Option<(IF, EXISTS)>,
    pub targets: Seq0<DropFunctionTarget<'input>, punct::Comma>,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

/// DROP ROUTINE statement — Postgres synonym for DROP FUNCTION/PROCEDURE
/// that dispatches by name/signature at lookup time.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropRoutineStmt<'input> {
    pub drop: DROP,
    pub routine: ROUTINE,
    pub if_exists: Option<(IF, EXISTS)>,
    pub targets: Seq0<DropFunctionTarget<'input>, punct::Comma>,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;
    use recursa::Parse;

    use crate::ast::ddl::function::{CreateFunctionStmt, DropFunctionStmt};

    #[test]
    fn parse_create_function_return_body() {
        let mut input =
            crate::tokens::test_input("CREATE FUNCTION f() RETURNS boolean RETURN false");
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_basic() {
        let mut input = crate::tokens::test_input(
            "create function sillysrf(int) returns setof int as 'values (1),(10),(2),($1)' language sql immutable",
        );
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "sillysrf");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_function_basic() {
        let mut input = crate::tokens::test_input("drop function sillysrf(int)");
        let stmt = DropFunctionStmt::parse(&mut input).unwrap();
        assert_eq!(
            stmt.targets.iter().next().unwrap().name.object(),
            "sillysrf"
        );
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_function_multi() {
        let mut input = crate::tokens::test_input("drop function a(), b(), c()");
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_named_param() {
        let mut input = crate::tokens::test_input(
            "create function polyf(x anyelement) returns anyelement as $$ select x + 1 $$ language sql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_function_cascade() {
        let mut input = crate::tokens::test_input("DROP FUNCTION int4_casttesttype(int4) CASCADE");
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_function_named_param() {
        let mut input = crate::tokens::test_input("drop function polyf(x anyelement)");
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_returns_trigger() {
        let mut input = crate::tokens::test_input(
            "create function f() returns trigger language plpgsql as $$ begin end $$",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_strict_immutable() {
        let mut input = crate::tokens::test_input(
            "create function f() returns int immutable strict language sql as 'SELECT 1'",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_options_reordered() {
        let mut input = crate::tokens::test_input(
            "create function f() returns int language sql strict as 'SELECT 1'",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_in_out_named() {
        let mut input = crate::tokens::test_input(
            "create function f(in i int, out j int) returns int as $$ begin return i+1; end $$ language plpgsql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_in_out_no_returns() {
        let mut input = crate::tokens::test_input(
            "create function f(in i int, out j int) as $$ begin end $$ language plpgsql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_setof_record() {
        let mut input = crate::tokens::test_input(
            "create function gs(v integer, out a integer, out b integer) returns setof record as $f$ select 1 $f$ language plpgsql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_polymorphic_out() {
        let mut input = crate::tokens::test_input(
            "create function poly(a anyelement, b anyarray, OUT x anyarray) as $$ begin end $$ language plpgsql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_param_eq_default() {
        let mut input = crate::tokens::test_input(
            "create function f(a int = 1, b int = 2) returns int as $$ select 1 $$ language sql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_param_default_keyword() {
        let mut input = crate::tokens::test_input(
            "create function f(a int default 1) returns int as $$ select 1 $$ language sql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_unnamed_default() {
        let mut input = crate::tokens::test_input(
            "create function dfunc(a int = 1, int = 2) returns int as $$ select 1 $$ language sql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_array_arg() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION stfnp(int[]) RETURNS int[] AS 'select $1' LANGUAGE SQL",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_array_arg_multi() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f(int[], text[]) RETURNS int[] AS 'select $1' LANGUAGE SQL",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_nested_array() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f(x int[][]) RETURNS int[][] AS 'select x' LANGUAGE SQL",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_function_multi_named_params() {
        let mut input = crate::tokens::test_input(
            "create function tg_hub_adjustslots(hname bpchar, oldn integer, newn integer) returns integer as ' begin return 1; end ' language plpgsql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn func_body_dollar_quoted() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM 1; END; $$",
        );
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "plpgsql");
        assert_eq!(body.body.trim(), "BEGIN PERFORM 1; END;");
    }

    #[test]
    fn func_body_single_quoted() {
        let mut input =
            crate::tokens::test_input("CREATE FUNCTION f() RETURNS int AS 'SELECT 1' LANGUAGE sql");
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "sql");
        assert_eq!(body.body, "SELECT 1");
    }

    #[test]
    fn func_body_tagged_dollar_quote() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $proc$ DECLARE x int; BEGIN x := 1; END; $proc$",
        );
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "plpgsql");
        assert_eq!(body.body.trim(), "DECLARE x int; BEGIN x := 1; END;");
    }

    #[test]
    fn func_returns_table() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f(int) RETURNS TABLE(a int, b int) AS $$ BEGIN RETURN QUERY SELECT 1, 2; END; $$ LANGUAGE plpgsql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn func_returns_table_varchar() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f() RETURNS TABLE(a varchar(5)) AS $$ SELECT 'hello'::varchar(5) $$ LANGUAGE sql",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// Postgres' `set_rest_more: ColId TO var_list | ColId '=' var_list`
    /// admits a comma-separated `var_list` after `TO` / `=`. The rules.sql
    /// regression exercises `SET datestyle to iso, mdy` as one option in
    /// a `createfunc_opt_list`.
    #[test]
    fn parse_create_function_set_var_list() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f() RETURNS integer AS 'select 1;' LANGUAGE SQL SET datestyle to iso, mdy",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// Multiple `SET` options on a single CREATE FUNCTION — each is its own
    /// `createfunc_opt_item`. The rules.sql regression chains five of them.
    #[test]
    fn parse_create_function_multiple_set_options() {
        let mut input = crate::tokens::test_input(
            "CREATE FUNCTION f() RETURNS integer AS 'select 1;' LANGUAGE SQL \
             SET search_path TO PG_CATALOG \
             SET extra_float_digits TO 2 \
             SET work_mem TO '4MB' \
             SET datestyle to iso, mdy \
             SET local_preload_libraries TO ''",
        );
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }
    #[test]
    fn alter_function_rename_to() {
        let stmt: AlterFunctionStmt =
            parse_stmt("ALTER FUNCTION alt_func1(int) RENAME TO alt_func2");
        assert_eq!(stmt.target.name.object(), "alt_func1");
        assert!(matches!(stmt.action, AlterFuncAction::Rename(_)));
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION alt_func1(int) RENAME TO alt_func2");
    }

    #[test]
    fn alter_function_owner_to() {
        let stmt: AlterFunctionStmt =
            parse_stmt("ALTER FUNCTION alt_func2(int) OWNER TO regress_alter_generic_user2");
        assert!(matches!(stmt.action, AlterFuncAction::Owner(_)));
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION alt_func2(int) OWNER TO regress_alter_generic_user2",
        );
    }

    #[test]
    fn alter_function_set_schema() {
        let stmt: AlterFunctionStmt =
            parse_stmt("ALTER FUNCTION alt_func2(int) SET SCHEMA alt_nsp2");
        assert!(matches!(stmt.action, AlterFuncAction::SetSchema(_)));
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION alt_func2(int) SET SCHEMA alt_nsp2");
    }

    #[test]
    fn alter_function_depends_on_extension() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION f(int) DEPENDS ON EXTENSION my_extension",
        );
    }

    #[test]
    fn alter_function_no_depends_on_extension() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION f(int) NO DEPENDS ON EXTENSION my_extension",
        );
    }

    #[test]
    fn alter_function_immutable() {
        let stmt: AlterFunctionStmt = parse_stmt("ALTER FUNCTION functest_C_1(int) IMMUTABLE");
        assert!(matches!(stmt.action, AlterFuncAction::Options(_)));
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_C_1(int) IMMUTABLE");
    }

    #[test]
    fn alter_function_strict() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_F_2(int) STRICT");
    }

    #[test]
    fn alter_function_called_on_null_input() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION functest_F_3(int) CALLED ON NULL INPUT",
        );
    }

    #[test]
    fn alter_function_returns_null_on_null_input() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION non_strict(text) RETURNS NULL ON NULL INPUT",
        );
    }

    #[test]
    fn alter_function_security_invoker() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_C_2(int) SECURITY INVOKER");
    }

    #[test]
    fn alter_function_security_definer() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_C_3(int) SECURITY DEFINER");
    }

    #[test]
    fn alter_function_external_security_definer() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION f(int) EXTERNAL SECURITY DEFINER");
    }

    #[test]
    fn alter_function_leakproof() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_E_1(int) LEAKPROOF");
    }

    #[test]
    fn alter_function_not_leakproof() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_E_2(int) NOT LEAKPROOF");
    }

    #[test]
    fn alter_function_cost() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_B_3(int) COST 100");
    }

    #[test]
    fn alter_function_rows() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION f(int) ROWS 200");
    }

    #[test]
    fn alter_function_support() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION my_int_eq(int, int) SUPPORT test_support_func",
        );
    }

    #[test]
    fn alter_function_parallel_safe() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION f(int) PARALLEL SAFE");
    }

    #[test]
    fn alter_function_volatile() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_B_2(int) VOLATILE");
    }

    #[test]
    fn alter_function_set_param() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION report_guc(text) SET work_mem = '2MB'");
    }

    #[test]
    fn alter_function_reset_all() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION report_guc(text) RESET ALL");
    }

    #[test]
    fn alter_function_multi_options_with_restrict() {
        // Multiple options space-separated, optional RESTRICT at end (opt_restrict
        // is the deprecated trailing modifier).
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION f(int) STRICT IMMUTABLE LEAKPROOF RESTRICT",
        );
    }

    #[test]
    fn alter_function_qualified_name() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION alter1.plus1(int) SET SCHEMA alter2");
    }

    #[test]
    fn alter_function_no_argtypes() {
        // function_with_argtypes admits bare name (no parens) per gram.y.
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION terminate_nothrow OWNER TO pg_signal_backend",
        );
    }

    #[test]
    fn alter_routine_rename_no_argtypes() {
        // ALTER ROUTINE accepts bare name (no parens).
        reparse_stable::<AlterRoutineStmt>("ALTER ROUTINE cp_testfunc1a RENAME TO cp_testfunc1");
    }

    #[test]
    fn alter_routine_rename_with_argtypes() {
        reparse_stable::<AlterRoutineStmt>(
            "ALTER ROUTINE cp_testfunc1(int) RENAME TO cp_testfunc1a",
        );
    }
}

// =========================================================================
// ALTER/DROP FUNCTION — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` mode keyword on a function
/// option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterFuncParallelMode {
    Safe(SAFE),
    Restricted(RESTRICTED),
    Unsafe(UNSAFE),
}

/// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` function option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncParallelItem {
    pub parallel: PARALLEL,
    pub mode: AlterFuncParallelMode,
}

/// `SECURITY { DEFINER | INVOKER }` mode keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterFuncSecurityMode {
    Definer(DEFINER),
    Invoker(INVOKER),
}

/// `SECURITY { DEFINER | INVOKER }` function option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncSecurityItem {
    pub security: SECURITY,
    pub mode: AlterFuncSecurityMode,
}

/// `EXTERNAL SECURITY { DEFINER | INVOKER }` function option — older
/// SQL-standard spelling, still accepted by gram.y.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncExternalSecurityItem {
    pub external: EXTERNAL,
    pub inner: AlterFuncSecurityItem,
}

/// `COST NumericOnly` function option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncCostItem<'input> {
    pub cost: COST,
    pub value: NumericOnly<'input>,
}

/// `ROWS NumericOnly` function option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncRowsItem<'input> {
    pub rows: ROWS,
    pub value: NumericOnly<'input>,
}

/// `SUPPORT any_name` function option — names a planner-support function.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncSupportItem<'input> {
    pub support: SUPPORT,
    pub name: QualifiedName<'input>,
}

/// One item of the function action list — gram.y's `common_func_opt_item`
/// (the options common to `CREATE FUNCTION` and `ALTER FUNCTION`).
///
/// The list itself is one-or-more **space-separated** items (no commas)
/// followed by an optional `RESTRICT` (see [`AlterFuncOptions`]).
///
/// Variant ordering: longest leading keyword sequence first, so the
/// longest-match peek picks the most specific variant. `CALLED ON NULL
/// INPUT` and `RETURNS NULL ON NULL INPUT` are 4 tokens; `EXTERNAL
/// SECURITY ...` and `NOT LEAKPROOF` and `PARALLEL ...` and `SECURITY
/// ...` are 2-3 tokens; the bare keyword variants are 1 token. All
/// leading tokens are distinct so longest-match is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CommonFuncOptItem<'input> {
    // Multi-keyword forms first.
    CalledOnNullInput((CALLED, ON, NULL, INPUT)),
    ReturnsNullOnNullInput((RETURNS, NULL, ON, NULL, INPUT)),
    ExternalSecurity(AlterFuncExternalSecurityItem),
    Security(AlterFuncSecurityItem),
    NotLeakproof((NOT, LEAKPROOF)),
    Parallel(AlterFuncParallelItem),
    Cost(AlterFuncCostItem<'input>),
    Rows(AlterFuncRowsItem<'input>),
    Support(AlterFuncSupportItem<'input>),
    // `SET name = value` and `RESET name | RESET ALL` — the
    // FunctionSetResetClause branch of common_func_opt_item.
    Set(crate::ast::session::set_reset::SetStmt<'input>),
    Reset(crate::ast::session::set_reset::ResetStmt<'input>),
    // Single-keyword forms.
    Leakproof(LEAKPROOF),
    Strict(STRICT),
    Immutable(IMMUTABLE),
    Stable(STABLE),
    Volatile(VOLATILE),
}

/// `common_func_opt_item …+ [RESTRICT]` — the action-list branch of
/// `ALTER FUNCTION / PROCEDURE / ROUTINE`. At least one option is
/// required (gram.y's `alterfunc_opt_list` is right-recursive with
/// `common_func_opt_item` as the base case, not empty). The trailing
/// `RESTRICT` is gram.y's deprecated `opt_restrict`, present for SQL
/// compliance and ignored semantically.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterFuncOptions<'input> {
    pub items: Seq1<CommonFuncOptItem<'input>, (), recursa::seq::OptionalTrailing>,
    pub restrict: Option<RESTRICT>,
}

/// One action on `ALTER { FUNCTION | PROCEDURE | ROUTINE }
/// function_with_argtypes action` — covers Postgres' `RenameStmt`,
/// `AlterOwnerStmt`, `AlterObjectSchemaStmt`, `AlterObjectDependsStmt`,
/// and the in-place `alterfunc_opt_list` action.
///
/// Variant ordering:
/// - `SetSchema` (`SET SCHEMA`) before `Options` (whose `Set` item
///   starts with `SET <ident>`) so the dispatch on `SET` commits to the
///   `SET SCHEMA` form when followed by the `SCHEMA` keyword.
/// - Other variants have distinct leading keywords (`RENAME`, `OWNER`,
///   `DEPENDS`, `NO`, plus all the `common_func_opt_item` first tokens),
///   so order is for clarity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterFuncAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    Depends(DependsOnExtension<'input>),
    Options(AlterFuncOptions<'input>),
}

/// `ALTER FUNCTION function_with_argtypes action` — Postgres'
/// `AlterFunctionStmt` (the `alterfunc_opt_list` branch) plus the
/// `RenameStmt` / `AlterOwnerStmt` / `AlterObjectSchemaStmt` /
/// `AlterObjectDependsStmt` branches that share the leading `ALTER
/// FUNCTION` keywords.
///
/// The argument signature reuses [`DropFunctionTarget`] (gram.y's
/// `function_with_argtypes`), which already covers both the `name(args)`
/// and bare-`name` shapes.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterFunctionStmt<'input> {
    pub alter: ALTER,
    pub function: FUNCTION,
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}

/// `ALTER ROUTINE function_with_argtypes action` — same action shape as
/// [`AlterFunctionStmt`]. `ROUTINE` is gram.y's dispatch-at-lookup
/// synonym that resolves to a function or procedure by name/signature.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterRoutineStmt<'input> {
    pub alter: ALTER,
    pub routine: ROUTINE,
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}
