/// SQL expression AST with derived Pratt parsing for operator precedence.
///
/// Handles atoms, prefix (NOT, unary minus), infix (AND, OR, comparisons,
/// arithmetic), and postfix operators (::type cast, IS [NOT] TRUE/FALSE/UNKNOWN/NULL,
/// IN (list)).
use recursa::seq::{OptionalTrailing, Seq0, Seq1};
use recursa_diagram::railroad;

use crate::ast::dml::values::Subquery;
use crate::tokens::{literal, punct};

use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
/// One or more adjacent string literals, concatenated by Postgres into a
/// single value: `'first' ' - next' 'third'`.
///
/// PostgreSQL only concatenates two adjacent string literals when the gap
/// between them contains **no comment**. A continuation therefore stops as
/// soon as a `/* … */` or `-- …` comment appears between two string parts.
///
/// `FormatTokens` / `Visit` are still derived — the field is an ordinary
/// `Seq1`, so the printer emits a break between parts exactly as for any
/// other separated list. Only `Parse` is hand-rolled (see below).
#[derive(recursa::Node, Debug, Clone)]
pub struct StringLitSeq0<'input> {
    pub parts: recursa::Vec1<literal::StringLit<'input>  >,
}


/// Content inside IN parentheses: either a subquery or expression list.
///
/// Variant ordering: `Exprs` is declared FIRST so the `(` first-set token
/// dispatches to the expression-list path. PG's `in_expr` is either
/// `select_with_parens` or `'(' expr_list ')'`; a leading `(` in the IN
/// content is almost always a `ParenExpr` element of an expression list
/// (e.g. `b IN ((select 1), (select 2))` in partition_prune.sql).
///
/// The bare `Subquery` branch still wins on its non-`(` leading tokens
/// (`SELECT`, `VALUES`, `TABLE`, `WITH`), since the first-set tree routes
/// those tokens unambiguously to `Subquery`.
///
/// Trade-off: a single `IN ((SELECT 1) UNION SELECT 2)` — where the IN
/// content is one parenthesised subquery WITH a `UNION`/`EXCEPT` set-op —
/// is no longer reachable, because `Exprs` consumes `(SELECT 1)` as one
/// `ParenExpr` element and then bails on `UNION`. The PG 17.9 regression
/// corpus does not exercise that form inside `IN (...)` (only inside JOIN
/// `(<...>) AS alias`), so the trade is worth the partition_prune Skip
/// fix. If a real workload needs it, model `(subquery)<setop><subquery>`
/// as a dedicated `InContent` variant declared before `Exprs`.
#[derive(recursa::Node, Debug, Clone)]
pub enum InContent<'input> {
    Exprs(#[sep(COMMA)] Vec<Expr<'input> >),
    Subquery(Box<Subquery<'input>>),
}

/// `IN (expr, ...)` or `IN (subquery)` postfix suffix.
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
pub struct InList<'input>(#[tok(LPAREN, this, RPAREN)] #[deref] pub  InContent<'input> );

/// A single typmod argument: an optionally-signed integer literal. Postgres'
/// gram.y allows `expr_list` here, but the corpus only exercises signed
/// integers (e.g. `numeric(3, -6)` in numeric.sql), so we model only that
/// shape. A leading `+` or `-` is permitted to mirror PG's behavior.
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform)]
pub struct TypeModifierArg<'input> {
    pub sign: Option<TypeModifierSign>,
    pub value: literal::IntegerLit<'input>,
}

/// Leading sign of a typmod argument.
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform)]
pub enum TypeModifierSign {
    #[tok(MINUS)] Neg,
    #[tok(PLUS)] Pos,
}

/// Parenthesized precision/scale for type names: `(10,2)`, `(3)`, `(3,-6)`.
#[railroad(label = "<Precision>")]
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform, derive_more::Deref)]
pub struct TypePrecision<'input>(
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    #[deref]
    pub   Vec<TypeModifierArg<'input> > ,
);

/// Type name for casts.
#[railroad(label = "<Type Name>")]
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform)]
pub enum TypeName<'input> {
    #[tok(BOOL)] Bool,
    #[tok(BOOLEAN)] Boolean,
    #[tok(TEXT)] Text,
    #[tok(INTEGER)] Integer,
    #[tok(INT)] Int,
    #[tok(SERIAL)] Serial,
    #[tok(NUMERIC)] Numeric,
    #[tok(VARCHAR)] Varchar,
    #[tok(DOUBLE, PRECISION)] /// `DOUBLE PRECISION` — two-keyword type. Listed before `Ident` so the
    /// DOUBLE match isn't accidentally consumed as a plain identifier.
    DoublePrecision,
    #[tok(TIMESTAMP)] /// `TIMESTAMP` (optional `WITH/WITHOUT TIME ZONE` qualifier handled
    /// at the `CastType` level so precision can sit between).
    Timestamp,
    #[tok(TIME)] /// `TIME` — same shape as `TIMESTAMP`.
    Time,
    #[tok(INTERVAL)] /// `INTERVAL` — qualifier (`YEAR TO MONTH` etc.) is currently not
    /// modeled at the type level; only the bare keyword is consumed.
    Interval,
    #[tok(BIT)] /// `BIT` and `BIT VARYING` (the optional `VARYING` modifier is handled
    /// at the `CastType` level).
    Bit,
    #[tok(CHARACTER)] /// `CHARACTER` and `CHARACTER VARYING` — same shape as `BIT`.
    Character,
    #[tok(UNKNOWN)] /// `UNKNOWN` — pseudo-type used for untyped literals; reserved keyword so
    /// it must be matched explicitly rather than falling through to `Ident`.
    Unknown,
    /// Qualified type name (`schema.type`) or a bare identifier.
    Ident(crate::ast::shared::names::QualifiedName<'input>),
}

/// Boolean test suffix: the part after `IS` in `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`.
///
/// NOT variants are listed first so the combined peek regex disambiguates
/// via longest match (e.g., `NOT TRUE` is longer than `TRUE`).
#[derive(recursa::Node, Debug, Clone)]
pub enum BoolTestKind {
    #[tok(NOT, TRUE)] IsNotTrue,
    #[tok(NOT, FALSE)] IsNotFalse,
    #[tok(NOT, UNKNOWN)] IsNotUnknown,
    #[tok(NOT, NULL)] IsNotNull,
    #[tok(TRUE)] IsTrue,
    #[tok(FALSE)] IsFalse,
    #[tok(UNKNOWN)] IsUnknown,
    #[tok(NULL)] IsNull,
}

/// Unicode normalisation form keyword — gram.y `unicode_normal_form`.
/// Used by `expr IS [NOT] [NFx] NORMALIZED` and `NORMALIZE(expr, NFx)`.
#[derive(recursa::Node, Debug, Clone)]
pub enum UnicodeNormalForm {
    #[tok(NFKC)] Nfkc,
    #[tok(NFKD)] Nfkd,
    #[tok(NFC)] Nfc,
    #[tok(NFD)] Nfd,
}

/// Tail of `expr IS [NOT] [NFx] NORMALIZED` — the `[NOT] [NFx] NORMALIZED`
/// part after the leading `IS`. Modelled as an enum so the postfix-Pratt
/// `IsNormalized(_, IS, IsNormalizedTail)` can dispatch on the second token.
///
/// Variant ordering: NOT-leading forms first (longer prefix), and within
/// each NOT/non-NOT bucket the form-prefixed variants come before the bare
/// `NORMALIZED` so the peek regex prefers the longer match.
#[derive(recursa::Node, Debug, Clone)]
pub enum IsNormalizedTail {
    NotForm(IsNotFormNormalizedTail),
    #[tok(NOT, NORMALIZED)] Not,
    Form(IsFormNormalizedTail),
    #[tok(NORMALIZED)] Plain,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct IsFormNormalizedTail {
    #[tok(this, NORMALIZED)]
    pub form: UnicodeNormalForm,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct IsNotFormNormalizedTail {
    #[tok(NOT, this, NORMALIZED)]
    pub form: UnicodeNormalForm,
}

// --- Atom wrapper structs ---

/// Qualified column reference: `table.column`
///
/// Uses AliasName for the table part to allow keywords like EXCLUDED, NEW, OLD.
#[derive(recursa::Node, Debug, Clone)]
pub struct QualifiedRef<'input> {
    pub table: literal::AliasName<'input>,
    #[tok(DOT, this)]
    pub column: literal::AliasName<'input>,
}

/// Qualified wildcard: `table.*`
#[derive(recursa::Node, Debug, Clone)]
pub struct QualifiedWildcard<'input> {
    #[tok(this, DOT, STAR)]
    pub table: literal::AliasName<'input>,
}

/// Window specification: `OVER window_name` or `OVER (inline_spec)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowSpec<'input> {
    #[tok(OVER, this)]
    pub body: WindowSpecBody<'input>,
}

/// Body of an OVER clause.
///
/// Variant ordering: Inline (starts with `(`) before Named (starts with an
/// identifier). They start with different tokens so peek disambiguation is
/// trivial.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowSpecBody<'input> {
    Inline(#[tok(LPAREN, this, RPAREN)]  InlineWindowSpec<'input> ),
    Named(crate::tokens::ColId<'input>),
}

/// Interior of an inline window spec (between the parens).
///
/// The optional `ref_name` is an existing-window reference (e.g.
/// `WINDOW w2 AS (w1 ORDER BY x)`). It relies on `Option<literal::Ident>`
/// peek-disambiguating cleanly against `PARTITION`/`ORDER`/`ROWS`/etc.
/// because keywords are rejected by `literal::Ident`.
#[derive(recursa::Node, Debug, Clone)]
pub struct InlineWindowSpec<'input> {
    pub ref_name: Option<literal::WindowRefNameIdent<'input>>,
    pub partition_by: Option<WindowPartitionBy<'input>>,
    pub order_by: Option<crate::ast::dml::select::OrderByClause<'input>>,
    pub frame: Option<WindowFrameClause<'input>>,
}

/// PARTITION BY in window: `PARTITION BY expr, ...`
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowPartitionBy<'input> {
    #[tok(PARTITION, BY, this)]
    #[sep(COMMA)]
    pub exprs: Vec<Expr<'input> >,
}

/// Frame unit: `ROWS | RANGE | GROUPS`.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameUnit {
    #[tok(ROWS)] Rows,
    #[tok(RANGE)] Range,
    #[tok(GROUPS)] Groups,
}

/// `WINDOW` frame clause: `unit BETWEEN start AND end [EXCLUDE ...]`
/// or `unit start`.
///
/// Variant ordering: `Between` (starts with `unit BETWEEN`) before `Single`
/// (starts with `unit <bound>`). Longest-match-wins.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameClause<'input> {
    Between(WindowFrameBetween<'input>),
    Single(WindowFrameSingle<'input>),
}

/// `unit BETWEEN start AND end [EXCLUDE ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameBetween<'input> {
    pub unit: WindowFrameUnit,
    #[tok(BETWEEN, this)]
    pub start: WindowFrameBound<'input>,
    #[tok(AND, this)]
    pub end: WindowFrameBound<'input>,
    pub exclude: Option<WindowFrameExclude>,
}

/// `unit start [EXCLUDE ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameSingle<'input> {
    pub unit: WindowFrameUnit,
    pub bound: WindowFrameBound<'input>,
    pub exclude: Option<WindowFrameExclude>,
}

/// A single frame bound.
///
/// Variant ordering: two-token forms first (`UNBOUNDED PRECEDING`,
/// `CURRENT ROW`, `UNBOUNDED FOLLOWING`), then the expr-prefixed forms
/// (`expr PRECEDING` / `expr FOLLOWING`). The expr forms start with an
/// expression and can't be confused with keyword-prefixed forms.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameBound<'input> {
    #[tok(UNBOUNDED, PRECEDING)] UnboundedPreceding,
    #[tok(UNBOUNDED, FOLLOWING)] UnboundedFollowing,
    #[tok(CURRENT, ROW)] CurrentRow,
    ExprPreceding(ExprPreceding<'input>),
    ExprFollowing(ExprFollowing<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct ExprPreceding<'input> {
    #[tok(this, PRECEDING)]
    pub expr: Box<Expr<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct ExprFollowing<'input> {
    #[tok(this, FOLLOWING)]
    pub expr: Box<Expr<'input>>,
}

/// `EXCLUDE { CURRENT ROW | GROUP | TIES | NO OTHERS }` frame exclusion.
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameExclude {
    #[tok(EXCLUDE, this)]
    pub target: WindowFrameExcludeTarget,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameExcludeTarget {
    #[tok(CURRENT, ROW)] CurrentRow,
    #[tok(GROUP)] Group,
    #[tok(TIES)] Ties,
    #[tok(NO, OTHERS)] NoOthers,
}

/// Function call: `name(arg1, arg2, ...)`
///
/// Keeps explicit `lparen` field rather than using `Surrounded` because the
/// derive macro chains `IS_TERMINAL` fields for `first_pattern` — the
/// `Ident + LParen` pattern is what disambiguates `FuncCall` from a plain
/// `Ident` in `TableRef` enum lookahead.
///
/// Function argument: optionally prefixed with `VARIADIC`.
///
/// Variant ordering: `Variadic` before `Plain` since `VARIADIC` keyword is
/// longer than starting an expression.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncArg<'input> {
    Named(NamedFuncArg<'input>),
    Variadic(VariadicArg<'input>),
    Plain(Box<Expr<'input>>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct VariadicArg<'input> {
    #[tok(VARIADIC, this)]
    pub value: Box<Expr<'input>>,
}

/// `=>` or `:=` — the two named-argument operators PostgreSQL accepts.
///
/// Variant ordering: both are distinct two-character punctuation tokens,
/// no ambiguity.
#[derive(recursa::Node, Debug, Clone)]
pub enum NamedArgOp {
    #[tok(FATARROW)] FatArrow,
    #[tok(COLONEQUALS)] ColonEquals,
}

/// Named function argument: `name => value` or `name := value` (Postgres).
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedFuncArg<'input> {
    pub name: literal::AliasName<'input>,
    pub arrow: NamedArgOp,
    pub value: Box<Expr<'input>>,
}

/// `WITHIN GROUP (ORDER BY ...)` clause for ordered-set aggregate functions.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithinGroupClause<'input> {
    #[tok(WITHIN, GROUP, LPAREN, this, RPAREN)]
    pub order_by:

        Box<crate::ast::dml::select::OrderByClause<'input>>

    ,
}

/// `FILTER (WHERE condition)` clause for filtered aggregates.
#[derive(recursa::Node, Debug, Clone)]
pub struct FilterClause<'input> {
    #[tok(FILTER, LPAREN, this, RPAREN)]
    pub body:
         Box<crate::ast::dml::select::WhereClause<'input>> ,
}

/// Function-name token: a regular qualified name OR one of the reserved
/// keywords PG accepts in `func_name` position.
///
/// PG's `func_name: type_function_name | ColId indirection` permits the
/// `type_func_name_keyword`s `LEFT` and `RIGHT` as unqualified function
/// names; `SET` is `unreserved_keyword` per kwlist.h and is therefore also
/// a legal `ColId` (function name), even though pg-sql keeps `SET` reserved
/// at the token level to disambiguate `UPDATE … SET …` from an
/// UPDATE-target-alias.
///
/// Listed here so a function call site reclaims these spellings — without
/// these the call would be lexed as the bare reserved keyword and refuse
/// to enter the function-call grammar.
///
/// Variant ordering: keyword variants first so their `LEFT(`/`RIGHT(` /
/// `SET(` form is matched before the generic `Ident(` fallback consumes
/// the parens as the start of a function call against a now-quoted
/// identifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncCallName<'input> {
    #[tok(LEFT)] Left,
    #[tok(RIGHT)] Right,
    #[tok(SET)] Set,
    Name(crate::ast::shared::names::QualifiedName<'input>),
}

/// Function call: `name([*] [DISTINCT] args [ORDER BY ...]) [WITHIN GROUP (...)] [FILTER (...)] [OVER (...)]`
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncCall<'input> {
    pub name: FuncCallName<'input>,
    #[tok(LPAREN, this)]
    #[presence(STAR)]
    pub star_arg: bool,
    #[presence(DISTINCT)]
    pub distinct: bool,
    #[sep(COMMA)]
    pub args: Vec<FuncArg<'input> >,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    #[tok(RPAREN, this)]
    pub within_group: Option<WithinGroupClause<'input>>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// The single-token name of a `QuotedFuncCall`: a `QuotedIdent` (`"foo"`)
/// or a `UnicodeQuotedIdent` (`U&"foo"`). Declared as a dedicated enum (not
/// `literal::Ident`) so the firstset extractor sees a fully-covered set
/// of token kinds and the Pratt nud kind-match can register
/// `Expr::QuotedFunc` under both QuotedIdent and UnicodeQuotedIdent.
///
/// `literal::Ident` is rejected here because its `Unquoted` variant uses
/// the `UnquotedIdent` token whose `Parse` is hand-written (it accepts a
/// dynamic set of soft-keyword kinds). The firstset extractor sees only
/// `QuotedIdent` and `UnicodeQuotedIdent` in `Ident`'s tree, so `Ident`
/// is "not fully covered" and any struct field of type `Ident` ends up
/// `Opaque` for the parent's first-set walk.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuotedFuncName<'input> {
    UnicodeQuoted(crate::tokens::UnicodeQuotedIdent<'input>),
    Quoted(crate::tokens::literal::QuotedIdent<'input>),
}

/// `"name"(...)` — function call whose name is a single quoted identifier.
///
/// Declared as a dedicated atom (and Pratt atom variant in `Expr`) so the
/// Pratt nud kind-match can register the function-call dispatch under the
/// `QuotedIdent` and `UnicodeQuotedIdent` token kinds — without this, the
/// kind-match arm for `QuotedIdent` commits to `Expr::ColumnRef` (the only
/// atom whose first-set token covers it cleanly) and never falls through
/// to the sequential `Expr::Func` arm. Quoted-ident function calls cannot
/// have a `schema.func` form (the dot-qualified case still routes through
/// `FuncCallName::Name` via the sequential `Expr::Func` fallback).
#[derive(recursa::Node, Debug, Clone)]
pub struct QuotedFuncCall<'input> {
    pub name: QuotedFuncName<'input>,
    #[tok(LPAREN, this)]
    #[presence(STAR)]
    pub star_arg: bool,
    #[presence(DISTINCT)]
    pub distinct: bool,
    #[sep(COMMA)]
    pub args: Vec<FuncArg<'input> >,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    #[tok(RPAREN, this)]
    pub within_group: Option<WithinGroupClause<'input>>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// A trailing `::cast` chain — one or more postfix casts applied to a
/// preceding value. Used by `CastedSubquery` to absorb `(SubSelect)::Typename`
/// where the cast belongs structurally to the parenthesised value but cannot
/// be reached via the ordinary `Subquery` variant of `ParenContent` (which
/// stops at the close paren and would strand `::cast`).
#[derive(recursa::Node, Debug, Clone)]
pub struct CastTail<'input> {
    #[tok(COLONCOLON, this)]
    pub ty: Box<CastType<'input>>,
}

/// `(SubSelect)::Typename [::Typename ...]` — a parenthesised subquery with
/// one or more trailing casts. Declared as a separate `ParenContent` variant
/// so it can be tried BEFORE the bare `Subquery` variant: without it, the
/// `Subquery::Paren` variant matches `(SubSelect)` greedily and strands
/// `::Typename` for the outer close-paren, which then fails. The single
/// required `cast` enforces a trailing `::Typename` for this variant —
/// without it parsing falls through to bare `Subquery`. Chained casts
/// (`(SubSelect)::int::text`) are captured in `extra_casts`, modelled
/// directly as a `Vec`-like loop via `OptionalTrailing` over `CastTail` so the
/// recursa derive can detect each subsequent `::`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CastedSubquery<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub subquery:  Box<Subquery<'input>> ,
    /// At least one cast is required — without it parsing falls through to
    /// bare `Subquery` (the variant declared after `CastedSubquery`).
    pub cast: CastTail<'input>,
    /// Chained casts (`(SubSelect)::int::text`), zero or more.
    pub extra_casts: Vec<CastTail<'input>>,
}

/// Content inside parentheses: either a casted subquery, a bare subquery, or
/// a comma-separated expression list.
///
/// Variant ordering:
/// - `CastedSubquery` first — matches `(SubSelect)::cast` as one unit so the
///   trailing `::cast` is not stranded for the outer close-paren.
/// - `Subquery` second — matches `(SELECT ...)`, `(SELECT ...) UNION ...`,
///   bare `SELECT ...`, `VALUES ...`, `TABLE ...`, `WITH ...` when no
///   trailing cast follows.
/// - `Exprs` last — anything else parses as a Pratt expression list.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParenContent<'input> {
    CastedSubquery(CastedSubquery<'input>),
    Subquery(Box<Subquery<'input>>),
    Exprs(#[sep(COMMA)] Vec<Expr<'input> >),
}

/// Parenthesized expression: `(expr)`, `(expr, expr, ...)`, or `(SELECT/VALUES ...)`
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
pub struct ParenExpr<'input>(
    #[tok(LPAREN, this, RPAREN)]
    #[deref] pub  ParenContent<'input> ,
);

/// Array slice content: `lower : upper`, `: upper`, `lower :`, or `:`.
///
/// Both bounds are optional; the colon is required.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubscriptSlice<'input> {
    pub lower: Option<Box<Expr<'input>>>,
    #[tok(COLON, this)]
    pub upper: Option<Box<Expr<'input>>>,
}

/// `.field` accessor in an indirection chain.
#[derive(recursa::Node, Debug, Clone)]
pub struct IndirectionField<'input> {
    #[tok(DOT, this)]
    pub name: literal::AliasName<'input>,
}

/// One element of an indirection chain on an `INSERT` / `UPDATE` column
/// target: `[idx]`, `[low:high]`, or `.field` (Postgres `opt_indirection`).
///
/// Variant ordering: `Slice` before `Index` — both open with `[`, the
/// colon-containing slice form is tried first.
#[derive(recursa::Node, Debug, Clone)]
pub enum IndirectionEl<'input> {
    Slice(#[tok(LBRACKET, this, RBRACKET)]  SubscriptSlice<'input> ),
    Index(#[tok(LBRACKET, this, RBRACKET)]  Box<Expr<'input>> ),
    Field(IndirectionField<'input>),
}

/// `ANY(expr)` or `ANY(subquery)` — quantified comparison operand.
///
/// Used on the right side of a comparison operator: `x = ANY(array_expr)`
/// or `x = ANY(SELECT ...)`. Also valid as a standalone expression atom.
#[derive(recursa::Node, Debug, Clone)]
pub struct AnyExpr<'input> {
    #[tok(ANY, LPAREN, this, RPAREN)]
    pub content:  ParenContent<'input> ,
}

/// `ALL(expr)` or `ALL(subquery)` — quantified comparison operand.
#[derive(recursa::Node, Debug, Clone)]
pub struct AllExpr<'input> {
    #[tok(ALL, LPAREN, this, RPAREN)]
    pub content:  ParenContent<'input> ,
}

/// `SOME(expr)` or `SOME(subquery)` — synonym for ANY.
#[derive(recursa::Node, Debug, Clone)]
pub struct SomeExpr<'input> {
    #[tok(SOME, LPAREN, this, RPAREN)]
    pub content:  ParenContent<'input> ,
}

/// EXISTS subquery: `EXISTS (SELECT ...)`
#[derive(recursa::Node, Debug, Clone)]
pub struct ExistsExpr<'input> {
    #[tok(EXISTS, LPAREN, this, RPAREN)]
    pub subquery:  Box<Subquery<'input>> ,
}

/// One element of an `ARRAY[...]` constructor: either an ordinary
/// expression or a nested bracketed sub-list (for multi-dimensional
/// literals like `ARRAY[[1,2],[3,4]]`).
///
/// Variant ordering: `Nested` leads with `[`, which no expression atom
/// does, so dispatch is unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum ArrayElement<'input> {
    Nested(#[tok(LBRACKET, this, RBRACKET)] #[sep(COMMA)]  Vec<ArrayElement<'input> > ),
    Expr(Box<Expr<'input>>),
}

/// ARRAY bracket constructor: `ARRAY[expr, ...]`, including the
/// multi-dimensional form `ARRAY[[1,2],[3,4]]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ArrayBracket<'input> {
    #[tok(ARRAY, LBRACKET, this, RBRACKET)]
    #[sep(COMMA)]
    pub elements: Vec<ArrayElement<'input> >,
}

/// ARRAY subquery constructor: `ARRAY(subquery)`
#[derive(recursa::Node, Debug, Clone)]
pub struct ArraySubquery<'input> {
    #[tok(ARRAY, LPAREN, this, RPAREN)]
    pub subquery:  Box<Subquery<'input>> ,
}

/// ARRAY constructor: `ARRAY[expr, ...]` or `ARRAY(subquery)`
///
/// Variant ordering: Bracket (`ARRAY[`) has a longer first_pattern than
/// Subquery (`ARRAY(`) because `[` is a different token than `(`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ArrayExpr<'input> {
    Bracket(ArrayBracket<'input>),
    Subquery(ArraySubquery<'input>),
}

/// ROW constructor: `ROW(expr, ...)`
#[derive(recursa::Node, Debug, Clone)]
pub struct RowExpr<'input> {
    #[tok(ROW, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub values:  Vec<Expr<'input> > ,
}

/// `WHEN cond THEN result` arm of a CASE expression.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseWhenArm<'input> {
    #[tok(WHEN, this)]
    pub condition: Box<Expr<'input>>,
    #[tok(THEN, this)]
    pub result: Box<Expr<'input>>,
}

/// `ELSE result` clause of a CASE expression.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseElse<'input> {
    #[tok(ELSE, this)]
    pub result: Box<Expr<'input>>,
}

/// Searched CASE: `CASE WHEN cond THEN result [...] [ELSE result] END`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseSearched<'input> {
    #[tok(CASE, this)]
    pub first_arm: CaseWhenArm<'input>,
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    #[tok(this, END)]
    pub else_clause: Option<CaseElse<'input>>,
}

/// Simple CASE: `CASE operand WHEN val THEN result [...] [ELSE result] END`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseSimple<'input> {
    #[tok(CASE, this)]
    pub operand: Box<Expr<'input>>,
    pub first_arm: CaseWhenArm<'input>,
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    #[tok(this, END)]
    pub else_clause: Option<CaseElse<'input>>,
}

/// CASE expression: searched form (first, since `CASE WHEN` is a longer
/// specific prefix than `CASE` followed by any expression) or simple form.
#[derive(recursa::Node, Debug, Clone)]
pub enum CaseExpr<'input> {
    Searched(CaseSearched<'input>),
    Simple(CaseSimple<'input>),
}

/// One `opt_array_bounds` element: `[]` or `[N]`.
///
/// Postgres syntax: `Typename opt_array_bounds` allows arbitrary repetition
/// of either form (`int4[]`, `int4[1]`, `varchar(4)[2][3]`, …). Variant
/// ordering: `Sized` (`[N]`, 3 tokens) before `Empty` (`[]`, 2 tokens) so
/// longest-match-wins picks the longer form when an integer literal is
/// present between the brackets.
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub enum ArraySuffix<'input> {
    Sized(ArraySuffixSized<'input>),
    Empty(ArraySuffixEmpty),
}

/// `[N]` array bound.
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub struct ArraySuffixSized<'input> {
    #[tok(LBRACKET, this, RBRACKET)]
    pub bounds:  literal::IntegerLit<'input> ,
}

/// `[]` array suffix (unbounded).
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub enum ArraySuffixEmpty { #[tok(LBRACKET, RBRACKET)] Value, }

/// Cast type with optional precision and zero-or-more array suffixes:
/// `numeric(10,0)`, `integer[]`, `int4[][][]`, `varchar(4)[2][3]`.
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub struct CastType<'input> {
    pub base: TypeName<'input>,
    #[presence(VARYING)]
    /// `VARYING` modifier (e.g., `BIT VARYING`, `CHARACTER VARYING`).
    /// Always precedes the precision parens.
    pub varying: bool,
    pub precision: Option<TypePrecision<'input>>,
    /// `WITH/WITHOUT TIME ZONE` qualifier on `TIME`/`TIMESTAMP` types.
    /// Always follows the precision parens.
    pub tz: Option<TimeZoneQualifier>,
    /// Interval qualifier (e.g. `DAY TO MINUTE`, `SECOND(2)`) when the base
    /// type is `INTERVAL`.
    pub interval_qualifier: Option<IntervalQualifier<'input>>,
    pub array_suffixes: Vec<ArraySuffix<'input>>,
    /// PG gram.y also accepts `SimpleTypename ARRAY` and
    /// `SimpleTypename ARRAY '[' Iconst ']'` — the keyword form for
    /// declaring an array type (e.g. `integer ARRAY[4]`, `text ARRAY`).
    /// In practice this is mutually exclusive with `array_suffixes`, but the
    /// grammar admits the suffix appearing AFTER the keyword form, so the
    /// field is parsed last.
    pub array_kw_suffix: Option<ArrayKwSuffix<'input>>,
}

/// `ARRAY` or `ARRAY[N]` post-type-name array suffix
/// (PG gram.y: `SimpleTypename ARRAY | SimpleTypename ARRAY '[' Iconst ']'`).
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub struct ArrayKwSuffix<'input> {
    #[tok(ARRAY, this)]
    pub bound: Option<ArraySuffixSized<'input>>,
}

/// NOT IN list: `expr NOT IN (val, ...)` suffix.
#[derive(recursa::Node, Debug, Clone)]
pub struct NotInSuffix<'input> {
    #[tok(NOT, IN, this)]
    pub list: InList<'input>,
}

/// Payload for function-style type cast: either a string literal (common
/// case `bool 'value'`) or a psql client variable substitution
/// (`bigint :'txid_current'`).
#[derive(recursa::Node, Debug, Clone)]
pub enum TypeCastValue<'input> {
    String(literal::StringLit<'input>),
    PsqlVar(literal::PsqlVariable<'input>),
}

/// Function-style type cast: `bool 'value'`, `text 'hello'`, `char(20) 'text'`,
/// `bigint :'var'`. Uses `CastType` (not bare `TypeName`) to support precision.
#[derive(recursa::Node, Debug, Clone)]
pub struct TypeCastFunc<'input> {
    pub type_name: CastType<'input>,
    pub value: TypeCastValue<'input>,
}

/// `WITH TIME ZONE` or `WITHOUT TIME ZONE` suffix for `TIMESTAMP`/`TIME`.
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub enum TimeZoneQualifier {
    #[tok(WITH, TIME, ZONE)] With,
    #[tok(WITHOUT, TIME, ZONE)] Without,
}

/// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TimestampLit<'input> {
    #[tok(TIMESTAMP, this)]
    /// Optional precision, e.g., `timestamp(6)`.
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub value: literal::StringLit<'input>,
}

/// `TIME [WITH|WITHOUT TIME ZONE] 'string'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TimeLit<'input> {
    #[tok(TIME, this)]
    /// Optional precision, e.g., `time(2)`.
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub value: literal::StringLit<'input>,
}

/// `SECOND [(p)]` — the SECOND keyword with optional fractional-second
/// precision. Used in interval qualifiers like `SECOND(2)` or
/// `DAY TO SECOND(2)`.
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub struct SecondWithPrecision<'input> {
    #[tok(SECOND, this)]
    pub precision: Option<TypePrecision<'input>>,
}

/// Optional qualifier after `INTERVAL 'str'`.
///
/// Variant ordering: multi-keyword `X TO Y` forms must come before the
/// single-keyword forms so longest-match-wins picks the fuller qualifier
/// when available. `*ToSecond` variants use `SecondWithPrecision` which
/// allows optional `(p)` precision.
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
pub enum IntervalQualifier<'input> {
    #[tok(YEAR, TO, MONTH)] YearToMonth,
    #[tok(DAY, TO, HOUR)] DayToHour,
    #[tok(DAY, TO, MINUTE)] DayToMinute,
    DayToSecond(#[tok(DAY, TO, this)] SecondWithPrecision<'input>),
    #[tok(HOUR, TO, MINUTE)] HourToMinute,
    HourToSecond(#[tok(HOUR, TO, this)] SecondWithPrecision<'input>),
    MinuteToSecond(#[tok(MINUTE, TO, this)] SecondWithPrecision<'input>),
    #[tok(YEAR)] Year,
    #[tok(MONTH)] Month,
    #[tok(DAY)] Day,
    #[tok(HOUR)] Hour,
    #[tok(MINUTE)] Minute,
    Second(SecondWithPrecision<'input>),
}

/// `INTERVAL 'str' [qualifier]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IntervalLit<'input> {
    #[tok(INTERVAL, this)]
    /// Optional precision, e.g. `interval(2)` or `interval(0)`.
    pub precision: Option<TypePrecision<'input>>,
    pub value: literal::StringLit<'input>,
    pub qualifier: Option<IntervalQualifier<'input>>,
}

// --- XML function atoms ---
//
// Postgres `xmlelement` / `xmlattributes` / `xmlforest` use special syntax
// that does not fit a plain `FuncCall` (positional comma-separated exprs):
//
//   xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])
//   xmlattributes(expr [AS alias] [, ...])
//   xmlforest(expr [AS alias] [, ...])
//
// They are modeled here as dedicated atoms declared before `FuncCall`.

/// A `name [AS alias]` argument to `xmlattributes` / `xmlforest`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlNamedArg<'input> {
    pub value: Box<Expr<'input>>,
    pub alias: Option<XmlNamedArgAlias<'input>>,
}

/// `AS alias` suffix on an XML named argument.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlNamedArgAlias<'input> {
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
}

/// `xmlattributes(expr [AS alias], ...)` — used as a positional argument
/// to `xmlelement`, but also can be parsed standalone.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlAttributes<'input> {
    #[tok(XMLATTRIBUTES, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:  Vec<XmlNamedArg<'input> > ,
}

/// Optional `, xmlattributes(...) [, content_exprs]` tail of `xmlelement`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlElementAttrsTail<'input> {
    #[tok(COMMA, this)]
    pub attrs: XmlAttributes<'input>,
    pub content: Option<XmlElementContentTail<'input>>,
}

/// Optional `, content_exprs` tail of `xmlelement`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlElementContentTail<'input> {
    #[tok(COMMA, this)]
    #[sep(COMMA)]
    pub exprs: Vec<Expr<'input> >,
}

/// Body of `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
///
/// Variant ordering: the `WithAttrs` form starts with `, xmlattributes(`
/// (longer match) and must be tried before `WithContent` which starts with
/// just `,`. Both trail an `xmlelement(NAME ident` head.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlElementTail<'input> {
    WithAttrs(XmlElementAttrsTail<'input>),
    WithContent(XmlElementContentTail<'input>),
}

/// Inner contents of an `xmlelement(...)` call.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlElementInner<'input> {
    #[tok(NAME, this)]
    pub element_name: literal::AliasName<'input>,
    pub tail: Option<XmlElementTail<'input>>,
}

/// `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlElement<'input> {
    #[tok(XMLELEMENT, LPAREN, this, RPAREN)]
    pub inner:  XmlElementInner<'input> ,
}

/// `xmlforest(expr [AS alias], ...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlForest<'input> {
    #[tok(XMLFOREST, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:  Vec<XmlNamedArg<'input> > ,
}

/// `xmlpi(NAME ident [, content])`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlPi<'input> {
    #[tok(XMLPI, LPAREN, this, RPAREN)]
    pub inner:  XmlPiInner<'input> ,
}

/// Inner contents of an `xmlpi(...)` call.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlPiInner<'input> {
    #[tok(NAME, this)]
    pub target: literal::AliasName<'input>,
    pub content: Option<XmlPiContentTail<'input>>,
}

/// Optional `, content_expr` tail of `xmlpi`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlPiContentTail<'input> {
    #[tok(COMMA, this)]
    pub expr: Box<Expr<'input>>,
}

// --- More XML function atoms: XMLSERIALIZE / XMLPARSE / XMLROOT / XMLEXISTS ---
//
// Like `xmlelement` etc. these use keyword-laced syntax (`DOCUMENT`/`CONTENT`,
// `VERSION`, `PASSING BY REF`, …) that a plain `FuncCall` cannot express.

/// `DOCUMENT` / `CONTENT` — the XML value category in `XMLSERIALIZE` / `XMLPARSE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlDocOrContent {
    #[tok(DOCUMENT)] Document,
    #[tok(CONTENT)] Content,
}

/// `INDENT` / `NO INDENT` — output indentation option of `XMLSERIALIZE`.
///
/// Variant ordering: `NoIndent` (`NO INDENT`, two tokens) before `Indent`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlIndentOption {
    #[tok(NO, INDENT)] NoIndent,
    #[tok(INDENT)] Indent,
}

/// Inner of `XMLSERIALIZE ( {DOCUMENT|CONTENT} ‹expr› AS ‹type› [[NO] INDENT] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlSerializeInner<'input> {
    pub which: XmlDocOrContent,
    pub value: Box<Expr<'input>>,
    #[tok(AS, this)]
    pub ty: CastType<'input>,
    pub indent: Option<XmlIndentOption>,
}

/// `XMLSERIALIZE ( {DOCUMENT|CONTENT} ‹expr› AS ‹type› [[NO] INDENT] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlSerialize<'input> {
    #[tok(XMLSERIALIZE, LPAREN, this, RPAREN)]
    pub inner:  XmlSerializeInner<'input> ,
}

/// Inner of `XMLPARSE ( {DOCUMENT|CONTENT} ‹expr› )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlParseInner<'input> {
    pub which: XmlDocOrContent,
    pub value: Box<Expr<'input>>,
}

/// `XMLPARSE ( {DOCUMENT|CONTENT} ‹expr› )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlParse<'input> {
    #[tok(XMLPARSE, LPAREN, this, RPAREN)]
    pub inner:  XmlParseInner<'input> ,
}

/// `VERSION {‹expr› | NO VALUE}` — the version argument of `XMLROOT`.
///
/// Variant ordering: `NoValue` (`NO VALUE`) before the catch-all `Expr`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlVersionValue<'input> {
    #[tok(NO, VALUE)] NoValue,
    Expr(Box<Expr<'input>>),
}

/// `VERSION {…}` clause of `XMLROOT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlRootVersion<'input> {
    #[tok(VERSION, this)]
    pub value: XmlVersionValue<'input>,
}

/// `STANDALONE {YES | NO [VALUE]}`.
///
/// Variant ordering: `NoValue` (`NO VALUE`) before bare `No`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlStandaloneValue {
    #[tok(YES)] Yes,
    #[tok(NO, VALUE)] NoValue,
    #[tok(NO)] No,
}

/// `, STANDALONE {…}` clause of `XMLROOT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlRootStandalone {
    #[tok(COMMA, STANDALONE, this)]
    pub value: XmlStandaloneValue,
}

/// Inner of `XMLROOT ( ‹xml› , VERSION {…} [, STANDALONE {…}] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlRootInner<'input> {
    pub value: Box<Expr<'input>>,
    #[tok(COMMA, this)]
    pub version: XmlRootVersion<'input>,
    pub standalone: Option<XmlRootStandalone>,
}

/// `XMLROOT ( ‹xml› , VERSION {…} [, STANDALONE {…}] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlRoot<'input> {
    #[tok(XMLROOT, LPAREN, this, RPAREN)]
    pub inner:  XmlRootInner<'input> ,
}

/// `BY REF` / `BY VALUE` qualifier of an `XMLEXISTS` / `XMLTABLE` PASSING clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlRefOrValue {
    #[tok(REF)] Ref,
    #[tok(VALUE)] Value,
}

/// `BY {REF|VALUE}` qualifier.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlPassingBy {
    #[tok(BY, this)]
    pub which: XmlRefOrValue,
}

/// Inner of `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlExistsInner<'input> {
    pub xpath: Box<Expr<'input>>,
    #[tok(PASSING, this)]
    pub by_before: Option<XmlPassingBy>,
    pub doc: Box<Expr<'input>>,
    pub by_after: Option<XmlPassingBy>,
}

/// `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlExists<'input> {
    #[tok(XMLEXISTS, LPAREN, this, RPAREN)]
    pub inner:  XmlExistsInner<'input> ,
}

/// The tail of an `IS DOCUMENT` predicate: `[NOT] DOCUMENT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IsDocumentTail {
    #[tok(this, DOCUMENT)]
    #[presence(NOT)]
    pub not: bool,
}

// --- SQL-standard string function atoms ---
//
// TRIM/SUBSTRING/POSITION/OVERLAY use special syntax with FROM/IN/PLACING/FOR
// separators inside parens that don't fit a comma-separated FuncCall.

/// Trim direction: `LEADING | TRAILING | BOTH`.
#[derive(recursa::Node, Debug, Clone)]
pub enum TrimDir {
    #[tok(LEADING)] Leading,
    #[tok(TRAILING)] Trailing,
    #[tok(BOTH)] Both,
}

/// Inside of `TRIM(...)`. Forms per gram.y `trim_list`:
///   `[LEADING|TRAILING|BOTH] [chars] FROM source`  — explicit FROM form
///   `[LEADING|TRAILING|BOTH] expr_list`            — direction + bare args
///                                                    (gram.y `a_expr FROM
///                                                    expr_list | FROM
///                                                    expr_list | expr_list`)
///   (a fully-positional `TRIM(src, chars)` form is left to ordinary FuncCall.)
///
/// `from_args` carries the explicit-FROM tail when present; otherwise
/// `bare_args` carries the bare expression list (single expr in PG's
/// regression corpus, but PG admits multiple).
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimInner<'input> {
    pub dir: Option<TrimDir>,
    pub tail: TrimTail<'input>,
}

/// Tail of `TRIM(...)` after the optional direction keyword.
///
/// Variant ordering: `FromArgs` first because its leading `FROM` token is
/// distinct from any `Expr` atom; `WithChars` second because the `[chars]
/// FROM source` form starts with an Expr; `BareArgs` last as the catch-all
/// `[expr, ...]` (no `FROM`) form for `trim(LEADING ' foo ')` shapes.
#[derive(recursa::Node, Debug, Clone)]
pub enum TrimTail<'input> {
    /// `FROM expr_list` — explicit-FROM, no leading chars.
    FromArgs(TrimFromArgs<'input>),
    /// `chars FROM source` — explicit-FROM with leading chars.
    WithChars(TrimWithChars<'input>),
    /// `expr_list` — no `FROM`, just the source-and-chars expression list.
    BareArgs(#[sep(COMMA)] recursa::Vec1<Expr<'input> >),
}

/// `FROM expr_list` tail of `TRIM(...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimFromArgs<'input> {
    #[tok(FROM, this)]
    #[sep(COMMA)]
    pub args: recursa::Vec1<Expr<'input> >,
}

/// `chars FROM source` tail of `TRIM(...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimWithChars<'input> {
    pub chars: Box<Expr<'input>>,
    #[tok(FROM, this)]
    #[sep(COMMA)]
    pub args: recursa::Vec1<Expr<'input> >,
}

/// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimCall<'input> {
    #[tok(TRIM, LPAREN, this, RPAREN)]
    pub inner:  TrimInner<'input> ,
}

/// `FOR len` suffix in `SUBSTRING(... FROM ... FOR ...)` / `OVERLAY(...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForCount<'input> {
    #[tok(FOR, this)]
    pub count: Box<Expr<'input>>,
}

/// `FROM start [FOR len]` form for SUBSTRING.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringFromFor<'input> {
    #[tok(FROM, this)]
    pub start: Box<Expr<'input>>,
    pub for_count: Option<ForCount<'input>>,
}

/// `SIMILAR pattern ESCAPE escape` form for SUBSTRING.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringSimilar<'input> {
    #[tok(SIMILAR, this)]
    pub pattern: Box<Expr<'input>>,
    #[tok(ESCAPE, this)]
    pub escape: Box<Expr<'input>>,
}

/// Tail of a SUBSTRING call after the source expression.
///
/// Variant ordering: `Similar` (`SIMILAR`) before `FromFor` (`FROM`) — distinct
/// first tokens, so order is not strictly required, but listed by length.
#[derive(recursa::Node, Debug, Clone)]
pub enum SubstringTail<'input> {
    Similar(SubstringSimilar<'input>),
    FromFor(SubstringFromFor<'input>),
    For(ForCount<'input>),
}

/// Inner of `SUBSTRING(...)`: `source` followed by FROM/SIMILAR tail.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringInner<'input> {
    pub source: Box<Expr<'input>>,
    pub tail: SubstringTail<'input>,
}

/// `COLLATION FOR (expr)` — SQL-standard collation introspection.
#[derive(recursa::Node, Debug, Clone)]
pub struct CollationForCall<'input> {
    #[tok(COLLATION, FOR, LPAREN, this, RPAREN)]
    pub arg:  Box<Expr<'input>> ,
}

/// `expr AS cast_type [COLLATE "c"]` — inner of `CAST(...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CastAsInner<'input> {
    pub value: Box<Expr<'input>>,
    #[tok(AS, this)]
    pub target: CastType<'input>,
    pub collate: Option<CollateSuffix<'input>>,
}

/// `COLLATE "name"` suffix appearing after a cast target type.
#[derive(recursa::Node, Debug, Clone)]
pub struct CollateSuffix<'input> {
    #[tok(COLLATE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `CAST(expr AS type [COLLATE "c"])` — SQL-standard cast form.
#[derive(recursa::Node, Debug, Clone)]
pub struct CastCall<'input> {
    #[tok(CAST, LPAREN, this, RPAREN)]
    pub inner:  CastAsInner<'input> ,
}

/// `SUBSTRING(source FROM start [FOR len])` /
/// `SUBSTRING(source SIMILAR pattern ESCAPE escape)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringCall<'input> {
    #[tok(SUBSTRING, LPAREN, this, RPAREN)]
    pub inner:  SubstringInner<'input> ,
}

/// Inner of `POSITION(needle IN haystack)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PositionInner<'input> {
    pub needle: Box<Expr<'input>>,
    #[tok(IN, this)]
    pub haystack: Box<Expr<'input>>,
}

/// `POSITION(needle IN haystack)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PositionCall<'input> {
    #[tok(POSITION, LPAREN, this, RPAREN)]
    pub inner:  PositionInner<'input> ,
}

/// Inner of `OVERLAY(source PLACING new FROM start [FOR len])`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OverlayInner<'input> {
    pub source: Box<Expr<'input>>,
    #[tok(PLACING, this)]
    pub new: Box<Expr<'input>>,
    #[tok(FROM, this)]
    pub start: Box<Expr<'input>>,
    pub for_count: Option<ForCount<'input>>,
}

/// `OVERLAY(source PLACING new FROM start [FOR len])`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OverlayCall<'input> {
    #[tok(OVERLAY, LPAREN, this, RPAREN)]
    pub inner:  OverlayInner<'input> ,
}

/// Field argument of `EXTRACT(field FROM source)`.
///
/// Variant ordering: `StringLit` before `Ident` — string literal has a
/// distinct first token (`'`) so order is not strictly required; listed
/// first to match the Postgres docs ordering.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExtractField<'input> {
    StringLit(StringLitSeq0<'input>),
    Ident(literal::AliasName<'input>),
}

/// Inner of `EXTRACT(field FROM source)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExtractInner<'input> {
    pub field: ExtractField<'input>,
    #[tok(FROM, this)]
    pub source: Box<Expr<'input>>,
}

/// `EXTRACT(field FROM source)` — Postgres-specific function syntax.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExtractCall<'input> {
    #[tok(EXTRACT, LPAREN, this, RPAREN)]
    pub inner:  ExtractInner<'input> ,
}

/// `UESCAPE 'c'` suffix that may follow a `U&'...'` literal.
#[derive(recursa::Node, Debug, Clone)]
pub struct UescapeSuffix<'input> {
    #[tok(UESCAPE, this)]
    pub escape_char: literal::StringLit<'input>,
}

/// `U&'...'` unicode string literal with optional `UESCAPE 'c'` suffix.
#[derive(recursa::Node, Debug, Clone)]
pub struct UnicodeStringLitWithEscape<'input> {
    #[lex(pattern = r"(?i:U)&'(?:[^']|'')*'")]
    pub lit: literal::UnicodeStringLit<'input>,
    pub uescape: Option<UescapeSuffix<'input>>,
}

/// `ESCAPE expr` clause on LIKE / SIMILAR TO / ILIKE operators.
#[derive(recursa::Node, Debug, Clone)]
pub struct EscapeClause<'input> {
    #[tok(ESCAPE, this)]
    pub char: Box<Expr<'input>>,
}

// --- SQL/JSON constructor atoms ---
//
// `JSON()`, `JSON_SCALAR()`, `JSON_SERIALIZE()`, `JSON_OBJECT()` and
// `JSON_ARRAY()` are SQL/JSON *grammar constructs*, not ordinary functions:
// Postgres declares them as `COL_NAME_KEYWORD`s with dedicated `gram.y`
// productions. Their syntax — `FORMAT JSON`, `RETURNING`, `key : value`,
// `KEY`/`VALUE`, `{WITH|WITHOUT} UNIQUE [KEYS]`, `{NULL|ABSENT} ON NULL` —
// cannot be expressed as a function-argument list, so each is modeled as a
// dedicated Pratt atom declared before `Func`.
//
// Legacy lowercase calls (`json_object(text[])`, `json_build_array(...)`)
// are unaffected: the soft keyword classifies as a token, but `FuncCall`
// reclaims it as an identifier (the JSON keywords are soft), so a plain
// comma-separated call falls through to the ordinary `Func` atom.

/// `ENCODING ‹name›` suffix of a `FORMAT JSON` clause (e.g. `ENCODING UTF8`).
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonEncoding<'input> {
    #[tok(ENCODING, this)]
    pub name: literal::AliasName<'input>,
}

/// `FORMAT JSON [ENCODING ‹name›]` — SQL/JSON input/output format specifier.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonFormat<'input> {
    #[tok(FORMAT, JSON, this)]
    pub encoding: Option<JsonEncoding<'input>>,
}

/// `RETURNING ‹data_type› [FORMAT JSON [ENCODING ...]]` — output type clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonReturning<'input> {
    #[tok(RETURNING, this)]
    pub ty: CastType<'input>,
    pub format: Option<JsonFormat<'input>>,
}

/// `WITH` / `WITHOUT` lead-in of a `UNIQUE KEYS` constraint.
#[derive(recursa::Node, Debug, Clone)]
pub enum WithOrWithout {
    #[tok(WITH)] With,
    #[tok(WITHOUT)] Without,
}

/// `{WITH|WITHOUT} UNIQUE [KEYS]` — duplicate-key handling for `JSON()` /
/// `JSON_OBJECT()`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonUniqueKeys {
    #[tok(this, UNIQUE, optional(KEYS))]
    pub with_or_without: WithOrWithout,
}

/// `NULL` / `ABSENT` lead-in of an `ON NULL` clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum NullOrAbsent {
    #[tok(NULL)] Null,
    #[tok(ABSENT)] Absent,
}

/// `{NULL|ABSENT} ON NULL` — null-input handling for `JSON_OBJECT()` /
/// `JSON_ARRAY()`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonOnNull {
    #[tok(this, ON, NULL)]
    pub which: NullOrAbsent,
}

/// Inner contents of `JSON ( ‹expr› [FORMAT JSON ...] [{WITH|WITHOUT} UNIQUE [KEYS]] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonConstructorInner<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
    pub unique: Option<JsonUniqueKeys>,
}

/// `JSON ( ‹expr› [FORMAT JSON ...] [{WITH|WITHOUT} UNIQUE [KEYS]] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonConstructor<'input> {
    #[tok(JSON, LPAREN, this, RPAREN)]
    pub inner:  JsonConstructorInner<'input> ,
}

/// `JSON_SCALAR ( ‹expr› )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonScalar<'input> {
    #[tok(JSON_SCALAR, LPAREN, this, RPAREN)]
    pub inner:  Box<Expr<'input>> ,
}

/// Inner contents of `JSON_SERIALIZE ( ‹expr› [FORMAT JSON ...] [RETURNING ...] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonSerializeInner<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_SERIALIZE ( ‹expr› [FORMAT JSON ...] [RETURNING ‹type› ...] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonSerialize<'input> {
    #[tok(JSON_SERIALIZE, LPAREN, this, RPAREN)]
    pub inner:  JsonSerializeInner<'input> ,
}

/// Key/value separator inside a `JSON_OBJECT` entry: `:` or the `VALUE` keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonKeyValueSep {
    #[tok(COLON)] Colon,
    #[tok(VALUE)] Value,
}

/// One `[KEY] ‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry of `JSON_OBJECT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectEntry<'input> {
    #[tok(optional(KEY), this)]
    pub key: Box<Expr<'input>>,
    pub sep: JsonKeyValueSep,
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
}

/// Inner contents of `JSON_OBJECT`: zero or more entries followed by the
/// optional `ON NULL`, `UNIQUE` and `RETURNING` clauses. The empty form
/// (`JSON_OBJECT()`) and the returning-only form (`JSON_OBJECT(RETURNING ...)`)
/// both fall out of `Seq0` accepting zero entries.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectArgs<'input> {
    // `Option<Seq1<…>>` (not `Seq0`) so the entry list is fork-and-tried:
    // `Expr::peek` is keyword-permissive (the `QualRef` atom leads with a
    // keyword-accepting `AliasName`), so a `Seq0` element gate would
    // over-commit on a trailing `RETURNING`/`)` and then hard-fail. The
    // `Option` swallows that, leaving the cursor for the clauses below.
    #[sep(COMMA)]
    pub entries: Option<recursa::Vec1<JsonObjectEntry<'input> >>,
    pub on_null: Option<JsonOnNull>,
    pub unique: Option<JsonUniqueKeys>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_OBJECT ( [entries] [{NULL|ABSENT} ON NULL] [{WITH|WITHOUT} UNIQUE [KEYS]] [RETURNING ...] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObject<'input> {
    #[tok(JSON_OBJECT, LPAREN, this, RPAREN)]
    pub args:  JsonObjectArgs<'input> ,
}

/// One `‹expr› [FORMAT JSON ...]` element of a `JSON_ARRAY` element list.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayElement<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
}

/// The value part of `JSON_ARRAY`: a subquery or a non-empty element list.
///
/// Both variants are non-nullable (a subquery leads with a query keyword,
/// `Seq1` requires ≥1 element), so the enum is dispatchable. The empty
/// `JSON_ARRAY()` / returning-only forms are handled by wrapping this in
/// `Option` at `JsonArrayArgs::body`.
///
/// Variant ordering: `Query` (leads with `SELECT`/`WITH`/`VALUES`/`TABLE`/`(`)
/// before `Elements` so a subquery is not mis-parsed as a single element.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonArrayBody<'input> {
    Query(Box<Subquery<'input>>),
    Elements(#[sep(COMMA)] recursa::Vec1<JsonArrayElement<'input> >),
}

/// Inner contents of `JSON_ARRAY`: an optional value part (subquery or
/// element list) followed by the optional `ON NULL` and `RETURNING` clauses.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayArgs<'input> {
    pub body: Option<JsonArrayBody<'input>>,
    pub on_null: Option<JsonOnNull>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_ARRAY ( ... )` — element-list or query form.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArray<'input> {
    #[tok(JSON_ARRAY, LPAREN, this, RPAREN)]
    pub args:  JsonArrayArgs<'input> ,
}

// --- SQL/JSON query function atoms ---
//
// `JSON_EXISTS()`, `JSON_VALUE()` and `JSON_QUERY()` test/extract values from
// a JSON context item using a jsonpath. Like the constructors they are
// grammar constructs with `PASSING`, `RETURNING`, wrapper/quotes and
// `ON EMPTY`/`ON ERROR` behavior clauses that no function-argument list can
// express. Modeled as dedicated atoms before `Func`.

/// One `‹value› AS ‹name›` binding of a `PASSING` clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonPassingArg<'input> {
    pub value: Box<Expr<'input>>,
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
}

/// `PASSING ‹value› AS ‹name› [, ...]` — jsonpath variable bindings.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonPassing<'input> {
    #[tok(PASSING, this)]
    #[sep(COMMA)]
    pub args: recursa::Vec1<JsonPassingArg<'input> >,
}

/// `DEFAULT ‹expr›` — the default-value form of an `ON EMPTY`/`ON ERROR` behavior.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonDefault<'input> {
    #[tok(DEFAULT, this)]
    pub value: Box<Expr<'input>>,
}

/// The behavior of an `ON EMPTY` / `ON ERROR` clause — the union of every
/// query function's accepted behaviors (`JSON_EXISTS` uses the boolean
/// forms, `JSON_VALUE`/`JSON_QUERY` the rest). Parsed permissively; which
/// behaviors are valid for which function is Postgres's concern.
///
/// Variant ordering: the two-keyword `EMPTY ARRAY`/`EMPTY OBJECT` forms
/// before bare `Empty` so longest-match-wins picks them.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonBehavior<'input> {
    #[tok(EMPTY, ARRAY)] EmptyArray,
    #[tok(EMPTY, OBJECT)] EmptyObject,
    #[tok(EMPTY)] Empty,
    #[tok(ERROR)] Error,
    #[tok(NULL)] Null,
    #[tok(TRUE)] True,
    #[tok(FALSE)] False,
    #[tok(UNKNOWN)] Unknown,
    Default(JsonDefault<'input>),
}

/// `EMPTY` or `ERROR` — the trigger of an `ON` behavior clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum EmptyOrError {
    #[tok(EMPTY)] Empty,
    #[tok(ERROR)] Error,
}

/// `‹behavior› ON {EMPTY|ERROR}` clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonOnBehavior<'input> {
    pub behavior: JsonBehavior<'input>,
    #[tok(ON, this)]
    pub trigger: EmptyOrError,
}

/// `CONDITIONAL` / `UNCONDITIONAL` modifier of a `WITH ... WRAPPER` clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum WrapperBehavior {
    #[tok(CONDITIONAL)] Conditional,
    #[tok(UNCONDITIONAL)] Unconditional,
}

/// `{WITH [CONDITIONAL|UNCONDITIONAL] | WITHOUT} [ARRAY] WRAPPER` — the
/// `JSON_QUERY` array-wrapper clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonWrapper {
    pub with_or_without: WithOrWithout,
    pub behavior: Option<WrapperBehavior>,
    #[tok(this, WRAPPER)]
    #[presence(ARRAY)]
    pub array: bool,
}

/// `ON SCALAR STRING` suffix of a `JSON_QUERY` quotes clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonQuotesOnScalar { #[tok(ON, SCALAR, STRING)] Value, }

/// `KEEP` / `OMIT` lead-in of a `JSON_QUERY` quotes clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum KeepOrOmit {
    #[tok(KEEP)] Keep,
    #[tok(OMIT)] Omit,
}

/// `{KEEP|OMIT} QUOTES [ON SCALAR STRING]` — the `JSON_QUERY` quotes clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonQuotes {
    pub keep_or_omit: KeepOrOmit,
    #[tok(QUOTES, this)]
    pub on_scalar: Option<JsonQuotesOnScalar>,
}

/// Inner contents of `JSON_EXISTS ( ‹context› , ‹path› [PASSING ...] [‹behavior› ON ERROR] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonExistsInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    #[tok(COMMA, this)]
    pub path: Box<Expr<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub on_error: Option<JsonOnBehavior<'input>>,
}

/// `JSON_EXISTS ( ... )` — tests whether a jsonpath matches.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonExists<'input> {
    #[tok(JSON_EXISTS, LPAREN, this, RPAREN)]
    pub inner:  JsonExistsInner<'input> ,
}

/// Inner contents of `JSON_VALUE`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonValueInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    #[tok(COMMA, this)]
    pub path: Box<Expr<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub returning: Option<JsonReturning<'input>>,
    // Two generic behavior slots: each `JsonOnBehavior` self-identifies its
    // `ON EMPTY` / `ON ERROR` trigger, so the pair is order-independent.
    pub on_behavior_1: Option<JsonOnBehavior<'input>>,
    pub on_behavior_2: Option<JsonOnBehavior<'input>>,
}

/// `JSON_VALUE ( ... )` — extracts a scalar SQL value via a jsonpath.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonValue<'input> {
    #[tok(JSON_VALUE, LPAREN, this, RPAREN)]
    pub inner:  JsonValueInner<'input> ,
}

/// Inner contents of `JSON_QUERY`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonQueryInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    #[tok(COMMA, this)]
    pub path: Box<Expr<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub returning: Option<JsonReturning<'input>>,
    pub wrapper: Option<JsonWrapper>,
    pub quotes: Option<JsonQuotes>,
    pub on_behavior_1: Option<JsonOnBehavior<'input>>,
    pub on_behavior_2: Option<JsonOnBehavior<'input>>,
}

/// `JSON_QUERY ( ... )` — extracts a JSON value via a jsonpath.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonQuery<'input> {
    #[tok(JSON_QUERY, LPAREN, this, RPAREN)]
    pub inner:  JsonQueryInner<'input> ,
}

// --- SQL/JSON aggregate atoms ---
//
// `JSON_OBJECTAGG()` and `JSON_ARRAYAGG()` aggregate rows into a JSON object
// or array. They are grammar constructs (the object form takes a `key :
// value` entry, the array form an `ORDER BY`) and, being aggregates, accept
// the ordinary `FILTER (WHERE ...)` and `OVER (...)` suffixes.

/// Inner contents of `JSON_OBJECTAGG`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectAggInner<'input> {
    pub entry: JsonObjectEntry<'input>,
    pub on_null: Option<JsonOnNull>,
    pub unique: Option<JsonUniqueKeys>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_OBJECTAGG ( ‹key› {: | VALUE} ‹value› ... ) [FILTER (...)] [OVER (...)]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectAgg<'input> {
    #[tok(JSON_OBJECTAGG, LPAREN, this, RPAREN)]
    pub inner:  JsonObjectAggInner<'input> ,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// Inner contents of `JSON_ARRAYAGG`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayAggInner<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub on_null: Option<JsonOnNull>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_ARRAYAGG ( ‹value› [ORDER BY ...] ... ) [FILTER (...)] [OVER (...)]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayAgg<'input> {
    #[tok(JSON_ARRAYAGG, LPAREN, this, RPAREN)]
    pub inner:  JsonArrayAggInner<'input> ,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

// --- `IS JSON` predicate ---

/// The JSON item type tested by an `IS JSON` predicate.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonTypeKind {
    #[tok(VALUE)] Value,
    #[tok(SCALAR)] Scalar,
    #[tok(ARRAY)] Array,
    #[tok(OBJECT)] Object,
}

/// The tail of an `IS JSON` predicate: `[NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}]
/// [{WITH|WITHOUT} UNIQUE [KEYS]]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IsJsonTail {
    #[tok(this, JSON)]
    #[presence(NOT)]
    pub not: bool,
    pub type_kind: Option<JsonTypeKind>,
    pub unique: Option<JsonUniqueKeys>,
}

/// Any value-producing SQL/JSON function — the constructors and query
/// functions grouped into one peekable type. Each variant leads with a
/// distinct soft keyword, so this peeks `true` only for a JSON function.
/// Lets non-Pratt contexts (e.g. a `CREATE INDEX` expression element)
/// accept the whole family. Aggregates and `JSON_TABLE` are excluded:
/// neither is a plain value expression usable as an index element.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonFuncExpr<'input> {
    Ctor(Box<JsonConstructor<'input>>),
    Scalar(Box<JsonScalar<'input>>),
    Serialize(Box<JsonSerialize<'input>>),
    Object(Box<JsonObject<'input>>),
    Array(Box<JsonArray<'input>>),
    Exists(Box<JsonExists<'input>>),
    Value(Box<JsonValue<'input>>),
    Query(Box<JsonQuery<'input>>),
}

// --- Pratt expression enum ---

/// SQL expression with Pratt-derived parsing.
#[derive(FormatTokens, Debug, Clone, Visit, Transform)]
#[pratt]
pub enum Expr<'input> {
    // --- Prefix ---
    #[parse(prefix, bp = 15)]
    Not( #[tok(NOT, this)] Box<Expr<'input>>),
    #[parse(prefix, bp = 12)]
    Neg( #[tok(MINUS, this)] Box<Expr<'input>>),
    /// Unary plus: `+expr` — identity operator on numeric types.
    #[parse(prefix, bp = 12)]
    Pos( #[tok(PLUS, this)] Box<Expr<'input>>),
    /// Unary geometric "center point": `@@ expr`. Postgres uses `@@` as
    /// a prefix operator on box / polygon / etc. (in addition to the
    /// text-search infix form).
    #[parse(prefix, bp = 12)]
    GeomCenter( #[tok(ATAT, this)] Box<Expr<'input>>),
    /// Bitwise NOT: `~ expr` (e.g. inet / bit / int bitwise complement).
    /// Must come before any infix `~` variant so the prefix form wins when
    /// `~` appears at the start of an operand.
    #[parse(prefix, bp = 12)]
    BitNot( #[tok(TILDE, this)] Box<Expr<'input>>),
    /// Geometric path/lseg length: `@-@ expr`. Must come before `Abs` (`@`)
    /// since `@-@` is longer.
    #[parse(prefix, bp = 12)]
    PathLength( #[tok(ATMINUSAT, this)] Box<Expr<'input>>),
    /// User-defined prefix: `@#@ expr` (e.g. factorial).
    #[parse(prefix, bp = 12)]
    AtHashAtPrefix( #[tok(ATHASHAT, this)] Box<Expr<'input>>),
    /// Geometric point-count: `# path` — number of points in a path.
    #[parse(prefix, bp = 12)]
    PointCount( #[tok(POUND, this)] Box<Expr<'input>>),
    /// Absolute value: `@ expr` (Postgres unary `@` operator).
    #[parse(prefix, bp = 12)]
    Abs( #[tok(AT, this)] Box<Expr<'input>>),
    /// User-defined prefix: `!=- expr`.
    #[parse(prefix, bp = 12)]
    BangEqMinusPrefix( #[tok(BANGEQMINUS, this)] Box<Expr<'input>>),
    /// Square root: `|/ expr` (Postgres unary `|/` operator).
    #[parse(prefix, bp = 12)]
    Sqrt( #[tok(PIPESLASH, this)] Box<Expr<'input>>),
    /// Cube root: `||/ expr` (Postgres unary `||/` operator).
    #[parse(prefix, bp = 12)]
    Cbrt( #[tok(PIPEPIPESLASH, this)] Box<Expr<'input>>),

    /// Catch-all prefix: any user-defined prefix operator not matched by a
    /// specific token. Declared LAST among prefixes.
    #[parse(prefix, bp = 12)]
    CustomPrefix(literal::CustomOp<'input>, Box<Expr<'input>>),

    // --- Postfix ---
    /// Postgres-style cast: `expr::type`
    #[parse(postfix, bp = 20)]
    Cast(Box<Expr<'input>>,  #[tok(COLONCOLON, this)] Box<CastType<'input>>),
    /// Composite field-star access: `(expr).*` — expand a composite/record
    /// value into its columns. Declared before `FieldAccess` so the longer
    /// `.*` form wins.
    #[parse(postfix, bp = 20)]
    FieldStar(#[tok(this, DOT, STAR)] Box<Expr<'input>>),
    /// Composite field access: `(expr).field` — project one column from a
    /// composite/record value.
    #[parse(postfix, bp = 20)]
    FieldAccess(Box<Expr<'input>>,  #[tok(DOT, this)] literal::AliasName<'input>),
    /// Array slice: `expr[low:high]`, `expr[:high]`, `expr[low:]`, `expr[:]`.
    /// Declared before `Subscript` so the colon-containing form is tried first
    /// when both peek `[`.
    #[parse(postfix, bp = 20)]
    Slice(
        Box<Expr<'input>>,
        #[tok(LBRACKET, this, RBRACKET)]
        SubscriptSlice<'input>,
    ),
    /// Array subscript: `expr[idx]`
    #[parse(postfix, bp = 20)]
    Subscript(
        Box<Expr<'input>>,
        #[tok(LBRACKET, this, RBRACKET)]
        Box<Expr<'input>>,
    ),
    /// `expr COLLATE "collation"` — collation specifier. Binds tighter than
    /// comparisons (bp 5) but looser than `::` cast (bp 20).
    #[parse(postfix, bp = 18)]
    Collate(Box<Expr<'input>>,  #[tok(COLLATE, this)] crate::tokens::ColId<'input>),
    /// `expr IS NOT DISTINCT FROM expr`. Declared before `IsDistinctFrom` so
    /// the longer `NOT` prefix wins disambiguation.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    IsNotDistinctFrom(
        Box<Expr<'input>>,
        #[tok(IS, NOT, DISTINCT, FROM, this)]
        Box<Expr<'input>>,
    ),
    /// `expr IS DISTINCT FROM expr`.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    IsDistinctFrom(Box<Expr<'input>>,  #[tok(IS, DISTINCT, FROM, this)] Box<Expr<'input>>),
    /// `expr IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] [{WITH|WITHOUT}
    /// UNIQUE [KEYS]]` — the SQL/JSON type predicate. Declared before
    /// `BoolTest` (both lead with `IS`); `BoolTest` rejects `JSON` as a
    /// `BoolTestKind`, so order is not load-bearing, only tidy.
    #[parse(postfix, bp = 8)]
    IsJson(Box<Expr<'input>>,  #[tok(IS, this)] IsJsonTail),
    /// `expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` — the Unicode
    /// normalisation predicate (gram.y rules 15198/15205/15212/15220).
    /// Declared before `BoolTest` (both lead with `IS`); `BoolTest` rejects
    /// `NORMALIZED`/`NFx` as a `BoolTestKind`, so order is not load-bearing.
    #[parse(postfix, bp = 8)]
    IsNormalized(Box<Expr<'input>>,  #[tok(IS, this)] IsNormalizedTail),
    /// `expr IS [NOT] DOCUMENT` — the XML document predicate.
    #[parse(postfix, bp = 8)]
    IsDocument(Box<Expr<'input>>,  #[tok(IS, this)] IsDocumentTail),
    /// Boolean test: `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`
    #[parse(postfix, bp = 8)]
    BoolTest(Box<Expr<'input>>,  #[tok(IS, this)] BoolTestKind),
    /// Postgres `expr NOTNULL` postfix null test (synonym for `IS NOT NULL`).
    #[parse(postfix, bp = 8)]
    Notnull(#[tok(this, NOTNULL)] Box<Expr<'input>>),
    /// Postgres `expr ISNULL` postfix null test (synonym for `IS NULL`).
    #[parse(postfix, bp = 8)]
    Isnull(#[tok(this, ISNULL)] Box<Expr<'input>>),
    /// `expr AT LOCAL` — convert to session timezone. Listed before
    /// `AtTimeZone` so `AT LOCAL` wins (distinct second token `LOCAL` vs `TIME`).
    #[parse(postfix, bp = 9)]
    AtLocal(#[tok(this, AT, LOCAL)] Box<Expr<'input>>),
    /// `expr AT TIME ZONE zone_expr` — convert to specified timezone.
    #[parse(postfix, bp = 9, inner_bp = 10)]
    AtTimeZone(Box<Expr<'input>>,    #[tok(AT, TIME, ZONE, this)] Box<Expr<'input>>),
    /// NOT IN list: `expr NOT IN (val, ...)`
    #[parse(postfix, bp = 6)]
    NotInExpr(Box<Expr<'input>>, NotInSuffix<'input>),
    /// `expr NOT ILIKE pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT ILIKE` is tried first (matters only if any rule shares a prefix;
    /// here `NOT ILIKE` vs `NOT LIKE` differ on the second token).
    #[parse(postfix, bp = 5, inner_bp = 6)]
    NotIlike(
        Box<Expr<'input>>,
        #[tok(NOT, ILIKE, this)]
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr NOT SIMILAR TO pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT SIMILAR TO` form wins longest-match-wins disambiguation.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    NotSimilarTo(
        Box<Expr<'input>>,
        #[tok(NOT, SIMILAR, TO, this)]
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr NOT LIKE pattern [ESCAPE char]`. Must come before the `Not` prefix atom so
    /// longest-match-wins prefers the postfix form.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    NotLike(
        Box<Expr<'input>>,
        #[tok(NOT, LIKE, this)]
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr SIMILAR TO pattern [ESCAPE char]` — SQL standard similar-to pattern match.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    SimilarTo(
        Box<Expr<'input>>,
        #[tok(SIMILAR, TO, this)]
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr ILIKE pattern [ESCAPE char]`
    #[parse(postfix, bp = 5, inner_bp = 6)]
    Ilike(
        Box<Expr<'input>>,
        #[tok(ILIKE, this)]
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr LIKE pattern [ESCAPE char]`
    #[parse(postfix, bp = 5, inner_bp = 6)]
    Like(
        Box<Expr<'input>>,
        #[tok(LIKE, this)]
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    // --- Locale-aware text comparison operators (4-char before 3-char) ---
    /// `expr ~<=~ expr` — locale-aware less-or-equal.
    #[parse(infix, bp = 5)]
    TildeLeqTilde(Box<Expr<'input>>,  #[tok(TILDELEQTILDE, this)] Box<Expr<'input>>),
    /// `expr ~>=~ expr` — locale-aware greater-or-equal.
    #[parse(infix, bp = 5)]
    TildeGeqTilde(Box<Expr<'input>>,  #[tok(TILDEGEQTILDE, this)] Box<Expr<'input>>),
    /// `expr ~<~ expr` — locale-aware less-than.
    #[parse(infix, bp = 5)]
    TildeLtTilde(Box<Expr<'input>>,  #[tok(TILDELTTILDE, this)] Box<Expr<'input>>),
    /// `expr ~>~ expr` — locale-aware greater-than.
    #[parse(infix, bp = 5)]
    TildeGtTilde(Box<Expr<'input>>,  #[tok(TILDEGTTILDE, this)] Box<Expr<'input>>),
    /// `expr !~* pattern` — POSIX case-insensitive negated regex match.
    #[parse(infix, bp = 5)]
    RegexNotIMatch(Box<Expr<'input>>,  #[tok(BANGTILDESTAR, this)] Box<Expr<'input>>),
    /// `expr ~* pattern` — POSIX case-insensitive regex match.
    #[parse(infix, bp = 5)]
    RegexIMatch(Box<Expr<'input>>,  #[tok(TILDESTAR, this)] Box<Expr<'input>>),
    /// `expr !~ pattern` — POSIX negated regex match.
    #[parse(infix, bp = 5)]
    RegexNotMatch(Box<Expr<'input>>,  #[tok(BANGTILDE, this)] Box<Expr<'input>>),
    /// `expr ~= expr` — geometric "same as" operator. Declared before `RegexMatch`
    /// so the longer `~=` wins longest-match.
    #[parse(infix, bp = 5)]
    GeomSame(Box<Expr<'input>>,  #[tok(TILDEEQ, this)] Box<Expr<'input>>),
    /// `expr ~ pattern` — POSIX regex match.
    #[parse(infix, bp = 5)]
    RegexMatch(Box<Expr<'input>>,  #[tok(TILDE, this)] Box<Expr<'input>>),
    /// `expr !~~* pattern` — operator-form `NOT ILIKE` (gram.y 14897).
    /// Declared before `LikeOpINeg` (`!~~`) so the longer `!~~*` wins.
    #[parse(infix, bp = 5)]
    LikeOpINeg(
        Box<Expr<'input>>,
        #[tok(BANGTILDETILDESTAR, this)]
        Box<Expr<'input>>,
    ),
    /// `expr ~~* pattern` — operator-form `ILIKE` (gram.y 14888).
    /// Declared before `LikeOpI` would be (no `~~*` longer prefix).
    #[parse(infix, bp = 5)]
    LikeOpI(Box<Expr<'input>>,  #[tok(TILDETILDESTAR, this)] Box<Expr<'input>>),
    /// `expr !~~ pattern` — operator-form `NOT LIKE` (gram.y 14874).
    #[parse(infix, bp = 5)]
    LikeOpNeg(Box<Expr<'input>>,  #[tok(BANGTILDETILDE, this)] Box<Expr<'input>>),
    /// `expr ~~ pattern` — operator-form `LIKE` (gram.y 14860).
    #[parse(infix, bp = 5)]
    LikeOp(Box<Expr<'input>>,  #[tok(TILDETILDE, this)] Box<Expr<'input>>),
    /// `(start, end) OVERLAPS (start, end)` — SQL time-period overlap test.
    /// Each operand is an ordinary parenthesized expression to the parser.
    #[parse(infix, bp = 5)]
    Overlaps(Box<Expr<'input>>,  #[tok(OVERLAPS, this)] Box<Expr<'input>>),
    /// Record comparison operators: `expr *= expr`, `*<>`, `*<`, `*<=`,
    /// `*>`, `*>=` — compare ROW/composite values field by field.
    #[parse(infix, bp = 5)]
    RecordLte(Box<Expr<'input>>,  #[tok(STARLTE, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordGte(Box<Expr<'input>>,  #[tok(STARGTE, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordNeq(Box<Expr<'input>>,  #[tok(STARNEQ, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordLt(Box<Expr<'input>>,  #[tok(STARLT, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordGt(Box<Expr<'input>>,  #[tok(STARGT, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordEq(Box<Expr<'input>>,  #[tok(STAREQ, this)] Box<Expr<'input>>),
    /// IN list: `expr IN (val, ...)`
    #[parse(postfix, bp = 6)]
    InExpr(Box<Expr<'input>>,  #[tok(IN, this)] InList<'input>),
    /// `expr NOT BETWEEN low AND high`. Declared before `BetweenExpr` so
    /// the longer `NOT BETWEEN` prefix wins disambiguation. `inner_bp = 3`
    /// keeps the low/high operands from swallowing the literal `AND` that
    /// separates them (the `AND` infix has `bp = 2`).
    #[parse(postfix, bp = 6, inner_bp = 3)]
    NotBetweenExpr(
        Box<Expr<'input>>,
        #[tok(NOT, BETWEEN, this)]
        Box<Expr<'input>>,
        #[tok(AND, this)]
        Box<Expr<'input>>,
    ),
    /// `expr BETWEEN low AND high`. See `NotBetweenExpr` for the
    /// `inner_bp` rationale.
    #[parse(postfix, bp = 6, inner_bp = 3)]
    BetweenExpr(
        Box<Expr<'input>>,
        #[tok(BETWEEN, this)]
        Box<Expr<'input>>,
        #[tok(AND, this)]
        Box<Expr<'input>>,
    ),

    // --- Infix ---
    // Multi-char operators before single-char to avoid partial matching.
    //
    // JSON / JSONB operators are listed FIRST among infix so that their
    // longer tokens are peeked before conflicting shorter ones
    // (e.g. `<@` before `<`, `->` before `-`). All use bp = 10 — same tier
    // as Concat/Add/Sub (which is Postgres's convention for these ops).
    /// JSON path as text: `expr #>> path`
    #[parse(infix, bp = 10)]
    JsonPathText(Box<Expr<'input>>,  #[tok(HASHARROWARROW, this)] Box<Expr<'input>>),
    /// JSON path: `expr #> path`
    #[parse(infix, bp = 10)]
    JsonPath(Box<Expr<'input>>,  #[tok(HASHARROW, this)] Box<Expr<'input>>),
    /// JSON field as text: `expr ->> field`
    #[parse(infix, bp = 10)]
    JsonFieldText(Box<Expr<'input>>,  #[tok(ARROWARROW, this)] Box<Expr<'input>>),
    /// JSON field: `expr -> field`
    #[parse(infix, bp = 10)]
    JsonField(Box<Expr<'input>>,  #[tok(ARROW, this)] Box<Expr<'input>>),
    /// Geometric parallel: `a ?|| b`. Must precede `JsonAnyKey` (`?|`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, bp = 5)]
    Parallel(
        Box<Expr<'input>>,
        #[tok(QUESTIONPIPEPIPE, this)]
        Box<Expr<'input>>,
    ),
    /// JSON any-key-exists: `expr ?| keys`
    #[parse(infix, bp = 10)]
    JsonAnyKey(Box<Expr<'input>>,  #[tok(QUESTIONPIPE, this)] Box<Expr<'input>>),
    /// JSON all-keys-exist: `expr ?& keys`
    #[parse(infix, bp = 10)]
    JsonAllKeys(Box<Expr<'input>>,  #[tok(QUESTIONAMP, this)] Box<Expr<'input>>),
    /// Geometric intersect: `a ?# b`. Must precede `JsonKey` (`?`).
    #[parse(infix, bp = 5)]
    Intersect(Box<Expr<'input>>,  #[tok(QUESTIONHASH, this)] Box<Expr<'input>>),
    /// Geometric perpendicular: `a ?-| b`. Must precede `Horizontal` (`?-`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, bp = 5)]
    Perpendicular(
        Box<Expr<'input>>,
        #[tok(QUESTIONDASHPIPE, this)]
        Box<Expr<'input>>,
    ),
    /// Geometric horizontal: `a ?- b`. Must precede `JsonKey` (`?`).
    #[parse(infix, bp = 5)]
    Horizontal(Box<Expr<'input>>,  #[tok(QUESTIONDASH, this)] Box<Expr<'input>>),
    /// Geometric "is horizontal" prefix: `?- s` — tests whether the
    /// LSEG/LINE `s` is horizontal. PG's geometry.sql uses this in WHERE.
    #[parse(prefix, bp = 12)]
    IsHorizontal( #[tok(QUESTIONDASH, this)] Box<Expr<'input>>),
    /// Geometric "is vertical" prefix: `?| s`.
    #[parse(prefix, bp = 12)]
    IsVertical( #[tok(QUESTIONPIPE, this)] Box<Expr<'input>>),
    /// Geometric "below": `a <^ b`.
    #[parse(infix, bp = 5)]
    Below(Box<Expr<'input>>,  #[tok(LTCARET, this)] Box<Expr<'input>>),
    /// Geometric "above": `a >^ b`.
    #[parse(infix, bp = 5)]
    Above(Box<Expr<'input>>,  #[tok(GTCARET, this)] Box<Expr<'input>>),
    /// JSON key-exists: `expr ? key`
    #[parse(infix, bp = 10)]
    JsonKey(Box<Expr<'input>>,  #[tok(QUESTION, this)] Box<Expr<'input>>),
    /// JSONB contains: `expr @> expr`
    #[parse(infix, bp = 10)]
    JsonContains(Box<Expr<'input>>,  #[tok(ATGT, this)] Box<Expr<'input>>),
    /// JSONB contained-by: `expr <@ expr`
    #[parse(infix, bp = 10)]
    JsonContainedBy(Box<Expr<'input>>,  #[tok(LTAT, this)] Box<Expr<'input>>),

    // --- Postgres text-search / jsonpath / range / geometric 3-char operators ---
    //
    // These must come BEFORE any variant whose infix token is a 2-char prefix
    // (e.g. `<<|` before `<<`, `&<|` before `&<`, `?#` before JsonKey `?`).
    // The scanner is longest-match at the token level, but Pratt operator
    // dispatch chooses variants in declaration order — so a shorter-prefix
    // variant declared first would swallow the `&<` / `<<` / `?` and leave
    // the trailing `|` / `#` dangling.
    /// Text-search / jsonb path match: `expr @@@ expr`.
    #[parse(infix, bp = 5)]
    TsMatch3(Box<Expr<'input>>,  #[tok(ATATAT, this)] Box<Expr<'input>>),
    /// User-defined triple-less-than: `a <<< b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, bp = 5)]
    TripleLt(Box<Expr<'input>>,  #[tok(LTLTLT, this)] Box<Expr<'input>>),
    /// Geometric strictly-below: `a <<| b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, bp = 5)]
    StrictlyBelow(Box<Expr<'input>>,  #[tok(LTLTPIPE, this)] Box<Expr<'input>>),
    /// Inet is-subset-or-equal: `a <<= b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, bp = 5)]
    SubsetEq(Box<Expr<'input>>,  #[tok(LTLTEQ, this)] Box<Expr<'input>>),
    /// Distance: `a <-> b`. Before any `<` variant.
    #[parse(infix, bp = 10)]
    Distance(Box<Expr<'input>>,  #[tok(LTMINUSGT, this)] Box<Expr<'input>>),
    /// User-defined triple-greater-than: `a >>> b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, bp = 5)]
    TripleGt(Box<Expr<'input>>,  #[tok(GTGTGT, this)] Box<Expr<'input>>),
    /// Inet is-superset-or-equal: `a >>= b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, bp = 5)]
    SupersetEq(Box<Expr<'input>>,  #[tok(GTGTEQ, this)] Box<Expr<'input>>),
    /// Range adjacent: `a -|- b`. Before `Sub` (`-`).
    #[parse(infix, bp = 5)]
    Adjacent(Box<Expr<'input>>,  #[tok(MINUSPIPEMINUS, this)] Box<Expr<'input>>),
    /// Geometric strictly-above: `a |>> b`. Before `Concat` (`||`).
    #[parse(infix, bp = 5)]
    StrictlyAbove(Box<Expr<'input>>,  #[tok(PIPEGTGT, this)] Box<Expr<'input>>),
    /// Geometric no-extend-below: `a |&> b`. Before `Concat` (`||`).
    #[parse(infix, bp = 5)]
    NoExtendBelow(Box<Expr<'input>>,  #[tok(PIPEAMPGT, this)] Box<Expr<'input>>),
    /// Geometric no-extend-above: `a &<| b`. Before `NoExtendRight` (`&<`).
    #[parse(infix, bp = 5)]
    NoExtendAbove(Box<Expr<'input>>,  #[tok(AMPLTPIPE, this)] Box<Expr<'input>>),

    // --- 2-char operators ---
    /// Text-search / jsonb path match: `expr @@ expr`.
    #[parse(infix, bp = 5)]
    TsMatch(Box<Expr<'input>>,  #[tok(ATAT, this)] Box<Expr<'input>>),
    /// Jsonpath exists: `expr @? path`.
    #[parse(infix, bp = 5)]
    JsonPathExists(Box<Expr<'input>>,  #[tok(ATQUESTION, this)] Box<Expr<'input>>),
    /// Range / array overlap: `a && b`.
    #[parse(infix, bp = 10)]
    Overlap(Box<Expr<'input>>,  #[tok(AMPAMP, this)] Box<Expr<'input>>),
    /// Range does-not-extend-right: `a &< b`.
    #[parse(infix, bp = 5)]
    NoExtendRight(Box<Expr<'input>>,  #[tok(AMPLT, this)] Box<Expr<'input>>),
    /// Range does-not-extend-left: `a &> b`.
    #[parse(infix, bp = 5)]
    NoExtendLeft(Box<Expr<'input>>,  #[tok(AMPGT, this)] Box<Expr<'input>>),
    /// Range strictly-left-of: `a << b`.
    #[parse(infix, bp = 5)]
    StrictlyLeft(Box<Expr<'input>>,  #[tok(LTLT, this)] Box<Expr<'input>>),
    /// Range strictly-right-of: `a >> b`.
    #[parse(infix, bp = 5)]
    StrictlyRight(Box<Expr<'input>>,  #[tok(GTGT, this)] Box<Expr<'input>>),

    // --- User-defined / custom infix operators ---
    /// `expr === expr` — user-defined triple-equal. Must come before `Eq` (`=`).
    #[parse(infix, bp = 5)]
    TripleEq(Box<Expr<'input>>,  #[tok(TRIPLEEQ, this)] Box<Expr<'input>>),
    /// `expr !== expr` — user-defined not-equal. Must come before `BangEq` (`!=`).
    #[parse(infix, bp = 5)]
    BangEqEq(Box<Expr<'input>>,  #[tok(BANGEQEQ, this)] Box<Expr<'input>>),
    /// `expr ## expr` — geometric closest-point / path intersection.
    /// Must come before `BitXor` (`#`).
    #[parse(infix, bp = 5)]
    GeomClosest(Box<Expr<'input>>,  #[tok(HASHHASH, this)] Box<Expr<'input>>),

    #[parse(infix, bp = 1)]
    Or(Box<Expr<'input>>,  #[tok(OR, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 2)]
    And(Box<Expr<'input>>,  #[tok(AND, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    BangEq(Box<Expr<'input>>,  #[tok(BANGEQ, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Neq(Box<Expr<'input>>,  #[tok(NEQ, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Lte(Box<Expr<'input>>,  #[tok(LTE, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Gte(Box<Expr<'input>>,  #[tok(GTE, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Eq(Box<Expr<'input>>,  #[tok(EQ, this)] Box<Expr<'input>>),

    /// Text starts-with: `expr ^@ expr` (PostgreSQL `starts_with` operator).
    /// `^@` is a single token (see `punct::CaretAt`); declared before
    /// `CustomInfix` so it wins the declaration-order tiebreak. bp=8 matches
    /// Postgres's generic `Op` precedence.
    #[parse(infix, bp = 8)]
    StartsWith(Box<Expr<'input>>,  #[tok(CARETAT, this)] Box<Expr<'input>>),
    /// JSONB delete-path: `expr #- path` (PostgreSQL jsonb delete-at-path
    /// operator). `#-` is a single token (see `punct::HashMinus`); declared
    /// before `CustomInfix` so it wins the declaration-order tiebreak. bp=10
    /// matches the neighbouring `#>`/`#>>` JSON path operators.
    #[parse(infix, bp = 10)]
    JsonDeletePath(Box<Expr<'input>>,  #[tok(HASHMINUS, this)] Box<Expr<'input>>),

    /// Catch-all infix: any user-defined operator not matched by a specific
    /// token above. Declared BEFORE single-char operators so 2+ char custom
    /// operators like `<%` or `~>` aren't consumed as the single-char prefix
    /// (`<`, `~`) plus garbage. Since `CustomOp` requires 2+ characters, bare
    /// single-char operators still fall through to the variants below.
    /// bp=8 matches Postgres's generic `Op` precedence (between comparison
    /// bp=5 and additive bp=10).
    #[parse(infix, bp = 8)]
    CustomInfix(
        Box<Expr<'input>>,
        literal::CustomOp<'input>,
        Box<Expr<'input>>,
    ),

    #[parse(infix, bp = 5)]
    Lt(Box<Expr<'input>>,  #[tok(LT, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Gt(Box<Expr<'input>>,  #[tok(GT, this)] Box<Expr<'input>>),
    /// String concatenation: `expr || expr`
    #[parse(infix, bp = 10)]
    Concat(Box<Expr<'input>>,  #[tok(CONCAT, this)] Box<Expr<'input>>),
    /// Bitwise OR: `expr | expr`. Must come after `Concat` (`||`) so the
    /// longer token matches first at the punctuation level.
    #[parse(infix, bp = 10)]
    BitOr(Box<Expr<'input>>,  #[tok(PIPE, this)] Box<Expr<'input>>),
    /// Bitwise AND: `expr & expr`.
    #[parse(infix, bp = 10)]
    BitAnd(Box<Expr<'input>>,  #[tok(AMP, this)] Box<Expr<'input>>),
    /// Bitwise XOR: `expr # expr` (Postgres bit-string / integer operator).
    #[parse(infix, bp = 10)]
    BitXor(Box<Expr<'input>>,  #[tok(POUND, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 10)]
    Add(Box<Expr<'input>>,  #[tok(PLUS, this)] Box<Expr<'input>>),
    #[parse(infix, bp = 10)]
    Sub(Box<Expr<'input>>,  #[tok(MINUS, this)] Box<Expr<'input>>),
    /// Multiplication: `expr * expr`
    #[parse(infix, bp = 11)]
    Mul(Box<Expr<'input>>,  #[tok(STAR, this)] Box<Expr<'input>>),
    /// Division: `expr / expr`
    #[parse(infix, bp = 11)]
    Div(Box<Expr<'input>>,  #[tok(SLASH, this)] Box<Expr<'input>>),
    /// Modulo: `expr % expr`
    #[parse(infix, bp = 11)]
    Mod(Box<Expr<'input>>,  #[tok(PERCENT, this)] Box<Expr<'input>>),
    /// Exponentiation: `expr ^ expr` (Postgres numeric power operator).
    #[parse(infix, bp = 13)]
    Pow(Box<Expr<'input>>,  #[tok(CARET, this)] Box<Expr<'input>>),

    // --- Atoms ---
    /// `ANY(expr)` or `ANY(subquery)` — quantified comparison operand.
    #[parse(atom)]
    AnyExpr(AnyExpr<'input>),
    /// `ALL(expr)` or `ALL(subquery)` — quantified comparison operand.
    #[parse(atom)]
    AllExpr(AllExpr<'input>),
    /// `SOME(expr)` or `SOME(subquery)` — synonym for ANY.
    #[parse(atom)]
    SomeExpr(SomeExpr<'input>),
    /// EXISTS subquery: `EXISTS (SELECT ...)`
    #[parse(atom)]
    Exists(ExistsExpr<'input>),
    /// ARRAY constructor: `ARRAY[...]` or `ARRAY(...)`
    #[parse(atom)]
    Array(ArrayExpr<'input>),
    /// ROW constructor: `ROW(...)`
    #[parse(atom)]
    RowExpr(RowExpr<'input>),
    /// CASE expression: `CASE [expr] WHEN ... THEN ... [ELSE ...] END`
    #[parse(atom)]
    Case(CaseExpr<'input>),
    /// Unicode string literal: `U&'...'` with optional `UESCAPE 'c'`. Must
    /// come before `CastFunc` and `StringLit` for the same reason as
    /// `EscapeStringLit`.
    #[parse(atom)]
    UnicodeStringLit(UnicodeStringLitWithEscape<'input>),
    /// Escape string literal: `E'foo\n'`. Must come before `CastFunc` and
    /// `StringLit` — `CastFunc` is `TypeName StringLit` and would match `e`
    /// as a type name followed by the string literal.
    #[parse(atom)]
    EscapeStringLit(#[lex(pattern = r"(?i:E)'(?:[^'\\]|\\.|'')*'")] literal::EscapeStringLit<'input>),
    /// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`. Must come before `CastFunc`
    /// since `timestamp` is also an identifier.
    #[parse(atom)]
    TimestampLit(TimestampLit<'input>),
    /// `TIME [WITH|WITHOUT TIME ZONE] 'string'`. Must come before `CastFunc`.
    #[parse(atom)]
    TimeLit(TimeLit<'input>),
    /// `INTERVAL 'string' [qualifier]`. Must come before `CastFunc` since
    /// `interval` would otherwise parse as an ident-based TypeName.
    #[parse(atom)]
    IntervalLit(IntervalLit<'input>),
    /// Function-style type cast: `bool 't'` -- must come before ColumnRef
    /// since type keywords like `bool` overlap with identifiers
    #[parse(atom)]
    CastFunc(TypeCastFunc<'input>),
    /// `xmlelement(NAME ident [, xmlattributes(...)] [, content])`. Must come
    /// before `Func` so `xmlelement(` is matched as the special form.
    #[parse(atom)]
    XmlElement(Box<XmlElement<'input>>),
    /// `xmlforest(expr [AS alias], ...)`. Before `Func` for the same reason.
    #[parse(atom)]
    XmlForest(XmlForest<'input>),
    /// `xmlattributes(expr [AS alias], ...)`. Before `Func`.
    #[parse(atom)]
    XmlAttributes(XmlAttributes<'input>),
    /// `xmlpi(NAME ident [, content])`. Before `Func`.
    #[parse(atom)]
    XmlPi(XmlPi<'input>),
    /// `XMLSERIALIZE({DOCUMENT|CONTENT} expr AS type [[NO] INDENT])`. Before `Func`.
    #[parse(atom)]
    XmlSerialize(Box<XmlSerialize<'input>>),
    /// `XMLPARSE({DOCUMENT|CONTENT} expr)`. Before `Func`.
    #[parse(atom)]
    XmlParse(Box<XmlParse<'input>>),
    /// `XMLROOT(xml, VERSION ... [, STANDALONE ...])`. Before `Func`.
    #[parse(atom)]
    XmlRoot(Box<XmlRoot<'input>>),
    /// `XMLEXISTS(xpath PASSING ... doc ...)`. Before `Func`.
    #[parse(atom)]
    XmlExists(Box<XmlExists<'input>>),
    /// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`. Before `Func`
    /// since `trim` is also a valid function-call identifier.
    #[parse(atom)]
    Trim(TrimCall<'input>),
    /// `CAST(expr AS type [COLLATE "c"])`. Before `Func`.
    #[parse(atom)]
    CastCall(CastCall<'input>),
    /// `COLLATION FOR (expr)`. Before `Func`.
    #[parse(atom)]
    CollationFor(CollationForCall<'input>),
    /// `SUBSTRING(source FROM ... | SIMILAR ...)`. Before `Func`.
    #[parse(atom)]
    Substring(SubstringCall<'input>),
    /// `POSITION(needle IN haystack)`. Before `Func`.
    #[parse(atom)]
    Position(PositionCall<'input>),
    /// `OVERLAY(source PLACING new FROM start [FOR len])`. Before `Func`.
    #[parse(atom)]
    Overlay(OverlayCall<'input>),
    /// `EXTRACT(field FROM source)`. Before `Func`.
    #[parse(atom)]
    Extract(ExtractCall<'input>),
    /// `JSON(...)` SQL/JSON value constructor. Before `Func`.
    #[parse(atom)]
    JsonCtor(Box<JsonConstructor<'input>>),
    /// `JSON_SCALAR(...)`. Before `Func`.
    #[parse(atom)]
    JsonScalar(Box<JsonScalar<'input>>),
    /// `JSON_SERIALIZE(...)`. Before `Func`.
    #[parse(atom)]
    JsonSerialize(Box<JsonSerialize<'input>>),
    /// `JSON_OBJECT(...)` SQL/JSON object constructor. Before `Func`.
    #[parse(atom)]
    JsonObject(Box<JsonObject<'input>>),
    /// `JSON_ARRAY(...)` SQL/JSON array constructor. Before `Func`.
    #[parse(atom)]
    JsonArray(Box<JsonArray<'input>>),
    /// `JSON_EXISTS(...)` SQL/JSON path predicate. Before `Func`.
    #[parse(atom)]
    JsonExists(Box<JsonExists<'input>>),
    /// `JSON_VALUE(...)` SQL/JSON scalar extraction. Before `Func`.
    #[parse(atom)]
    JsonValue(Box<JsonValue<'input>>),
    /// `JSON_QUERY(...)` SQL/JSON value extraction. Before `Func`.
    #[parse(atom)]
    JsonQuery(Box<JsonQuery<'input>>),
    /// `JSON_OBJECTAGG(...)` SQL/JSON object aggregate. Before `Func`.
    #[parse(atom)]
    JsonObjectAgg(Box<JsonObjectAgg<'input>>),
    /// `JSON_ARRAYAGG(...)` SQL/JSON array aggregate. Before `Func`.
    #[parse(atom)]
    JsonArrayAgg(Box<JsonArrayAgg<'input>>),
    /// `"name"(...)` function call where the name is a single quoted ident.
    /// Declared before `Func` (and before `ColumnRef`) so the Pratt nud
    /// kind-match registers it under the `QuotedIdent` /
    /// `UnicodeQuotedIdent` kinds — closing the gap where `Expr::ColumnRef`
    /// would otherwise commit to `"normalize"` and strand the trailing
    /// `(args)` for the next outer rule. See `QuotedFuncCall`.
    #[parse(atom)]
    QuotedFunc(Box<QuotedFuncCall<'input>>),
    /// Function call: `func(args)` -- must come before ColumnRef
    #[parse(atom)]
    Func(Box<FuncCall<'input>>),
    #[tok(USER)] /// `USER` — the reserved-keyword spelling of `CURRENT_USER` as a
    /// zero-arg function reference. PG's gram.y `func_expr_common_subexpr`
    /// includes `USER { … }` as a synonym for `CURRENT_USER`. pg-sql keeps
    /// `USER` reserved at the token level (for the `CREATE USER ...`
    /// statement disambiguation), so it cannot lex as an `UnquotedIdent`
    /// the way `current_date`/`session_user` do — model it as its own
    /// atom. Declared before `ColumnRef` for clarity (ColumnRef cannot
    /// match a reserved keyword anyway).
    #[parse(atom)]
    User,
    /// Qualified wildcard: `table.*` -- must come before QualRef and ColumnRef
    #[parse(atom)]
    QualWild(QualifiedWildcard<'input>),
    /// Qualified column reference: `table.column` -- must come before ColumnRef
    #[parse(atom)]
    QualRef(QualifiedRef<'input>),
    /// Parenthesized expression: `(expr)`
    #[parse(atom)]
    Paren(ParenExpr<'input>),
    /// Numeric literal: `77.7` -- must come before IntegerLit for longest match
    #[parse(atom)]
    NumericLit(literal::NumericLit<'input>),
    /// Integer literal: `42`
    #[parse(atom)]
    IntegerLit(literal::IntegerLit<'input>),
    /// Dollar-quoted string literal: `$$...$$` or `$tag$...$tag$`.
    /// Listed before `StringLit` since it has a distinct prefix (`$`).
    #[parse(atom)]
    DollarStringLit(literal::DollarStringLit<'input>),
    /// Bit-string literal: `B'10'`. Must come before `StringLit` (and before
    /// any plain `Ident` / `ColumnRef`) for the same reason as
    /// `EscapeStringLit`: the lexer's longest-match-wins picks
    /// `BitStringLit` over `Ident`+`StringLit` only when the prefixed token
    /// is also declared first at the atom level. Without this ordering, the
    /// formatter would round-trip `B'10'` as `B '10'` (inserted space).
    #[parse(atom)]
    BitStringLit(#[lex(pattern = r"(?i:B)'[^']*'")] literal::BitStringLit<'input>),
    /// Hex-string literal: `X'1FF'`. Same ordering rationale as
    /// `BitStringLit` — must precede `StringLit` and any plain `Ident`.
    #[parse(atom)]
    HexStringLit(#[lex(pattern = r"(?i:X)'[^']*'")] literal::HexStringLit<'input>),
    /// String literal sequence: `'hello'` or `'first' 'second' ...` —
    /// Postgres concatenates adjacent string literals into one.
    #[parse(atom)]
    StringLit(StringLitSeq0<'input>),
    #[tok(TRUE)] /// Boolean true
    #[parse(atom)]
    BoolTrue,
    #[tok(FALSE)] /// Boolean false
    #[parse(atom)]
    BoolFalse,
    #[tok(NULL)] /// NULL
    #[parse(atom)]
    Null,
    #[tok(DEFAULT)] /// `DEFAULT` — placeholder usable in INSERT/UPDATE value positions.
    #[parse(atom)]
    Default,
    /// Positional parameter reference: `$1`, `$2`, etc. Used in function bodies
    /// and prepared statements.
    #[parse(atom)]
    PositionalParam(#[lex(matcher)] literal::DollarNum<'input>),
    /// Unqualified column reference: `f1` or `"Foo"`
    #[parse(atom)]
    ColumnRef(crate::tokens::ColId<'input>),
    /// psql client variable substitution: `:foo`, `:'foo'`, `:"foo"`.
    #[parse(atom)]
    PsqlVar(literal::PsqlVariable<'input>),
    #[tok(STAR)] /// Bare wildcard: `*`
    #[parse(atom)]
    Star,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::shared::expr::Expr;

    /// Parse `src` as an `Expr` through the logos lex pass.
    ///
    /// Takes `&'static str` because `test_input` leaks the `LexResult` and
    /// the returned `Expr` borrows the source for that `'static` lifetime.
    fn parse_expr_classified(src: &'static str) -> Expr<'static> {
        let lexed = crate::tokens::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
        assert!(
            input.is_eof(),
            "leftover parsing {src:?}: {:?}",
            &input.source()[input.byte_offset()..]
        );
        expr
    }

    #[test]
    fn parse_json_constructors() {
        // JSON()
        assert!(matches!(
            parse_expr_classified("JSON('{}' FORMAT JSON)"),
            Expr::JsonCtor(_)
        ));
        assert!(matches!(
            parse_expr_classified("JSON('1'::json WITH UNIQUE KEYS)"),
            Expr::JsonCtor(_)
        ));
        // JSON_SCALAR()
        assert!(matches!(
            parse_expr_classified("JSON_SCALAR('123')"),
            Expr::JsonScalar(_)
        ));
        // JSON_SERIALIZE()
        assert!(matches!(
            parse_expr_classified("JSON_SERIALIZE('{}' RETURNING bytea)"),
            Expr::JsonSerialize(_)
        ));
        // JSON_OBJECT() — entries, KEY/VALUE, all clauses, empty, returning-only
        for src in [
            "JSON_OBJECT('a': 1, 'b': 2)",
            "JSON_OBJECT(KEY 'a' VALUE 2 + 3)",
            "JSON_OBJECT('a': 1 ABSENT ON NULL WITH UNIQUE RETURNING jsonb)",
            "JSON_OBJECT()",
            "JSON_OBJECT(RETURNING jsonb)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonObject(_)),
                "{src}"
            );
        }
        // JSON_ARRAY() — element list, query form, empty, returning-only
        for src in [
            "JSON_ARRAY(1, 2, 3)",
            "JSON_ARRAY('a', NULL ABSENT ON NULL RETURNING jsonb)",
            "JSON_ARRAY(SELECT i FROM t)",
            "JSON_ARRAY()",
            "JSON_ARRAY(RETURNING jsonb)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonArray(_)),
                "{src}"
            );
        }
    }

    /// A legacy lowercase `json_object(...)`-style call with plain
    /// comma-separated arguments is NOT the SQL/JSON construct — it must
    /// fall through to an ordinary function call via soft-keyword
    /// identifier reclamation.
    #[test]
    fn legacy_json_object_call_is_ordinary_func() {
        assert!(matches!(
            parse_expr_classified("json_build_array(1, 2)"),
            Expr::Func(_)
        ));
    }

    #[test]
    fn parse_json_query_functions() {
        // JSON_EXISTS — path, PASSING, ON ERROR.
        for src in [
            "JSON_EXISTS(jsonb '1', '$.a')",
            "JSON_EXISTS(js, '$.a' ERROR ON ERROR)",
            "JSON_EXISTS(js, '$ ? (@ > $x)' PASSING 1 AS x, 2 AS y)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonExists(_)),
                "{src}"
            );
        }
        // JSON_VALUE — RETURNING, DEFAULT behavior, ON EMPTY/ERROR.
        for src in [
            "JSON_VALUE(js, '$')",
            "JSON_VALUE(jsonb '123', '$' RETURNING int)",
            "JSON_VALUE(js, '$' RETURNING char(5) DEFAULT '0' ON ERROR)",
            "JSON_VALUE(js, '$' ERROR ON EMPTY NULL ON ERROR)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonValue(_)),
                "{src}"
            );
        }
        // JSON_QUERY — wrapper, quotes, behaviors.
        for src in [
            "JSON_QUERY(js, '$')",
            "JSON_QUERY(js, '$' WITH UNCONDITIONAL ARRAY WRAPPER)",
            "JSON_QUERY(js, '$' WITHOUT WRAPPER)",
            "JSON_QUERY(js, '$' OMIT QUOTES EMPTY ARRAY ON EMPTY)",
            "JSON_QUERY(js, '$' KEEP QUOTES ON SCALAR STRING ERROR ON ERROR)",
            "JSON_QUERY(js, '$' RETURNING bytea FORMAT JSON EMPTY OBJECT ON ERROR)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonQuery(_)),
                "{src}"
            );
        }
        // The result is an ordinary expression operand.
        assert!(matches!(
            parse_expr_classified("JSON_VALUE(js, '$' RETURNING int) + 234"),
            Expr::Add(..)
        ));
    }

    #[test]
    fn parse_json_aggregates() {
        for src in [
            "JSON_OBJECTAGG('b': 1 RETURNING text)",
            "JSON_OBJECTAGG(k VALUE v ABSENT ON NULL WITH UNIQUE)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonObjectAgg(_)),
                "{src}"
            );
        }
        for src in [
            "JSON_ARRAYAGG(i)",
            "JSON_ARRAYAGG(i ORDER BY i DESC RETURNING jsonb)",
            "JSON_ARRAYAGG(bar) FILTER (WHERE bar > 2) OVER (PARTITION BY x)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonArrayAgg(_)),
                "{src}"
            );
        }
    }

    #[test]
    fn parse_multidim_array_literal() {
        for src in [
            "ARRAY[1, 2, 3]",
            "ARRAY[[1,2],[3,4]]",
            "ARRAY[[[1],[2]],[[3],[4]]]",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::Array(_)),
                "{src}"
            );
        }
    }

    #[test]
    fn parse_overlaps() {
        assert!(matches!(
            parse_expr_classified(
                "(timestamp '2000-11-27', interval '12 hours') \
                 OVERLAPS (timestamp '2000-11-27', interval '12 hours')"
            ),
            Expr::Overlaps(..)
        ));
    }

    #[test]
    fn parse_xml_functions() {
        assert!(matches!(
            parse_expr_classified("xmlserialize(CONTENT x AS text NO INDENT)"),
            Expr::XmlSerialize(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlparse(DOCUMENT '<foo/>')"),
            Expr::XmlParse(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlroot(x, VERSION NO VALUE, STANDALONE YES)"),
            Expr::XmlRoot(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlexists('/a' PASSING BY REF doc BY REF)"),
            Expr::XmlExists(_)
        ));
        assert!(matches!(
            parse_expr_classified("x IS DOCUMENT"),
            Expr::IsDocument(..)
        ));
        assert!(matches!(
            parse_expr_classified("x IS NOT DOCUMENT"),
            Expr::IsDocument(..)
        ));
    }

    #[test]
    fn parse_is_json_predicate() {
        for src in [
            "js IS JSON",
            "js IS NOT JSON",
            "js IS JSON ARRAY",
            "js IS JSON OBJECT WITH UNIQUE KEYS",
            "js IS JSON SCALAR",
            "js IS JSON VALUE WITHOUT UNIQUE",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::IsJson(..)),
                "{src}"
            );
        }
        // `IS NULL` still resolves to the boolean test, not `IS JSON`.
        assert!(matches!(
            parse_expr_classified("js IS NULL"),
            Expr::BoolTest(..)
        ));
    }

    // --- Atom tests ---

    #[test]
    fn parse_integer_literal() {
        let lexed = crate::tokens::lex("42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntegerLit(_)));
        assert!(input.is_eof());
    }

    /// Regression: the Pratt-enum kind-match `peek` (emitted when a
    /// classifier is installed) must not answer `false` for atoms that are
    /// not covered by cached token kinds — identifier column-refs and
    /// `FuncCall` reach the parser only through the sequential fallback.
    /// A wrongly-`false` peek made `Seq1<SelectItem, Comma>` skip every
    /// identifier-led SELECT list, dropping fixture coverage to ~52%.
    #[test]
    fn pratt_peek_classified_covers_identifier_atoms() {
        for src in ["a", "abc", "foo(1)", "count(*)"] {
            let plain_lexed = crate::tokens::lex(src);
            assert_eq!(plain_lexed.errors().count(), 0, "lex errors in plain");
            let mut plain = plain_lexed.input();
            assert!(
                Expr::peek(&mut plain),
                "Expr::peek (no classifier) should accept {src:?}"
            );
            let classified_lexed = crate::tokens::lex(src);
            assert_eq!(classified_lexed.errors().count(), 0, "lex errors in classified");
            let mut classified = classified_lexed.input();
            assert!(
                Expr::peek(&mut classified),
                "Expr::peek (classified) should accept {src:?}"
            );
        }
    }

    /// `^@` (text starts-with) is a single PostgreSQL operator token. With the
    /// classifier active it must NOT split into `Caret` + `At`.
    #[test]
    fn parse_starts_with_operator_classified() {
        assert!(matches!(
            parse_expr_classified("a ^@ b"),
            Expr::StartsWith(..)
        ));
    }

    /// `#-` (jsonb delete-path) is a single PostgreSQL operator token. With the
    /// classifier active it must NOT split into `Pound` + `Minus`.
    #[test]
    fn parse_json_delete_path_operator_classified() {
        assert!(matches!(
            parse_expr_classified("a #- b"),
            Expr::JsonDeletePath(..)
        ));
    }

    #[test]
    fn parse_dollar_string_literal_expr() {
        // Regression: json.sql uses `$$'foo'$$::json` and similar.
        let lexed = crate::tokens::lex("$$''$$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::DollarStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_string_literal() {
        let lexed = crate::tokens::lex("'hello'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_adjacent_string_literals() {
        let lexed = crate::tokens::lex("'a' 'b'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2);
        } else {
            panic!("expected Expr::StringLit, got {:?}", expr);
        }
        assert!(input.is_eof());
    }

    /// PostgreSQL concatenates adjacent string literals across whitespace, but
    /// NOT when a comment sits between the two parts. pg-sql must not merge
    /// `'a' /* c */ 'b'` into a single 2-part literal — the second part must
    /// be left unconsumed (so the comment-bearing continuation is rejected).
    #[test]
    fn reject_string_continuation_across_comment() {
        let lexed = crate::tokens::lex("'first line'\n/* not allowed */\n' - next line'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(
                seq.parts.len(),
                1,
                "comment between parts must break the continuation"
            );
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
        assert!(
            !input.is_eof(),
            "the second string part must be left unconsumed"
        );
    }

    /// A legitimate newline-separated 3-part string continuation (no comment)
    /// must still concatenate under the classifier — the regression guard for
    /// `strings.sql`'s "Three lines to one" fixture.
    #[test]
    fn parse_three_part_string_continuation_classified() {
        let expr = parse_expr_classified("'first line'\n' - next line'\n\t' - third line'");
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 3, "all three parts must concatenate");
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
    }

    /// A legitimate newline-separated string continuation (no comment) must
    /// still concatenate — the regression guard for `reject_…_across_comment`.
    #[test]
    fn parse_string_continuation_across_newline() {
        let lexed = crate::tokens::lex("'first line'\n' - next line'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2);
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_three_part_string_concat() {
        // 3-part adjacent string literal concatenation. Postgres concatenates
        // these into a single value at parse time.
        let lexed = crate::tokens::lex("'first' 'second' 'third'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 3);
        } else {
            panic!("expected StringLit, got {:?}", expr);
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_four_part_string_concat() {
        let lexed = crate::tokens::lex("'a' 'b' 'c' 'd'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 4);
        } else {
            panic!("expected StringLit");
        }
    }

    #[test]
    fn parse_three_adjacent_strings_with_quoted_alias() {
        use crate::ast::dml::select::SelectStmt;
        let lexed = crate::tokens::lex("SELECT 'first line' ' - next line' ' - third line' AS \"Three lines to one\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_three_adjacent_strings_with_alias() {
        // SELECT 'first line' ' - next line' AS foo
        use crate::ast::dml::select::SelectStmt;
        let lexed = crate::tokens::lex("SELECT 'first line' ' - next line' AS foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlelement_simple() {
        let lexed = crate::tokens::lex("xmlelement(name foo, 'content')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlelement_with_attributes() {
        let lexed = crate::tokens::lex("xmlelement(name foo, xmlattributes(1 as a, 2 as b), 'content')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlpi_basic() {
        let lexed = crate::tokens::lex("xmlpi(name foo)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlpi_with_content() {
        let lexed = crate::tokens::lex("xmlpi(name foo, 'bar')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unicode_string_lit_basic() {
        let lexed = crate::tokens::lex(r"U&'d\0061t\+000061'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unicode_string_lit_uescape() {
        let lexed = crate::tokens::lex(r"U&'d!0061t\+000061' UESCAPE '!'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_func_with_precision() {
        // `char(20) 'characters'` — function-style type cast with precision.
        let lexed = crate::tokens::lex("char(20) 'characters'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_unicode_string_with_backslash() {
        // `U&' \'` — backslash is literal content, not an escape. The string
        // ends at the second quote. UESCAPE '!' follows.
        let lexed = crate::tokens::lex(r"U&' \' UESCAPE '!'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_xmlforest() {
        let lexed = crate::tokens::lex("xmlforest(a, b AS bee, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlForest(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_exponent_numeric() {
        use crate::ast::dml::select::SelectStmt;
        for sql in [
            "SELECT 4.5e10",
            "SELECT 4.4e131071",
            "SELECT 1.5e-5",
            "SELECT round(4.5e10, -5)",
            "SELECT .5",
            "SELECT 2e3",
        ] {
            let lexed = crate::tokens::lex(sql);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
            assert!(input.is_eof(), "leftover for {sql}");
        }
    }

    #[test]
    fn parse_escape_string_literal() {
        let lexed = crate::tokens::lex(r"E'r_\_view%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_order_by() {
        let lexed = crate::tokens::lex("jsonb_agg(q ORDER BY x, y)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_psql_var() {
        let lexed = crate::tokens::lex(":foo_oid");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_psql_var_in_func_call() {
        let lexed = crate::tokens::lex("pg_stat_get_function_calls(:func_oid)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_both_from() {
        let lexed = crate::tokens::lex("TRIM(BOTH FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_leading_from() {
        let lexed = crate::tokens::lex("TRIM(LEADING FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_trailing_from() {
        let lexed = crate::tokens::lex("TRIM(TRAILING FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_both_chars_from() {
        let lexed = crate::tokens::lex("TRIM(BOTH 'x' FROM 'xxhixx')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// `TRIM([LEADING|TRAILING|BOTH] expr_list)` — gram.y `trim_list`
    /// includes the bare `expr_list` form (no FROM separator), so
    /// `TRIM(TRAILING ' foo ')` is valid: trim trailing whitespace from
    /// `' foo '`. Exercised by create_view.tt201v.
    #[test]
    fn parse_trim_direction_no_from() {
        for src in [
            "TRIM(TRAILING ' foo ')",
            "TRIM(LEADING ' foo ')",
            "TRIM(BOTH ' foo ')",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _expr = Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    /// `USER` is the SQL-standard zero-arg synonym for `CURRENT_USER`.
    /// pg-sql keeps `USER` reserved at the token level (for the
    /// `CREATE USER ...` statement), so it cannot lex as an
    /// `UnquotedIdent` and needs a dedicated `Expr::User` atom.
    #[test]
    fn parse_user_zero_arg_atom() {
        for src in ["SELECT USER", "SELECT USER AS us", "SELECT * FROM USER"] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_substring_from() {
        let lexed = crate::tokens::lex("SUBSTRING('1234567890' FROM 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_from_for() {
        let lexed = crate::tokens::lex("SUBSTRING('1234567890' FROM 4 FOR 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_notnull_isnull() {
        let lexed = crate::tokens::lex("x.c NOTNULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Notnull(..)));
        assert!(input.is_eof());
        let lexed = crate::tokens::lex("x.c ISNULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Isnull(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_collation_for() {
        let lexed = crate::tokens::lex("collation for ('foo')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let lexed = crate::tokens::lex("collation for ((SELECT a FROM t LIMIT 1))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_call() {
        let lexed = crate::tokens::lex("CAST('42' AS text COLLATE \"C\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let lexed = crate::tokens::lex("CAST(b AS varchar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_for_only() {
        let lexed = crate::tokens::lex("substring(d FOR 30)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_similar_escape() {
        let lexed = crate::tokens::lex("SUBSTRING('abcdefg' SIMILAR 'a#\"%#\"g' ESCAPE '#')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_position_in() {
        let lexed = crate::tokens::lex("POSITION('4' IN '1234567890')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlay_placing_from() {
        let lexed = crate::tokens::lex("OVERLAY('abcdef' PLACING '45' FROM 4)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlay_placing_from_for() {
        let lexed = crate::tokens::lex("OVERLAY('abcdef' PLACING '45' FROM 4 FOR 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_epoch_from_date() {
        let lexed = crate::tokens::lex("EXTRACT(EPOCH FROM DATE '1970-01-01')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Extract(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_century_from_ident() {
        let lexed = crate::tokens::lex("EXTRACT(CENTURY FROM d)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_string_field() {
        let lexed = crate::tokens::lex("EXTRACT('year' FROM t)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_named_arg_mixed() {
        let lexed = crate::tokens::lex("f(a, b => 1, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_path_query_silent() {
        let lexed = crate::tokens::lex("jsonb_path_query('[1]', 'strict $[1]', silent => true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_all_named_args() {
        let lexed = crate::tokens::lex("f(silent => false, verbose => true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_year_from_now() {
        let lexed = crate::tokens::lex("EXTRACT(year FROM now())");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_is_distinct_from() {
        let lexed = crate::tokens::lex("a IS DISTINCT FROM b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_is_not_distinct_from() {
        let lexed = crate::tokens::lex("a IS NOT DISTINCT FROM b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_power_operator() {
        let lexed = crate::tokens::lex("2^1000");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_double_precision_type_cast() {
        let lexed = crate::tokens::lex("3.14::double precision");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_searched() {
        let lexed = crate::tokens::lex("CASE WHEN 1 < 2 THEN 3 END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Case(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_searched_with_else() {
        let lexed = crate::tokens::lex("CASE WHEN 1 < 2 THEN 3 WHEN 4 < 5 THEN 6 ELSE 7 END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_simple() {
        let lexed = crate::tokens::lex("CASE x WHEN 1 THEN 'a' WHEN 2 THEN 'b' ELSE 'c' END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_nested() {
        let lexed = crate::tokens::lex("CASE WHEN (CASE WHEN 1=1 THEN 1 END) > 0 THEN 'y' END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_within_group() {
        let lexed = crate::tokens::lex("percentile_disc(0.5) WITHIN GROUP (ORDER BY v)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_within_group_multi() {
        let lexed = crate::tokens::lex("rank(1, 2) WITHIN GROUP (ORDER BY a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_filter() {
        let lexed = crate::tokens::lex("sum(x) FILTER (WHERE y > 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_filter_over() {
        let lexed = crate::tokens::lex("sum(x) FILTER (WHERE y > 0) OVER (PARTITION BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_order_by_nulls_first() {
        let lexed = crate::tokens::lex("jsonb_agg(q ORDER BY x NULLS FIRST, y)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_variadic() {
        let lexed = crate::tokens::lex("jsonb_build_array(VARIADIC a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_timestamp_with_tz_literal() {
        let lexed = crate::tokens::lex("timestamp with time zone '2001-12-27 04:05:06+08'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_timestamp_precision_without_tz_literal() {
        // Regression: timestamp.sql uses `timestamp(2) without time zone 'now'`.
        let lexed = crate::tokens::lex("timestamp(2) without time zone 'now'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_time_zone() {
        let lexed = crate::tokens::lex("f1 AT TIME ZONE 'UTC+10'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_time_zone_interval() {
        let lexed = crate::tokens::lex("f1 AT TIME ZONE INTERVAL '-10:00'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_local() {
        let lexed = crate::tokens::lex("f1 AT LOCAL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtLocal(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_time_literal() {
        let lexed = crate::tokens::lex("time '12:34'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TimeLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_date_literal_as_castfunc() {
        let lexed = crate::tokens::lex("date '2024-01-01'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        // `date` is an Ident-based TypeName, so this parses as CastFunc.
        assert!(matches!(expr, Expr::CastFunc(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_bare() {
        let lexed = crate::tokens::lex("interval '1 hour'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_year() {
        let lexed = crate::tokens::lex("INTERVAL '1' YEAR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_year_to_month() {
        let lexed = crate::tokens::lex("INTERVAL '1-2' YEAR TO MONTH");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_named_arg_colon_equals() {
        let lexed = crate::tokens::lex("make_interval(years := 1, months := 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unary_plus() {
        let lexed = crate::tokens::lex("+42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Pos(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_positional_param() {
        let lexed = crate::tokens::lex("$1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::PositionalParam(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_positional_param_in_expr() {
        let lexed = crate::tokens::lex("$1 + $2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_eof());
    }

    /// `$1` must preserve its digits when reformatted — a positional parameter
    /// is not interchangeable with `$2`. The token must capture the number.
    #[test]
    fn positional_param_preserves_digits() {
        use recursa::fmt::FormatStyle;
        let lexed = crate::tokens::lex("$2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        let formatted = crate::formatter::format_tokens_sql(&expr, FormatStyle::default());
        assert_eq!(formatted.trim(), "$2");
    }

    #[test]
    fn parse_interval_with_precision() {
        for src in [
            "INTERVAL(0) '1 day 01:23:45.6789'",
            "interval(2) '1 day 01:23:45.6789'",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let expr = Expr::parse(&mut input).unwrap().into_ast();
            assert!(matches!(expr, Expr::IntervalLit(_)), "failed for {src:?}");
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_interval_second_precision() {
        let lexed = crate::tokens::lex("INTERVAL '1.234' second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_day_to_second_precision() {
        let lexed = crate::tokens::lex("INTERVAL '1 2:03:04.5678' day to second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_interval_day_to_minute() {
        let lexed = crate::tokens::lex("f1::INTERVAL DAY TO MINUTE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_minute_to_second_precision() {
        let lexed = crate::tokens::lex("INTERVAL '12:34.5678' minute to second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_day_to_hour() {
        let lexed = crate::tokens::lex("INTERVAL '1 2:03' DAY TO HOUR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_hour_to_second() {
        let lexed = crate::tokens::lex("INTERVAL '1' HOUR TO SECOND");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_escape_string_literal_lowercase_e() {
        let lexed = crate::tokens::lex("e'foo'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bool_true() {
        let lexed = crate::tokens::lex("true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTrue(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bool_false() {
        let lexed = crate::tokens::lex("false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolFalse(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_null() {
        let lexed = crate::tokens::lex("null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Null(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_ref() {
        let lexed = crate::tokens::lex("f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::ColumnRef(_)));
    }

    #[test]
    fn parse_qualified_column_ref() {
        let lexed = crate::tokens::lex("BOOLTBL1.f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QualRef(_)));
    }

    #[test]
    fn parse_qualified_wildcard() {
        let lexed = crate::tokens::lex("BOOLTBL1.*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QualWild(_)));
    }

    #[test]
    fn parse_star() {
        let lexed = crate::tokens::lex("*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Star(_)));
    }

    #[test]
    fn parse_function_call_no_args() {
        let lexed = crate::tokens::lex("foo()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_with_args() {
        let lexed = crate::tokens::lex("pg_input_is_valid('true', 'bool')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_booleq() {
        let lexed = crate::tokens::lex("booleq(bool 'false', f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_parenthesized_expr() {
        let lexed = crate::tokens::lex("(1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Paren(_)));
    }

    // --- Type cast function-style: bool 'foo' ---

    #[test]
    fn parse_type_cast_bool_string() {
        let lexed = crate::tokens::lex("bool 't'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    #[test]
    fn parse_type_cast_boolean_string() {
        let lexed = crate::tokens::lex("boolean 'false'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    // --- Prefix operators ---

    #[test]
    fn parse_not_expr() {
        let lexed = crate::tokens::lex("not false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Not(_, _)));
    }

    // --- Infix operators ---

    #[test]
    fn parse_and_expr() {
        let lexed = crate::tokens::lex("true AND false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::And(..)));
    }

    #[test]
    fn parse_or_expr() {
        let lexed = crate::tokens::lex("true OR false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn parse_eq_expr() {
        let lexed = crate::tokens::lex("f1 = true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Eq(..)));
    }

    #[test]
    fn parse_neq_expr() {
        let lexed = crate::tokens::lex("f1 <> false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Neq(..)));
    }

    // --- Postfix: :: type cast ---

    #[test]
    fn parse_cast_colon_colon() {
        let lexed = crate::tokens::lex("0::boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    #[test]
    fn parse_chained_cast() {
        let lexed = crate::tokens::lex("'TrUe'::text::boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        // Outer should be Cast
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Postfix: IS [NOT] TRUE/FALSE/UNKNOWN/NULL ---

    #[test]
    fn parse_is_true() {
        let lexed = crate::tokens::lex("f1 IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_false() {
        let lexed = crate::tokens::lex("f1 IS NOT FALSE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_unknown() {
        let lexed = crate::tokens::lex("b IS UNKNOWN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_unknown() {
        let lexed = crate::tokens::lex("b IS NOT UNKNOWN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    // --- Postfix: BETWEEN / NOT BETWEEN ---

    #[test]
    fn parse_between_expr() {
        let lexed = crate::tokens::lex("a BETWEEN 12 AND 17");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn parse_not_between_expr() {
        let lexed = crate::tokens::lex("a NOT BETWEEN 1 AND 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotBetweenExpr(..)));
    }

    #[test]
    fn parse_between_as_value() {
        // BETWEEN yields a boolean value that can appear in a SELECT list.
        let lexed = crate::tokens::lex("x BETWEEN a AND b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn between_does_not_break_and_parse() {
        // A plain AND expression must still parse as And, not be confused
        // with the BETWEEN postfix.
        let lexed = crate::tokens::lex("a AND b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::And(..)));
    }

    // --- Precedence ---

    #[test]
    fn and_binds_tighter_than_or() {
        // a OR b AND c should parse as a OR (b AND c)
        let lexed = crate::tokens::lex("true OR false AND true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        // Top-level should be OR
        match &expr {
            Expr::Or(..) => {}
            other => panic!("expected OR at top level, got {other:?}"),
        }
    }

    #[test]
    fn comparison_binds_tighter_than_and() {
        // a AND b = c should parse as a AND (b = c)
        let lexed = crate::tokens::lex("true AND f1 = false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        match &expr {
            Expr::And(..) => {}
            other => panic!("expected AND at top level, got {other:?}"),
        }
    }

    #[test]
    fn bool_cast_or_expr() {
        // bool 't' or bool 'f' should parse as (bool 't') OR (bool 'f')
        let lexed = crate::tokens::lex("bool 't' or bool 'f'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn is_true_in_select_item() {
        // b IS TRUE should parse without consuming AS that follows
        let lexed = crate::tokens::lex("b IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn cast_chain_in_expression() {
        // true::boolean::text should chain
        let lexed = crate::tokens::lex("true::boolean::text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Arithmetic operators ---

    #[test]
    fn parse_addition() {
        let lexed = crate::tokens::lex("4+4");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_subtraction() {
        let lexed = crate::tokens::lex("10-3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Sub(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unary_minus() {
        let lexed = crate::tokens::lex("-1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Neg(..)));
        assert!(input.is_eof());
    }

    // --- Numeric literal ---

    #[test]
    fn parse_numeric_literal() {
        let lexed = crate::tokens::lex("77.7");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NumericLit(_)));
        assert!(input.is_eof());
    }

    // --- IN expression ---

    #[test]
    fn parse_in_expr() {
        let lexed = crate::tokens::lex("f1 IN (1, 2, 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::InExpr(..)));
        assert!(input.is_eof());
    }

    // --- JSON / JSONB operators ---

    #[test]
    fn parse_json_field() {
        let lexed = crate::tokens::lex("data -> 'key'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonField(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_field_text() {
        let lexed = crate::tokens::lex("data ->> 'key'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonFieldText(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path() {
        let lexed = crate::tokens::lex("data #> '{a,b}'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonPath(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path_text() {
        let lexed = crate::tokens::lex("data #>> '{a,b}'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonPathText(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_contains() {
        let lexed = crate::tokens::lex("a @> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonContains(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_contained_by() {
        let lexed = crate::tokens::lex("a <@ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonContainedBy(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_key_exists() {
        let lexed = crate::tokens::lex("a ? 'k'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonKey(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_any_key() {
        let lexed = crate::tokens::lex("a ?| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonAnyKey(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_all_keys() {
        let lexed = crate::tokens::lex("a ?& b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonAllKeys(..)));
        assert!(input.is_eof());
    }

    // --- Postgres text-search / range / geometric operators ---

    #[test]
    fn parse_ts_match() {
        let lexed = crate::tokens::lex("a @@ 'foo|bar'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TsMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ts_match3() {
        let lexed = crate::tokens::lex("a @@@ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TsMatch3(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path_exists() {
        let lexed = crate::tokens::lex("j @? '$.a'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonPathExists(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlap() {
        let lexed = crate::tokens::lex("r && s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Overlap(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_left() {
        let lexed = crate::tokens::lex("a << b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyLeft(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_right() {
        let lexed = crate::tokens::lex("a >> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyRight(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_subset_eq() {
        let lexed = crate::tokens::lex("a <<= b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SubsetEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_superset_eq() {
        let lexed = crate::tokens::lex("a >>= b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SupersetEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_adjacent() {
        let lexed = crate::tokens::lex("a -|- b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Adjacent(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_distance() {
        let lexed = crate::tokens::lex("p1 <-> p2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Distance(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_right() {
        let lexed = crate::tokens::lex("a &< b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendRight(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_left() {
        let lexed = crate::tokens::lex("a &> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendLeft(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_above() {
        let lexed = crate::tokens::lex("a |>> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyAbove(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_below() {
        let lexed = crate::tokens::lex("a <<| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyBelow(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_above() {
        let lexed = crate::tokens::lex("a &<| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendAbove(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_below() {
        let lexed = crate::tokens::lex("a |&> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendBelow(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_intersect() {
        let lexed = crate::tokens::lex("a ?# b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Intersect(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_horizontal() {
        let lexed = crate::tokens::lex("a ?- b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Horizontal(..)));
        assert!(input.is_eof());
    }

    // --- LIKE / ILIKE ---

    #[test]
    fn parse_like_expr() {
        let lexed = crate::tokens::lex("table_name LIKE 'foo%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_like_escape_string() {
        let lexed = crate::tokens::lex(r"table_name LIKE E'r_\_view%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_like_expr() {
        let lexed = crate::tokens::lex("table_name NOT LIKE 'bar%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_expr() {
        let lexed = crate::tokens::lex("x SIMILAR TO 'a%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_similar_to_expr() {
        let lexed = crate::tokens::lex("x NOT SIMILAR TO 'a%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ilike_expr() {
        let lexed = crate::tokens::lex("name ILIKE '%FOO%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_ilike_expr() {
        let lexed = crate::tokens::lex("name NOT ILIKE '%bar%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_like_escape() {
        let lexed = crate::tokens::lex("'hawkeye' LIKE 'h%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_like_escape() {
        let lexed = crate::tokens::lex("'hawkeye' NOT LIKE 'h%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_escape() {
        let lexed = crate::tokens::lex("'abcdefg' SIMILAR TO '_bcd#%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_similar_to_escape() {
        let lexed = crate::tokens::lex("'abc' NOT SIMILAR TO 'a%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ilike_escape() {
        let lexed = crate::tokens::lex("name ILIKE '%FOO%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_ilike_escape() {
        let lexed = crate::tokens::lex("name NOT ILIKE '%bar%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_escape_null() {
        let lexed = crate::tokens::lex("'abcdefg' SIMILAR TO '_bcd%' ESCAPE NULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    // --- Regex match operators ---

    #[test]
    fn parse_regex_match() {
        let lexed = crate::tokens::lex("relname ~ '^foo'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_not_match() {
        let lexed = crate::tokens::lex("name !~ 'bar'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexNotMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_imatch() {
        let lexed = crate::tokens::lex("name ~* 'FOO'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexIMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_not_imatch() {
        let lexed = crate::tokens::lex("name !~* '.*'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexNotIMatch(..)));
        assert!(input.is_eof());
    }

    // --- COLLATE postfix ---

    #[test]
    fn parse_collate_postfix() {
        let lexed = crate::tokens::lex("a COLLATE \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Collate(..)));
        assert!(input.is_eof());
    }

    // --- DEFAULT atom ---

    #[test]
    fn parse_default_atom() {
        let lexed = crate::tokens::lex("DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Default(_)));
        assert!(input.is_eof());
    }

    // --- Subquery expression ---

    #[test]
    fn parse_subquery_expr() {
        let lexed = crate::tokens::lex("(SELECT 1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Paren(_)));
        assert!(input.is_eof());
    }

    // --- Locale-aware text comparison operators ---

    #[test]
    fn parse_tilde_lt_tilde_infix() {
        let lexed = crate::tokens::lex("f1 ~<~ 'YX'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeLtTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_leq_tilde_infix() {
        let lexed = crate::tokens::lex("t ~<=~ 'Aztec'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeLeqTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_geq_tilde_infix() {
        let lexed = crate::tokens::lex("t ~>=~ 'Worth'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeGeqTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_gt_tilde_infix() {
        let lexed = crate::tokens::lex("t ~>~ 'Worth'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeGtTilde(..)));
        assert!(input.is_eof());
    }

    // --- User-defined equality/inequality ---

    #[test]
    fn parse_triple_eq_infix() {
        let lexed = crate::tokens::lex("a === 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TripleEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bang_eq_eq_infix() {
        let lexed = crate::tokens::lex("a !== 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BangEqEq(..)));
        assert!(input.is_eof());
    }

    // --- Geometric closest-point / intersection ---

    #[test]
    fn parse_hash_hash_infix() {
        let lexed = crate::tokens::lex("p.f1 ## l.s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::GeomClosest(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: geometric path length `@-@` ---

    #[test]
    fn parse_at_minus_at_prefix() {
        let lexed = crate::tokens::lex("@-@ s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::PathLength(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: user-defined `@#@` ---

    #[test]
    fn parse_at_hash_at_prefix() {
        let lexed = crate::tokens::lex("@#@ 24");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtHashAtPrefix(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: user-defined `!=-` ---

    #[test]
    fn parse_bang_eq_minus_prefix() {
        let lexed = crate::tokens::lex("!=- 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BangEqMinusPrefix(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: geometric `#` (number of points in path) ---

    #[test]
    fn parse_pound_prefix() {
        let lexed = crate::tokens::lex("#thepath");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::PointCount(..)));
        assert!(input.is_eof());
    }

    // --- Infix: geometric `?||` (parallel) and `?-|` (perpendicular) ---

    #[test]
    fn parse_question_pipe_pipe_infix() {
        let lexed = crate::tokens::lex("a ?|| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Parallel(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_question_dash_pipe_infix() {
        let lexed = crate::tokens::lex("a ?-| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Perpendicular(..)));
        assert!(input.is_eof());
    }

    // --- Infix: geometric `<^` (below) and `>^` (above) ---

    #[test]
    fn parse_lt_caret_infix() {
        let lexed = crate::tokens::lex("a <^ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Below(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_gt_caret_infix() {
        let lexed = crate::tokens::lex("a >^ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Above(..)));
        assert!(input.is_eof());
    }

    // --- Infix: user-defined `<<<` and `>>>` ---

    #[test]
    fn parse_triple_lt_infix() {
        let lexed = crate::tokens::lex("a <<< 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TripleLt(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_triple_gt_infix() {
        let lexed = crate::tokens::lex("a >>> 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TripleGt(..)));
        assert!(input.is_eof());
    }

    // --- Infix: user-defined `<%` ---

    #[test]
    fn parse_lt_percent_infix() {
        let lexed = crate::tokens::lex("a <% b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CustomInfix(..)));
        assert!(input.is_eof());
    }

    // --- Subquery quantifier: ANY / ALL / SOME ---

    #[test]
    fn parse_eq_any_subquery() {
        // `a = ANY(SELECT 1)` — comparison with quantified subquery.
        let lexed = crate::tokens::lex("a = ANY(SELECT 1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_eq_all_array() {
        // `a = ALL('{ab}')` — comparison with quantified array.
        let lexed = crate::tokens::lex("a = ALL('{ab}')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_not_tilde_all() {
        // `a !~ ALL('{ab}')` — regex not-match with ALL quantifier.
        let lexed = crate::tokens::lex("a !~ ALL('{ab}')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_eq_some_subquery() {
        // `a = SOME(SELECT 1)` — SOME is synonym for ANY.
        let lexed = crate::tokens::lex("a = SOME(SELECT 1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    // --- Array slice subscripts ---

    #[test]
    fn parse_array_slice_full() {
        // `a[1:2]` — full slice with lower and upper bounds.
        let lexed = crate::tokens::lex("a[1:2]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// Slice on a parenthesised cast: `(arr::int[])[1:2]` — PG accepts the
    /// postfix subscript on any a_expr including a parenthesised cast.
    /// Slices with a reserved keyword (NULL/TRUE/FALSE) as a bound rely on
    /// the `pg_lex` post-processor splitting `:NULL` PsqlVars; the
    /// jsonb-string-range form `[ 'a':'b' ]` is a separate limitation —
    /// PsqlVar's `:'…'` quoted form is preserved to keep psql-style
    /// `COPY ... :'filename'` round-tripping.
    #[test]
    fn parse_array_slice_on_paren_cast() {
        for src in [
            "('{1,2,3}'::int[])[1:2]",
            "a[1:3]",
            "a[NULL:3]",
            "a[1:NULL]",
            "('{1,2,3}'::int[])[1:NULL]",
            "('{{{1},{2},{3}},{{4},{5},{6}}}'::int[])[1][1:NULL][1]",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _expr = Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_array_slice_lower_only() {
        // `a[1:]` — slice with only lower bound.
        let lexed = crate::tokens::lex("a[1:]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_array_slice_upper_only() {
        // `a[:2]` — slice with only upper bound.
        let lexed = crate::tokens::lex("a[:2]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_array_slice_unbounded() {
        // `a[:]` — unbounded slice (all elements).
        let lexed = crate::tokens::lex("a[:]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_subscript_unchanged() {
        // `a[1]` — regular subscript still works.
        let lexed = crate::tokens::lex("a[1]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Subscript(..)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_any_array_literal() {
        // `ANY('{red,green}'::rainbow[])` — bare ANY as atom.
        let lexed = crate::tokens::lex("ANY('{red,green}'::rainbow[])");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AnyExpr(_)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_all_array_literal() {
        // `ALL('{red,red}'::rainbow[])` — bare ALL as atom.
        let lexed = crate::tokens::lex("ALL('{red,red}'::rainbow[])");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AllExpr(_)));
        assert!(
            input.is_eof(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// `IN ((SELECT 1), (SELECT 2))` — gram.y `in_expr → '(' expr_list ')'`
    /// where each `expr_list` element is a `ParenExpr` wrapping a subquery.
    /// `InContent::Exprs` must dispatch on the `(` first-set token; the bare
    /// `Subquery` branch only wins on `SELECT`/`VALUES`/`WITH`/`TABLE`.
    #[test]
    fn parse_in_list_of_parenthesised_subqueries() {
        for src in [
            "SELECT * FROM t WHERE b IN ((select 1), (select 2))",
            // Mixed parenthesised subquery + bare expr.
            "SELECT * FROM t WHERE b IN (1, (select 2))",
            // Single bare subquery (no surrounding paren) — still a Subquery.
            "SELECT * FROM t WHERE b IN (select 1)",
            // Single value list.
            "SELECT * FROM t WHERE b IN (1, 2, 3)",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    /// `(SubSelect)::Typename` is gram.y `c_expr → '(' SubSelect ')' typecast`.
    /// In the recursa model, the trailing `::Typename` is a Pratt postfix on
    /// the `(SubSelect)` ParenExpr — at top level that's fine, but inside
    /// another parenthesised context (`ANY(...)`, `((...))`, ...) the
    /// outer `ParenContent` would otherwise stop at the inner close-paren
    /// and strand `::Typename` for the next outer close. `CastedSubquery`
    /// is a dedicated `ParenContent` variant that absorbs the trailing
    /// cast so the round-trip works in every nesting position.
    #[test]
    fn parse_paren_subquery_cast_in_nested_contexts() {
        for src in [
            "SELECT ((select 1)::int)",
            "SELECT ((select 1)::int[])",
            "SELECT ANY((select array['abc']::text[])::text[])",
            "SELECT any((select array_agg(i) from generate_series(1, 100, 15) i)::int[])",
            // Chained casts inside the nested paren context.
            "SELECT ((select 1)::int::text)",
            // Bare Subquery must still match when no trailing cast follows.
            "SELECT ((select 1))",
            "SELECT ((select 1) UNION select 2)",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    /// `B'…'` and `X'…'` must parse as a single literal atom and round-trip
    /// through the formatter byte-for-byte. The previous behaviour lexed the
    /// prefix as an identifier followed by an ordinary `StringLit`, which the
    /// formatter then separated with a space (`B '10'`). Exact-equality
    /// assertion subsumes the narrower "no inserted space" check and also
    /// catches related unfaithfulness modes (prefix dropped, case-folded,
    /// doubled, etc.).
    #[test]
    fn bit_and_hex_string_literals_round_trip_without_space() {
        use crate::formatter::format_tokens_sql;
        use recursa::fmt::FormatStyle;

        for src in ["B'10'", "X'1FF'", "b'001'", "x'42f'", "B''"] {
            let expr = parse_expr_classified(src);
            // Confirm the atom is the dedicated bit/hex variant, not a
            // StringLit / ColumnRef pair.
            assert!(
                matches!(expr, Expr::BitStringLit(_) | Expr::HexStringLit(_)),
                "expected BitStringLit/HexStringLit atom for {src:?}, got {:?}",
                std::mem::discriminant(&expr),
            );
            let formatted = format_tokens_sql(&expr, FormatStyle::default());
            assert_eq!(
                formatted.trim(),
                src,
                "non-exact round-trip for {src:?}: {formatted:?}",
            );
        }
    }
}
