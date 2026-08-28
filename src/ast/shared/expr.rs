/// SQL expression AST with derived Pratt parsing for operator precedence.
///
/// Handles atoms, prefix (NOT, unary minus), infix (AND, OR, comparisons,
/// arithmetic), and postfix operators (::type cast, IS [NOT] TRUE/FALSE/UNKNOWN/NULL,
/// IN (list)).
use recursa::seq::{OptionalTrailing, Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
pub struct StringLitSeq0<'input> {
    pub parts: Seq1<literal::StringLit<'input>, (), OptionalTrailing>,
}

/// **Manual `Parse` impl — recursa-side limitation.** PostgreSQL stops
/// concatenating adjacent string literals when a comment sits between two
/// parts, but recursa's `Seq` machinery cannot express that: the inter-element
/// gap of ignored tokens (whitespace/comments) is skipped *before* any
/// separator or element type is consulted, so no derived combinator can
/// inspect the raw gap.
///
/// This impl parses each part itself and inspects the raw source slice between
/// one part's end and the next part's start, then hands the collected parts to
/// `Seq1::from_pairs` so `FormatTokens` / `Visit` stay derived. Filed as a
/// recursa-side limitation: the framework needs a way to surface the
/// inter-element gap to a separator type (e.g. a separator hook that runs
/// before the ignored-token skip).
impl<'input> ::recursa_core::Parse<'input> for StringLitSeq0<'input> {
    type Prefix = ();

    fn meta() -> &'static ::recursa_core::Meta {
        static META: ::recursa_core::Meta = ::recursa_core::Meta {
            name: "string_lit_seq0",
            tags: &[],
        };
        &META
    }

    fn peek(input: &mut ::recursa_core::Input<'input>) -> bool {
        let mut fork = input.fork();
        literal::StringLit::peek(&mut fork)
    }

    fn parse(
        input: &mut ::recursa_core::Input<'input>,
    ) -> ::std::result::Result<Self, ::recursa_core::ParseError> {
        // `byte_offset()` is the source byte position of the token under the
        // cursor — the inter-token gap inspection below slices raw `source`.
        let part_start = input.byte_offset();
        let first = literal::StringLit::parse(input)?;
        // The literal text includes the surrounding quotes, so its byte
        // length is the token's exact span in the source.
        let mut prev_end = part_start + first.0.len();

        // `(elem, Some(()))` for every non-final part and `(elem, None)` for
        // the last — the pair shape `Seq1<_, (), OptionalTrailing>` expects.
        // The final element's separator is patched to `None` after the loop.
        let mut pairs: ::vec1::Vec1<(literal::StringLit<'input>, Option<()>)> =
            ::vec1::Vec1::new((first, None));
        loop {
            let next_start = input.byte_offset();

            // The gap between the previous part and the next token consists
            // only of ignored content (whitespace and comments). A comment in
            // that gap breaks the continuation — PostgreSQL would reject it.
            // The lexer strips comments from the token array, but the raw
            // `source` slice between the two byte offsets still contains them.
            let gap = &input.source()[prev_end..next_start];
            if gap.contains("/*") || gap.contains("--") {
                break;
            }
            if !literal::StringLit::peek(input) {
                break;
            }

            // The previous part is no longer final: give it a separator.
            pairs.last_mut().1 = Some(());

            let part_start = input.byte_offset();
            let part = literal::StringLit::parse(input)?;
            prev_end = part_start + part.0.len();
            pairs.push((part, None));
        }

        Ok(Self {
            parts: Seq1::from_pairs(pairs),
        })
    }
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum InContent<'input> {
    Exprs(Seq0<Expr<'input>, punct::Comma>),
    Subquery(Box<Subquery<'input>>),
}

/// `IN (expr, ...)` or `IN (subquery)` postfix suffix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
#[recursa::parser(rules = SqlRules)]
pub struct InList<'input>(#[deref] pub Surrounded<punct::LParen, InContent<'input>, punct::RParen>);

/// A single typmod argument: an optionally-signed integer literal. Postgres'
/// gram.y allows `expr_list` here, but the corpus only exercises signed
/// integers (e.g. `numeric(3, -6)` in numeric.sql), so we model only that
/// shape. A leading `+` or `-` is permitted to mirror PG's behavior.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TypeModifierArg<'input> {
    pub sign: Option<TypeModifierSign>,
    pub value: literal::IntegerLit<'input>,
}

/// Leading sign of a typmod argument.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TypeModifierSign {
    Neg(punct::Minus),
    Pos(punct::Plus),
}

/// Parenthesized precision/scale for type names: `(10,2)`, `(3)`, `(3,-6)`.
#[railroad(label = "<Precision>")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform, derive_more::Deref)]
#[recursa::parser(rules = SqlRules)]
pub struct TypePrecision<'input>(
    #[deref]
    pub  Surrounded<punct::LParen, Seq0<TypeModifierArg<'input>, punct::Comma>, punct::RParen>,
);

/// Type name for casts.
#[railroad(label = "<Type Name>")]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, PartialEq, Eq, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TypeName<'input> {
    Bool(BOOL),
    Boolean(BOOLEAN),
    Text(TEXT),
    Integer(INTEGER),
    Int(INT),
    Serial(SERIAL),
    Numeric(NUMERIC),
    Varchar(VARCHAR),
    /// `DOUBLE PRECISION` — two-keyword type. Listed before `Ident` so the
    /// DOUBLE match isn't accidentally consumed as a plain identifier.
    DoublePrecision((DOUBLE, PRECISION)),
    /// `TIMESTAMP` (optional `WITH/WITHOUT TIME ZONE` qualifier handled
    /// at the `CastType` level so precision can sit between).
    Timestamp(TIMESTAMP),
    /// `TIME` — same shape as `TIMESTAMP`.
    Time(TIME),
    /// `INTERVAL` — qualifier (`YEAR TO MONTH` etc.) is currently not
    /// modeled at the type level; only the bare keyword is consumed.
    Interval(INTERVAL),
    /// `BIT` and `BIT VARYING` (the optional `VARYING` modifier is handled
    /// at the `CastType` level).
    Bit(BIT),
    /// `CHARACTER` and `CHARACTER VARYING` — same shape as `BIT`.
    Character(CHARACTER),
    /// `UNKNOWN` — pseudo-type used for untyped literals; reserved keyword so
    /// it must be matched explicitly rather than falling through to `Ident`.
    Unknown(UNKNOWN),
    /// Qualified type name (`schema.type`) or a bare identifier.
    Ident(crate::ast::shared::names::QualifiedName<'input>),
}

/// Boolean test suffix: the part after `IS` in `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`.
///
/// NOT variants are listed first so the combined peek regex disambiguates
/// via longest match (e.g., `NOT TRUE` is longer than `TRUE`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum BoolTestKind {
    IsNotTrue((NOT, TRUE)),
    IsNotFalse((NOT, FALSE)),
    IsNotUnknown((NOT, UNKNOWN)),
    IsNotNull((NOT, NULL)),
    IsTrue(TRUE),
    IsFalse(FALSE),
    IsUnknown(UNKNOWN),
    IsNull(NULL),
}

/// Unicode normalisation form keyword — gram.y `unicode_normal_form`.
/// Used by `expr IS [NOT] [NFx] NORMALIZED` and `NORMALIZE(expr, NFx)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum UnicodeNormalForm {
    Nfkc(NFKC),
    Nfkd(NFKD),
    Nfc(NFC),
    Nfd(NFD),
}

/// Tail of `expr IS [NOT] [NFx] NORMALIZED` — the `[NOT] [NFx] NORMALIZED`
/// part after the leading `IS`. Modelled as an enum so the postfix-Pratt
/// `IsNormalized(_, IS, IsNormalizedTail)` can dispatch on the second token.
///
/// Variant ordering: NOT-leading forms first (longer prefix), and within
/// each NOT/non-NOT bucket the form-prefixed variants come before the bare
/// `NORMALIZED` so the peek regex prefers the longer match.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum IsNormalizedTail {
    NotForm(IsNotFormNormalizedTail),
    Not((NOT, NORMALIZED)),
    Form(IsFormNormalizedTail),
    Plain(NORMALIZED),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IsFormNormalizedTail {
    pub form: UnicodeNormalForm,
    pub normalized: NORMALIZED,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IsNotFormNormalizedTail {
    pub not: NOT,
    pub form: UnicodeNormalForm,
    pub normalized: NORMALIZED,
}

// --- Atom wrapper structs ---

/// Qualified column reference: `table.column`
///
/// Uses AliasName for the table part to allow keywords like EXCLUDED, NEW, OLD.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct QualifiedRef<'input> {
    pub table: literal::AliasName<'input>,
    pub dot: punct::Dot,
    pub column: literal::AliasName<'input>,
}

/// Qualified wildcard: `table.*`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct QualifiedWildcard<'input> {
    pub table: literal::AliasName<'input>,
    pub dot: punct::Dot,
    pub star: punct::Star,
}

/// Window specification: `OVER window_name` or `OVER (inline_spec)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct WindowSpec<'input> {
    pub over: OVER,
    pub body: WindowSpecBody<'input>,
}

/// Body of an OVER clause.
///
/// Variant ordering: Inline (starts with `(`) before Named (starts with an
/// identifier). They start with different tokens so peek disambiguation is
/// trivial.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum WindowSpecBody<'input> {
    Inline(Surrounded<punct::LParen, InlineWindowSpec<'input>, punct::RParen>),
    Named(crate::tokens::ColId<'input>),
}

/// Interior of an inline window spec (between the parens).
///
/// The optional `ref_name` is an existing-window reference (e.g.
/// `WINDOW w2 AS (w1 ORDER BY x)`). It relies on `Option<literal::Ident>`
/// peek-disambiguating cleanly against `PARTITION`/`ORDER`/`ROWS`/etc.
/// because keywords are rejected by `literal::Ident`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct InlineWindowSpec<'input> {
    pub ref_name: Option<literal::WindowRefNameIdent<'input>>,
    pub partition_by: Option<WindowPartitionBy<'input>>,
    pub order_by: Option<crate::ast::dml::select::OrderByClause<'input>>,
    pub frame: Option<WindowFrameClause<'input>>,
}

/// PARTITION BY in window: `PARTITION BY expr, ...`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct WindowPartitionBy<'input> {
    pub partition: PARTITION,
    pub by: BY,
    pub exprs: Seq0<Expr<'input>, punct::Comma>,
}

/// Frame unit: `ROWS | RANGE | GROUPS`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum WindowFrameUnit {
    Rows(ROWS),
    Range(RANGE),
    Groups(GROUPS),
}

/// `WINDOW` frame clause: `unit BETWEEN start AND end [EXCLUDE ...]`
/// or `unit start`.
///
/// Variant ordering: `Between` (starts with `unit BETWEEN`) before `Single`
/// (starts with `unit <bound>`). Longest-match-wins.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum WindowFrameClause<'input> {
    Between(WindowFrameBetween<'input>),
    Single(WindowFrameSingle<'input>),
}

/// `unit BETWEEN start AND end [EXCLUDE ...]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct WindowFrameBetween<'input> {
    pub unit: WindowFrameUnit,
    pub between: BETWEEN,
    pub start: WindowFrameBound<'input>,
    pub and: AND,
    pub end: WindowFrameBound<'input>,
    pub exclude: Option<WindowFrameExclude>,
}

/// `unit start [EXCLUDE ...]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum WindowFrameBound<'input> {
    UnboundedPreceding((UNBOUNDED, PRECEDING)),
    UnboundedFollowing((UNBOUNDED, FOLLOWING)),
    CurrentRow((CURRENT, ROW)),
    ExprPreceding(ExprPreceding<'input>),
    ExprFollowing(ExprFollowing<'input>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct ExprPreceding<'input> {
    pub expr: Box<Expr<'input>>,
    pub preceding: PRECEDING,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct ExprFollowing<'input> {
    pub expr: Box<Expr<'input>>,
    pub following: FOLLOWING,
}

/// `EXCLUDE { CURRENT ROW | GROUP | TIES | NO OTHERS }` frame exclusion.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct WindowFrameExclude {
    pub exclude: EXCLUDE,
    pub target: WindowFrameExcludeTarget,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum WindowFrameExcludeTarget {
    CurrentRow((CURRENT, ROW)),
    Group(GROUP),
    Ties(TIES),
    NoOthers((NO, OTHERS)),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncArg<'input> {
    Named(NamedFuncArg<'input>),
    Variadic(VariadicArg<'input>),
    Plain(Box<Expr<'input>>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct VariadicArg<'input> {
    pub variadic: VARIADIC,
    pub value: Box<Expr<'input>>,
}

/// `=>` or `:=` — the two named-argument operators PostgreSQL accepts.
///
/// Variant ordering: both are distinct two-character punctuation tokens,
/// no ambiguity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NamedArgOp {
    FatArrow(punct::FatArrow),
    ColonEquals(punct::ColonEquals),
}

/// Named function argument: `name => value` or `name := value` (Postgres).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct NamedFuncArg<'input> {
    pub name: literal::AliasName<'input>,
    pub arrow: NamedArgOp,
    pub value: Box<Expr<'input>>,
}

/// `WITHIN GROUP (ORDER BY ...)` clause for ordered-set aggregate functions.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WithinGroupClause<'input> {
    pub within: WITHIN,
    pub group: GROUP,
    pub order_by: Surrounded<
        punct::LParen,
        Box<crate::ast::dml::select::OrderByClause<'input>>,
        punct::RParen,
    >,
}

/// `FILTER (WHERE condition)` clause for filtered aggregates.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FilterClause<'input> {
    pub filter: FILTER,
    pub body:
        Surrounded<punct::LParen, Box<crate::ast::dml::select::WhereClause<'input>>, punct::RParen>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncCallName<'input> {
    Left(LEFT),
    Right(RIGHT),
    Set(SET),
    Name(crate::ast::shared::names::QualifiedName<'input>),
}

/// Function call: `name([*] [DISTINCT] args [ORDER BY ...]) [WITHIN GROUP (...)] [FILTER (...)] [OVER (...)]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncCall<'input> {
    pub name: FuncCallName<'input>,
    pub lparen: punct::LParen,
    pub star_arg: Option<punct::Star>,
    pub distinct: Option<DISTINCT>,
    pub args: Seq0<FuncArg<'input>, punct::Comma>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub rparen: punct::RParen,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct QuotedFuncCall<'input> {
    pub name: QuotedFuncName<'input>,
    pub lparen: punct::LParen,
    pub star_arg: Option<punct::Star>,
    pub distinct: Option<DISTINCT>,
    pub args: Seq0<FuncArg<'input>, punct::Comma>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub rparen: punct::RParen,
    pub within_group: Option<WithinGroupClause<'input>>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// A trailing `::cast` chain — one or more postfix casts applied to a
/// preceding value. Used by `CastedSubquery` to absorb `(SubSelect)::Typename`
/// where the cast belongs structurally to the parenthesised value but cannot
/// be reached via the ordinary `Subquery` variant of `ParenContent` (which
/// stops at the close paren and would strand `::cast`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastTail<'input> {
    pub cast: punct::ColonColon,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastedSubquery<'input> {
    pub subquery: Surrounded<punct::LParen, Box<Subquery<'input>>, punct::RParen>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ParenContent<'input> {
    CastedSubquery(CastedSubquery<'input>),
    Subquery(Box<Subquery<'input>>),
    Exprs(Seq0<Expr<'input>, punct::Comma>),
}

/// Parenthesized expression: `(expr)`, `(expr, expr, ...)`, or `(SELECT/VALUES ...)`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
#[recursa::parser(rules = SqlRules)]
pub struct ParenExpr<'input>(
    #[deref] pub Surrounded<punct::LParen, ParenContent<'input>, punct::RParen>,
);

/// Array slice content: `lower : upper`, `: upper`, `lower :`, or `:`.
///
/// Both bounds are optional; the colon is required.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct SubscriptSlice<'input> {
    pub lower: Option<Box<Expr<'input>>>,
    pub colon: punct::Colon,
    pub upper: Option<Box<Expr<'input>>>,
}

/// `.field` accessor in an indirection chain.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IndirectionField<'input> {
    pub dot: punct::Dot,
    pub name: literal::AliasName<'input>,
}

/// One element of an indirection chain on an `INSERT` / `UPDATE` column
/// target: `[idx]`, `[low:high]`, or `.field` (Postgres `opt_indirection`).
///
/// Variant ordering: `Slice` before `Index` — both open with `[`, the
/// colon-containing slice form is tried first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum IndirectionEl<'input> {
    Slice(Surrounded<punct::LBracket, SubscriptSlice<'input>, punct::RBracket>),
    Index(Surrounded<punct::LBracket, Box<Expr<'input>>, punct::RBracket>),
    Field(IndirectionField<'input>),
}

/// `ANY(expr)` or `ANY(subquery)` — quantified comparison operand.
///
/// Used on the right side of a comparison operator: `x = ANY(array_expr)`
/// or `x = ANY(SELECT ...)`. Also valid as a standalone expression atom.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct AnyExpr<'input> {
    pub any: ANY,
    pub content: Surrounded<punct::LParen, ParenContent<'input>, punct::RParen>,
}

/// `ALL(expr)` or `ALL(subquery)` — quantified comparison operand.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct AllExpr<'input> {
    pub all: ALL,
    pub content: Surrounded<punct::LParen, ParenContent<'input>, punct::RParen>,
}

/// `SOME(expr)` or `SOME(subquery)` — synonym for ANY.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct SomeExpr<'input> {
    pub some: SOME,
    pub content: Surrounded<punct::LParen, ParenContent<'input>, punct::RParen>,
}

/// EXISTS subquery: `EXISTS (SELECT ...)`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct ExistsExpr<'input> {
    pub exists: EXISTS,
    pub subquery: Surrounded<punct::LParen, Box<Subquery<'input>>, punct::RParen>,
}

/// One element of an `ARRAY[...]` constructor: either an ordinary
/// expression or a nested bracketed sub-list (for multi-dimensional
/// literals like `ARRAY[[1,2],[3,4]]`).
///
/// Variant ordering: `Nested` leads with `[`, which no expression atom
/// does, so dispatch is unambiguous.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum ArrayElement<'input> {
    Nested(Surrounded<punct::LBracket, Seq0<ArrayElement<'input>, punct::Comma>, punct::RBracket>),
    Expr(Box<Expr<'input>>),
}

/// ARRAY bracket constructor: `ARRAY[expr, ...]`, including the
/// multi-dimensional form `ARRAY[[1,2],[3,4]]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct ArrayBracket<'input> {
    pub array: ARRAY,
    pub lbracket: punct::LBracket,
    pub elements: Seq0<ArrayElement<'input>, punct::Comma>,
    pub rbracket: punct::RBracket,
}

/// ARRAY subquery constructor: `ARRAY(subquery)`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct ArraySubquery<'input> {
    pub array: ARRAY,
    pub subquery: Surrounded<punct::LParen, Box<Subquery<'input>>, punct::RParen>,
}

/// ARRAY constructor: `ARRAY[expr, ...]` or `ARRAY(subquery)`
///
/// Variant ordering: Bracket (`ARRAY[`) has a longer first_pattern than
/// Subquery (`ARRAY(`) because `[` is a different token than `(`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum ArrayExpr<'input> {
    Bracket(ArrayBracket<'input>),
    Subquery(ArraySubquery<'input>),
}

/// ROW constructor: `ROW(expr, ...)`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct RowExpr<'input> {
    pub row: ROW,
    pub values: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
}

/// `WHEN cond THEN result` arm of a CASE expression.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct CaseWhenArm<'input> {
    pub when: WHEN,
    pub condition: Box<Expr<'input>>,
    pub then: THEN,
    pub result: Box<Expr<'input>>,
}

/// `ELSE result` clause of a CASE expression.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct CaseElse<'input> {
    pub r#else: ELSE,
    pub result: Box<Expr<'input>>,
}

/// Searched CASE: `CASE WHEN cond THEN result [...] [ELSE result] END`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct CaseSearched<'input> {
    pub case: CASE,
    pub first_arm: CaseWhenArm<'input>,
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    pub else_clause: Option<CaseElse<'input>>,
    pub end: END,
}

/// Simple CASE: `CASE operand WHEN val THEN result [...] [ELSE result] END`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct CaseSimple<'input> {
    pub case: CASE,
    pub operand: Box<Expr<'input>>,
    pub first_arm: CaseWhenArm<'input>,
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    pub else_clause: Option<CaseElse<'input>>,
    pub end: END,
}

/// CASE expression: searched form (first, since `CASE WHEN` is a longer
/// specific prefix than `CASE` followed by any expression) or simple form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub enum ArraySuffix<'input> {
    Sized(ArraySuffixSized<'input>),
    Empty(ArraySuffixEmpty),
}

/// `[N]` array bound.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub struct ArraySuffixSized<'input> {
    pub bounds: Surrounded<punct::LBracket, literal::IntegerLit<'input>, punct::RBracket>,
}

/// `[]` array suffix (unbounded).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub struct ArraySuffixEmpty {
    pub bounds: Surrounded<punct::LBracket, (), punct::RBracket>,
}

/// Cast type with optional precision and zero-or-more array suffixes:
/// `numeric(10,0)`, `integer[]`, `int4[][][]`, `varchar(4)[2][3]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub struct CastType<'input> {
    pub base: TypeName<'input>,
    /// `VARYING` modifier (e.g., `BIT VARYING`, `CHARACTER VARYING`).
    /// Always precedes the precision parens.
    pub varying: Option<VARYING>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub struct ArrayKwSuffix<'input> {
    pub array: ARRAY,
    pub bound: Option<ArraySuffixSized<'input>>,
}

/// NOT IN list: `expr NOT IN (val, ...)` suffix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct NotInSuffix<'input> {
    pub not: NOT,
    pub r#in: IN,
    pub list: InList<'input>,
}

/// Payload for function-style type cast: either a string literal (common
/// case `bool 'value'`) or a psql client variable substitution
/// (`bigint :'txid_current'`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub enum TypeCastValue<'input> {
    String(literal::StringLit<'input>),
    PsqlVar(literal::PsqlVar<'input>),
}

/// Function-style type cast: `bool 'value'`, `text 'hello'`, `char(20) 'text'`,
/// `bigint :'var'`. Uses `CastType` (not bare `TypeName`) to support precision.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct TypeCastFunc<'input> {
    pub type_name: CastType<'input>,
    pub value: TypeCastValue<'input>,
}

/// `WITH TIME ZONE` or `WITHOUT TIME ZONE` suffix for `TIMESTAMP`/`TIME`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub enum TimeZoneQualifier {
    With((WITH, TIME, ZONE)),
    Without((WITHOUT, TIME, ZONE)),
}

/// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct TimestampLit<'input> {
    pub timestamp: TIMESTAMP,
    /// Optional precision, e.g., `timestamp(6)`.
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub value: literal::StringLit<'input>,
}

/// `TIME [WITH|WITHOUT TIME ZONE] 'string'`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct TimeLit<'input> {
    pub time: TIME,
    /// Optional precision, e.g., `time(2)`.
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub value: literal::StringLit<'input>,
}

/// `SECOND [(p)]` — the SECOND keyword with optional fractional-second
/// precision. Used in interval qualifiers like `SECOND(2)` or
/// `DAY TO SECOND(2)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub struct SecondWithPrecision<'input> {
    pub second: SECOND,
    pub precision: Option<TypePrecision<'input>>,
}

/// Optional qualifier after `INTERVAL 'str'`.
///
/// Variant ordering: multi-keyword `X TO Y` forms must come before the
/// single-keyword forms so longest-match-wins picks the fuller qualifier
/// when available. `*ToSecond` variants use `SecondWithPrecision` which
/// allows optional `(p)` precision.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone, PartialEq, Eq)]
#[recursa::parser(rules = SqlRules)]
pub enum IntervalQualifier<'input> {
    YearToMonth((YEAR, TO, MONTH)),
    DayToHour((DAY, TO, HOUR)),
    DayToMinute((DAY, TO, MINUTE)),
    DayToSecond((DAY, TO, SecondWithPrecision<'input>)),
    HourToMinute((HOUR, TO, MINUTE)),
    HourToSecond((HOUR, TO, SecondWithPrecision<'input>)),
    MinuteToSecond((MINUTE, TO, SecondWithPrecision<'input>)),
    Year(YEAR),
    Month(MONTH),
    Day(DAY),
    Hour(HOUR),
    Minute(MINUTE),
    Second(SecondWithPrecision<'input>),
}

/// `INTERVAL 'str' [qualifier]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct IntervalLit<'input> {
    pub interval: INTERVAL,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlNamedArg<'input> {
    pub value: Box<Expr<'input>>,
    pub alias: Option<XmlNamedArgAlias<'input>>,
}

/// `AS alias` suffix on an XML named argument.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlNamedArgAlias<'input> {
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
}

/// `xmlattributes(expr [AS alias], ...)` — used as a positional argument
/// to `xmlelement`, but also can be parsed standalone.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlAttributes<'input> {
    pub kw: XMLATTRIBUTES,
    pub args: Surrounded<punct::LParen, Seq0<XmlNamedArg<'input>, punct::Comma>, punct::RParen>,
}

/// Optional `, xmlattributes(...) [, content_exprs]` tail of `xmlelement`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlElementAttrsTail<'input> {
    pub comma: punct::Comma,
    pub attrs: XmlAttributes<'input>,
    pub content: Option<XmlElementContentTail<'input>>,
}

/// Optional `, content_exprs` tail of `xmlelement`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlElementContentTail<'input> {
    pub comma: punct::Comma,
    pub exprs: Seq0<Expr<'input>, punct::Comma>,
}

/// Body of `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
///
/// Variant ordering: the `WithAttrs` form starts with `, xmlattributes(`
/// (longer match) and must be tried before `WithContent` which starts with
/// just `,`. Both trail an `xmlelement(NAME ident` head.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlElementTail<'input> {
    WithAttrs(XmlElementAttrsTail<'input>),
    WithContent(XmlElementContentTail<'input>),
}

/// Inner contents of an `xmlelement(...)` call.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlElementInner<'input> {
    pub name: NAME,
    pub element_name: literal::AliasName<'input>,
    pub tail: Option<XmlElementTail<'input>>,
}

/// `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlElement<'input> {
    pub kw: XMLELEMENT,
    pub inner: Surrounded<punct::LParen, XmlElementInner<'input>, punct::RParen>,
}

/// `xmlforest(expr [AS alias], ...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlForest<'input> {
    pub kw: XMLFOREST,
    pub args: Surrounded<punct::LParen, Seq0<XmlNamedArg<'input>, punct::Comma>, punct::RParen>,
}

/// `xmlpi(NAME ident [, content])`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlPi<'input> {
    pub kw: XMLPI,
    pub inner: Surrounded<punct::LParen, XmlPiInner<'input>, punct::RParen>,
}

/// Inner contents of an `xmlpi(...)` call.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlPiInner<'input> {
    pub name: NAME,
    pub target: literal::AliasName<'input>,
    pub content: Option<XmlPiContentTail<'input>>,
}

/// Optional `, content_expr` tail of `xmlpi`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlPiContentTail<'input> {
    pub comma: punct::Comma,
    pub expr: Box<Expr<'input>>,
}

// --- More XML function atoms: XMLSERIALIZE / XMLPARSE / XMLROOT / XMLEXISTS ---
//
// Like `xmlelement` etc. these use keyword-laced syntax (`DOCUMENT`/`CONTENT`,
// `VERSION`, `PASSING BY REF`, …) that a plain `FuncCall` cannot express.

/// `DOCUMENT` / `CONTENT` — the XML value category in `XMLSERIALIZE` / `XMLPARSE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlDocOrContent {
    Document(DOCUMENT),
    Content(CONTENT),
}

/// `INDENT` / `NO INDENT` — output indentation option of `XMLSERIALIZE`.
///
/// Variant ordering: `NoIndent` (`NO INDENT`, two tokens) before `Indent`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlIndentOption {
    NoIndent((NO, INDENT)),
    Indent(INDENT),
}

/// Inner of `XMLSERIALIZE ( {DOCUMENT|CONTENT} ‹expr› AS ‹type› [[NO] INDENT] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlSerializeInner<'input> {
    pub which: XmlDocOrContent,
    pub value: Box<Expr<'input>>,
    pub r#as: AS,
    pub ty: CastType<'input>,
    pub indent: Option<XmlIndentOption>,
}

/// `XMLSERIALIZE ( {DOCUMENT|CONTENT} ‹expr› AS ‹type› [[NO] INDENT] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlSerialize<'input> {
    pub kw: XMLSERIALIZE,
    pub inner: Surrounded<punct::LParen, XmlSerializeInner<'input>, punct::RParen>,
}

/// Inner of `XMLPARSE ( {DOCUMENT|CONTENT} ‹expr› )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlParseInner<'input> {
    pub which: XmlDocOrContent,
    pub value: Box<Expr<'input>>,
}

/// `XMLPARSE ( {DOCUMENT|CONTENT} ‹expr› )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlParse<'input> {
    pub kw: XMLPARSE,
    pub inner: Surrounded<punct::LParen, XmlParseInner<'input>, punct::RParen>,
}

/// `VERSION {‹expr› | NO VALUE}` — the version argument of `XMLROOT`.
///
/// Variant ordering: `NoValue` (`NO VALUE`) before the catch-all `Expr`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlVersionValue<'input> {
    NoValue((NO, VALUE)),
    Expr(Box<Expr<'input>>),
}

/// `VERSION {…}` clause of `XMLROOT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlRootVersion<'input> {
    pub version: VERSION,
    pub value: XmlVersionValue<'input>,
}

/// `STANDALONE {YES | NO [VALUE]}`.
///
/// Variant ordering: `NoValue` (`NO VALUE`) before bare `No`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlStandaloneValue {
    Yes(YES),
    NoValue((NO, VALUE)),
    No(NO),
}

/// `, STANDALONE {…}` clause of `XMLROOT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlRootStandalone {
    pub comma: punct::Comma,
    pub standalone: STANDALONE,
    pub value: XmlStandaloneValue,
}

/// Inner of `XMLROOT ( ‹xml› , VERSION {…} [, STANDALONE {…}] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlRootInner<'input> {
    pub value: Box<Expr<'input>>,
    pub comma: punct::Comma,
    pub version: XmlRootVersion<'input>,
    pub standalone: Option<XmlRootStandalone>,
}

/// `XMLROOT ( ‹xml› , VERSION {…} [, STANDALONE {…}] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlRoot<'input> {
    pub kw: XMLROOT,
    pub inner: Surrounded<punct::LParen, XmlRootInner<'input>, punct::RParen>,
}

/// `BY REF` / `BY VALUE` qualifier of an `XMLEXISTS` / `XMLTABLE` PASSING clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlRefOrValue {
    Ref(REF),
    Value(VALUE),
}

/// `BY {REF|VALUE}` qualifier.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlPassingBy {
    pub by: BY,
    pub which: XmlRefOrValue,
}

/// Inner of `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlExistsInner<'input> {
    pub xpath: Box<Expr<'input>>,
    pub passing: PASSING,
    pub by_before: Option<XmlPassingBy>,
    pub doc: Box<Expr<'input>>,
    pub by_after: Option<XmlPassingBy>,
}

/// `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlExists<'input> {
    pub kw: XMLEXISTS,
    pub inner: Surrounded<punct::LParen, XmlExistsInner<'input>, punct::RParen>,
}

/// The tail of an `IS DOCUMENT` predicate: `[NOT] DOCUMENT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IsDocumentTail {
    pub not: Option<NOT>,
    pub document: DOCUMENT,
}

// --- SQL-standard string function atoms ---
//
// TRIM/SUBSTRING/POSITION/OVERLAY use special syntax with FROM/IN/PLACING/FOR
// separators inside parens that don't fit a comma-separated FuncCall.

/// Trim direction: `LEADING | TRAILING | BOTH`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TrimDir {
    Leading(LEADING),
    Trailing(TRAILING),
    Both(BOTH),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TrimTail<'input> {
    /// `FROM expr_list` — explicit-FROM, no leading chars.
    FromArgs(TrimFromArgs<'input>),
    /// `chars FROM source` — explicit-FROM with leading chars.
    WithChars(TrimWithChars<'input>),
    /// `expr_list` — no `FROM`, just the source-and-chars expression list.
    BareArgs(Seq1<Expr<'input>, punct::Comma>),
}

/// `FROM expr_list` tail of `TRIM(...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TrimFromArgs<'input> {
    pub from: FROM,
    pub args: Seq1<Expr<'input>, punct::Comma>,
}

/// `chars FROM source` tail of `TRIM(...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TrimWithChars<'input> {
    pub chars: Box<Expr<'input>>,
    pub from: FROM,
    pub args: Seq1<Expr<'input>, punct::Comma>,
}

/// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TrimCall<'input> {
    pub kw: TRIM,
    pub inner: Surrounded<punct::LParen, TrimInner<'input>, punct::RParen>,
}

/// `FOR len` suffix in `SUBSTRING(... FROM ... FOR ...)` / `OVERLAY(...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForCount<'input> {
    pub r#for: FOR,
    pub count: Box<Expr<'input>>,
}

/// `FROM start [FOR len]` form for SUBSTRING.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubstringFromFor<'input> {
    pub from: FROM,
    pub start: Box<Expr<'input>>,
    pub for_count: Option<ForCount<'input>>,
}

/// `SIMILAR pattern ESCAPE escape` form for SUBSTRING.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubstringSimilar<'input> {
    pub similar: SIMILAR,
    pub pattern: Box<Expr<'input>>,
    pub escape_kw: ESCAPE,
    pub escape: Box<Expr<'input>>,
}

/// Tail of a SUBSTRING call after the source expression.
///
/// Variant ordering: `Similar` (`SIMILAR`) before `FromFor` (`FROM`) — distinct
/// first tokens, so order is not strictly required, but listed by length.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SubstringTail<'input> {
    Similar(SubstringSimilar<'input>),
    FromFor(SubstringFromFor<'input>),
    For(ForCount<'input>),
}

/// Inner of `SUBSTRING(...)`: `source` followed by FROM/SIMILAR tail.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubstringInner<'input> {
    pub source: Box<Expr<'input>>,
    pub tail: SubstringTail<'input>,
}

/// `COLLATION FOR (expr)` — SQL-standard collation introspection.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CollationForCall<'input> {
    pub collation: COLLATION,
    pub r#for: FOR,
    pub arg: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `expr AS cast_type [COLLATE "c"]` — inner of `CAST(...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastAsInner<'input> {
    pub value: Box<Expr<'input>>,
    pub r#as: AS,
    pub target: CastType<'input>,
    pub collate: Option<CollateSuffix<'input>>,
}

/// `COLLATE "name"` suffix appearing after a cast target type.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CollateSuffix<'input> {
    pub collate: COLLATE,
    pub name: crate::tokens::ColId<'input>,
}

/// `CAST(expr AS type [COLLATE "c"])` — SQL-standard cast form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CastCall<'input> {
    pub kw: CAST,
    pub inner: Surrounded<punct::LParen, CastAsInner<'input>, punct::RParen>,
}

/// `SUBSTRING(source FROM start [FOR len])` /
/// `SUBSTRING(source SIMILAR pattern ESCAPE escape)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubstringCall<'input> {
    pub kw: SUBSTRING,
    pub inner: Surrounded<punct::LParen, SubstringInner<'input>, punct::RParen>,
}

/// Inner of `POSITION(needle IN haystack)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PositionInner<'input> {
    pub needle: Box<Expr<'input>>,
    pub r#in: IN,
    pub haystack: Box<Expr<'input>>,
}

/// `POSITION(needle IN haystack)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PositionCall<'input> {
    pub kw: POSITION,
    pub inner: Surrounded<punct::LParen, PositionInner<'input>, punct::RParen>,
}

/// Inner of `OVERLAY(source PLACING new FROM start [FOR len])`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OverlayInner<'input> {
    pub source: Box<Expr<'input>>,
    pub placing: PLACING,
    pub new: Box<Expr<'input>>,
    pub from: FROM,
    pub start: Box<Expr<'input>>,
    pub for_count: Option<ForCount<'input>>,
}

/// `OVERLAY(source PLACING new FROM start [FOR len])`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OverlayCall<'input> {
    pub kw: OVERLAY,
    pub inner: Surrounded<punct::LParen, OverlayInner<'input>, punct::RParen>,
}

/// Field argument of `EXTRACT(field FROM source)`.
///
/// Variant ordering: `StringLit` before `Ident` — string literal has a
/// distinct first token (`'`) so order is not strictly required; listed
/// first to match the Postgres docs ordering.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ExtractField<'input> {
    StringLit(StringLitSeq0<'input>),
    Ident(literal::AliasName<'input>),
}

/// Inner of `EXTRACT(field FROM source)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExtractInner<'input> {
    pub field: ExtractField<'input>,
    pub from: FROM,
    pub source: Box<Expr<'input>>,
}

/// `EXTRACT(field FROM source)` — Postgres-specific function syntax.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ExtractCall<'input> {
    pub kw: EXTRACT,
    pub inner: Surrounded<punct::LParen, ExtractInner<'input>, punct::RParen>,
}

/// `UESCAPE 'c'` suffix that may follow a `U&'...'` literal.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UescapeSuffix<'input> {
    pub uescape: UESCAPE,
    pub escape_char: literal::StringLit<'input>,
}

/// `U&'...'` unicode string literal with optional `UESCAPE 'c'` suffix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UnicodeStringLitWithEscape<'input> {
    pub lit: literal::UnicodeStringLit<'input>,
    pub uescape: Option<UescapeSuffix<'input>>,
}

/// `ESCAPE expr` clause on LIKE / SIMILAR TO / ILIKE operators.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct EscapeClause<'input> {
    pub escape: ESCAPE,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonEncoding<'input> {
    pub encoding: ENCODING,
    pub name: literal::AliasName<'input>,
}

/// `FORMAT JSON [ENCODING ‹name›]` — SQL/JSON input/output format specifier.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonFormat<'input> {
    pub format: FORMAT,
    pub json: JSON,
    pub encoding: Option<JsonEncoding<'input>>,
}

/// `RETURNING ‹data_type› [FORMAT JSON [ENCODING ...]]` — output type clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonReturning<'input> {
    pub returning: RETURNING,
    pub ty: CastType<'input>,
    pub format: Option<JsonFormat<'input>>,
}

/// `WITH` / `WITHOUT` lead-in of a `UNIQUE KEYS` constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WithOrWithout {
    With(WITH),
    Without(WITHOUT),
}

/// `{WITH|WITHOUT} UNIQUE [KEYS]` — duplicate-key handling for `JSON()` /
/// `JSON_OBJECT()`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonUniqueKeys {
    pub with_or_without: WithOrWithout,
    pub unique: UNIQUE,
    pub keys: Option<KEYS>,
}

/// `NULL` / `ABSENT` lead-in of an `ON NULL` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NullOrAbsent {
    Null(NULL),
    Absent(ABSENT),
}

/// `{NULL|ABSENT} ON NULL` — null-input handling for `JSON_OBJECT()` /
/// `JSON_ARRAY()`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonOnNull {
    pub which: NullOrAbsent,
    pub on: ON,
    pub null: NULL,
}

/// Inner contents of `JSON ( ‹expr› [FORMAT JSON ...] [{WITH|WITHOUT} UNIQUE [KEYS]] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonConstructorInner<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
    pub unique: Option<JsonUniqueKeys>,
}

/// `JSON ( ‹expr› [FORMAT JSON ...] [{WITH|WITHOUT} UNIQUE [KEYS]] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonConstructor<'input> {
    pub kw: JSON,
    pub inner: Surrounded<punct::LParen, JsonConstructorInner<'input>, punct::RParen>,
}

/// `JSON_SCALAR ( ‹expr› )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonScalar<'input> {
    pub kw: JSON_SCALAR,
    pub inner: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// Inner contents of `JSON_SERIALIZE ( ‹expr› [FORMAT JSON ...] [RETURNING ...] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonSerializeInner<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_SERIALIZE ( ‹expr› [FORMAT JSON ...] [RETURNING ‹type› ...] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonSerialize<'input> {
    pub kw: JSON_SERIALIZE,
    pub inner: Surrounded<punct::LParen, JsonSerializeInner<'input>, punct::RParen>,
}

/// Key/value separator inside a `JSON_OBJECT` entry: `:` or the `VALUE` keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JsonKeyValueSep {
    Colon(punct::Colon),
    Value(VALUE),
}

/// One `[KEY] ‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry of `JSON_OBJECT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonObjectEntry<'input> {
    pub key_kw: Option<KEY>,
    pub key: Box<Expr<'input>>,
    pub sep: JsonKeyValueSep,
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
}

/// Inner contents of `JSON_OBJECT`: zero or more entries followed by the
/// optional `ON NULL`, `UNIQUE` and `RETURNING` clauses. The empty form
/// (`JSON_OBJECT()`) and the returning-only form (`JSON_OBJECT(RETURNING ...)`)
/// both fall out of `Seq0` accepting zero entries.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonObjectArgs<'input> {
    // `Option<Seq1<…>>` (not `Seq0`) so the entry list is fork-and-tried:
    // `Expr::peek` is keyword-permissive (the `QualRef` atom leads with a
    // keyword-accepting `AliasName`), so a `Seq0` element gate would
    // over-commit on a trailing `RETURNING`/`)` and then hard-fail. The
    // `Option` swallows that, leaving the cursor for the clauses below.
    pub entries: Option<Seq1<JsonObjectEntry<'input>, punct::Comma>>,
    pub on_null: Option<JsonOnNull>,
    pub unique: Option<JsonUniqueKeys>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_OBJECT ( [entries] [{NULL|ABSENT} ON NULL] [{WITH|WITHOUT} UNIQUE [KEYS]] [RETURNING ...] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonObject<'input> {
    pub kw: JSON_OBJECT,
    pub args: Surrounded<punct::LParen, JsonObjectArgs<'input>, punct::RParen>,
}

/// One `‹expr› [FORMAT JSON ...]` element of a `JSON_ARRAY` element list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JsonArrayBody<'input> {
    Query(Box<Subquery<'input>>),
    Elements(Seq1<JsonArrayElement<'input>, punct::Comma>),
}

/// Inner contents of `JSON_ARRAY`: an optional value part (subquery or
/// element list) followed by the optional `ON NULL` and `RETURNING` clauses.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonArrayArgs<'input> {
    pub body: Option<JsonArrayBody<'input>>,
    pub on_null: Option<JsonOnNull>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_ARRAY ( ... )` — element-list or query form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonArray<'input> {
    pub kw: JSON_ARRAY,
    pub args: Surrounded<punct::LParen, JsonArrayArgs<'input>, punct::RParen>,
}

// --- SQL/JSON query function atoms ---
//
// `JSON_EXISTS()`, `JSON_VALUE()` and `JSON_QUERY()` test/extract values from
// a JSON context item using a jsonpath. Like the constructors they are
// grammar constructs with `PASSING`, `RETURNING`, wrapper/quotes and
// `ON EMPTY`/`ON ERROR` behavior clauses that no function-argument list can
// express. Modeled as dedicated atoms before `Func`.

/// One `‹value› AS ‹name›` binding of a `PASSING` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonPassingArg<'input> {
    pub value: Box<Expr<'input>>,
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
}

/// `PASSING ‹value› AS ‹name› [, ...]` — jsonpath variable bindings.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonPassing<'input> {
    pub passing: PASSING,
    pub args: Seq1<JsonPassingArg<'input>, punct::Comma>,
}

/// `DEFAULT ‹expr›` — the default-value form of an `ON EMPTY`/`ON ERROR` behavior.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonDefault<'input> {
    pub default: DEFAULT,
    pub value: Box<Expr<'input>>,
}

/// The behavior of an `ON EMPTY` / `ON ERROR` clause — the union of every
/// query function's accepted behaviors (`JSON_EXISTS` uses the boolean
/// forms, `JSON_VALUE`/`JSON_QUERY` the rest). Parsed permissively; which
/// behaviors are valid for which function is Postgres's concern.
///
/// Variant ordering: the two-keyword `EMPTY ARRAY`/`EMPTY OBJECT` forms
/// before bare `Empty` so longest-match-wins picks them.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JsonBehavior<'input> {
    EmptyArray((EMPTY, ARRAY)),
    EmptyObject((EMPTY, OBJECT)),
    Empty(EMPTY),
    Error(ERROR),
    Null(NULL),
    True(TRUE),
    False(FALSE),
    Unknown(UNKNOWN),
    Default(JsonDefault<'input>),
}

/// `EMPTY` or `ERROR` — the trigger of an `ON` behavior clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum EmptyOrError {
    Empty(EMPTY),
    Error(ERROR),
}

/// `‹behavior› ON {EMPTY|ERROR}` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonOnBehavior<'input> {
    pub behavior: JsonBehavior<'input>,
    pub on: ON,
    pub trigger: EmptyOrError,
}

/// `CONDITIONAL` / `UNCONDITIONAL` modifier of a `WITH ... WRAPPER` clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WrapperBehavior {
    Conditional(CONDITIONAL),
    Unconditional(UNCONDITIONAL),
}

/// `{WITH [CONDITIONAL|UNCONDITIONAL] | WITHOUT} [ARRAY] WRAPPER` — the
/// `JSON_QUERY` array-wrapper clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonWrapper {
    pub with_or_without: WithOrWithout,
    pub behavior: Option<WrapperBehavior>,
    pub array: Option<ARRAY>,
    pub wrapper: WRAPPER,
}

/// `ON SCALAR STRING` suffix of a `JSON_QUERY` quotes clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonQuotesOnScalar {
    pub on: ON,
    pub scalar: SCALAR,
    pub string: STRING,
}

/// `KEEP` / `OMIT` lead-in of a `JSON_QUERY` quotes clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum KeepOrOmit {
    Keep(KEEP),
    Omit(OMIT),
}

/// `{KEEP|OMIT} QUOTES [ON SCALAR STRING]` — the `JSON_QUERY` quotes clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonQuotes {
    pub keep_or_omit: KeepOrOmit,
    pub quotes: QUOTES,
    pub on_scalar: Option<JsonQuotesOnScalar>,
}

/// Inner contents of `JSON_EXISTS ( ‹context› , ‹path› [PASSING ...] [‹behavior› ON ERROR] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonExistsInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    pub comma: punct::Comma,
    pub path: Box<Expr<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub on_error: Option<JsonOnBehavior<'input>>,
}

/// `JSON_EXISTS ( ... )` — tests whether a jsonpath matches.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonExists<'input> {
    pub kw: JSON_EXISTS,
    pub inner: Surrounded<punct::LParen, JsonExistsInner<'input>, punct::RParen>,
}

/// Inner contents of `JSON_VALUE`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonValueInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    pub comma: punct::Comma,
    pub path: Box<Expr<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub returning: Option<JsonReturning<'input>>,
    // Two generic behavior slots: each `JsonOnBehavior` self-identifies its
    // `ON EMPTY` / `ON ERROR` trigger, so the pair is order-independent.
    pub on_behavior_1: Option<JsonOnBehavior<'input>>,
    pub on_behavior_2: Option<JsonOnBehavior<'input>>,
}

/// `JSON_VALUE ( ... )` — extracts a scalar SQL value via a jsonpath.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonValue<'input> {
    pub kw: JSON_VALUE,
    pub inner: Surrounded<punct::LParen, JsonValueInner<'input>, punct::RParen>,
}

/// Inner contents of `JSON_QUERY`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonQueryInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    pub comma: punct::Comma,
    pub path: Box<Expr<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub returning: Option<JsonReturning<'input>>,
    pub wrapper: Option<JsonWrapper>,
    pub quotes: Option<JsonQuotes>,
    pub on_behavior_1: Option<JsonOnBehavior<'input>>,
    pub on_behavior_2: Option<JsonOnBehavior<'input>>,
}

/// `JSON_QUERY ( ... )` — extracts a JSON value via a jsonpath.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonQuery<'input> {
    pub kw: JSON_QUERY,
    pub inner: Surrounded<punct::LParen, JsonQueryInner<'input>, punct::RParen>,
}

// --- SQL/JSON aggregate atoms ---
//
// `JSON_OBJECTAGG()` and `JSON_ARRAYAGG()` aggregate rows into a JSON object
// or array. They are grammar constructs (the object form takes a `key :
// value` entry, the array form an `ORDER BY`) and, being aggregates, accept
// the ordinary `FILTER (WHERE ...)` and `OVER (...)` suffixes.

/// Inner contents of `JSON_OBJECTAGG`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonObjectAggInner<'input> {
    pub entry: JsonObjectEntry<'input>,
    pub on_null: Option<JsonOnNull>,
    pub unique: Option<JsonUniqueKeys>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_OBJECTAGG ( ‹key› {: | VALUE} ‹value› ... ) [FILTER (...)] [OVER (...)]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonObjectAgg<'input> {
    pub kw: JSON_OBJECTAGG,
    pub inner: Surrounded<punct::LParen, JsonObjectAggInner<'input>, punct::RParen>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// Inner contents of `JSON_ARRAYAGG`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonArrayAggInner<'input> {
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub on_null: Option<JsonOnNull>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_ARRAYAGG ( ‹value› [ORDER BY ...] ... ) [FILTER (...)] [OVER (...)]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonArrayAgg<'input> {
    pub kw: JSON_ARRAYAGG,
    pub inner: Surrounded<punct::LParen, JsonArrayAggInner<'input>, punct::RParen>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

// --- `IS JSON` predicate ---

/// The JSON item type tested by an `IS JSON` predicate.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JsonTypeKind {
    Value(VALUE),
    Scalar(SCALAR),
    Array(ARRAY),
    Object(OBJECT),
}

/// The tail of an `IS JSON` predicate: `[NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}]
/// [{WITH|WITHOUT} UNIQUE [KEYS]]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct IsJsonTail {
    pub not: Option<NOT>,
    pub json: JSON,
    pub type_kind: Option<JsonTypeKind>,
    pub unique: Option<JsonUniqueKeys>,
}

/// Any value-producing SQL/JSON function — the constructors and query
/// functions grouped into one peekable type. Each variant leads with a
/// distinct soft keyword, so this peeks `true` only for a JSON function.
/// Lets non-Pratt contexts (e.g. a `CREATE INDEX` expression element)
/// accept the whole family. Aggregates and `JSON_TABLE` are excluded:
/// neither is a plain value expression usable as an index element.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Debug, Clone, Visit, Transform)]
#[recursa::parser(rules = SqlRules, pratt)]
pub enum Expr<'input> {
    // --- Prefix ---
    #[parse(prefix, bp = 15)]
    Not(NOT, Box<Expr<'input>>),
    #[parse(prefix, bp = 12)]
    Neg(punct::Minus, Box<Expr<'input>>),
    /// Unary plus: `+expr` — identity operator on numeric types.
    #[parse(prefix, bp = 12)]
    Pos(punct::Plus, Box<Expr<'input>>),
    /// Unary geometric "center point": `@@ expr`. Postgres uses `@@` as
    /// a prefix operator on box / polygon / etc. (in addition to the
    /// text-search infix form).
    #[parse(prefix, bp = 12)]
    GeomCenter(punct::AtAt, Box<Expr<'input>>),
    /// Bitwise NOT: `~ expr` (e.g. inet / bit / int bitwise complement).
    /// Must come before any infix `~` variant so the prefix form wins when
    /// `~` appears at the start of an operand.
    #[parse(prefix, bp = 12)]
    BitNot(punct::Tilde, Box<Expr<'input>>),
    /// Geometric path/lseg length: `@-@ expr`. Must come before `Abs` (`@`)
    /// since `@-@` is longer.
    #[parse(prefix, bp = 12)]
    PathLength(punct::AtMinusAt, Box<Expr<'input>>),
    /// User-defined prefix: `@#@ expr` (e.g. factorial).
    #[parse(prefix, bp = 12)]
    AtHashAtPrefix(punct::AtHashAt, Box<Expr<'input>>),
    /// Geometric point-count: `# path` — number of points in a path.
    #[parse(prefix, bp = 12)]
    PointCount(punct::Pound, Box<Expr<'input>>),
    /// Absolute value: `@ expr` (Postgres unary `@` operator).
    #[parse(prefix, bp = 12)]
    Abs(punct::At, Box<Expr<'input>>),
    /// User-defined prefix: `!=- expr`.
    #[parse(prefix, bp = 12)]
    BangEqMinusPrefix(punct::BangEqMinus, Box<Expr<'input>>),
    /// Square root: `|/ expr` (Postgres unary `|/` operator).
    #[parse(prefix, bp = 12)]
    Sqrt(punct::PipeSlash, Box<Expr<'input>>),
    /// Cube root: `||/ expr` (Postgres unary `||/` operator).
    #[parse(prefix, bp = 12)]
    Cbrt(punct::PipePipeSlash, Box<Expr<'input>>),

    /// Catch-all prefix: any user-defined prefix operator not matched by a
    /// specific token. Declared LAST among prefixes.
    #[parse(prefix, bp = 12)]
    CustomPrefix(literal::CustomOp<'input>, Box<Expr<'input>>),

    // --- Postfix ---
    /// Postgres-style cast: `expr::type`
    #[parse(postfix, bp = 20)]
    Cast(Box<Expr<'input>>, punct::ColonColon, Box<CastType<'input>>),
    /// Composite field-star access: `(expr).*` — expand a composite/record
    /// value into its columns. Declared before `FieldAccess` so the longer
    /// `.*` form wins.
    #[parse(postfix, bp = 20)]
    FieldStar(Box<Expr<'input>>, punct::Dot, punct::Star),
    /// Composite field access: `(expr).field` — project one column from a
    /// composite/record value.
    #[parse(postfix, bp = 20)]
    FieldAccess(Box<Expr<'input>>, punct::Dot, literal::AliasName<'input>),
    /// Array slice: `expr[low:high]`, `expr[:high]`, `expr[low:]`, `expr[:]`.
    /// Declared before `Subscript` so the colon-containing form is tried first
    /// when both peek `[`.
    #[parse(postfix, bp = 20)]
    Slice(
        Box<Expr<'input>>,
        punct::LBracket,
        SubscriptSlice<'input>,
        punct::RBracket,
    ),
    /// Array subscript: `expr[idx]`
    #[parse(postfix, bp = 20)]
    Subscript(
        Box<Expr<'input>>,
        punct::LBracket,
        Box<Expr<'input>>,
        punct::RBracket,
    ),
    /// `expr COLLATE "collation"` — collation specifier. Binds tighter than
    /// comparisons (bp 5) but looser than `::` cast (bp 20).
    #[parse(postfix, bp = 18)]
    Collate(Box<Expr<'input>>, COLLATE, crate::tokens::ColId<'input>),
    /// `expr IS NOT DISTINCT FROM expr`. Declared before `IsDistinctFrom` so
    /// the longer `NOT` prefix wins disambiguation.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    IsNotDistinctFrom(
        Box<Expr<'input>>,
        IS,
        NOT,
        DISTINCT,
        FROM,
        Box<Expr<'input>>,
    ),
    /// `expr IS DISTINCT FROM expr`.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    IsDistinctFrom(Box<Expr<'input>>, (IS, DISTINCT, FROM), Box<Expr<'input>>),
    /// `expr IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] [{WITH|WITHOUT}
    /// UNIQUE [KEYS]]` — the SQL/JSON type predicate. Declared before
    /// `BoolTest` (both lead with `IS`); `BoolTest` rejects `JSON` as a
    /// `BoolTestKind`, so order is not load-bearing, only tidy.
    #[parse(postfix, bp = 8)]
    IsJson(Box<Expr<'input>>, IS, IsJsonTail),
    /// `expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` — the Unicode
    /// normalisation predicate (gram.y rules 15198/15205/15212/15220).
    /// Declared before `BoolTest` (both lead with `IS`); `BoolTest` rejects
    /// `NORMALIZED`/`NFx` as a `BoolTestKind`, so order is not load-bearing.
    #[parse(postfix, bp = 8)]
    IsNormalized(Box<Expr<'input>>, IS, IsNormalizedTail),
    /// `expr IS [NOT] DOCUMENT` — the XML document predicate.
    #[parse(postfix, bp = 8)]
    IsDocument(Box<Expr<'input>>, IS, IsDocumentTail),
    /// Boolean test: `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`
    #[parse(postfix, bp = 8)]
    BoolTest(Box<Expr<'input>>, IS, BoolTestKind),
    /// Postgres `expr NOTNULL` postfix null test (synonym for `IS NOT NULL`).
    #[parse(postfix, bp = 8)]
    Notnull(Box<Expr<'input>>, NOTNULL),
    /// Postgres `expr ISNULL` postfix null test (synonym for `IS NULL`).
    #[parse(postfix, bp = 8)]
    Isnull(Box<Expr<'input>>, ISNULL),
    /// `expr AT LOCAL` — convert to session timezone. Listed before
    /// `AtTimeZone` so `AT LOCAL` wins (distinct second token `LOCAL` vs `TIME`).
    #[parse(postfix, bp = 9)]
    AtLocal(Box<Expr<'input>>, AT, LOCAL),
    /// `expr AT TIME ZONE zone_expr` — convert to specified timezone.
    #[parse(postfix, bp = 9, inner_bp = 10)]
    AtTimeZone(Box<Expr<'input>>, AT, TIME, ZONE, Box<Expr<'input>>),
    /// NOT IN list: `expr NOT IN (val, ...)`
    #[parse(postfix, bp = 6)]
    NotInExpr(Box<Expr<'input>>, NotInSuffix<'input>),
    /// `expr NOT ILIKE pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT ILIKE` is tried first (matters only if any rule shares a prefix;
    /// here `NOT ILIKE` vs `NOT LIKE` differ on the second token).
    #[parse(postfix, bp = 5, inner_bp = 6)]
    NotIlike(
        Box<Expr<'input>>,
        NOT,
        ILIKE,
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr NOT SIMILAR TO pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT SIMILAR TO` form wins longest-match-wins disambiguation.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    NotSimilarTo(
        Box<Expr<'input>>,
        NOT,
        SIMILAR,
        TO,
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr NOT LIKE pattern [ESCAPE char]`. Must come before the `Not` prefix atom so
    /// longest-match-wins prefers the postfix form.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    NotLike(
        Box<Expr<'input>>,
        NOT,
        LIKE,
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr SIMILAR TO pattern [ESCAPE char]` — SQL standard similar-to pattern match.
    #[parse(postfix, bp = 5, inner_bp = 6)]
    SimilarTo(
        Box<Expr<'input>>,
        SIMILAR,
        TO,
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr ILIKE pattern [ESCAPE char]`
    #[parse(postfix, bp = 5, inner_bp = 6)]
    Ilike(
        Box<Expr<'input>>,
        ILIKE,
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr LIKE pattern [ESCAPE char]`
    #[parse(postfix, bp = 5, inner_bp = 6)]
    Like(
        Box<Expr<'input>>,
        LIKE,
        Box<Expr<'input>>,
        Option<EscapeClause<'input>>,
    ),
    // --- Locale-aware text comparison operators (4-char before 3-char) ---
    /// `expr ~<=~ expr` — locale-aware less-or-equal.
    #[parse(infix, bp = 5)]
    TildeLeqTilde(Box<Expr<'input>>, punct::TildeLeqTilde, Box<Expr<'input>>),
    /// `expr ~>=~ expr` — locale-aware greater-or-equal.
    #[parse(infix, bp = 5)]
    TildeGeqTilde(Box<Expr<'input>>, punct::TildeGeqTilde, Box<Expr<'input>>),
    /// `expr ~<~ expr` — locale-aware less-than.
    #[parse(infix, bp = 5)]
    TildeLtTilde(Box<Expr<'input>>, punct::TildeLtTilde, Box<Expr<'input>>),
    /// `expr ~>~ expr` — locale-aware greater-than.
    #[parse(infix, bp = 5)]
    TildeGtTilde(Box<Expr<'input>>, punct::TildeGtTilde, Box<Expr<'input>>),
    /// `expr !~* pattern` — POSIX case-insensitive negated regex match.
    #[parse(infix, bp = 5)]
    RegexNotIMatch(Box<Expr<'input>>, punct::BangTildeStar, Box<Expr<'input>>),
    /// `expr ~* pattern` — POSIX case-insensitive regex match.
    #[parse(infix, bp = 5)]
    RegexIMatch(Box<Expr<'input>>, punct::TildeStar, Box<Expr<'input>>),
    /// `expr !~ pattern` — POSIX negated regex match.
    #[parse(infix, bp = 5)]
    RegexNotMatch(Box<Expr<'input>>, punct::BangTilde, Box<Expr<'input>>),
    /// `expr ~= expr` — geometric "same as" operator. Declared before `RegexMatch`
    /// so the longer `~=` wins longest-match.
    #[parse(infix, bp = 5)]
    GeomSame(Box<Expr<'input>>, punct::TildeEq, Box<Expr<'input>>),
    /// `expr ~ pattern` — POSIX regex match.
    #[parse(infix, bp = 5)]
    RegexMatch(Box<Expr<'input>>, punct::Tilde, Box<Expr<'input>>),
    /// `expr !~~* pattern` — operator-form `NOT ILIKE` (gram.y 14897).
    /// Declared before `LikeOpINeg` (`!~~`) so the longer `!~~*` wins.
    #[parse(infix, bp = 5)]
    LikeOpINeg(
        Box<Expr<'input>>,
        punct::BangTildeTildeStar,
        Box<Expr<'input>>,
    ),
    /// `expr ~~* pattern` — operator-form `ILIKE` (gram.y 14888).
    /// Declared before `LikeOpI` would be (no `~~*` longer prefix).
    #[parse(infix, bp = 5)]
    LikeOpI(Box<Expr<'input>>, punct::TildeTildeStar, Box<Expr<'input>>),
    /// `expr !~~ pattern` — operator-form `NOT LIKE` (gram.y 14874).
    #[parse(infix, bp = 5)]
    LikeOpNeg(Box<Expr<'input>>, punct::BangTildeTilde, Box<Expr<'input>>),
    /// `expr ~~ pattern` — operator-form `LIKE` (gram.y 14860).
    #[parse(infix, bp = 5)]
    LikeOp(Box<Expr<'input>>, punct::TildeTilde, Box<Expr<'input>>),
    /// `(start, end) OVERLAPS (start, end)` — SQL time-period overlap test.
    /// Each operand is an ordinary parenthesized expression to the parser.
    #[parse(infix, bp = 5)]
    Overlaps(Box<Expr<'input>>, OVERLAPS, Box<Expr<'input>>),
    /// Record comparison operators: `expr *= expr`, `*<>`, `*<`, `*<=`,
    /// `*>`, `*>=` — compare ROW/composite values field by field.
    #[parse(infix, bp = 5)]
    RecordLte(Box<Expr<'input>>, punct::StarLte, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordGte(Box<Expr<'input>>, punct::StarGte, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordNeq(Box<Expr<'input>>, punct::StarNeq, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordLt(Box<Expr<'input>>, punct::StarLt, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordGt(Box<Expr<'input>>, punct::StarGt, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    RecordEq(Box<Expr<'input>>, punct::StarEq, Box<Expr<'input>>),
    /// IN list: `expr IN (val, ...)`
    #[parse(postfix, bp = 6)]
    InExpr(Box<Expr<'input>>, IN, InList<'input>),
    /// `expr NOT BETWEEN low AND high`. Declared before `BetweenExpr` so
    /// the longer `NOT BETWEEN` prefix wins disambiguation. `inner_bp = 3`
    /// keeps the low/high operands from swallowing the literal `AND` that
    /// separates them (the `AND` infix has `bp = 2`).
    #[parse(postfix, bp = 6, inner_bp = 3)]
    NotBetweenExpr(
        Box<Expr<'input>>,
        NOT,
        BETWEEN,
        Box<Expr<'input>>,
        AND,
        Box<Expr<'input>>,
    ),
    /// `expr BETWEEN low AND high`. See `NotBetweenExpr` for the
    /// `inner_bp` rationale.
    #[parse(postfix, bp = 6, inner_bp = 3)]
    BetweenExpr(
        Box<Expr<'input>>,
        BETWEEN,
        Box<Expr<'input>>,
        AND,
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
    JsonPathText(Box<Expr<'input>>, punct::HashArrowArrow, Box<Expr<'input>>),
    /// JSON path: `expr #> path`
    #[parse(infix, bp = 10)]
    JsonPath(Box<Expr<'input>>, punct::HashArrow, Box<Expr<'input>>),
    /// JSON field as text: `expr ->> field`
    #[parse(infix, bp = 10)]
    JsonFieldText(Box<Expr<'input>>, punct::ArrowArrow, Box<Expr<'input>>),
    /// JSON field: `expr -> field`
    #[parse(infix, bp = 10)]
    JsonField(Box<Expr<'input>>, punct::Arrow, Box<Expr<'input>>),
    /// Geometric parallel: `a ?|| b`. Must precede `JsonAnyKey` (`?|`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, bp = 5)]
    Parallel(
        Box<Expr<'input>>,
        punct::QuestionPipePipe,
        Box<Expr<'input>>,
    ),
    /// JSON any-key-exists: `expr ?| keys`
    #[parse(infix, bp = 10)]
    JsonAnyKey(Box<Expr<'input>>, punct::QuestionPipe, Box<Expr<'input>>),
    /// JSON all-keys-exist: `expr ?& keys`
    #[parse(infix, bp = 10)]
    JsonAllKeys(Box<Expr<'input>>, punct::QuestionAmp, Box<Expr<'input>>),
    /// Geometric intersect: `a ?# b`. Must precede `JsonKey` (`?`).
    #[parse(infix, bp = 5)]
    Intersect(Box<Expr<'input>>, punct::QuestionHash, Box<Expr<'input>>),
    /// Geometric perpendicular: `a ?-| b`. Must precede `Horizontal` (`?-`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, bp = 5)]
    Perpendicular(
        Box<Expr<'input>>,
        punct::QuestionDashPipe,
        Box<Expr<'input>>,
    ),
    /// Geometric horizontal: `a ?- b`. Must precede `JsonKey` (`?`).
    #[parse(infix, bp = 5)]
    Horizontal(Box<Expr<'input>>, punct::QuestionDash, Box<Expr<'input>>),
    /// Geometric "is horizontal" prefix: `?- s` — tests whether the
    /// LSEG/LINE `s` is horizontal. PG's geometry.sql uses this in WHERE.
    #[parse(prefix, bp = 12)]
    IsHorizontal(punct::QuestionDash, Box<Expr<'input>>),
    /// Geometric "is vertical" prefix: `?| s`.
    #[parse(prefix, bp = 12)]
    IsVertical(punct::QuestionPipe, Box<Expr<'input>>),
    /// Geometric "below": `a <^ b`.
    #[parse(infix, bp = 5)]
    Below(Box<Expr<'input>>, punct::LtCaret, Box<Expr<'input>>),
    /// Geometric "above": `a >^ b`.
    #[parse(infix, bp = 5)]
    Above(Box<Expr<'input>>, punct::GtCaret, Box<Expr<'input>>),
    /// JSON key-exists: `expr ? key`
    #[parse(infix, bp = 10)]
    JsonKey(Box<Expr<'input>>, punct::Question, Box<Expr<'input>>),
    /// JSONB contains: `expr @> expr`
    #[parse(infix, bp = 10)]
    JsonContains(Box<Expr<'input>>, punct::AtGt, Box<Expr<'input>>),
    /// JSONB contained-by: `expr <@ expr`
    #[parse(infix, bp = 10)]
    JsonContainedBy(Box<Expr<'input>>, punct::LtAt, Box<Expr<'input>>),

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
    TsMatch3(Box<Expr<'input>>, punct::AtAtAt, Box<Expr<'input>>),
    /// User-defined triple-less-than: `a <<< b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, bp = 5)]
    TripleLt(Box<Expr<'input>>, punct::LtLtLt, Box<Expr<'input>>),
    /// Geometric strictly-below: `a <<| b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, bp = 5)]
    StrictlyBelow(Box<Expr<'input>>, punct::LtLtPipe, Box<Expr<'input>>),
    /// Inet is-subset-or-equal: `a <<= b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, bp = 5)]
    SubsetEq(Box<Expr<'input>>, punct::LtLtEq, Box<Expr<'input>>),
    /// Distance: `a <-> b`. Before any `<` variant.
    #[parse(infix, bp = 10)]
    Distance(Box<Expr<'input>>, punct::LtMinusGt, Box<Expr<'input>>),
    /// User-defined triple-greater-than: `a >>> b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, bp = 5)]
    TripleGt(Box<Expr<'input>>, punct::GtGtGt, Box<Expr<'input>>),
    /// Inet is-superset-or-equal: `a >>= b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, bp = 5)]
    SupersetEq(Box<Expr<'input>>, punct::GtGtEq, Box<Expr<'input>>),
    /// Range adjacent: `a -|- b`. Before `Sub` (`-`).
    #[parse(infix, bp = 5)]
    Adjacent(Box<Expr<'input>>, punct::MinusPipeMinus, Box<Expr<'input>>),
    /// Geometric strictly-above: `a |>> b`. Before `Concat` (`||`).
    #[parse(infix, bp = 5)]
    StrictlyAbove(Box<Expr<'input>>, punct::PipeGtGt, Box<Expr<'input>>),
    /// Geometric no-extend-below: `a |&> b`. Before `Concat` (`||`).
    #[parse(infix, bp = 5)]
    NoExtendBelow(Box<Expr<'input>>, punct::PipeAmpGt, Box<Expr<'input>>),
    /// Geometric no-extend-above: `a &<| b`. Before `NoExtendRight` (`&<`).
    #[parse(infix, bp = 5)]
    NoExtendAbove(Box<Expr<'input>>, punct::AmpLtPipe, Box<Expr<'input>>),

    // --- 2-char operators ---
    /// Text-search / jsonb path match: `expr @@ expr`.
    #[parse(infix, bp = 5)]
    TsMatch(Box<Expr<'input>>, punct::AtAt, Box<Expr<'input>>),
    /// Jsonpath exists: `expr @? path`.
    #[parse(infix, bp = 5)]
    JsonPathExists(Box<Expr<'input>>, punct::AtQuestion, Box<Expr<'input>>),
    /// Range / array overlap: `a && b`.
    #[parse(infix, bp = 10)]
    Overlap(Box<Expr<'input>>, punct::AmpAmp, Box<Expr<'input>>),
    /// Range does-not-extend-right: `a &< b`.
    #[parse(infix, bp = 5)]
    NoExtendRight(Box<Expr<'input>>, punct::AmpLt, Box<Expr<'input>>),
    /// Range does-not-extend-left: `a &> b`.
    #[parse(infix, bp = 5)]
    NoExtendLeft(Box<Expr<'input>>, punct::AmpGt, Box<Expr<'input>>),
    /// Range strictly-left-of: `a << b`.
    #[parse(infix, bp = 5)]
    StrictlyLeft(Box<Expr<'input>>, punct::LtLt, Box<Expr<'input>>),
    /// Range strictly-right-of: `a >> b`.
    #[parse(infix, bp = 5)]
    StrictlyRight(Box<Expr<'input>>, punct::GtGt, Box<Expr<'input>>),

    // --- User-defined / custom infix operators ---
    /// `expr === expr` — user-defined triple-equal. Must come before `Eq` (`=`).
    #[parse(infix, bp = 5)]
    TripleEq(Box<Expr<'input>>, punct::TripleEq, Box<Expr<'input>>),
    /// `expr !== expr` — user-defined not-equal. Must come before `BangEq` (`!=`).
    #[parse(infix, bp = 5)]
    BangEqEq(Box<Expr<'input>>, punct::BangEqEq, Box<Expr<'input>>),
    /// `expr ## expr` — geometric closest-point / path intersection.
    /// Must come before `BitXor` (`#`).
    #[parse(infix, bp = 5)]
    GeomClosest(Box<Expr<'input>>, punct::HashHash, Box<Expr<'input>>),

    #[parse(infix, bp = 1)]
    Or(Box<Expr<'input>>, OR, Box<Expr<'input>>),
    #[parse(infix, bp = 2)]
    And(Box<Expr<'input>>, AND, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    BangEq(Box<Expr<'input>>, punct::BangEq, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Neq(Box<Expr<'input>>, punct::Neq, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Lte(Box<Expr<'input>>, punct::Lte, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Gte(Box<Expr<'input>>, punct::Gte, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Eq(Box<Expr<'input>>, punct::Eq, Box<Expr<'input>>),

    /// Text starts-with: `expr ^@ expr` (PostgreSQL `starts_with` operator).
    /// `^@` is a single token (see `punct::CaretAt`); declared before
    /// `CustomInfix` so it wins the declaration-order tiebreak. bp=8 matches
    /// Postgres's generic `Op` precedence.
    #[parse(infix, bp = 8)]
    StartsWith(Box<Expr<'input>>, punct::CaretAt, Box<Expr<'input>>),
    /// JSONB delete-path: `expr #- path` (PostgreSQL jsonb delete-at-path
    /// operator). `#-` is a single token (see `punct::HashMinus`); declared
    /// before `CustomInfix` so it wins the declaration-order tiebreak. bp=10
    /// matches the neighbouring `#>`/`#>>` JSON path operators.
    #[parse(infix, bp = 10)]
    JsonDeletePath(Box<Expr<'input>>, punct::HashMinus, Box<Expr<'input>>),

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
    Lt(Box<Expr<'input>>, punct::Lt, Box<Expr<'input>>),
    #[parse(infix, bp = 5)]
    Gt(Box<Expr<'input>>, punct::Gt, Box<Expr<'input>>),
    /// String concatenation: `expr || expr`
    #[parse(infix, bp = 10)]
    Concat(Box<Expr<'input>>, punct::Concat, Box<Expr<'input>>),
    /// Bitwise OR: `expr | expr`. Must come after `Concat` (`||`) so the
    /// longer token matches first at the punctuation level.
    #[parse(infix, bp = 10)]
    BitOr(Box<Expr<'input>>, punct::Pipe, Box<Expr<'input>>),
    /// Bitwise AND: `expr & expr`.
    #[parse(infix, bp = 10)]
    BitAnd(Box<Expr<'input>>, punct::Amp, Box<Expr<'input>>),
    /// Bitwise XOR: `expr # expr` (Postgres bit-string / integer operator).
    #[parse(infix, bp = 10)]
    BitXor(Box<Expr<'input>>, punct::Pound, Box<Expr<'input>>),
    #[parse(infix, bp = 10)]
    Add(Box<Expr<'input>>, punct::Plus, Box<Expr<'input>>),
    #[parse(infix, bp = 10)]
    Sub(Box<Expr<'input>>, punct::Minus, Box<Expr<'input>>),
    /// Multiplication: `expr * expr`
    #[parse(infix, bp = 11)]
    Mul(Box<Expr<'input>>, punct::Star, Box<Expr<'input>>),
    /// Division: `expr / expr`
    #[parse(infix, bp = 11)]
    Div(Box<Expr<'input>>, punct::Slash, Box<Expr<'input>>),
    /// Modulo: `expr % expr`
    #[parse(infix, bp = 11)]
    Mod(Box<Expr<'input>>, punct::Percent, Box<Expr<'input>>),
    /// Exponentiation: `expr ^ expr` (Postgres numeric power operator).
    #[parse(infix, bp = 13)]
    Pow(Box<Expr<'input>>, punct::Caret, Box<Expr<'input>>),

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
    EscapeStringLit(literal::EscapeStringLit<'input>),
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
    /// `USER` — the reserved-keyword spelling of `CURRENT_USER` as a
    /// zero-arg function reference. PG's gram.y `func_expr_common_subexpr`
    /// includes `USER { … }` as a synonym for `CURRENT_USER`. pg-sql keeps
    /// `USER` reserved at the token level (for the `CREATE USER ...`
    /// statement disambiguation), so it cannot lex as an `UnquotedIdent`
    /// the way `current_date`/`session_user` do — model it as its own
    /// atom. Declared before `ColumnRef` for clarity (ColumnRef cannot
    /// match a reserved keyword anyway).
    #[parse(atom)]
    User(USER),
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
    BitStringLit(literal::BitStringLit<'input>),
    /// Hex-string literal: `X'1FF'`. Same ordering rationale as
    /// `BitStringLit` — must precede `StringLit` and any plain `Ident`.
    #[parse(atom)]
    HexStringLit(literal::HexStringLit<'input>),
    /// String literal sequence: `'hello'` or `'first' 'second' ...` —
    /// Postgres concatenates adjacent string literals into one.
    #[parse(atom)]
    StringLit(StringLitSeq0<'input>),
    /// Boolean true
    #[parse(atom)]
    BoolTrue(TRUE),
    /// Boolean false
    #[parse(atom)]
    BoolFalse(FALSE),
    /// NULL
    #[parse(atom)]
    Null(NULL),
    /// `DEFAULT` — placeholder usable in INSERT/UPDATE value positions.
    #[parse(atom)]
    Default(DEFAULT),
    /// Positional parameter reference: `$1`, `$2`, etc. Used in function bodies
    /// and prepared statements.
    #[parse(atom)]
    PositionalParam(literal::DollarNum<'input>),
    /// Unqualified column reference: `f1` or `"Foo"`
    #[parse(atom)]
    ColumnRef(crate::tokens::ColId<'input>),
    /// psql client variable substitution: `:foo`, `:'foo'`, `:"foo"`.
    #[parse(atom)]
    PsqlVar(literal::PsqlVar<'input>),
    /// Bare wildcard: `*`
    #[parse(atom)]
    Star(punct::Star),
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
        let mut input = crate::tokens::test_input(src);
        let expr = Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
        assert!(
            input.is_empty(),
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
        let mut input = crate::tokens::test_input("42");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntegerLit(_)));
        assert!(input.is_empty());
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
            let mut plain = crate::tokens::test_input(src);
            assert!(
                Expr::peek(&mut plain),
                "Expr::peek (no classifier) should accept {src:?}"
            );
            let mut classified = crate::tokens::test_input(src);
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
        let mut input = crate::tokens::test_input("$$''$$");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::DollarStringLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_string_literal() {
        let mut input = crate::tokens::test_input("'hello'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::StringLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_adjacent_string_literals() {
        let mut input = crate::tokens::test_input("'a' 'b'");
        let expr = Expr::parse(&mut input).unwrap();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2);
        } else {
            panic!("expected Expr::StringLit, got {:?}", expr);
        }
        assert!(input.is_empty());
    }

    /// PostgreSQL concatenates adjacent string literals across whitespace, but
    /// NOT when a comment sits between the two parts. pg-sql must not merge
    /// `'a' /* c */ 'b'` into a single 2-part literal — the second part must
    /// be left unconsumed (so the comment-bearing continuation is rejected).
    #[test]
    fn reject_string_continuation_across_comment() {
        let mut input =
            crate::tokens::test_input("'first line'\n/* not allowed */\n' - next line'");
        let expr = Expr::parse(&mut input).unwrap();
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
            !input.is_empty(),
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
        let mut input = crate::tokens::test_input("'first line'\n' - next line'");
        let expr = Expr::parse(&mut input).unwrap();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2);
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
        assert!(input.is_empty());
    }

    #[test]
    fn parse_three_part_string_concat() {
        // 3-part adjacent string literal concatenation. Postgres concatenates
        // these into a single value at parse time.
        let mut input = crate::tokens::test_input("'first' 'second' 'third'");
        let expr = Expr::parse(&mut input).unwrap();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 3);
        } else {
            panic!("expected StringLit, got {:?}", expr);
        }
        assert!(input.is_empty());
    }

    #[test]
    fn parse_four_part_string_concat() {
        let mut input = crate::tokens::test_input("'a' 'b' 'c' 'd'");
        let expr = Expr::parse(&mut input).unwrap();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 4);
        } else {
            panic!("expected StringLit");
        }
    }

    #[test]
    fn parse_three_adjacent_strings_with_quoted_alias() {
        use crate::ast::dml::select::SelectStmt;
        let mut input = crate::tokens::test_input(
            "SELECT 'first line' ' - next line' ' - third line' AS \"Three lines to one\"",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_three_adjacent_strings_with_alias() {
        // SELECT 'first line' ' - next line' AS foo
        use crate::ast::dml::select::SelectStmt;
        let mut input = crate::tokens::test_input("SELECT 'first line' ' - next line' AS foo");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_xmlelement_simple() {
        let mut input = crate::tokens::test_input("xmlelement(name foo, 'content')");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_xmlelement_with_attributes() {
        let mut input = crate::tokens::test_input(
            "xmlelement(name foo, xmlattributes(1 as a, 2 as b), 'content')",
        );
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_xmlpi_basic() {
        let mut input = crate::tokens::test_input("xmlpi(name foo)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_xmlpi_with_content() {
        let mut input = crate::tokens::test_input("xmlpi(name foo, 'bar')");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_unicode_string_lit_basic() {
        let mut input = crate::tokens::test_input(r"U&'d\0061t\+000061'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_unicode_string_lit_uescape() {
        let mut input = crate::tokens::test_input(r"U&'d!0061t\+000061' UESCAPE '!'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_cast_func_with_precision() {
        // `char(20) 'characters'` — function-style type cast with precision.
        let mut input = crate::tokens::test_input("char(20) 'characters'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::CastFunc(_)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_unicode_string_with_backslash() {
        // `U&' \'` — backslash is literal content, not an escape. The string
        // ends at the second quote. UESCAPE '!' follows.
        let mut input = crate::tokens::test_input(r"U&' \' UESCAPE '!'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_xmlforest() {
        let mut input = crate::tokens::test_input("xmlforest(a, b AS bee, c)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::XmlForest(_)));
        assert!(input.is_empty());
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
            let mut input = crate::tokens::test_input(sql);
            let _stmt = SelectStmt::parse(&mut input).unwrap();
            assert!(input.is_empty(), "leftover for {sql}");
        }
    }

    #[test]
    fn parse_escape_string_literal() {
        let mut input = crate::tokens::test_input(r"E'r_\_view%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_order_by() {
        let mut input = crate::tokens::test_input("jsonb_agg(q ORDER BY x, y)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_psql_var() {
        let mut input = crate::tokens::test_input(":foo_oid");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_psql_var_in_func_call() {
        let mut input = crate::tokens::test_input("pg_stat_get_function_calls(:func_oid)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_trim_both_from() {
        let mut input = crate::tokens::test_input("TRIM(BOTH FROM '  hi  ')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_trim_leading_from() {
        let mut input = crate::tokens::test_input("TRIM(LEADING FROM '  hi  ')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_trim_trailing_from() {
        let mut input = crate::tokens::test_input("TRIM(TRAILING FROM '  hi  ')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_trim_both_chars_from() {
        let mut input = crate::tokens::test_input("TRIM(BOTH 'x' FROM 'xxhixx')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
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
            let mut input = crate::tokens::test_input(src);
            let _expr = Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
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
            let mut input = crate::tokens::test_input(src);
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_substring_from() {
        let mut input = crate::tokens::test_input("SUBSTRING('1234567890' FROM 3)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_substring_from_for() {
        let mut input = crate::tokens::test_input("SUBSTRING('1234567890' FROM 4 FOR 3)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_notnull_isnull() {
        let mut input = crate::tokens::test_input("x.c NOTNULL");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Notnull(..)));
        assert!(input.is_empty());
        let mut input = crate::tokens::test_input("x.c ISNULL");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Isnull(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_collation_for() {
        let mut input = crate::tokens::test_input("collation for ('foo')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
        let mut input = crate::tokens::test_input("collation for ((SELECT a FROM t LIMIT 1))");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_cast_call() {
        let mut input = crate::tokens::test_input("CAST('42' AS text COLLATE \"C\")");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
        let mut input = crate::tokens::test_input("CAST(b AS varchar)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_substring_for_only() {
        let mut input = crate::tokens::test_input("substring(d FOR 30)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_substring_similar_escape() {
        let mut input =
            crate::tokens::test_input("SUBSTRING('abcdefg' SIMILAR 'a#\"%#\"g' ESCAPE '#')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_position_in() {
        let mut input = crate::tokens::test_input("POSITION('4' IN '1234567890')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_overlay_placing_from() {
        let mut input = crate::tokens::test_input("OVERLAY('abcdef' PLACING '45' FROM 4)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_overlay_placing_from_for() {
        let mut input = crate::tokens::test_input("OVERLAY('abcdef' PLACING '45' FROM 4 FOR 2)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_extract_epoch_from_date() {
        let mut input = crate::tokens::test_input("EXTRACT(EPOCH FROM DATE '1970-01-01')");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Extract(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_extract_century_from_ident() {
        let mut input = crate::tokens::test_input("EXTRACT(CENTURY FROM d)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_extract_string_field() {
        let mut input = crate::tokens::test_input("EXTRACT('year' FROM t)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_named_arg_mixed() {
        let mut input = crate::tokens::test_input("f(a, b => 1, c)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_jsonb_path_query_silent() {
        let mut input =
            crate::tokens::test_input("jsonb_path_query('[1]', 'strict $[1]', silent => true)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_all_named_args() {
        let mut input = crate::tokens::test_input("f(silent => false, verbose => true)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_extract_year_from_now() {
        let mut input = crate::tokens::test_input("EXTRACT(year FROM now())");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_is_distinct_from() {
        let mut input = crate::tokens::test_input("a IS DISTINCT FROM b");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_is_not_distinct_from() {
        let mut input = crate::tokens::test_input("a IS NOT DISTINCT FROM b");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_power_operator() {
        let mut input = crate::tokens::test_input("2^1000");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_double_precision_type_cast() {
        let mut input = crate::tokens::test_input("3.14::double precision");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_case_searched() {
        let mut input = crate::tokens::test_input("CASE WHEN 1 < 2 THEN 3 END");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Case(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_case_searched_with_else() {
        let mut input =
            crate::tokens::test_input("CASE WHEN 1 < 2 THEN 3 WHEN 4 < 5 THEN 6 ELSE 7 END");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_case_simple() {
        let mut input =
            crate::tokens::test_input("CASE x WHEN 1 THEN 'a' WHEN 2 THEN 'b' ELSE 'c' END");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_case_nested() {
        let mut input =
            crate::tokens::test_input("CASE WHEN (CASE WHEN 1=1 THEN 1 END) > 0 THEN 'y' END");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_within_group() {
        let mut input = crate::tokens::test_input("percentile_disc(0.5) WITHIN GROUP (ORDER BY v)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_within_group_multi() {
        let mut input = crate::tokens::test_input("rank(1, 2) WITHIN GROUP (ORDER BY a, b)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_filter() {
        let mut input = crate::tokens::test_input("sum(x) FILTER (WHERE y > 0)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_filter_over() {
        let mut input =
            crate::tokens::test_input("sum(x) FILTER (WHERE y > 0) OVER (PARTITION BY z)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_order_by_nulls_first() {
        let mut input = crate::tokens::test_input("jsonb_agg(q ORDER BY x NULLS FIRST, y)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_func_call_variadic() {
        let mut input = crate::tokens::test_input("jsonb_build_array(VARIADIC a)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_timestamp_with_tz_literal() {
        let mut input =
            crate::tokens::test_input("timestamp with time zone '2001-12-27 04:05:06+08'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_timestamp_precision_without_tz_literal() {
        // Regression: timestamp.sql uses `timestamp(2) without time zone 'now'`.
        let mut input = crate::tokens::test_input("timestamp(2) without time zone 'now'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_at_time_zone() {
        let mut input = crate::tokens::test_input("f1 AT TIME ZONE 'UTC+10'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_at_time_zone_interval() {
        let mut input = crate::tokens::test_input("f1 AT TIME ZONE INTERVAL '-10:00'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_at_local() {
        let mut input = crate::tokens::test_input("f1 AT LOCAL");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::AtLocal(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_time_literal() {
        let mut input = crate::tokens::test_input("time '12:34'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TimeLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_date_literal_as_castfunc() {
        let mut input = crate::tokens::test_input("date '2024-01-01'");
        let expr = Expr::parse(&mut input).unwrap();
        // `date` is an Ident-based TypeName, so this parses as CastFunc.
        assert!(matches!(expr, Expr::CastFunc(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_literal_bare() {
        let mut input = crate::tokens::test_input("interval '1 hour'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_literal_year() {
        let mut input = crate::tokens::test_input("INTERVAL '1' YEAR");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_literal_year_to_month() {
        let mut input = crate::tokens::test_input("INTERVAL '1-2' YEAR TO MONTH");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_named_arg_colon_equals() {
        let mut input = crate::tokens::test_input("make_interval(years := 1, months := 2)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_unary_plus() {
        let mut input = crate::tokens::test_input("+42");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Pos(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_positional_param() {
        let mut input = crate::tokens::test_input("$1");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::PositionalParam(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_positional_param_in_expr() {
        let mut input = crate::tokens::test_input("$1 + $2");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_empty());
    }

    /// `$1` must preserve its digits when reformatted — a positional parameter
    /// is not interchangeable with `$2`. The token must capture the number.
    #[test]
    fn positional_param_preserves_digits() {
        use recursa::fmt::FormatStyle;
        let mut input = crate::tokens::test_input("$2");
        let expr = Expr::parse(&mut input).unwrap();
        let formatted = crate::formatter::format_tokens_sql(&expr, FormatStyle::default());
        assert_eq!(formatted.trim(), "$2");
    }

    #[test]
    fn parse_interval_with_precision() {
        for src in [
            "INTERVAL(0) '1 day 01:23:45.6789'",
            "interval(2) '1 day 01:23:45.6789'",
        ] {
            let mut input = crate::tokens::test_input(src);
            let expr = Expr::parse(&mut input).unwrap();
            assert!(matches!(expr, Expr::IntervalLit(_)), "failed for {src:?}");
            assert!(input.is_empty(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_interval_second_precision() {
        let mut input = crate::tokens::test_input("INTERVAL '1.234' second(2)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_day_to_second_precision() {
        let mut input = crate::tokens::test_input("INTERVAL '1 2:03:04.5678' day to second(2)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_cast_interval_day_to_minute() {
        let mut input = crate::tokens::test_input("f1::INTERVAL DAY TO MINUTE");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Cast(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_minute_to_second_precision() {
        let mut input = crate::tokens::test_input("INTERVAL '12:34.5678' minute to second(2)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_literal_day_to_hour() {
        let mut input = crate::tokens::test_input("INTERVAL '1 2:03' DAY TO HOUR");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_interval_literal_hour_to_second() {
        let mut input = crate::tokens::test_input("INTERVAL '1' HOUR TO SECOND");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_escape_string_literal_lowercase_e() {
        let mut input = crate::tokens::test_input("e'foo'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_bool_true() {
        let mut input = crate::tokens::test_input("true");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolTrue(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_bool_false() {
        let mut input = crate::tokens::test_input("false");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolFalse(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_null() {
        let mut input = crate::tokens::test_input("null");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Null(_)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_column_ref() {
        let mut input = crate::tokens::test_input("f1");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::ColumnRef(_)));
    }

    #[test]
    fn parse_qualified_column_ref() {
        let mut input = crate::tokens::test_input("BOOLTBL1.f1");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::QualRef(_)));
    }

    #[test]
    fn parse_qualified_wildcard() {
        let mut input = crate::tokens::test_input("BOOLTBL1.*");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::QualWild(_)));
    }

    #[test]
    fn parse_star() {
        let mut input = crate::tokens::test_input("*");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Star(_)));
    }

    #[test]
    fn parse_function_call_no_args() {
        let mut input = crate::tokens::test_input("foo()");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_with_args() {
        let mut input = crate::tokens::test_input("pg_input_is_valid('true', 'bool')");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_booleq() {
        let mut input = crate::tokens::test_input("booleq(bool 'false', f1)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_parenthesized_expr() {
        let mut input = crate::tokens::test_input("(1)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Paren(_)));
    }

    // --- Type cast function-style: bool 'foo' ---

    #[test]
    fn parse_type_cast_bool_string() {
        let mut input = crate::tokens::test_input("bool 't'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    #[test]
    fn parse_type_cast_boolean_string() {
        let mut input = crate::tokens::test_input("boolean 'false'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    // --- Prefix operators ---

    #[test]
    fn parse_not_expr() {
        let mut input = crate::tokens::test_input("not false");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Not(_, _)));
    }

    // --- Infix operators ---

    #[test]
    fn parse_and_expr() {
        let mut input = crate::tokens::test_input("true AND false");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::And(..)));
    }

    #[test]
    fn parse_or_expr() {
        let mut input = crate::tokens::test_input("true OR false");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn parse_eq_expr() {
        let mut input = crate::tokens::test_input("f1 = true");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Eq(..)));
    }

    #[test]
    fn parse_neq_expr() {
        let mut input = crate::tokens::test_input("f1 <> false");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Neq(..)));
    }

    // --- Postfix: :: type cast ---

    #[test]
    fn parse_cast_colon_colon() {
        let mut input = crate::tokens::test_input("0::boolean");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    #[test]
    fn parse_chained_cast() {
        let mut input = crate::tokens::test_input("'TrUe'::text::boolean");
        let expr = Expr::parse(&mut input).unwrap();
        // Outer should be Cast
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Postfix: IS [NOT] TRUE/FALSE/UNKNOWN/NULL ---

    #[test]
    fn parse_is_true() {
        let mut input = crate::tokens::test_input("f1 IS TRUE");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_false() {
        let mut input = crate::tokens::test_input("f1 IS NOT FALSE");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_unknown() {
        let mut input = crate::tokens::test_input("b IS UNKNOWN");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_unknown() {
        let mut input = crate::tokens::test_input("b IS NOT UNKNOWN");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    // --- Postfix: BETWEEN / NOT BETWEEN ---

    #[test]
    fn parse_between_expr() {
        let mut input = crate::tokens::test_input("a BETWEEN 12 AND 17");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn parse_not_between_expr() {
        let mut input = crate::tokens::test_input("a NOT BETWEEN 1 AND 5");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotBetweenExpr(..)));
    }

    #[test]
    fn parse_between_as_value() {
        // BETWEEN yields a boolean value that can appear in a SELECT list.
        let mut input = crate::tokens::test_input("x BETWEEN a AND b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn between_does_not_break_and_parse() {
        // A plain AND expression must still parse as And, not be confused
        // with the BETWEEN postfix.
        let mut input = crate::tokens::test_input("a AND b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::And(..)));
    }

    // --- Precedence ---

    #[test]
    fn and_binds_tighter_than_or() {
        // a OR b AND c should parse as a OR (b AND c)
        let mut input = crate::tokens::test_input("true OR false AND true");
        let expr = Expr::parse(&mut input).unwrap();
        // Top-level should be OR
        match &expr {
            Expr::Or(..) => {}
            other => panic!("expected OR at top level, got {other:?}"),
        }
    }

    #[test]
    fn comparison_binds_tighter_than_and() {
        // a AND b = c should parse as a AND (b = c)
        let mut input = crate::tokens::test_input("true AND f1 = false");
        let expr = Expr::parse(&mut input).unwrap();
        match &expr {
            Expr::And(..) => {}
            other => panic!("expected AND at top level, got {other:?}"),
        }
    }

    #[test]
    fn bool_cast_or_expr() {
        // bool 't' or bool 'f' should parse as (bool 't') OR (bool 'f')
        let mut input = crate::tokens::test_input("bool 't' or bool 'f'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn is_true_in_select_item() {
        // b IS TRUE should parse without consuming AS that follows
        let mut input = crate::tokens::test_input("b IS TRUE");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BoolTest(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn cast_chain_in_expression() {
        // true::boolean::text should chain
        let mut input = crate::tokens::test_input("true::boolean::text");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Arithmetic operators ---

    #[test]
    fn parse_addition() {
        let mut input = crate::tokens::test_input("4+4");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_subtraction() {
        let mut input = crate::tokens::test_input("10-3");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Sub(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_unary_minus() {
        let mut input = crate::tokens::test_input("-1");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Neg(..)));
        assert!(input.is_empty());
    }

    // --- Numeric literal ---

    #[test]
    fn parse_numeric_literal() {
        let mut input = crate::tokens::test_input("77.7");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NumericLit(_)));
        assert!(input.is_empty());
    }

    // --- IN expression ---

    #[test]
    fn parse_in_expr() {
        let mut input = crate::tokens::test_input("f1 IN (1, 2, 3)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::InExpr(..)));
        assert!(input.is_empty());
    }

    // --- JSON / JSONB operators ---

    #[test]
    fn parse_json_field() {
        let mut input = crate::tokens::test_input("data -> 'key'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonField(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_json_field_text() {
        let mut input = crate::tokens::test_input("data ->> 'key'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonFieldText(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_json_path() {
        let mut input = crate::tokens::test_input("data #> '{a,b}'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonPath(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_json_path_text() {
        let mut input = crate::tokens::test_input("data #>> '{a,b}'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonPathText(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_jsonb_contains() {
        let mut input = crate::tokens::test_input("a @> b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonContains(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_jsonb_contained_by() {
        let mut input = crate::tokens::test_input("a <@ b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonContainedBy(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_jsonb_key_exists() {
        let mut input = crate::tokens::test_input("a ? 'k'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonKey(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_jsonb_any_key() {
        let mut input = crate::tokens::test_input("a ?| b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonAnyKey(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_jsonb_all_keys() {
        let mut input = crate::tokens::test_input("a ?& b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonAllKeys(..)));
        assert!(input.is_empty());
    }

    // --- Postgres text-search / range / geometric operators ---

    #[test]
    fn parse_ts_match() {
        let mut input = crate::tokens::test_input("a @@ 'foo|bar'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TsMatch(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_ts_match3() {
        let mut input = crate::tokens::test_input("a @@@ b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TsMatch3(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_json_path_exists() {
        let mut input = crate::tokens::test_input("j @? '$.a'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::JsonPathExists(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_overlap() {
        let mut input = crate::tokens::test_input("r && s");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Overlap(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_strictly_left() {
        let mut input = crate::tokens::test_input("a << b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::StrictlyLeft(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_strictly_right() {
        let mut input = crate::tokens::test_input("a >> b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::StrictlyRight(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_subset_eq() {
        let mut input = crate::tokens::test_input("a <<= b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::SubsetEq(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_superset_eq() {
        let mut input = crate::tokens::test_input("a >>= b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::SupersetEq(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_adjacent() {
        let mut input = crate::tokens::test_input("a -|- b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Adjacent(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_distance() {
        let mut input = crate::tokens::test_input("p1 <-> p2");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Distance(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_no_extend_right() {
        let mut input = crate::tokens::test_input("a &< b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NoExtendRight(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_no_extend_left() {
        let mut input = crate::tokens::test_input("a &> b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NoExtendLeft(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_strictly_above() {
        let mut input = crate::tokens::test_input("a |>> b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::StrictlyAbove(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_strictly_below() {
        let mut input = crate::tokens::test_input("a <<| b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::StrictlyBelow(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_no_extend_above() {
        let mut input = crate::tokens::test_input("a &<| b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NoExtendAbove(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_no_extend_below() {
        let mut input = crate::tokens::test_input("a |&> b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NoExtendBelow(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_intersect() {
        let mut input = crate::tokens::test_input("a ?# b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Intersect(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_horizontal() {
        let mut input = crate::tokens::test_input("a ?- b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Horizontal(..)));
        assert!(input.is_empty());
    }

    // --- LIKE / ILIKE ---

    #[test]
    fn parse_like_expr() {
        let mut input = crate::tokens::test_input("table_name LIKE 'foo%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_like_escape_string() {
        let mut input = crate::tokens::test_input(r"table_name LIKE E'r_\_view%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_not_like_expr() {
        let mut input = crate::tokens::test_input("table_name NOT LIKE 'bar%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_similar_to_expr() {
        let mut input = crate::tokens::test_input("x SIMILAR TO 'a%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_not_similar_to_expr() {
        let mut input = crate::tokens::test_input("x NOT SIMILAR TO 'a%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_ilike_expr() {
        let mut input = crate::tokens::test_input("name ILIKE '%FOO%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_not_ilike_expr() {
        let mut input = crate::tokens::test_input("name NOT ILIKE '%bar%'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_like_escape() {
        let mut input = crate::tokens::test_input("'hawkeye' LIKE 'h%' ESCAPE '#'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_not_like_escape() {
        let mut input = crate::tokens::test_input("'hawkeye' NOT LIKE 'h%' ESCAPE '#'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_similar_to_escape() {
        let mut input = crate::tokens::test_input("'abcdefg' SIMILAR TO '_bcd#%' ESCAPE '#'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_not_similar_to_escape() {
        let mut input = crate::tokens::test_input("'abc' NOT SIMILAR TO 'a%' ESCAPE '#'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_ilike_escape() {
        let mut input = crate::tokens::test_input("name ILIKE '%FOO%' ESCAPE '#'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_not_ilike_escape() {
        let mut input = crate::tokens::test_input("name NOT ILIKE '%bar%' ESCAPE '#'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_similar_to_escape_null() {
        let mut input = crate::tokens::test_input("'abcdefg' SIMILAR TO '_bcd%' ESCAPE NULL");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_empty());
    }

    // --- Regex match operators ---

    #[test]
    fn parse_regex_match() {
        let mut input = crate::tokens::test_input("relname ~ '^foo'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::RegexMatch(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_regex_not_match() {
        let mut input = crate::tokens::test_input("name !~ 'bar'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::RegexNotMatch(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_regex_imatch() {
        let mut input = crate::tokens::test_input("name ~* 'FOO'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::RegexIMatch(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_regex_not_imatch() {
        let mut input = crate::tokens::test_input("name !~* '.*'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::RegexNotIMatch(..)));
        assert!(input.is_empty());
    }

    // --- COLLATE postfix ---

    #[test]
    fn parse_collate_postfix() {
        let mut input = crate::tokens::test_input("a COLLATE \"C\"");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Collate(..)));
        assert!(input.is_empty());
    }

    // --- DEFAULT atom ---

    #[test]
    fn parse_default_atom() {
        let mut input = crate::tokens::test_input("DEFAULT");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Default(_)));
        assert!(input.is_empty());
    }

    // --- Subquery expression ---

    #[test]
    fn parse_subquery_expr() {
        let mut input = crate::tokens::test_input("(SELECT 1)");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Paren(_)));
        assert!(input.is_empty());
    }

    // --- Locale-aware text comparison operators ---

    #[test]
    fn parse_tilde_lt_tilde_infix() {
        let mut input = crate::tokens::test_input("f1 ~<~ 'YX'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TildeLtTilde(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_tilde_leq_tilde_infix() {
        let mut input = crate::tokens::test_input("t ~<=~ 'Aztec'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TildeLeqTilde(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_tilde_geq_tilde_infix() {
        let mut input = crate::tokens::test_input("t ~>=~ 'Worth'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TildeGeqTilde(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_tilde_gt_tilde_infix() {
        let mut input = crate::tokens::test_input("t ~>~ 'Worth'");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TildeGtTilde(..)));
        assert!(input.is_empty());
    }

    // --- User-defined equality/inequality ---

    #[test]
    fn parse_triple_eq_infix() {
        let mut input = crate::tokens::test_input("a === 1");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TripleEq(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_bang_eq_eq_infix() {
        let mut input = crate::tokens::test_input("a !== 1");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BangEqEq(..)));
        assert!(input.is_empty());
    }

    // --- Geometric closest-point / intersection ---

    #[test]
    fn parse_hash_hash_infix() {
        let mut input = crate::tokens::test_input("p.f1 ## l.s");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::GeomClosest(..)));
        assert!(input.is_empty());
    }

    // --- Prefix: geometric path length `@-@` ---

    #[test]
    fn parse_at_minus_at_prefix() {
        let mut input = crate::tokens::test_input("@-@ s");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::PathLength(..)));
        assert!(input.is_empty());
    }

    // --- Prefix: user-defined `@#@` ---

    #[test]
    fn parse_at_hash_at_prefix() {
        let mut input = crate::tokens::test_input("@#@ 24");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::AtHashAtPrefix(..)));
        assert!(input.is_empty());
    }

    // --- Prefix: user-defined `!=-` ---

    #[test]
    fn parse_bang_eq_minus_prefix() {
        let mut input = crate::tokens::test_input("!=- 10");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::BangEqMinusPrefix(..)));
        assert!(input.is_empty());
    }

    // --- Prefix: geometric `#` (number of points in path) ---

    #[test]
    fn parse_pound_prefix() {
        let mut input = crate::tokens::test_input("#thepath");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::PointCount(..)));
        assert!(input.is_empty());
    }

    // --- Infix: geometric `?||` (parallel) and `?-|` (perpendicular) ---

    #[test]
    fn parse_question_pipe_pipe_infix() {
        let mut input = crate::tokens::test_input("a ?|| b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Parallel(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_question_dash_pipe_infix() {
        let mut input = crate::tokens::test_input("a ?-| b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Perpendicular(..)));
        assert!(input.is_empty());
    }

    // --- Infix: geometric `<^` (below) and `>^` (above) ---

    #[test]
    fn parse_lt_caret_infix() {
        let mut input = crate::tokens::test_input("a <^ b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Below(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_gt_caret_infix() {
        let mut input = crate::tokens::test_input("a >^ b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Above(..)));
        assert!(input.is_empty());
    }

    // --- Infix: user-defined `<<<` and `>>>` ---

    #[test]
    fn parse_triple_lt_infix() {
        let mut input = crate::tokens::test_input("a <<< 5");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TripleLt(..)));
        assert!(input.is_empty());
    }

    #[test]
    fn parse_triple_gt_infix() {
        let mut input = crate::tokens::test_input("a >>> 0");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::TripleGt(..)));
        assert!(input.is_empty());
    }

    // --- Infix: user-defined `<%` ---

    #[test]
    fn parse_lt_percent_infix() {
        let mut input = crate::tokens::test_input("a <% b");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::CustomInfix(..)));
        assert!(input.is_empty());
    }

    // --- Subquery quantifier: ANY / ALL / SOME ---

    #[test]
    fn parse_eq_any_subquery() {
        // `a = ANY(SELECT 1)` — comparison with quantified subquery.
        let mut input = crate::tokens::test_input("a = ANY(SELECT 1)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_eq_all_array() {
        // `a = ALL('{ab}')` — comparison with quantified array.
        let mut input = crate::tokens::test_input("a = ALL('{ab}')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_not_tilde_all() {
        // `a !~ ALL('{ab}')` — regex not-match with ALL quantifier.
        let mut input = crate::tokens::test_input("a !~ ALL('{ab}')");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_eq_some_subquery() {
        // `a = SOME(SELECT 1)` — SOME is synonym for ANY.
        let mut input = crate::tokens::test_input("a = SOME(SELECT 1)");
        let _expr = Expr::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    // --- Array slice subscripts ---

    #[test]
    fn parse_array_slice_full() {
        // `a[1:2]` — full slice with lower and upper bounds.
        let mut input = crate::tokens::test_input("a[1:2]");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_empty(),
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
            let mut input = crate::tokens::test_input(src);
            let _expr = Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_array_slice_lower_only() {
        // `a[1:]` — slice with only lower bound.
        let mut input = crate::tokens::test_input("a[1:]");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_array_slice_upper_only() {
        // `a[:2]` — slice with only upper bound.
        let mut input = crate::tokens::test_input("a[:2]");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_array_slice_unbounded() {
        // `a[:]` — unbounded slice (all elements).
        let mut input = crate::tokens::test_input("a[:]");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Slice(..)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_subscript_unchanged() {
        // `a[1]` — regular subscript still works.
        let mut input = crate::tokens::test_input("a[1]");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::Subscript(..)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_any_array_literal() {
        // `ANY('{red,green}'::rainbow[])` — bare ANY as atom.
        let mut input = crate::tokens::test_input("ANY('{red,green}'::rainbow[])");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::AnyExpr(_)));
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_all_array_literal() {
        // `ALL('{red,red}'::rainbow[])` — bare ALL as atom.
        let mut input = crate::tokens::test_input("ALL('{red,red}'::rainbow[])");
        let expr = Expr::parse(&mut input).unwrap();
        assert!(matches!(expr, Expr::AllExpr(_)));
        assert!(
            input.is_empty(),
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
            let mut input = crate::tokens::test_input(src);
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
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
            let mut input = crate::tokens::test_input(src);
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
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
