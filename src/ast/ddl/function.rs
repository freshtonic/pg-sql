/// CREATE FUNCTION / DROP FUNCTION statement AST.
use crate::ast::shared::expr::{CastType, Expr, TypeName};
use crate::tokens::literal;
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
// ---------------------------------------------------------------------------

/// SETOF type: `SETOF typename`
#[derive(recursa::Node, Debug, Clone)]
pub struct SetofReturn<'input> {
    #[tok(SETOF, this)]
    pub type_name: TypeName<'input>,
}

/// Function return type: `SETOF type` or plain `type`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ReturnType<'input> {
    Setof(SetofReturn<'input>),
    Plain(TypeName<'input>),
}

/// LANGUAGE clause: `LANGUAGE name` or `LANGUAGE 'name'`. Postgres accepts
/// the language name as an identifier or as a single-quoted string literal.
#[derive(recursa::Node, Debug, Clone)]
pub enum LanguageName<'input> {
    Ident(literal::AliasName<'input>),
    String(literal::StringLit<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct LanguageOption<'input> {
    #[tok(LANGUAGE, this)]
    pub name: LanguageName<'input>,
}

/// Function body: either single-quoted string, dollar-quoted string, or a
/// psql client variable substitution (e.g., `AS :'regresslib'` for C-language
/// shared libraries passed in via psql `\set`).
///
/// Variant ordering: dollar-quoted before single-quoted before psql var
/// (different first chars).
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncBodyPart<'input> {
    Dollar(literal::DollarStringLit<'input>),
    String(literal::StringLit<'input>),
    PsqlVar(literal::PsqlVariable<'input>),
}

/// Full function body — `AS body [, symbol]`. The second comma-separated
/// form is used for C-language functions where the first part names the
/// shared object file and the second names the exported C symbol.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncBody<'input> {
    pub obj_file: FuncBodyPart<'input>,
    #[tok(COMMA, this)]
    pub symbol: Option<FuncBodyPart<'input>>,
}

/// Function return type name, including both ordinary cast types and the
/// PostgreSQL-specific `qualified%TYPE` reference form.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncReturnTypeName<'input> {
    pub value: FunctionType<'input>,
}

/// RETURNS clause for functions: `RETURNS [SETOF] type`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncReturnsClause<'input> {
    #[tok(RETURNS, this)]
    pub return_type: FuncReturnType<'input>,
}

/// A single column in `RETURNS TABLE(col type, ...)`: `name type`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableColumn<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub type_name: CastType<'input>,
}

/// `TABLE(col type, ...)` — tabular function return type.
#[derive(recursa::Node, Debug, Clone)]
#[tok(TABLE, LPAREN, this, RPAREN)]
pub struct FuncReturnsTable<'input> {
    #[sep(COMMA)]
    pub columns: recursa::Vec1<TableColumn<'input>>,
}

/// Function return type: TABLE(...), SETOF type, or plain type.
///
/// `Table` before `Setof` and `Plain` — `TABLE` is a keyword that won't
/// match as an identifier-based type.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncReturnType<'input> {
    Table(FuncReturnsTable<'input>),
    Setof(FuncSetofReturn<'input>),
    Plain(FuncReturnTypeName<'input>),
}

/// SETOF type for function returns.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncSetofReturn<'input> {
    #[tok(SETOF, this)]
    pub type_name: FuncReturnTypeName<'input>,
}

// --- Function parameters ---

/// Argument mode prefix: `IN | OUT | INOUT | VARIADIC`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ArgMode {
    #[tok(IN)]
    In,
    #[tok(INOUT)]
    Inout,
    #[tok(OUT)]
    Out,
    #[tok(VARIADIC)]
    Variadic,
}

/// Fixed PostgreSQL built-in type names. Identifier-spelled type names are
/// factored separately so their shared qualified prefix can be parsed once.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionBuiltinTypeName {
    #[tok(BOOLEAN)]
    Boolean,
    #[tok(INTEGER)]
    Integer,
    #[tok(INT)]
    Int,
    #[tok(NUMERIC)]
    Numeric,
    #[tok(VARCHAR)]
    Varchar,
    #[tok(TIMESTAMP)]
    Timestamp,
    #[tok(TIME)]
    Time,
    #[tok(INTERVAL)]
    Interval,
    #[tok(BIT)]
    Bit,
    #[tok(CHARACTER)]
    Character,
}

/// Suffix shared by built-in and identifier-spelled cast types.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionCastTypeTail<'input> {
    /// `PRECISION` in `DOUBLE PRECISION`.
    #[presence(PRECISION)]
    pub precision_keyword: bool,
    #[presence(VARYING)]
    pub varying: bool,
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub interval_qualifier: Option<IntervalQualifier<'input>>,
    pub array_suffixes: Vec<ArraySuffix<'input>>,
    pub array_kw_suffix: Option<ArrayKwSuffix<'input>>,
}

/// A built-in type plus the ordinary cast-type suffixes.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionBuiltinType<'input> {
    pub base: FunctionBuiltinTypeName,
    pub tail: FunctionCastTypeTail<'input>,
}

/// One dotted attribute in an identifier-spelled function type.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionTypeNamePart<'input> {
    #[tok(DOT, this)]
    pub name: literal::AliasName<'input>,
}

/// Shared qualified-name prefix of a cast type and `%TYPE` reference.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionTypeName<'input> {
    pub first: crate::tokens::type_function_name<'input>,
    pub rest: Vec<FunctionTypeNamePart<'input>>,
}

/// `%TYPE` suffix on a function type reference.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionPctTypeSuffix {
    #[tok(PERCENT, TYPE)]
    Value,
}

/// The suffix following a shared identifier-spelled type name.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionIdentifierTypeSuffix<'input> {
    Pct(FunctionPctTypeSuffix),
    Cast(FunctionCastTypeTail<'input>),
}

/// Identifier-spelled cast type or `qualified%TYPE` reference.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionIdentifierType<'input> {
    pub name: FunctionTypeName<'input>,
    pub suffix: FunctionIdentifierTypeSuffix<'input>,
}

/// A function type with the qualified identifier prefix factored before the
/// `%TYPE` versus cast-suffix decision.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionType<'input> {
    Builtin(FunctionBuiltinType<'input>),
    Identifier(FunctionIdentifierType<'input>),
}

/// A function parameter type uses the same factored grammar as a return type.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncArgType<'input> {
    pub value: FunctionType<'input>,
}

/// `[mode] name type [default]` -- a named function parameter with mode first.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedFuncParam<'input> {
    pub mode: Option<ArgMode>,
    pub name: crate::tokens::type_function_name<'input>,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// `name mode type [default]` -- a named function parameter with mode after name.
///
/// Postgres allows `f2 OUT anyelement` where the mode follows the name.
#[derive(recursa::Node, Debug, Clone)]
pub struct NameModeParam<'input> {
    pub name: crate::tokens::type_function_name<'input>,
    pub mode: ArgMode,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// `[mode] type [default]` -- an unnamed function parameter.
#[derive(recursa::Node, Debug, Clone)]
pub struct UnnamedFuncParam<'input> {
    pub mode: Option<ArgMode>,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// Default value separator: `DEFAULT` or `=`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParamDefaultSep {
    #[tok(DEFAULT)]
    Default,
    #[tok(EQ)]
    Eq,
}

/// `DEFAULT expr` or `= expr` trailing default on a function parameter.
#[derive(recursa::Node, Debug, Clone)]
pub struct ParamDefault<'input> {
    pub sep: ParamDefaultSep,
    pub value: Expr<'input>,
}

/// The shared `[mode] type-or-name [type] [default]` shape of named and
/// unnamed parameters. A second type means the first identifier is the
/// parameter name; without it, the first value is the unnamed parameter's
/// type. Factoring this shape avoids asking bounded lookahead to distinguish
/// two arbitrarily long type prefixes.
#[derive(recursa::Node, Debug, Clone)]
pub struct StandardFuncParam<'input> {
    pub mode: Option<ArgMode>,
    pub first: FuncArgType<'input>,
    pub named_type: Option<FuncArgType<'input>>,
    pub default: Option<ParamDefault<'input>>,
}

/// A single function parameter.
///
/// Variant ordering:
/// - `NameMode` (`ident mode type`) — longest, has ident then mode keyword
/// - `Standard` factors `[mode] ident type` and `[mode] type` into one shape
///
/// `NameMode` must come first because `name mode type` would otherwise
/// be parsed by `Named` as name=ident, type=mode_keyword (wrong).
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncParam<'input> {
    NameMode(NameModeParam<'input>),
    Standard(StandardFuncParam<'input>),
}

// --- Function options (unordered list) ---

/// `IMMUTABLE` / `STABLE` / `VOLATILE` volatility.
#[derive(recursa::Node, Debug, Clone)]
pub enum VolatilityOption {
    #[tok(IMMUTABLE)]
    Immutable,
    #[tok(STABLE)]
    Stable,
    #[tok(VOLATILE)]
    Volatile,
}

/// `PARALLEL SAFE` / `PARALLEL RESTRICTED` / `PARALLEL UNSAFE` parallelism
/// declaration.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParallelMode {
    #[tok(SAFE)]
    Safe,
    #[tok(RESTRICTED)]
    Restricted,
    #[tok(UNSAFE)]
    Unsafe,
}

/// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` function option.
#[derive(recursa::Node, Debug, Clone)]
pub struct ParallelOption {
    #[tok(PARALLEL, this)]
    pub mode: ParallelMode,
}

/// Separator between a SET config parameter name and its value — either
/// `=` or `TO`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetAssignSep {
    #[tok(EQ)]
    Eq,
    #[tok(TO)]
    To,
}

/// `SET config_param { = | TO } var_list` function option — per-function GUC
/// override applied when the function runs.
///
/// Postgres `set_rest_more: ColId TO var_list | ColId '=' var_list` admits a
/// comma-separated `var_list`, so values like `SET datestyle to iso, mdy`
/// (rules.sql) parse cleanly.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetFuncOption<'input> {
    #[tok(SET, this)]
    pub name: literal::AliasName<'input>,
    pub sep: SetAssignSep,
    #[sep(COMMA)]
    pub values: recursa::Vec1<crate::ast::session::set_reset::SetValue<'input>>,
}

/// `STRICT` / `CALLED ON NULL INPUT` / `RETURNS NULL ON NULL INPUT`.
///
/// Variant ordering: longer (multi-keyword) forms before `Strict`.
#[derive(recursa::Node, Debug, Clone)]
pub enum StrictnessOption {
    #[tok(CALLED, ON, NULL, INPUT)]
    CalledOnNullInput,
    #[tok(RETURNS, NULL, ON, NULL, INPUT)]
    ReturnsNullOnNullInput,
    #[tok(STRICT)]
    Strict,
}

/// `AS body` clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct AsOption<'input> {
    #[tok(AS, this)]
    pub body: FuncBody<'input>,
}

/// A single function option clause.
///
/// Variant ordering: multi-token options listed before single-keyword
/// options, and `StrictnessOption` (which itself has multi-keyword variants)
/// listed before plain `VolatilityOption`.
#[derive(recursa::Node, Debug, Clone)]
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
    #[tok(WINDOW)]
    /// `WINDOW` — declares the function as a window function.
    Window,
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
    /// END`); non-empty bodies remain outside the issue-9 strict-statement
    /// grammar and surface as a structured parse error.
    BeginAtomicEmpty(BeginAtomicEmpty),
}

/// Empty `BEGIN ATOMIC END` body. Non-empty bodies are not yet modelled —
/// see `FuncOption::BeginAtomicEmpty` for the rationale.
#[derive(recursa::Node, Debug, Clone)]
pub enum BeginAtomicEmpty {
    #[tok(BEGIN, ATOMIC, END)]
    Value,
}

/// `RETURN expr` option on CREATE FUNCTION (SQL-standard body form).
#[derive(recursa::Node, Debug, Clone)]
pub struct ReturnOption<'input> {
    #[tok(RETURN, this)]
    pub expr: Expr<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SecurityMode {
    #[tok(DEFINER)]
    Definer,
    #[tok(INVOKER)]
    Invoker,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SecurityOption {
    #[tok(SECURITY, this)]
    pub mode: SecurityMode,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct ExternalSecurityOption {
    #[tok(EXTERNAL, this)]
    pub inner: SecurityOption,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum LeakproofOption {
    #[tok(NOT, LEAKPROOF)]
    NotLeakproof,
    #[tok(LEAKPROOF)]
    Leakproof,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CostOption<'input> {
    #[tok(COST, this)]
    pub value: Expr<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct RowsOption<'input> {
    #[tok(ROWS, this)]
    pub value: Expr<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SupportOption<'input> {
    #[tok(SUPPORT, this)]
    pub name: crate::ast::shared::names::QualifiedName<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(TRANSFORM, this)]
pub struct TransformOption<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<TransformForType<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct TransformForType<'input> {
    #[tok(FOR, TYPE, this)]
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

/// Parenthesized function or procedure parameter list.
///
/// The wrapper keeps the delimiters around the complete comma-separated list
/// while dereferencing to the underlying vector for callers.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct FunctionParameters<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<FuncParam<'input>>,
);

/// CREATE [OR REPLACE] FUNCTION statement.
///
/// Function options after the signature/RETURNS may appear in any order.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateFunctionStmt<'input> {
    #[tok(CREATE, this, FUNCTION)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub name: crate::ast::shared::names::FuncDefName<'input>,
    pub args: FunctionParameters<'input>,
    pub returns: Option<FuncReturnsClause<'input>>,
    pub options: Vec<FuncOption<'input>>,
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
                LanguageName::String(s) => strip_quotes(s.text()),
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
        FuncBodyPart::Dollar(d) => strip_dollar_quotes(d.text()),
        FuncBodyPart::String(s) => strip_quotes(s.text()),
        FuncBodyPart::PsqlVar(v) => match &v.name {
            literal::PsqlVariableValue::Name(name) => name.text(),
            literal::PsqlVariableValue::String(string) => strip_quotes(string.text()),
        },
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
#[derive(recursa::Node, Debug, Clone)]
pub struct DropFunctionTarget<'input> {
    pub name: crate::ast::shared::names::FuncDefName<'input>,
    pub args: Option<FunctionParameters<'input>>,
}

/// DROP FUNCTION statement: `DROP FUNCTION name[(args)] [, name[(args)] ...]`.
///
/// The argument list on each target is optional: when the function name is
/// unambiguous in the current schema, Postgres allows omitting the signature.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropFunctionStmt<'input> {
    #[tok(DROP, FUNCTION, this)]
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    pub targets: Vec<DropFunctionTarget<'input>>,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

/// DROP ROUTINE statement — Postgres synonym for DROP FUNCTION/PROCEDURE
/// that dispatches by name/signature at lookup time.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropRoutineStmt<'input> {
    #[tok(DROP, ROUTINE, this)]
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    pub targets: Vec<DropFunctionTarget<'input>>,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/function.tests.rs"
));

// =========================================================================
// ALTER/DROP FUNCTION — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` mode keyword on a function
/// option.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterFuncParallelMode {
    #[tok(SAFE)]
    Safe,
    #[tok(RESTRICTED)]
    Restricted,
    #[tok(UNSAFE)]
    Unsafe,
}

/// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` function option.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncParallelItem {
    #[tok(PARALLEL, this)]
    pub mode: AlterFuncParallelMode,
}

/// `SECURITY { DEFINER | INVOKER }` mode keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterFuncSecurityMode {
    #[tok(DEFINER)]
    Definer,
    #[tok(INVOKER)]
    Invoker,
}

/// `SECURITY { DEFINER | INVOKER }` function option.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncSecurityItem {
    #[tok(SECURITY, this)]
    pub mode: AlterFuncSecurityMode,
}

/// `EXTERNAL SECURITY { DEFINER | INVOKER }` function option — older
/// SQL-standard spelling, still accepted by gram.y.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncExternalSecurityItem {
    #[tok(EXTERNAL, this)]
    pub inner: AlterFuncSecurityItem,
}

/// `COST NumericOnly` function option.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncCostItem<'input> {
    #[tok(COST, this)]
    pub value: NumericOnly<'input>,
}

/// `ROWS NumericOnly` function option.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncRowsItem<'input> {
    #[tok(ROWS, this)]
    pub value: NumericOnly<'input>,
}

/// `SUPPORT any_name` function option — names a planner-support function.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncSupportItem<'input> {
    #[tok(SUPPORT, this)]
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
#[derive(recursa::Node, Debug, Clone)]
pub enum CommonFuncOptItem<'input> {
    // Multi-keyword forms first.
    #[tok(CALLED, ON, NULL, INPUT)]
    CalledOnNullInput,
    #[tok(RETURNS, NULL, ON, NULL, INPUT)]
    ReturnsNullOnNullInput,
    ExternalSecurity(AlterFuncExternalSecurityItem),
    Security(AlterFuncSecurityItem),
    #[tok(NOT, LEAKPROOF)]
    NotLeakproof,
    Parallel(AlterFuncParallelItem),
    Cost(AlterFuncCostItem<'input>),
    Rows(AlterFuncRowsItem<'input>),
    Support(AlterFuncSupportItem<'input>),
    // `SET name = value` and `RESET name | RESET ALL` — the
    // FunctionSetResetClause branch of common_func_opt_item.
    Set(crate::ast::session::set_reset::SetStmt<'input>),
    Reset(crate::ast::session::set_reset::ResetStmt<'input>),
    // Single-keyword forms.
    #[tok(LEAKPROOF)]
    Leakproof,
    #[tok(STRICT)]
    Strict,
    #[tok(IMMUTABLE)]
    Immutable,
    #[tok(STABLE)]
    Stable,
    #[tok(VOLATILE)]
    Volatile,
}

/// `common_func_opt_item …+ [RESTRICT]` — the action-list branch of
/// `ALTER FUNCTION / PROCEDURE / ROUTINE`. At least one option is
/// required (gram.y's `alterfunc_opt_list` is right-recursive with
/// `common_func_opt_item` as the base case, not empty). The trailing
/// `RESTRICT` is gram.y's deprecated `opt_restrict`, present for SQL
/// compliance and ignored semantically.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncOptions<'input> {
    pub items: recursa::Vec1<CommonFuncOptItem<'input>>,
    #[presence(RESTRICT)]
    pub restrict: bool,
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
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFunctionStmt<'input> {
    #[tok(ALTER, FUNCTION, this)]
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}

/// `ALTER ROUTINE function_with_argtypes action` — same action shape as
/// [`AlterFunctionStmt`]. `ROUTINE` is gram.y's dispatch-at-lookup
/// synonym that resolves to a function or procedure by name/signature.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterRoutineStmt<'input> {
    #[tok(ALTER, ROUTINE, this)]
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}
