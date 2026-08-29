/// CREATE FUNCTION / DROP FUNCTION statement AST.
use recursa::seq::{OptionalTrailing, Seq0, Seq1};

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
#[cfg_attr(feature = "arbitrary", derive(::recursa::arbitrary::Arbitrary))]
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

/// Function return type name -- extends TypeName with additional types
/// that are valid as function return types (e.g., `trigger`), and allows
/// array suffixes via `CastType`. Also accepts the `qualified%TYPE`
/// reference-type form (PG `func_type: type_function_name attrs '%' TYPE_P`).
///
/// Variant ordering: `PctType` first so its longer prefix wins on
/// `name.attr%TYPE` over the bare `Base` form.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncReturnTypeName<'input> {
    #[tok(TRIGGER)] Trigger,
    PctType(PctTypeRef<'input>),
    Base(CastType<'input>),
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
pub struct FuncReturnsTable<'input> {
    #[tok(TABLE, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns:  recursa::Vec1<TableColumn<'input> > ,
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
    #[tok(IN)] In,
    #[tok(INOUT)] Inout,
    #[tok(OUT)] Out,
    #[tok(VARIADIC)] Variadic,
}

/// `qualified_name%TYPE` — the PG-specific reference-type form used in
/// function parameter and return types. `gram.y::func_type` is
/// `type_function_name attrs '%' TYPE_P` (a qualified name with at least
/// one `.attr` segment). We accept a plain qualified name with one or
/// more parts so simple `name.col%TYPE` and longer chains
/// (`hobbies_r.person.name%TYPE`) both round-trip.
#[derive(recursa::Node, Debug, Clone)]
pub struct PctTypeRef<'input> {
    #[tok(this, PERCENT, TYPE)]
    pub name: crate::ast::shared::names::QualifiedName<'input>,
}

/// A function parameter / return-type slot. Either a regular type (with
/// optional precision / array suffix) or the `qualified_name%TYPE`
/// reference-type form. The `%TYPE` variant is listed first so its longer
/// match wins via declaration-order tiebreak.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncArgType<'input> {
    PctType(PctTypeRef<'input>),
    Cast(CastType<'input>),
}

/// `[mode] name type [default]` -- a named function parameter with mode first.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedFuncParam<'input> {
    pub mode: Option<ArgMode>,
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(type_function_name))]
    pub name: crate::tokens::type_function_name<'input>,
    pub type_name: FuncArgType<'input>,
    pub default: Option<ParamDefault<'input>>,
}

/// `name mode type [default]` -- a named function parameter with mode after name.
///
/// Postgres allows `f2 OUT anyelement` where the mode follows the name.
#[derive(recursa::Node, Debug, Clone)]
pub struct NameModeParam<'input> {
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(type_function_name))]
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
    #[tok(DEFAULT)] Default,
    #[tok(EQ)] Eq,
}

/// `DEFAULT expr` or `= expr` trailing default on a function parameter.
#[derive(recursa::Node, Debug, Clone)]
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
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncParam<'input> {
    NameMode(NameModeParam<'input>),
    Named(NamedFuncParam<'input>),
    Unnamed(UnnamedFuncParam<'input>),
}

// --- Function options (unordered list) ---

/// `IMMUTABLE` / `STABLE` / `VOLATILE` volatility.
#[derive(recursa::Node, Debug, Clone)]
pub enum VolatilityOption {
    #[tok(IMMUTABLE)] Immutable,
    #[tok(STABLE)] Stable,
    #[tok(VOLATILE)] Volatile,
}

/// `PARALLEL SAFE` / `PARALLEL RESTRICTED` / `PARALLEL UNSAFE` parallelism
/// declaration.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParallelMode {
    #[tok(SAFE)] Safe,
    #[tok(RESTRICTED)] Restricted,
    #[tok(UNSAFE)] Unsafe,
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
    #[tok(EQ)] Eq,
    #[tok(TO)] To,
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
    pub values: recursa::Vec1<crate::ast::session::set_reset::SetValue<'input> >,
}

/// `STRICT` / `CALLED ON NULL INPUT` / `RETURNS NULL ON NULL INPUT`.
///
/// Variant ordering: longer (multi-keyword) forms before `Strict`.
#[derive(recursa::Node, Debug, Clone)]
pub enum StrictnessOption {
    #[tok(CALLED, ON, NULL, INPUT)] CalledOnNullInput,
    #[tok(RETURNS, NULL, ON, NULL, INPUT)] ReturnsNullOnNullInput,
    #[tok(STRICT)] Strict,
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
    #[tok(WINDOW)] /// `WINDOW` — declares the function as a window function.
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
    /// END`); non-empty bodies surface as
    /// [`crate::ast::FileItem::ParseError`] until a peek-postcondition
    /// lands on `BeginAtomicStmt`.
    BeginAtomicEmpty(BeginAtomicEmpty),
}

/// Empty `BEGIN ATOMIC END` body. Non-empty bodies are not yet modelled —
/// see `FuncOption::BeginAtomicEmpty` for the rationale.
#[derive(recursa::Node, Debug, Clone)]
pub enum BeginAtomicEmpty { #[tok(BEGIN, ATOMIC, END)] Value, }

/// `RETURN expr` option on CREATE FUNCTION (SQL-standard body form).
#[derive(recursa::Node, Debug, Clone)]
pub struct ReturnOption<'input> {
    #[tok(RETURN, this)]
    pub expr: Expr<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SecurityMode {
    #[tok(DEFINER)] Definer,
    #[tok(INVOKER)] Invoker,
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
    #[tok(NOT, LEAKPROOF)] NotLeakproof,
    #[tok(LEAKPROOF)] Leakproof,
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
pub struct TransformOption<'input> {
    #[tok(TRANSFORM, this)]
    #[sep(COMMA)]
    pub items: Vec<TransformForType<'input> >,
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

/// CREATE [OR REPLACE] FUNCTION statement.
///
/// Function options after the signature/RETURNS may appear in any order.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateFunctionStmt<'input> {
    #[tok(CREATE, this, FUNCTION)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub name: crate::ast::shared::names::FuncDefName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:  Vec<FuncParam<'input> > ,
    pub returns: Option<FuncReturnsClause<'input>>,
    pub options: Vec<FuncOption<'input>  >,
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
#[derive(recursa::Node, Debug, Clone)]
pub struct DropFunctionTarget<'input> {
    pub name: crate::ast::shared::names::FuncDefName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:
        Option< Vec<FuncParam<'input> > >,
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
    pub targets: Vec<DropFunctionTarget<'input> >,
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
    pub targets: Vec<DropFunctionTarget<'input> >,
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
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS boolean RETURN false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_basic() {
        let lexed = crate::tokens::lex("create function sillysrf(int) returns setof int as 'values (1),(10),(2),($1)' language sql immutable");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "sillysrf");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_basic() {
        let lexed = crate::tokens::lex("drop function sillysrf(int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(
            stmt.targets.iter().next().unwrap().name.object(),
            "sillysrf"
        );
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_multi() {
        let lexed = crate::tokens::lex("drop function a(), b(), c()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_named_param() {
        let lexed = crate::tokens::lex("create function polyf(x anyelement) returns anyelement as $$ select x + 1 $$ language sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_cascade() {
        let lexed = crate::tokens::lex("DROP FUNCTION int4_casttesttype(int4) CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_named_param() {
        let lexed = crate::tokens::lex("drop function polyf(x anyelement)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_returns_trigger() {
        let lexed = crate::tokens::lex("create function f() returns trigger language plpgsql as $$ begin end $$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_strict_immutable() {
        let lexed = crate::tokens::lex("create function f() returns int immutable strict language sql as 'SELECT 1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_options_reordered() {
        let lexed = crate::tokens::lex("create function f() returns int language sql strict as 'SELECT 1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_in_out_named() {
        let lexed = crate::tokens::lex("create function f(in i int, out j int) returns int as $$ begin return i+1; end $$ language plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_in_out_no_returns() {
        let lexed = crate::tokens::lex("create function f(in i int, out j int) as $$ begin end $$ language plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_setof_record() {
        let lexed = crate::tokens::lex("create function gs(v integer, out a integer, out b integer) returns setof record as $f$ select 1 $f$ language plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_polymorphic_out() {
        let lexed = crate::tokens::lex("create function poly(a anyelement, b anyarray, OUT x anyarray) as $$ begin end $$ language plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_param_eq_default() {
        let lexed = crate::tokens::lex("create function f(a int = 1, b int = 2) returns int as $$ select 1 $$ language sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_param_default_keyword() {
        let lexed = crate::tokens::lex("create function f(a int default 1) returns int as $$ select 1 $$ language sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_unnamed_default() {
        let lexed = crate::tokens::lex("create function dfunc(a int = 1, int = 2) returns int as $$ select 1 $$ language sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_array_arg() {
        let lexed = crate::tokens::lex("CREATE FUNCTION stfnp(int[]) RETURNS int[] AS 'select $1' LANGUAGE SQL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_array_arg_multi() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f(int[], text[]) RETURNS int[] AS 'select $1' LANGUAGE SQL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_nested_array() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f(x int[][]) RETURNS int[][] AS 'select x' LANGUAGE SQL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_multi_named_params() {
        let lexed = crate::tokens::lex("create function tg_hub_adjustslots(hname bpchar, oldn integer, newn integer) returns integer as ' begin return 1; end ' language plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn func_body_dollar_quoted() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM 1; END; $$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "plpgsql");
        assert_eq!(body.body.trim(), "BEGIN PERFORM 1; END;");
    }

    #[test]
    fn func_body_single_quoted() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS int AS 'SELECT 1' LANGUAGE sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "sql");
        assert_eq!(body.body, "SELECT 1");
    }

    #[test]
    fn func_body_tagged_dollar_quote() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $proc$ DECLARE x int; BEGIN x := 1; END; $proc$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "plpgsql");
        assert_eq!(body.body.trim(), "DECLARE x int; BEGIN x := 1; END;");
    }

    #[test]
    fn func_returns_table() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f(int) RETURNS TABLE(a int, b int) AS $$ BEGIN RETURN QUERY SELECT 1, 2; END; $$ LANGUAGE plpgsql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn func_returns_table_varchar() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS TABLE(a varchar(5)) AS $$ SELECT 'hello'::varchar(5) $$ LANGUAGE sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
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
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS integer AS 'select 1;' LANGUAGE SQL SET datestyle to iso, mdy");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// Multiple `SET` options on a single CREATE FUNCTION — each is its own
    /// `createfunc_opt_item`. The rules.sql regression chains five of them.
    #[test]
    fn parse_create_function_multiple_set_options() {
        let lexed = crate::tokens::lex("CREATE FUNCTION f() RETURNS integer AS 'select 1;' LANGUAGE SQL \
             SET search_path TO PG_CATALOG \
             SET extra_float_digits TO 2 \
             SET work_mem TO '4MB' \
             SET datestyle to iso, mdy \
             SET local_preload_libraries TO ''");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
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
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterFuncParallelMode {
    #[tok(SAFE)] Safe,
    #[tok(RESTRICTED)] Restricted,
    #[tok(UNSAFE)] Unsafe,
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
    #[tok(DEFINER)] Definer,
    #[tok(INVOKER)] Invoker,
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
    #[tok(CALLED, ON, NULL, INPUT)] CalledOnNullInput,
    #[tok(RETURNS, NULL, ON, NULL, INPUT)] ReturnsNullOnNullInput,
    ExternalSecurity(AlterFuncExternalSecurityItem),
    Security(AlterFuncSecurityItem),
    #[tok(NOT, LEAKPROOF)] NotLeakproof,
    Parallel(AlterFuncParallelItem),
    Cost(AlterFuncCostItem<'input>),
    Rows(AlterFuncRowsItem<'input>),
    Support(AlterFuncSupportItem<'input>),
    // `SET name = value` and `RESET name | RESET ALL` — the
    // FunctionSetResetClause branch of common_func_opt_item.
    Set(crate::ast::session::set_reset::SetStmt<'input>),
    Reset(crate::ast::session::set_reset::ResetStmt<'input>),
    // Single-keyword forms.
    #[tok(LEAKPROOF)] Leakproof,
    #[tok(STRICT)] Strict,
    #[tok(IMMUTABLE)] Immutable,
    #[tok(STABLE)] Stable,
    #[tok(VOLATILE)] Volatile,
}

/// `common_func_opt_item …+ [RESTRICT]` — the action-list branch of
/// `ALTER FUNCTION / PROCEDURE / ROUTINE`. At least one option is
/// required (gram.y's `alterfunc_opt_list` is right-recursive with
/// `common_func_opt_item` as the base case, not empty). The trailing
/// `RESTRICT` is gram.y's deprecated `opt_restrict`, present for SQL
/// compliance and ignored semantically.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterFuncOptions<'input> {
    pub items: recursa::Vec1<CommonFuncOptItem<'input>  >,
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
