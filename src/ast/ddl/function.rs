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

recursa::ast_node! {
    /// SETOF type: `SETOF typename`
    #[derive(Debug)]
    pub struct SetofReturn {
        #[tok(SETOF, this)]
        pub type_name: TypeName,
    }
}

recursa::ast_node! {
    /// Function return type: `SETOF type` or plain `type`.
    #[derive(Debug)]
    pub enum ReturnType {
        Setof(SetofReturn),
        Plain(TypeName),
    }
}

recursa::ast_node! {
    /// LANGUAGE clause: `LANGUAGE name` or `LANGUAGE 'name'`. Postgres accepts
    /// the language name as an identifier or as a single-quoted string literal.
    #[derive(Debug)]
    pub enum LanguageName {
        Ident(literal::AliasName),
        String(literal::StringLit),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct LanguageOption {
        #[tok(LANGUAGE, this)]
        pub name: LanguageName,
    }
}

recursa::ast_node! {
    /// Function body: either a single-quoted or a dollar-quoted string, which is
    /// gram.y's `func_as: Sconst | Sconst ',' Sconst` payload.
    ///
    /// psql's `AS :'regresslib'` is not a form of this: psql substitutes the
    /// variable before the server lexes, so what reaches gram.y is an ordinary
    /// `Sconst`. Render such a script through the `pg-psql` crate first.
    ///
    /// Variant ordering: dollar-quoted before single-quoted (different first
    /// chars).
    #[derive(Debug)]
    pub enum FuncBodyPart {
        Dollar(literal::DollarStringLit),
        String(literal::StringLit),
    }
}

recursa::ast_node! {
    /// Full function body — `AS body [, symbol]`. The second comma-separated
    /// form is used for C-language functions where the first part names the
    /// shared object file and the second names the exported C symbol.
    #[derive(Debug)]
    pub struct FuncBody {
        pub obj_file: FuncBodyPart,
        #[tok(COMMA, this)]
        pub symbol: Option<FuncBodyPart>,
    }
}

recursa::ast_node! {
    /// Function return type name, including both ordinary cast types and the
    /// PostgreSQL-specific `qualified%TYPE` reference form.
    #[derive(Debug)]
    pub struct FuncReturnTypeName {
        pub value: FunctionType,
    }
}

recursa::ast_node! {
    /// RETURNS clause for functions: `RETURNS [SETOF] type`.
    #[derive(Debug)]
    pub struct FuncReturnsClause {
        #[tok(RETURNS, this)]
        pub return_type: FuncReturnType,
    }
}

recursa::ast_node! {
    /// A single column in `RETURNS TABLE(col type, ...)`: `name type`.
    #[derive(Debug)]
    pub struct TableColumn {
        pub name: crate::tokens::ColId,
        pub type_name: CastType,
    }
}

recursa::ast_node! {
    /// `TABLE(col type, ...)` — tabular function return type.
    #[derive(Debug)]
    #[tok(TABLE, LPAREN, this, RPAREN)]
    pub struct FuncReturnsTable {
        #[sep(COMMA)]
        pub columns: one_or_many!(TableColumn),
    }
}

recursa::ast_node! {
    /// Function return type: TABLE(...), SETOF type, or plain type.
    ///
    /// `Table` before `Setof` and `Plain` — `TABLE` is a keyword that won't
    /// match as an identifier-based type.
    #[derive(Debug)]
    pub enum FuncReturnType {
        Table(FuncReturnsTable),
        Setof(FuncSetofReturn),
        Plain(FuncReturnTypeName),
    }
}

recursa::ast_node! {
    /// SETOF type for function returns.
    #[derive(Debug)]
    pub struct FuncSetofReturn {
        #[tok(SETOF, this)]
        pub type_name: FuncReturnTypeName,
    }
}

// --- Function parameters ---

recursa::ast_node! {
    /// Argument mode prefix: `IN | OUT | INOUT | VARIADIC`.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// Fixed PostgreSQL built-in type names. Identifier-spelled type names are
    /// factored separately so their shared qualified prefix can be parsed once.
    ///
    /// `JSON` belongs here rather than with the identifier-spelled names because
    /// it is a `COL_NAME` keyword and so is excluded from `type_function_name`.
    /// PostgreSQL reaches it through the dedicated `JsonType` production
    /// (gram.y `SimpleTypename: … | JsonType`), which is what makes
    /// `RETURNS json` and `f(node json)` legal while keeping `json` out of
    /// `func_name`.
    #[derive(Debug)]
    pub enum FunctionBuiltinTypeName {
        #[tok(BOOLEAN)]
        Boolean,
        #[tok(JSON)]
        Json,
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
}

recursa::ast_node! {
    /// Suffix shared by built-in and identifier-spelled cast types.
    #[derive(Debug)]
    pub struct FunctionCastTypeTail {
        /// `PRECISION` in `DOUBLE PRECISION`.
        #[presence(PRECISION)]
        pub precision_keyword: bool,
        #[presence(VARYING)]
        pub varying: bool,
        pub precision: Option<TypePrecision>,
        pub tz: Option<TimeZoneQualifier>,
        pub interval_qualifier: Option<IntervalQualifier>,
        pub array_suffixes: zero_or_many!(ArraySuffix),
        pub array_kw_suffix: Option<ArrayKwSuffix>,
    }
}

recursa::ast_node! {
    /// A built-in type plus the ordinary cast-type suffixes.
    #[derive(Debug)]
    pub struct FunctionBuiltinType {
        pub base: FunctionBuiltinTypeName,
        pub tail: FunctionCastTypeTail,
    }
}

recursa::ast_node! {
    /// One dotted attribute in an identifier-spelled function type.
    #[derive(Debug)]
    pub struct FunctionTypeNamePart {
        #[tok(DOT, this)]
        pub name: literal::AliasName,
    }
}

recursa::ast_node! {
    /// Shared qualified-name prefix of a cast type and `%TYPE` reference.
    #[derive(Debug)]
    pub struct FunctionTypeName {
        pub first: crate::tokens::type_function_name,
        pub rest: zero_or_many!(FunctionTypeNamePart),
    }
}

recursa::ast_node! {
    /// `%TYPE` suffix on a function type reference.
    #[derive(Debug)]
    pub enum FunctionPctTypeSuffix {
        #[tok(PERCENT, TYPE)]
        Value,
    }
}

recursa::ast_node! {
    /// The suffix following a shared identifier-spelled type name.
    #[derive(Debug)]
    pub enum FunctionIdentifierTypeSuffix {
        Pct(FunctionPctTypeSuffix),
        Cast(FunctionGenericTypeTail),
    }
}

recursa::ast_node! {
    /// gram.y `GenericType: type_function_name [attrs] opt_type_modifiers` with
    /// `Typename`'s `opt_array_bounds` / `ARRAY` forms: an identifier-spelled
    /// type takes type modifiers and array bounds only. The datetime, interval
    /// and `VARYING` tails belong to the keyword-spelled types; on a generic
    /// type they made `f(mytype year)` ambiguous between a type with an
    /// interval qualifier and a parameter named `mytype` of type `year`.
    #[derive(Debug)]
    pub struct FunctionGenericTypeTail {
        pub precision: Option<TypePrecision>,
        pub array_suffixes: zero_or_many!(ArraySuffix),
        pub array_kw_suffix: Option<ArrayKwSuffix>,
    }
}

recursa::ast_node! {
    /// Identifier-spelled cast type or `qualified%TYPE` reference.
    #[derive(Debug)]
    pub struct FunctionIdentifierType {
        pub name: FunctionTypeName,
        pub suffix: FunctionIdentifierTypeSuffix,
    }
}

recursa::ast_node! {
    /// A function type with the qualified identifier prefix factored before the
    /// `%TYPE` versus cast-suffix decision.
    #[derive(Debug)]
    pub enum FunctionType {
        Builtin(FunctionBuiltinType),
        Identifier(FunctionIdentifierType),
    }
}

recursa::ast_node! {
    /// A function parameter type uses the same factored grammar as a return type.
    #[derive(Debug)]
    pub struct FuncArgType {
        pub value: FunctionType,
    }
}

recursa::ast_node! {
    /// `[mode] name type [default]` -- a named function parameter with mode first.
    #[derive(Debug)]
    pub struct NamedArg {
        pub mode: Option<ArgMode>,
        pub name: crate::tokens::type_function_name,
        pub type_name: FuncArgType,
    }
}

recursa::ast_node! {
    /// `name mode type [default]` -- a named function parameter with mode after name.
    ///
    /// Postgres allows `f2 OUT anyelement` where the mode follows the name.
    #[derive(Debug)]
    pub struct NameModeArg {
        pub name: crate::tokens::type_function_name,
        pub mode: ArgMode,
        pub type_name: FuncArgType,
    }
}

recursa::ast_node! {
    /// `[mode] type [default]` -- an unnamed function parameter.
    #[derive(Debug)]
    pub struct UnnamedArg {
        pub mode: Option<ArgMode>,
        pub type_name: FuncArgType,
    }
}

recursa::ast_node! {
    /// Default value separator: `DEFAULT` or `=`.
    #[derive(Debug)]
    pub enum ParamDefaultSep {
        #[tok(DEFAULT)]
        Default,
        #[tok(EQ)]
        Eq,
    }
}

recursa::ast_node! {
    /// `DEFAULT expr` or `= expr` trailing default on a function parameter.
    #[derive(Debug)]
    pub struct ParamDefault {
        pub sep: ParamDefaultSep,
        pub value: Expr,
    }
}

recursa::ast_node! {
    /// gram.y `func_arg: arg_class param_name func_type | param_name arg_class
    /// func_type | param_name func_type | arg_class func_type | func_type`; a
    /// `param_name` is one `type_function_name`, never a type.
    ///
    /// Variant ordering: `NameMode` and `Named` start with a name; the token
    /// after it (an argument mode, a type, or the end of the argument) decides.
    #[derive(Debug)]
    pub enum FunctionArg {
        /// `param_name arg_class func_type`.
        NameMode(NameModeArg),
        /// `[arg_class] param_name func_type`.
        Named(NamedArg),
        /// `[arg_class] func_type`.
        Unnamed(UnnamedArg),
    }
}

recursa::ast_node! {
    /// gram.y `func_arg_with_default: func_arg | func_arg DEFAULT a_expr |
    /// func_arg '=' a_expr`, the parameter of `CREATE FUNCTION` / `CREATE
    /// PROCEDURE`. Aggregates take a bare `func_arg`.
    #[derive(Debug)]
    pub struct FuncParam {
        pub arg: FunctionArg,
        pub default: Option<ParamDefault>,
    }
}

// --- Function options (unordered list) ---

recursa::ast_node! {
    /// `IMMUTABLE` / `STABLE` / `VOLATILE` volatility.
    #[derive(Debug)]
    pub enum VolatilityOption {
        #[tok(IMMUTABLE)]
        Immutable,
        #[tok(STABLE)]
        Stable,
        #[tok(VOLATILE)]
        Volatile,
    }
}

recursa::ast_node! {
    /// `PARALLEL SAFE` / `PARALLEL RESTRICTED` / `PARALLEL UNSAFE` parallelism
    /// declaration.
    #[derive(Debug)]
    pub enum ParallelMode {
        #[tok(SAFE)]
        Safe,
        #[tok(RESTRICTED)]
        Restricted,
        #[tok(UNSAFE)]
        Unsafe,
    }
}

recursa::ast_node! {
    /// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` function option.
    #[derive(Debug)]
    pub struct ParallelOption {
        #[tok(PARALLEL, this)]
        pub mode: ParallelMode,
    }
}

recursa::ast_node! {
    /// Separator between a SET config parameter name and its value — either
    /// `=` or `TO`.
    #[derive(Debug)]
    pub enum SetAssignSep {
        #[tok(EQ)]
        Eq,
        #[tok(TO)]
        To,
    }
}

recursa::ast_node! {
    /// `SET config_param { = | TO } var_list` function option — per-function GUC
    /// override applied when the function runs.
    ///
    /// Postgres `set_rest_more: ColId TO var_list | ColId '=' var_list` admits a
    /// comma-separated `var_list`, so values like `SET datestyle to iso, mdy`
    /// (rules.sql) parse cleanly.
    #[derive(Debug)]
    pub struct SetFuncOption {
        #[tok(SET, this)]
        pub name: literal::AliasName,
        pub sep: SetAssignSep,
        #[sep(COMMA)]
        pub values: one_or_many!(crate::ast::session::set_reset::SetValue),
    }
}

recursa::ast_node! {
    /// `STRICT` / `CALLED ON NULL INPUT` / `RETURNS NULL ON NULL INPUT`.
    ///
    /// Variant ordering: longer (multi-keyword) forms before `Strict`.
    #[derive(Debug)]
    pub enum StrictnessOption {
        #[tok(CALLED, ON, NULL, INPUT)]
        CalledOnNullInput,
        #[allow(
            clippy::duplicated_attributes,
            reason = "NULL occurs twice in the PostgreSQL RETURNS NULL ON NULL INPUT syntax"
        )]
        #[tok(RETURNS, NULL, ON, NULL, INPUT)]
        ReturnsNullOnNullInput,
        #[tok(STRICT)]
        Strict,
    }
}

recursa::ast_node! {
    /// `AS body` clause.
    #[derive(Debug)]
    pub struct AsOption {
        #[tok(AS, this)]
        pub body: FuncBody,
    }
}

recursa::ast_node! {
    /// A single function option clause.
    ///
    /// Variant ordering: multi-token options listed before single-keyword
    /// options, and `StrictnessOption` (which itself has multi-keyword variants)
    /// listed before plain `VolatilityOption`.
    #[derive(Debug)]
    pub enum FuncOption {
        Strictness(StrictnessOption),
        Volatility(VolatilityOption),
        Parallel(ParallelOption),
        Set(SetFuncOption),
        Language(LanguageOption),
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
        Cost(CostOption),
        /// `ROWS numeric`.
        Rows(RowsOption),
        /// `SUPPORT qualified_name` — planner support function.
        Support(SupportOption),
        /// `TRANSFORM FOR TYPE typ [, ...]`.
        Transform(TransformOption),
        As(AsOption),
    }
}

recursa::ast_node! {
    /// gram.y `opt_routine_body`, the SQL-standard body that follows the option
    /// list of `CREATE FUNCTION` / `CREATE PROCEDURE`: `RETURN a_expr` or
    /// `BEGIN ATOMIC ... END`. It comes after every option, so the `RETURN`
    /// expression is never followed by an option such as `NOT LEAKPROOF`.
    ///
    /// Only the empty `BEGIN ATOMIC END` shape is modeled; populating the body
    /// would require a peek-time predicate on the inner statement list to stop
    /// before the closing `END` keyword. The corpus only exercises the empty
    /// form (`CREATE PROCEDURE ptest8(x text) BEGIN ATOMIC END`); non-empty
    /// bodies remain outside the issue-9 strict-statement grammar and surface
    /// as a structured parse error.
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum RoutineBody {
        /// `RETURN expr` — SQL-standard single-expression function body.
        Return(ReturnOption),
        /// `BEGIN ATOMIC END`.
        BeginAtomicEmpty(BeginAtomicEmpty),
    }
}

recursa::ast_node! {
    /// Empty `BEGIN ATOMIC END` body. Non-empty bodies are not yet modelled —
    /// see `FuncOption::BeginAtomicEmpty` for the rationale.
    #[derive(Debug)]
    pub enum BeginAtomicEmpty {
        #[tok(BEGIN, ATOMIC, END)]
        Value,
    }
}

recursa::ast_node! {
    /// `RETURN expr` option on CREATE FUNCTION (SQL-standard body form).
    #[derive(Debug)]
    pub struct ReturnOption {
        #[tok(RETURN, this)]
        pub expr: Expr,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SecurityMode {
        #[tok(DEFINER)]
        Definer,
        #[tok(INVOKER)]
        Invoker,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SecurityOption {
        #[tok(SECURITY, this)]
        pub mode: SecurityMode,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct ExternalSecurityOption {
        #[tok(EXTERNAL, this)]
        pub inner: SecurityOption,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum LeakproofOption {
        #[tok(NOT, LEAKPROOF)]
        NotLeakproof,
        #[tok(LEAKPROOF)]
        Leakproof,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct CostOption {
        /// gram.y `common_func_opt_item: COST NumericOnly`.
        #[tok(COST, this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct RowsOption {
        /// gram.y `common_func_opt_item: ROWS NumericOnly`.
        #[tok(ROWS, this)]
        pub value: crate::ast::shared::numbers::NumericOnly,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SupportOption {
        #[tok(SUPPORT, this)]
        pub name: crate::ast::shared::names::QualifiedName,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(TRANSFORM, this)]
    pub struct TransformOption {
        #[sep(COMMA)]
        pub items: one_or_many!(TransformForType),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct TransformForType {
        #[tok(FOR, TYPE, this)]
        pub type_name: CastType,
    }
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

recursa::ast_node! {
    /// Parenthesized function or procedure parameter list.
    ///
    /// The wrapper keeps the delimiters around the complete comma-separated list
    /// while dereferencing to the underlying vector for callers.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct FunctionParameters(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(FuncParam),
    );
}

recursa::ast_node! {
    /// CREATE [OR REPLACE] FUNCTION statement.
    ///
    /// Function options after the signature/RETURNS may appear in any order.
    #[derive(Debug)]
    #[tok(CREATE, this)]
    pub struct CreateFunctionStmt {
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        #[tok(FUNCTION, this)]
        pub name: crate::ast::shared::names::FuncDefName,
        pub args: FunctionParameters,
        pub returns: Option<FuncReturnsClause>,
        pub options: zero_or_many!(FuncOption),
        /// gram.y `opt_routine_body`, after `opt_createfunc_opt_list`.
        pub body: Option<RoutineBody>,
    }
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

recursa::ast_node! {
    /// A single entry in a `DROP FUNCTION` target list: optional qualified name
    /// plus an optional parenthesized signature.
    #[derive(Debug)]
    pub struct DropFunctionTarget {
        pub name: crate::ast::shared::names::FuncDefName,
        pub args: Option<FunctionParameters>,
    }
}

recursa::ast_node! {
    /// DROP FUNCTION statement: `DROP FUNCTION name[(args)] [, name[(args)] ...]`.
    ///
    /// The argument list on each target is optional: when the function name is
    /// unambiguous in the current schema, Postgres allows omitting the signature.
    #[derive(Debug)]
    #[tok(DROP, FUNCTION, this)]
    pub struct DropFunctionStmt {
        #[presence(IF, EXISTS)]
        pub if_exists: bool,
        #[sep(COMMA)]
        /// gram.y `function_with_argtypes_list`: one or more targets.
        pub targets: one_or_many!(DropFunctionTarget),
        pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
    }
}

recursa::ast_node! {
    /// DROP ROUTINE statement — Postgres synonym for DROP FUNCTION/PROCEDURE
    /// that dispatches by name/signature at lookup time.
    #[derive(Debug)]
    #[tok(DROP, ROUTINE, this)]
    pub struct DropRoutineStmt {
        #[presence(IF, EXISTS)]
        pub if_exists: bool,
        #[sep(COMMA)]
        /// gram.y `function_with_argtypes_list`: one or more targets.
        pub targets: one_or_many!(DropFunctionTarget),
        pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
    }
}

// =========================================================================
// ALTER/DROP FUNCTION — appended from simple_stmts.rs during physical extraction.
// =========================================================================

recursa::ast_node! {
    /// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` mode keyword on a function
    /// option.
    #[derive(Debug)]
    pub enum AlterFuncParallelMode {
        #[tok(SAFE)]
        Safe,
        #[tok(RESTRICTED)]
        Restricted,
        #[tok(UNSAFE)]
        Unsafe,
    }
}

recursa::ast_node! {
    /// `PARALLEL { SAFE | RESTRICTED | UNSAFE }` function option.
    #[derive(Debug)]
    pub struct AlterFuncParallelItem {
        #[tok(PARALLEL, this)]
        pub mode: AlterFuncParallelMode,
    }
}

recursa::ast_node! {
    /// `SECURITY { DEFINER | INVOKER }` mode keyword.
    #[derive(Debug)]
    pub enum AlterFuncSecurityMode {
        #[tok(DEFINER)]
        Definer,
        #[tok(INVOKER)]
        Invoker,
    }
}

recursa::ast_node! {
    /// `SECURITY { DEFINER | INVOKER }` function option.
    #[derive(Debug)]
    pub struct AlterFuncSecurityItem {
        #[tok(SECURITY, this)]
        pub mode: AlterFuncSecurityMode,
    }
}

recursa::ast_node! {
    /// `EXTERNAL SECURITY { DEFINER | INVOKER }` function option — older
    /// SQL-standard spelling, still accepted by gram.y.
    #[derive(Debug)]
    pub struct AlterFuncExternalSecurityItem {
        #[tok(EXTERNAL, this)]
        pub inner: AlterFuncSecurityItem,
    }
}

recursa::ast_node! {
    /// `COST NumericOnly` function option.
    #[derive(Debug)]
    pub struct AlterFuncCostItem {
        #[tok(COST, this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `ROWS NumericOnly` function option.
    #[derive(Debug)]
    pub struct AlterFuncRowsItem {
        #[tok(ROWS, this)]
        pub value: NumericOnly,
    }
}

recursa::ast_node! {
    /// `SUPPORT any_name` function option — names a planner-support function.
    #[derive(Debug)]
    pub struct AlterFuncSupportItem {
        #[tok(SUPPORT, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum CommonFuncOptItem {
        // Multi-keyword forms first.
        #[tok(CALLED, ON, NULL, INPUT)]
        CalledOnNullInput,
        #[allow(
            clippy::duplicated_attributes,
            reason = "NULL occurs twice in the PostgreSQL RETURNS NULL ON NULL INPUT syntax"
        )]
        #[tok(RETURNS, NULL, ON, NULL, INPUT)]
        ReturnsNullOnNullInput,
        ExternalSecurity(AlterFuncExternalSecurityItem),
        Security(AlterFuncSecurityItem),
        #[tok(NOT, LEAKPROOF)]
        NotLeakproof,
        Parallel(AlterFuncParallelItem),
        Cost(AlterFuncCostItem),
        Rows(AlterFuncRowsItem),
        Support(AlterFuncSupportItem),
        // `SET name = value` and `RESET name | RESET ALL` — the
        // FunctionSetResetClause branch of common_func_opt_item.
        Set(crate::ast::session::set_reset::SetStmt),
        Reset(crate::ast::session::set_reset::ResetStmt),
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
}

recursa::ast_node! {
    /// `common_func_opt_item …+ [RESTRICT]` — the action-list branch of
    /// `ALTER FUNCTION / PROCEDURE / ROUTINE`. At least one option is
    /// required (gram.y's `alterfunc_opt_list` is right-recursive with
    /// `common_func_opt_item` as the base case, not empty). The trailing
    /// `RESTRICT` is gram.y's deprecated `opt_restrict`, present for SQL
    /// compliance and ignored semantically.
    #[derive(Debug)]
    pub struct AlterFuncOptions {
        pub items: one_or_many!(CommonFuncOptItem),
        #[presence(RESTRICT)]
        pub restrict: bool,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterFuncAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
        Depends(DependsOnExtension),
        Options(AlterFuncOptions),
    }
}

recursa::ast_node! {
    /// `ALTER FUNCTION function_with_argtypes action` — Postgres'
    /// `AlterFunctionStmt` (the `alterfunc_opt_list` branch) plus the
    /// `RenameStmt` / `AlterOwnerStmt` / `AlterObjectSchemaStmt` /
    /// `AlterObjectDependsStmt` branches that share the leading `ALTER
    /// FUNCTION` keywords.
    ///
    /// The argument signature reuses [`DropFunctionTarget`] (gram.y's
    /// `function_with_argtypes`), which already covers both the `name(args)`
    /// and bare-`name` shapes.
    #[derive(Debug)]
    pub struct AlterFunctionStmt {
        #[tok(ALTER, FUNCTION, this)]
        pub target: crate::ast::ddl::function::DropFunctionTarget,
        pub action: AlterFuncAction,
    }
}

recursa::ast_node! {
    /// `ALTER ROUTINE function_with_argtypes action` — same action shape as
    /// [`AlterFunctionStmt`]. `ROUTINE` is gram.y's dispatch-at-lookup
    /// synonym that resolves to a function or procedure by name/signature.
    #[derive(Debug)]
    pub struct AlterRoutineStmt {
        #[tok(ALTER, ROUTINE, this)]
        pub target: crate::ast::ddl::function::DropFunctionTarget,
        pub action: AlterFuncAction,
    }
}
