/// SQL expression AST with derived Pratt parsing for operator precedence.
///
/// Handles atoms, prefix (NOT, unary minus), infix (AND, OR, comparisons,
/// arithmetic), and postfix operators (::type cast, IS [NOT] TRUE/FALSE/UNKNOWN/NULL,
/// IN (list)).
use crate::ast::dml::select::SelectStmt;
use crate::ast::dml::values::{SetOpCombiner, Subquery, TableStmt};
use crate::tokens::literal;

/// Required opening delimiter for structurally parenthesized SQL forms.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParenthesizedOpen {
    #[tok(LPAREN)]
    Value,
}

/// Required closing delimiter for structurally parenthesized SQL forms.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParenthesizedClose {
    #[tok(RPAREN)]
    Value,
}

/// A PostgreSQL query admitted directly inside an enclosing construct.
/// Parentheses are accepted here only when followed by a required set
/// operation, avoiding the exact `(SELECT ...)` language also represented by
/// a scalar subquery expression.
#[derive(recursa::Node, Debug, Clone)]
pub enum DirectSubquery<'input> {
    /// A parenthesized query followed by a required set operation. Requiring
    /// the continuation keeps a plain `(SELECT ...)` expression on the
    /// ordinary parenthesized-expression path.
    ParenthesizedSet(DirectParenthesizedSet<'input>),
    Table(TableStmt<'input>),
    Body(DirectCompoundBody<'input>),
}

/// A query whose left operand is parenthesized and whose set-operation
/// continuation is required, such as `(SELECT 1) UNION SELECT 2`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DirectParenthesizedSet<'input> {
    pub open: ParenthesizedOpen,
    pub left: Box<Subquery<'input>>,
    pub close: ParenthesizedClose,
    pub set_op: SetOpCombiner<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset_1: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
    pub limit_offset_2: Option<Box<crate::ast::dml::select::LimitOffsetItem<'input>>>,
}

/// SELECT/WITH/VALUES query body with an optional set-operation continuation.
/// The local VALUES form requires at least one row; the older shared
/// `ValuesBody` uses a nullable row vector, which makes bare `VALUES`
/// indistinguishable from a column expression in bounded lookahead.
#[derive(recursa::Node, Debug, Clone)]
pub struct DirectCompoundBody<'input> {
    pub body: DirectSelectBody<'input>,
    pub set_op: Option<SetOpCombiner<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum DirectSelectBody<'input> {
    WithBody(Box<crate::ast::shared::with_clause::WithStatement<'input>>),
    Select(Box<SelectStmt<'input>>),
    Values(DirectValuesBody<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(VALUES, this)]
pub struct DirectValuesBody<'input> {
    #[sep(COMMA)]
    pub rows: recursa::Vec1<DirectValuesRow<'input>>,
}

/// A VALUES row with required delimiters represented as semantic markers.
/// This prevents the nullable element list from hiding the leading `(` in
/// FIRST-k analysis.
#[derive(recursa::Node, Debug, Clone)]
pub struct DirectValuesRow<'input> {
    pub open: DirectValuesOpen,
    #[sep(COMMA)]
    pub values: Option<recursa::Vec1<Expr<'input>>>,
    pub close: DirectValuesClose,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum DirectValuesOpen {
    #[tok(LPAREN)]
    Value,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum DirectValuesClose {
    #[tok(RPAREN)]
    Value,
}

/// One or more adjacent string literals, concatenated by Postgres into a
/// single value: `'first' ' - next' 'third'`.
///
/// PostgreSQL only concatenates two adjacent string literals when their gap
/// contains a newline. A block comment does not itself satisfy that rule, but
/// a later newline in the same gap does; the lexer records that classification
/// before the generated parser sees the string parts.
#[derive(recursa::Node, Debug, Clone)]
pub struct StringLitSeq0<'input> {
    pub parts: recursa::Vec1<literal::StringLit<'input>>,
}

/// Content inside IN parentheses: either a subquery or expression list.
///
/// PG's `in_expr` is either `select_with_parens` or `'(' expr_list ')'`.
/// Both alternatives can begin with arbitrarily nested parentheses, so the
/// generated parser uses Recursa's proven balanced-delimiter dispatch before
/// applying the bounded decision after the matching close parenthesis.
///
/// The bare `Subquery` branch still wins on its non-`(` leading tokens
/// (`SELECT`, `VALUES`, `TABLE`, `WITH`), since the first-set tree routes
/// those tokens unambiguously to `Subquery`.
///
/// This keeps both the expression-list form (`IN ((SELECT 1), (SELECT 2))`)
/// and a grouped set query (`IN ((SELECT 1) UNION SELECT 2)`) reachable
/// without declaration-order priority or parser-specific source scanning.
#[derive(recursa::Node, Debug, Clone)]
pub enum InContent<'input> {
    Exprs(#[sep(COMMA)] recursa::Vec1<Expr<'input>>),
    Subquery(Box<DirectSubquery<'input>>),
}

/// `IN (expr, ...)` or `IN (subquery)` postfix suffix.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
pub struct InList<'input>(
    #[tok(LPAREN, this, RPAREN)]
    #[deref]
    pub InContent<'input>,
);

/// A single typmod argument: an optionally-signed integer literal. Postgres'
/// gram.y allows `expr_list` here, but the corpus only exercises signed
/// integers (e.g. `numeric(3, -6)` in numeric.sql), so we model only that
/// shape. A leading `+` or `-` is permitted to mirror PG's behavior.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub struct TypeModifierArg<'input> {
    pub sign: Option<TypeModifierSign>,
    pub value: literal::IntegerLit<'input>,
}

/// Leading sign of a typmod argument.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum TypeModifierSign {
    #[tok(MINUS)]
    Neg,
    #[tok(PLUS)]
    Pos,
}

/// Parenthesized precision/scale for type names: `(10,2)`, `(3)`, `(3,-6)`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct TypePrecision<'input>(
    #[sep(COMMA)]
    #[deref]
    pub Vec<TypeModifierArg<'input>>,
);

/// Type name for casts.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum TypeName<'input> {
    #[tok(BOOL)]
    Bool,
    #[tok(BOOLEAN)]
    Boolean,
    #[tok(TEXT)]
    Text,
    #[tok(INTEGER)]
    Integer,
    #[tok(INT)]
    Int,
    #[tok(SERIAL)]
    Serial,
    #[tok(NUMERIC)]
    Numeric,
    #[tok(VARCHAR)]
    Varchar,
    #[tok(DOUBLE, PRECISION)]
    DoublePrecision,
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
    #[tok(UNKNOWN)]
    Unknown,
    /// Qualified type name (`schema.type`) or a bare identifier.
    Ident(TypeNameIdent<'input>),
}

/// Identifier-spelled type name using the type-name-specific admission set.
/// Fixed legacy spellings are excluded so they retain their public enum
/// variants; `json` is included despite its `COL_NAME` keyword category.
#[derive(recursa::Node, Debug, Clone)]
pub struct TypeNameIdent<'input> {
    #[sep(DOT)]
    pub parts: recursa::Vec1<crate::tokens::type_name_ident<'input>>,
}

impl<'input> TypeNameIdent<'input> {
    pub fn object(&self) -> &str {
        self.parts
            .last()
            .expect("Recursa Vec1 always contains at least one value")
            .text()
    }
}

impl PartialEq for TypeNameIdent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.parts.len() == other.parts.len()
            && self
                .parts
                .iter()
                .zip(other.parts.iter())
                .all(|(left, right)| left.text() == right.text())
    }
}

impl Eq for TypeNameIdent<'_> {}

/// Boolean test suffix: the part after `IS` in `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`.
///
/// NOT variants are listed first so the combined peek regex disambiguates
/// via longest match (e.g., `NOT TRUE` is longer than `TRUE`).
#[derive(recursa::Node, Debug, Clone)]
pub enum BoolTestKind {
    #[tok(NOT, TRUE)]
    IsNotTrue,
    #[tok(NOT, FALSE)]
    IsNotFalse,
    #[tok(NOT, UNKNOWN)]
    IsNotUnknown,
    #[tok(NOT, NULL)]
    IsNotNull,
    #[tok(TRUE)]
    IsTrue,
    #[tok(FALSE)]
    IsFalse,
    #[tok(UNKNOWN)]
    IsUnknown,
    #[tok(NULL)]
    IsNull,
}

/// Unicode normalisation form keyword — gram.y `unicode_normal_form`.
/// Used by `expr IS [NOT] [NFx] NORMALIZED` and `NORMALIZE(expr, NFx)`.
#[derive(recursa::Node, Debug, Clone)]
pub enum UnicodeNormalForm {
    #[tok(NFKC)]
    Nfkc,
    #[tok(NFKD)]
    Nfkd,
    #[tok(NFC)]
    Nfc,
    #[tok(NFD)]
    Nfd,
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
    #[tok(NOT, NORMALIZED)]
    Not,
    Form(IsFormNormalizedTail),
    #[tok(NORMALIZED)]
    Plain,
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
#[tok(OVER, this)]
pub struct WindowSpec<'input> {
    #[pretty(break_before = soft)]
    pub body: WindowSpecBody<'input>,
}

/// Body of an OVER clause.
///
/// Variant ordering: Inline (starts with `(`) before Named (starts with an
/// identifier). They start with different tokens so peek disambiguation is
/// trivial.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowSpecBody<'input> {
    Inline(#[tok(LPAREN, this, RPAREN)] InlineWindowSpec<'input>),
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
#[tok(PARTITION, BY, this)]
pub struct WindowPartitionBy<'input> {
    #[sep(COMMA)]
    pub exprs: Vec<Expr<'input>>,
}

/// Frame unit: `ROWS | RANGE | GROUPS`.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameUnit {
    #[tok(ROWS)]
    Rows,
    #[tok(RANGE)]
    Range,
    #[tok(GROUPS)]
    Groups,
}

/// `WINDOW` frame clause: `unit (BETWEEN start AND end | bound) [EXCLUDE ...]`.
/// The common unit prefix is represented once so the bounded-lookahead
/// decision begins at `BETWEEN` versus the first bound token.
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameClause<'input> {
    pub unit: WindowFrameUnit,
    pub body: WindowFrameBody<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameBody<'input> {
    Between(WindowFrameBetween<'input>),
    Single(WindowFrameSingle<'input>),
}

/// `unit BETWEEN start AND end [EXCLUDE ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameBetween<'input> {
    #[tok(BETWEEN, this)]
    pub start: WindowFrameBound<'input>,
    #[tok(AND, this)]
    pub end: WindowFrameBound<'input>,
    pub exclude: Option<WindowFrameExclude>,
}

/// `unit start [EXCLUDE ...]`
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameSingle<'input> {
    pub bound: WindowFrameBound<'input>,
    pub exclude: Option<WindowFrameExclude>,
}

/// A single frame bound.
///
/// `UNBOUNDED` is admitted as an expression word and therefore shares the
/// ordinary expression-plus-direction representation. `CURRENT ROW` remains
/// the one fixed form without a direction suffix.
#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameBound<'input> {
    #[tok(CURRENT, ROW)]
    CurrentRow,
    Offset(WindowFrameOffset<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameOffset<'input> {
    pub expr: Box<Expr<'input>>,
    pub direction: WindowFrameDirection,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameDirection {
    #[tok(PRECEDING)]
    Preceding,
    #[tok(FOLLOWING)]
    Following,
}

/// `EXCLUDE { CURRENT ROW | GROUP | TIES | NO OTHERS }` frame exclusion.
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowFrameExclude {
    #[tok(EXCLUDE, this)]
    pub target: WindowFrameExcludeTarget,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum WindowFrameExcludeTarget {
    #[tok(CURRENT, ROW)]
    CurrentRow,
    #[tok(GROUP)]
    Group,
    #[tok(TIES)]
    Ties,
    #[tok(NO, OTHERS)]
    NoOthers,
}

/// PostgreSQL's `func_arg_expr`: a named or positional function argument.
///
/// `VARIADIC` is deliberately not an argument variant. PostgreSQL admits it
/// only as the sole argument or after the final comma, so the surrounding
/// application states own that token and make its cardinality structural.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncArg<'input> {
    Named(NamedFuncArg<'input>),
    Plain(Box<Expr<'input>>),
}

/// `=>` or `:=` — the two named-argument operators PostgreSQL accepts.
///
/// Variant ordering: both are distinct two-character punctuation tokens,
/// no ambiguity.
#[derive(recursa::Node, Debug, Clone)]
pub enum NamedArgOp {
    #[tok(FATARROW)]
    FatArrow,
    #[tok(COLONEQUALS)]
    ColonEquals,
}

/// Named function argument: `name => value` or `name := value` (Postgres).
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedFuncArg<'input> {
    pub name: crate::tokens::type_function_name<'input>,
    pub arrow: NamedArgOp,
    pub value: Box<Expr<'input>>,
}

/// One or more ordinary PostgreSQL `func_arg_expr` values.
///
/// The comma is a separator for the whole sequence, rather than a leading
/// token on every argument after the first. That keeps commas attached to the
/// preceding argument when the sequence is pretty-printed.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
pub struct FunctionArgumentSequence<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<FuncArg<'input>>,
);

/// The `VARIADIC func_arg_expr` at either legal variadic site.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionVariadicArgument<'input> {
    #[tok(VARIADIC, this)]
    pub argument: FuncArg<'input>,
}

/// The sole `VARIADIC func_arg_expr` application form.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionLeadingVariadicArguments<'input> {
    pub variadic: FunctionVariadicArgument<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
}

/// The state after a comma in an ordinary application.
///
/// `VARIADIC` cannot start [`FuncArg`], so these alternatives have disjoint
/// FIRST sets. The variadic branch has no continuation and is terminal by
/// construction.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionArgumentAfterComma<'input> {
    Variadic(FunctionVariadicArgument<'input>),
    Next(Box<FunctionOrdinaryArgumentSequence<'input>>),
}

/// One comma followed by either the next plain argument or the one terminal
/// variadic argument.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionArgumentContinuation<'input> {
    #[tok(COMMA, this)]
    pub next: FunctionArgumentAfterComma<'input>,
}

/// A non-empty plain list with at most one trailing variadic argument.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionOrdinaryArgumentSequence<'input> {
    pub first: FuncArg<'input>,
    pub next: Option<Box<FunctionArgumentContinuation<'input>>>,
}

impl FunctionOrdinaryArgumentSequence<'_> {
    pub fn has_trailing_variadic(&self) -> bool {
        match self.next.as_deref().map(|continuation| &continuation.next) {
            None => false,
            Some(FunctionArgumentAfterComma::Variadic(_)) => true,
            Some(FunctionArgumentAfterComma::Next(next)) => next.has_trailing_variadic(),
        }
    }
}

/// A plain non-empty argument list, optionally ending in one variadic
/// argument, followed by the aggregate's optional inner `ORDER BY`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionOrdinaryArguments<'input> {
    pub args: FunctionOrdinaryArgumentSequence<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
}

/// `ALL` followed by a required ordinary argument list and optional order.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionAllArguments<'input> {
    #[tok(ALL, this)]
    pub args: FunctionArgumentSequence<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
}

/// `DISTINCT` followed by a required ordinary argument list and optional
/// aggregate order.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionDistinctArguments<'input> {
    #[tok(DISTINCT, this)]
    pub args: FunctionArgumentSequence<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
}

/// The dedicated PostgreSQL `func_name '(' '*' ')'` application body.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionCallStar {
    #[tok(STAR)]
    Value,
}

/// Non-empty content inside a PostgreSQL `func_application`.
///
/// A wildcard is not an expression in PostgreSQL. Its exclusive alternative
/// here prevents invalid authored states such as `f(*, 1)` or
/// `f(DISTINCT *)`.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionCallBody<'input> {
    Star(FunctionCallStar),
    All(FunctionAllArguments<'input>),
    Distinct(FunctionDistinctArguments<'input>),
    LeadingVariadic(FunctionLeadingVariadicArguments<'input>),
    Args(FunctionOrdinaryArguments<'input>),
}

/// A complete PostgreSQL `func_application` after its function name.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionCallApplication<'input> {
    pub open: FunctionCallOpen,
    pub body: Option<FunctionCallBody<'input>>,
    pub close: FunctionCallClose,
}

/// A named `func_application` without aggregate/window suffixes.
///
/// PostgreSQL reuses this exact grammar in function-table positions.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionApplicationExpr<'input> {
    pub name: FuncCallName<'input>,
    pub application: FunctionCallApplication<'input>,
}

/// `WITHIN GROUP (ORDER BY ...)` clause for ordered-set aggregate functions.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WITHIN, GROUP, this)]
pub struct WithinGroupClause<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[pretty(break_before = soft)]
    pub order_by: Box<crate::ast::dml::select::OrderByClause<'input>>,
}

/// `FILTER (WHERE condition)` clause for filtered aggregates.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FILTER, this)]
pub struct FilterClause<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[pretty(break_before = soft)]
    pub body: Box<crate::ast::dml::select::WhereClause<'input>>,
}

/// Function name in call position.
///
/// PostgreSQL's `func_name` admits a `type_function_name` directly, while a
/// dotted name begins with `ColId`. Keeping those two admission sets here is
/// important: `QualifiedName` is intentionally broader and would also admit
/// every `COL_NAME` keyword, making the dedicated XML/JSON expression forms
/// indistinguishable from an ordinary function call.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncCallName<'input> {
    Qualified(FuncCallQualifiedName<'input>),
    Name(crate::tokens::type_function_name<'input>),
}

/// A dotted function name. At least one dotted tail is required so this does
/// not overlap the unqualified `type_function_name` alternative above.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncCallQualifiedName<'input> {
    pub first: crate::tokens::ColId<'input>,
    pub tail: recursa::Vec1<FuncCallNamePart<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct FuncCallNamePart<'input> {
    #[tok(DOT, this)]
    pub name: literal::Ident<'input>,
}

/// Required opening delimiter shared by ordinary and quoted function calls.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionCallOpen {
    #[tok(LPAREN)]
    Value,
}

/// Required closing delimiter shared by ordinary and quoted function calls.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionCallClose {
    #[tok(RPAREN)]
    Value,
}

/// Function application with no `WITHIN GROUP` suffix.
///
/// This state admits every PostgreSQL `func_application`; `FILTER` and
/// `OVER` remain available independently.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionPlainTail<'input> {
    pub open: FunctionCallOpen,
    pub body: Option<FunctionCallBody<'input>>,
    pub close: FunctionCallClose,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// A required argument list with no inner aggregate order.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionWithinGroupArguments<'input> {
    pub args: FunctionArgumentSequence<'input>,
}

/// `ALL` arguments eligible for a following `WITHIN GROUP`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionWithinGroupAllArguments<'input> {
    #[tok(ALL, this)]
    pub args: FunctionArgumentSequence<'input>,
}

/// Application bodies that pass PostgreSQL's `func_expr` WITHIN checks.
///
/// DISTINCT, VARIADIC, and an inner ORDER BY have no representation here.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionWithinGroupBody<'input> {
    Star(FunctionCallStar),
    All(FunctionWithinGroupAllArguments<'input>),
    Args(FunctionWithinGroupArguments<'input>),
}

/// A parenthesized application restricted to WITHIN-compatible bodies.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionWithinGroupApplication<'input> {
    pub open: FunctionCallOpen,
    pub body: Option<FunctionWithinGroupBody<'input>>,
    pub close: FunctionCallClose,
}

/// A function application whose required WITHIN suffix is valid by shape.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionWithinGroupTail<'input> {
    pub open: FunctionCallOpen,
    pub body: Option<FunctionWithinGroupBody<'input>>,
    pub close: FunctionCallClose,
    pub within_group: WithinGroupClause<'input>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// Function-style typed literal after a shared function/type name.
///
/// PostgreSQL requires a non-empty plain expression list here and rejects
/// named arguments and aggregate ORDER BY in its grammar action. Keeping the
/// type-modifier list as `Expr` makes all of those exclusions structural.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionTypedLiteralTail<'input> {
    pub open: FunctionCallOpen,
    #[sep(COMMA)]
    pub typmods: recursa::Vec1<Expr<'input>>,
    pub close: FunctionCallClose,
    pub value: TypeCastValue<'input>,
}

/// Valid continuations after a function/type name.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionCallTail<'input> {
    TypedLiteral(FunctionTypedLiteralTail<'input>),
    WithinGroup(FunctionWithinGroupTail<'input>),
    Plain(FunctionPlainTail<'input>),
}

/// Function expression with a staged, state-valid continuation.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncCall<'input> {
    pub name: FuncCallName<'input>,
    pub tail: FunctionCallTail<'input>,
}

/// Single identifier retained for the standalone quoted-call compatibility surface.
///
/// Expression parsing admits quoted names through [`FuncCallName`] and
/// [`Expr::Func`]; this wrapper remains public for callers that parse or
/// construct [`QuotedFuncCall`] directly.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuotedFuncName<'input> {
    Name(crate::tokens::literal::Ident<'input>),
}

/// `"name"(...)` as a standalone quoted-call compatibility surface.
///
/// [`Expr`] routes quoted function names through [`Expr::Func`] and
/// [`FuncCallName`]. This public type remains available to callers that use
/// the narrower unqualified quoted-call grammar directly.
#[derive(recursa::Node, Debug, Clone)]
pub struct QuotedFuncCall<'input> {
    pub name: QuotedFuncName<'input>,
    pub tail: FunctionCallTail<'input>,
}

/// Content inside parentheses: either a query or a non-empty,
/// comma-separated expression list.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParenContent<'input> {
    Subquery(Box<DirectSubquery<'input>>),
    Exprs(#[sep(COMMA)] recursa::Vec1<Expr<'input>>),
}

/// Terminal dot-star indirection.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParenthesizedDotStar {
    #[tok(DOT, STAR)]
    Value,
}

/// One non-star element in the indirection chain following parenthesized
/// content.
#[derive(recursa::Node, Debug, Clone)]
pub enum ParenthesizedIndirection<'input> {
    Field(IndirectionField<'input>),
    Subscript(BracketSubscript<'input>),
    Star(ParenthesizedDotStar),
}

/// Parenthesized scalar, row, or subquery content, optionally followed by an
/// arbitrary field, wildcard, or subscript indirection chain.
///
/// Owning the common `(` prefix in one Pratt atom keeps `(expr)`, `(a, b)`,
/// `(SELECT ...)`, `(expr).*`, and `(expr).field` in one declarative grammar
/// branch. A singleton [`ParenContent::Exprs`] is the authored precedence
/// grouping path used by Pretty. The grammar admits `.*` in the common chain;
/// enforcing that it is terminal belongs to the later PostgreSQL
/// semantic-validation layer.
#[derive(recursa::Node, Debug, Clone)]
pub struct ParenthesizedExpr<'input> {
    pub open: ParenthesizedOpen,
    pub content: ParenContent<'input>,
    pub close: ParenthesizedClose,
    pub indirection: Vec<ParenthesizedIndirection<'input>>,
}

/// Array slice content: `lower : upper`, `: upper`, `lower :`, or `:`.
///
/// Both bounds are optional; the colon is required.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubscriptSlice<'input> {
    pub lower: Option<Box<Expr<'input>>>,
    #[tok(COLON, this)]
    pub upper: Option<Box<Expr<'input>>>,
}

/// Required `:` plus the optional upper bound of an array slice.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubscriptSliceSuffix<'input> {
    pub colon: SubscriptColon,
    pub upper: Option<Box<Expr<'input>>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SubscriptColon {
    #[tok(COLON)]
    Value,
}

/// Colon-prefixed client value. In an ordinary expression this retains psql's
/// `:name` / `:'name'` spelling. Inside a bracket it also gives the
/// lower-unbounded slice forms (`[:2]`, `[:]`) one non-nullable expression
/// representation, avoiding an exact grammar overlap between a slice colon
/// and a psql-variable colon.
#[derive(recursa::Node, Debug, Clone)]
pub struct PsqlVariableExpr<'input> {
    pub colon: PsqlColon,
    pub value: Option<PsqlVariableExprValue<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum PsqlColon {
    #[tok(COLON)]
    Value,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum PsqlVariableExprValue<'input> {
    Psql(literal::PsqlVariableValue<'input>),
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    Paren(ParenthesizedExpr<'input>),
    #[tok(NULL)]
    Null,
    #[tok(TRUE)]
    True,
    #[tok(FALSE)]
    False,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct BracketSubscriptValue<'input> {
    pub lower: Box<Expr<'input>>,
    pub slice: Option<SubscriptSliceSuffix<'input>>,
}

/// Shared payload for both an index and a slice.
#[derive(recursa::Node, Debug, Clone)]
pub struct BracketSubscript<'input> {
    pub open: SubscriptOpen,
    pub content: BracketSubscriptValue<'input>,
    pub close: SubscriptClose,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SubscriptOpen {
    #[tok(LBRACKET)]
    Value,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SubscriptClose {
    #[tok(RBRACKET)]
    Value,
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
    Subscript(BracketSubscript<'input>),
    Field(IndirectionField<'input>),
}

/// Operator in PostgreSQL's `subquery_Op` production.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedComparisonOperator<'input> {
    #[tok(NOT, LIKE)]
    NotLike,
    #[tok(NOT, ILIKE)]
    NotIlike,
    #[tok(LIKE)]
    Like,
    #[tok(ILIKE)]
    Ilike,
    Decorated(QuantifiedDecoratedOperator<'input>),
    Plain(crate::ast::shared::names::OperatorName<'input>),
}

/// `OPERATOR(any_operator)` in a quantified comparison.
#[derive(recursa::Node, Debug, Clone)]
pub struct QuantifiedDecoratedOperator<'input> {
    #[tok(OPERATOR, LPAREN, this, RPAREN)]
    pub name: crate::ast::shared::names::QualifiedOperatorName<'input>,
}

/// `ANY`, `SOME`, or `ALL` following a comparison operator.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedComparisonKind {
    #[tok(ANY)]
    Any,
    #[tok(SOME)]
    Some,
    #[tok(ALL)]
    All,
}

/// The single expression or query inside a quantified comparison.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedComparisonOperand<'input> {
    Subquery(Box<DirectSubquery<'input>>),
    Expr(Box<Expr<'input>>),
}

/// `operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonSuffix<'input> {
    pub operator: QuantifiedComparisonOperator<'input>,
    pub kind: QuantifiedComparisonKind,
    #[tok(LPAREN, this, RPAREN)]
    pub operand: QuantifiedComparisonOperand<'input>,
}

/// EXISTS subquery: `EXISTS (SELECT ...)`
#[derive(recursa::Node, Debug, Clone)]
pub struct ExistsExpr<'input> {
    #[tok(EXISTS, LPAREN, this, RPAREN)]
    pub subquery: Box<Subquery<'input>>,
}

/// One element of an `ARRAY[...]` constructor: either an ordinary
/// expression or a nested bracketed sub-list (for multi-dimensional
/// literals like `ARRAY[[1,2],[3,4]]`).
///
/// Variant ordering: `Nested` leads with `[`, which no expression atom
/// does, so dispatch is unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum ArrayElement<'input> {
    Nested(NestedArrayElements<'input>),
    Expr(Box<Expr<'input>>),
}

/// One bracketed sub-list inside a multi-dimensional `ARRAY[...]` literal.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LBRACKET, this, RBRACKET)]
pub struct NestedArrayElements<'input> {
    #[sep(COMMA)]
    pub elements: Vec<ArrayElement<'input>>,
}

/// ARRAY bracket constructor: `ARRAY[expr, ...]`, including the
/// multi-dimensional form `ARRAY[[1,2],[3,4]]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ARRAY, LBRACKET, this, RBRACKET)]
pub struct ArrayBracket<'input> {
    #[sep(COMMA)]
    pub elements: recursa::Vec1<ArrayElement<'input>>,
}

/// ARRAY subquery constructor: `ARRAY(subquery)`
#[derive(recursa::Node, Debug, Clone)]
pub struct ArraySubquery<'input> {
    #[tok(ARRAY, LPAREN, this, RPAREN)]
    pub subquery: Box<Subquery<'input>>,
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
#[tok(ROW, LPAREN, this, RPAREN)]
pub struct RowExpr<'input> {
    #[sep(COMMA)]
    pub values: recursa::Vec1<Expr<'input>>,
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

/// Searched CASE body: `WHEN cond THEN result [...] [ELSE result]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseSearched<'input> {
    pub first_arm: CaseWhenArm<'input>,
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    pub else_clause: Option<CaseElse<'input>>,
}

/// Simple CASE body: `operand WHEN val THEN result [...] [ELSE result]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseSimple<'input> {
    pub operand: Box<Expr<'input>>,
    pub first_arm: CaseWhenArm<'input>,
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    pub else_clause: Option<CaseElse<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum CaseBody<'input> {
    Searched(CaseSearched<'input>),
    Simple(CaseSimple<'input>),
}

/// CASE expression with its common `CASE` / `END` delimiters factored out.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseExpr<'input> {
    #[tok(CASE, this, END)]
    pub body: CaseBody<'input>,
}

/// One `opt_array_bounds` element: `[]` or `[N]`.
///
/// Postgres syntax: `Typename opt_array_bounds` allows arbitrary repetition
/// of either form (`int4[]`, `int4[1]`, `varchar(4)[2][3]`, …). Variant
/// ordering: `Sized` (`[N]`, 3 tokens) before `Empty` (`[]`, 2 tokens) so
/// longest-match-wins picks the longer form when an integer literal is
/// present between the brackets.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum ArraySuffix<'input> {
    Sized(ArraySuffixSized<'input>),
    Empty(ArraySuffixEmpty),
}

/// `[N]` array bound.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub struct ArraySuffixSized<'input> {
    #[tok(LBRACKET, this, RBRACKET)]
    pub bounds: literal::IntegerLit<'input>,
}

/// `[]` array suffix (unbounded).
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum ArraySuffixEmpty {
    #[tok(LBRACKET, RBRACKET)]
    Value,
}

/// Cast type with a base-specific modifier and zero-or-more array suffixes:
/// `numeric(10,0)`, `timestamp with time zone`, `interval day to minute`,
/// `integer[]`, `int4[][][]`, `varchar(4)[2][3]`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub struct CastType<'input> {
    pub head: CastTypeHead<'input>,
    pub array_suffixes: Vec<ArraySuffix<'input>>,
    /// PG gram.y also accepts `SimpleTypename ARRAY` and
    /// `SimpleTypename ARRAY '[' Iconst ']'` — the keyword form for
    /// declaring an array type (e.g. `integer ARRAY[4]`, `text ARRAY`).
    /// In practice this is mutually exclusive with `array_suffixes`, but the
    /// grammar admits the suffix appearing AFTER the keyword form, so the
    /// field is parsed last.
    pub array_kw_suffix: Option<ArrayKwSuffix<'input>>,
}

/// The modifier-bearing portion of a cast type.
///
/// PostgreSQL gives date/time and interval types dedicated productions. In
/// particular, `WITH/WITHOUT TIME ZONE` is not a suffix on an arbitrary type;
/// keeping it structural prevents a following `WITH UNIQUE KEYS` JSON clause
/// from being consumed as part of a `json` cast.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum CastTypeHead<'input> {
    DateTime(DateTimeCastType<'input>),
    Interval(IntervalCastType<'input>),
    General(GeneralCastType<'input>),
}

/// `TIMESTAMP` or `TIME`, with their optional precision and timezone suffix.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub struct DateTimeCastType<'input> {
    pub base: DateTimeCastTypeName,
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
}

#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum DateTimeCastTypeName {
    #[tok(TIMESTAMP)]
    Timestamp,
    #[tok(TIME)]
    Time,
}

/// `INTERVAL`, optionally with either a full-type precision or a field range.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
#[tok(INTERVAL, this)]
pub struct IntervalCastType<'input> {
    pub modifier: Option<IntervalCastTypeModifier<'input>>,
}

#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum IntervalCastTypeModifier<'input> {
    Precision(TypePrecision<'input>),
    Qualifier(IntervalQualifier<'input>),
}

/// A type without the date/time- or interval-specific suffix grammar.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub struct GeneralCastType<'input> {
    pub base: GeneralCastTypeName<'input>,
    #[presence(VARYING)]
    /// `VARYING` modifier (e.g., `BIT VARYING`, `CHARACTER VARYING`).
    /// Always precedes the precision parens.
    pub varying: bool,
    pub precision: Option<TypePrecision<'input>>,
}

/// A cast base without `TIME`, `TIMESTAMP`, or `INTERVAL`.
///
/// This mirrors [`TypeName`] for the general PostgreSQL type production while
/// making the three suffix-bearing families disjoint by construction.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum GeneralCastTypeName<'input> {
    #[tok(BOOL)]
    Bool,
    #[tok(BOOLEAN)]
    Boolean,
    #[tok(TEXT)]
    Text,
    #[tok(INTEGER)]
    Integer,
    #[tok(INT)]
    Int,
    #[tok(SERIAL)]
    Serial,
    #[tok(NUMERIC)]
    Numeric,
    #[tok(VARCHAR)]
    Varchar,
    #[tok(DOUBLE, PRECISION)]
    DoublePrecision,
    #[tok(BIT)]
    Bit,
    #[tok(CHARACTER)]
    Character,
    #[tok(UNKNOWN)]
    Unknown,
    Ident(TypeNameIdent<'input>),
}

/// `ARRAY` or `ARRAY[N]` post-type-name array suffix
/// (PG gram.y: `SimpleTypename ARRAY | SimpleTypename ARRAY '[' Iconst ']'`).
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
#[tok(ARRAY, this)]
pub struct ArrayKwSuffix<'input> {
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

/// Fixed-keyword name accepted by PostgreSQL's function-style typed literal
/// syntax.
///
/// These are `COL_NAME` keywords and therefore cannot also be generic
/// function names.
#[derive(recursa::Node, Debug, Clone)]
pub enum FixedTypeCastFuncName {
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
    #[tok(BIT)]
    Bit,
    #[tok(CHARACTER)]
    Character,
}

/// Function-style typed literal for a fixed-keyword type. The optional typmod
/// list is kept on this same node so `numeric '1'` and
/// `numeric(10, 2) '1.00'` share their prefix honestly.
#[derive(recursa::Node, Debug, Clone)]
pub struct FixedTypeCastFunc<'input> {
    pub type_name: FixedTypeCastFuncName,
    #[presence(VARYING)]
    pub varying: bool,
    pub typmods: Option<TypePrecision<'input>>,
    pub value: TypeCastValue<'input>,
}

/// Function-style typed literal for an identifier-spelled type without
/// typmods: `bool 'value'`, `text 'hello'`, `bigint :'var'`, or
/// `double precision 'value'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedTypeCastFunc<'input> {
    pub type_name: crate::tokens::type_function_name<'input>,
    #[presence(PRECISION)]
    pub precision: bool,
    pub value: TypeCastValue<'input>,
}

/// Function-style typed literal. Fixed-keyword type names can carry typmods
/// directly; identifier-spelled types with typmods use
/// [`FunctionCallTail::TypedLiteral`], where the complete call/typmod prefix
/// is shared.
#[derive(recursa::Node, Debug, Clone)]
pub enum TypeCastFunc<'input> {
    Fixed(FixedTypeCastFunc<'input>),
    Named(NamedTypeCastFunc<'input>),
}

/// `WITH TIME ZONE` or `WITHOUT TIME ZONE` suffix for `TIMESTAMP`/`TIME`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum TimeZoneQualifier {
    #[tok(WITH, TIME, ZONE)]
    With,
    #[tok(WITHOUT, TIME, ZONE)]
    Without,
}

/// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TimestampLit<'input> {
    pub timestamp: TimestampKeyword,
    /// Optional precision, e.g., `timestamp(6)`.
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub value: literal::StringLit<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum TimestampKeyword {
    #[tok(TIMESTAMP)]
    Value,
}

/// `TIME [WITH|WITHOUT TIME ZONE] 'string'`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TimeLit<'input> {
    pub time: TimeKeyword,
    /// Optional precision, e.g., `time(2)`.
    pub precision: Option<TypePrecision<'input>>,
    pub tz: Option<TimeZoneQualifier>,
    pub value: literal::StringLit<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum TimeKeyword {
    #[tok(TIME)]
    Value,
}

/// `SECOND [(p)]` — the SECOND keyword with optional fractional-second
/// precision. Used in interval qualifiers like `SECOND(2)` or
/// `DAY TO SECOND(2)`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
#[tok(SECOND, this)]
pub struct SecondWithPrecision<'input> {
    pub precision: Option<TypePrecision<'input>>,
}

/// Optional qualifier after `INTERVAL 'str'`.
///
/// Variant ordering: multi-keyword `X TO Y` forms must come before the
/// single-keyword forms so longest-match-wins picks the fuller qualifier
/// when available. `*ToSecond` variants use `SecondWithPrecision` which
/// allows optional `(p)` precision.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum IntervalQualifier<'input> {
    #[tok(YEAR, TO, MONTH)]
    YearToMonth,
    #[tok(DAY, TO, HOUR)]
    DayToHour,
    #[tok(DAY, TO, MINUTE)]
    DayToMinute,
    DayToSecond(#[tok(DAY, TO, this)] SecondWithPrecision<'input>),
    #[tok(HOUR, TO, MINUTE)]
    HourToMinute,
    HourToSecond(#[tok(HOUR, TO, this)] SecondWithPrecision<'input>),
    MinuteToSecond(#[tok(MINUTE, TO, this)] SecondWithPrecision<'input>),
    #[tok(YEAR)]
    Year,
    #[tok(MONTH)]
    Month,
    #[tok(DAY)]
    Day,
    #[tok(HOUR)]
    Hour,
    #[tok(MINUTE)]
    Minute,
    Second(SecondWithPrecision<'input>),
}

/// `INTERVAL 'str' [qualifier]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IntervalLit<'input> {
    pub interval: IntervalKeyword,
    /// Optional precision, e.g. `interval(2)` or `interval(0)`.
    pub precision: Option<TypePrecision<'input>>,
    pub value: literal::StringLit<'input>,
    pub qualifier: Option<IntervalQualifier<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum IntervalKeyword {
    #[tok(INTERVAL)]
    Value,
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
#[tok(XMLATTRIBUTES, LPAREN, this, RPAREN)]
pub struct XmlAttributes<'input> {
    #[sep(COMMA)]
    pub args: recursa::Vec1<XmlNamedArg<'input>>,
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
#[tok(COMMA, this)]
pub struct XmlElementContentTail<'input> {
    #[sep(COMMA)]
    pub exprs: recursa::Vec1<Expr<'input>>,
}

/// Body of `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
///
/// Variant ordering: the `WithAttrs` form starts with `, xmlattributes(`
/// (longer match) and must be tried before `WithContent` which starts with
/// just `,`. Both trail an `xmlelement(NAME ident` head.
pub type XmlElementTail<'input> = XmlElementContentTail<'input>;

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
    pub inner: XmlElementInner<'input>,
}

/// `xmlforest(expr [AS alias], ...)`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(XMLFOREST, LPAREN, this, RPAREN)]
pub struct XmlForest<'input> {
    #[sep(COMMA)]
    pub args: recursa::Vec1<XmlNamedArg<'input>>,
}

/// `xmlpi(NAME ident [, content])`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlPi<'input> {
    #[tok(XMLPI, LPAREN, this, RPAREN)]
    pub inner: XmlPiInner<'input>,
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
    #[tok(DOCUMENT)]
    Document,
    #[tok(CONTENT)]
    Content,
}

/// `INDENT` / `NO INDENT` — output indentation option of `XMLSERIALIZE`.
///
/// Variant ordering: `NoIndent` (`NO INDENT`, two tokens) before `Indent`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlIndentOption {
    #[tok(NO, INDENT)]
    NoIndent,
    #[tok(INDENT)]
    Indent,
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
    pub inner: XmlSerializeInner<'input>,
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
    pub inner: XmlParseInner<'input>,
}

/// `VERSION {‹expr› | NO VALUE}` — the version argument of `XMLROOT`.
///
/// Variant ordering: `NoValue` (`NO VALUE`) before the catch-all `Expr`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlVersionValue<'input> {
    #[tok(NO, VALUE)]
    NoValue,
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
    #[tok(YES)]
    Yes,
    #[tok(NO, VALUE)]
    NoValue,
    #[tok(NO)]
    No,
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
    pub inner: XmlRootInner<'input>,
}

/// `BY REF` / `BY VALUE` qualifier of an `XMLEXISTS` / `XMLTABLE` PASSING clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlRefOrValue {
    #[tok(REF)]
    Ref,
    #[tok(VALUE)]
    Value,
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
    pub passing: XmlExistsPassing<'input>,
}

/// Required `PASSING` clause of `XMLEXISTS`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlExistsPassing<'input> {
    #[tok(PASSING, this)]
    pub document: XmlExistsDocument<'input>,
    pub by_after: Option<XmlPassingBy>,
}

/// The document expression, optionally introduced by `BY REF` / `BY VALUE`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlExistsDocument<'input> {
    Qualified(XmlExistsQualifiedDocument<'input>),
    Plain(Box<Expr<'input>>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct XmlExistsQualifiedDocument<'input> {
    pub by: XmlPassingBy,
    pub doc: Box<Expr<'input>>,
}

/// `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlExists<'input> {
    #[tok(XMLEXISTS, LPAREN, this, RPAREN)]
    pub inner: XmlExistsInner<'input>,
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
    #[tok(LEADING)]
    Leading,
    #[tok(TRAILING)]
    Trailing,
    #[tok(BOTH)]
    Both,
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
    Values(TrimValues<'input>),
}

/// `FROM expr_list` tail of `TRIM(...)`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FROM, this)]
pub struct TrimFromArgs<'input> {
    #[sep(COMMA)]
    pub args: recursa::Vec1<Expr<'input>>,
}

/// `chars FROM source` tail of `TRIM(...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimWithChars<'input> {
    pub chars: Box<Expr<'input>>,
    #[tok(FROM, this)]
    #[sep(COMMA)]
    pub args: recursa::Vec1<Expr<'input>>,
}

/// A value-led TRIM tail. A following `FROM` turns the first value into the
/// trim character; comma suffixes represent the ordinary function form.
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimValues<'input> {
    pub first: Box<Expr<'input>>,
    pub from: Option<TrimFromArgs<'input>>,
    pub more: Vec<TrimMoreArg<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct TrimMoreArg<'input> {
    #[tok(COMMA, this)]
    pub value: Box<Expr<'input>>,
}

/// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct TrimCall<'input> {
    #[tok(TRIM, LPAREN, this, RPAREN)]
    pub inner: TrimInner<'input>,
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
    pub arg: Box<Expr<'input>>,
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
    pub inner: CastAsInner<'input>,
}

/// `SUBSTRING(source FROM start [FOR len])` /
/// `SUBSTRING(source SIMILAR pattern ESCAPE escape)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringCall<'input> {
    #[tok(SUBSTRING, LPAREN, this, RPAREN)]
    pub inner: SubstringInner<'input>,
}

/// Inner of `POSITION(needle IN haystack)`.
///
/// PostgreSQL's `position_list` parses the needle as `b_expr`, its restricted
/// expression grammar, so the delimiter `IN` cannot be consumed as the
/// ordinary expression-level `IN` extender. The exclusions below are the
/// infix/postfix productions present in `a_expr` but absent from `b_expr` in
/// the vendored PostgreSQL 17 grammar. Symbolic operators, comparisons,
/// casts, `IS [NOT] DISTINCT FROM`, and `IS [NOT] DOCUMENT` remain enabled.
/// Parentheses start a fresh unrestricted expression, matching PostgreSQL's
/// rule that `(a_expr)` is itself a `b_expr` atom.
#[derive(recursa::Node, Debug, Clone)]
pub struct PositionInner<'input> {
    #[parse(pratt(exclude(
        Collate,
        QuantifiedComparison,
        IsJson,
        IsNormalized,
        BoolTest,
        Notnull,
        Isnull,
        AtLocal,
        AtTimeZone,
        NotInExpr,
        NotIlike,
        NotSimilarTo,
        NotLike,
        SimilarTo,
        Ilike,
        Like,
        Overlaps,
        InExpr,
        NotBetweenExpr,
        BetweenExpr,
        Or,
        And
    )))]
    pub needle: Box<Expr<'input>>,
    #[tok(IN, this)]
    pub haystack: Box<Expr<'input>>,
}

/// `POSITION(needle IN haystack)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PositionCall<'input> {
    #[tok(POSITION, LPAREN, this, RPAREN)]
    pub inner: PositionInner<'input>,
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
    pub inner: OverlayInner<'input>,
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
    pub inner: ExtractInner<'input>,
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
#[tok(FORMAT, JSON, this)]
pub struct JsonFormat<'input> {
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
    #[tok(WITH)]
    With,
    #[tok(WITHOUT)]
    Without,
}

/// `{WITH|WITHOUT} UNIQUE [KEYS]` — duplicate-key handling for `JSON()` /
/// `JSON_OBJECT()`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonUniqueKeys {
    #[tok(this, UNIQUE)]
    pub with_or_without: WithOrWithout,
    /// Whether the optional `KEYS` noise word occurred, preserved for
    /// round-trip rendering.
    #[presence(KEYS)]
    pub keys: bool,
}

/// `NULL` / `ABSENT` lead-in of an `ON NULL` clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum NullOrAbsent {
    #[tok(NULL)]
    Null,
    #[tok(ABSENT)]
    Absent,
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
    pub inner: JsonConstructorInner<'input>,
}

/// `JSON_SCALAR ( ‹expr› )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonScalar<'input> {
    #[tok(JSON_SCALAR, LPAREN, this, RPAREN)]
    pub inner: Box<Expr<'input>>,
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
    pub inner: JsonSerializeInner<'input>,
}

/// Key/value separator inside a `JSON_OBJECT` entry: `:` or the `VALUE` keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonKeyValueSep {
    #[tok(COLON)]
    Colon,
    #[tok(VALUE)]
    Value,
}

/// One `[KEY] ‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry of `JSON_OBJECT`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectEntry<'input> {
    pub key: Box<Expr<'input>>,
    pub sep: JsonKeyValueSep,
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
}

/// Non-empty entry form of `JSON_OBJECT`, followed by the optional `ON NULL`,
/// `UNIQUE` and `RETURNING` clauses.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectArgs<'input> {
    #[sep(COMMA)]
    pub entries: recursa::Vec1<JsonObjectEntry<'input>>,
    pub on_null: Option<JsonOnNull>,
    pub unique: Option<JsonUniqueKeys>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_OBJECT` has distinct PostgreSQL productions for a non-empty entry
/// list and for the empty/returning-only form. Keeping those paths distinct
/// prevents the expression-led entry parser from claiming the reserved
/// `RETURNING` token as an entry key.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonObject<'input> {
    Entries(#[tok(JSON_OBJECT, LPAREN, this, RPAREN)] JsonObjectArgs<'input>),
    Returning(#[tok(JSON_OBJECT, LPAREN, this, RPAREN)] JsonReturning<'input>),
    #[tok(JSON_OBJECT, LPAREN, RPAREN)]
    Empty,
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
    Query(Box<DirectSubquery<'input>>),
    Elements(#[sep(COMMA)] recursa::Vec1<JsonArrayElement<'input>>),
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
    pub args: JsonArrayArgs<'input>,
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
#[tok(PASSING, this)]
pub struct JsonPassing<'input> {
    #[sep(COMMA)]
    pub args: recursa::Vec1<JsonPassingArg<'input>>,
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
    #[tok(EMPTY, ARRAY)]
    EmptyArray,
    #[tok(EMPTY, OBJECT)]
    EmptyObject,
    #[tok(EMPTY)]
    Empty,
    #[tok(ERROR)]
    Error,
    #[tok(NULL)]
    Null,
    #[tok(TRUE)]
    True,
    #[tok(FALSE)]
    False,
    #[tok(UNKNOWN)]
    Unknown,
    Default(JsonDefault<'input>),
}

/// `EMPTY` or `ERROR` — the trigger of an `ON` behavior clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum EmptyOrError {
    #[tok(EMPTY)]
    Empty,
    #[tok(ERROR)]
    Error,
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
    #[tok(CONDITIONAL)]
    Conditional,
    #[tok(UNCONDITIONAL)]
    Unconditional,
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
pub enum JsonQuotesOnScalar {
    #[tok(ON, SCALAR, STRING)]
    Value,
}

/// `KEEP` / `OMIT` lead-in of a `JSON_QUERY` quotes clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum KeepOrOmit {
    #[tok(KEEP)]
    Keep,
    #[tok(OMIT)]
    Omit,
}

/// `{KEEP|OMIT} QUOTES [ON SCALAR STRING]` — the `JSON_QUERY` quotes clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonQuotes {
    pub keep_or_omit: KeepOrOmit,
    pub quotes: JsonQuotesKeyword,
    pub on_scalar: Option<JsonQuotesOnScalar>,
}

/// Required `QUOTES` keyword in a `JSON_QUERY` quotes clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonQuotesKeyword {
    #[tok(QUOTES)]
    Value,
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
    pub inner: JsonExistsInner<'input>,
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
    pub inner: JsonValueInner<'input>,
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
    pub inner: JsonQueryInner<'input>,
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
    pub inner: JsonObjectAggInner<'input>,
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
    pub inner: JsonArrayAggInner<'input>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

// --- `IS JSON` predicate ---

/// The JSON item type tested by an `IS JSON` predicate.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonTypeKind {
    #[tok(VALUE)]
    Value,
    #[tok(SCALAR)]
    Scalar,
    #[tok(ARRAY)]
    Array,
    #[tok(OBJECT)]
    Object,
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
#[derive(recursa::Node, Debug, Clone)]
#[pratt]
pub enum Expr<'input> {
    // --- Prefix ---
    #[parse(prefix, bp = 15)]
    Not(#[tok(NOT, this)] Box<Self>),
    #[parse(prefix, bp = 12)]
    Neg(#[tok(MINUS, this)] Box<Self>),
    /// Unary plus: `+expr` — identity operator on numeric types.
    #[parse(prefix, bp = 12)]
    Pos(#[tok(PLUS, this)] Box<Self>),
    /// Unary geometric "center point": `@@ expr`. Postgres uses `@@` as
    /// a prefix operator on box / polygon / etc. (in addition to the
    /// text-search infix form).
    #[parse(prefix, bp = 12)]
    GeomCenter(#[tok(ATAT, this)] Box<Self>),
    /// Bitwise NOT: `~ expr` (e.g. inet / bit / int bitwise complement).
    /// Must come before any infix `~` variant so the prefix form wins when
    /// `~` appears at the start of an operand.
    #[parse(prefix, bp = 12)]
    BitNot(#[tok(TILDE, this)] Box<Self>),
    /// Geometric path/lseg length: `@-@ expr`. Must come before `Abs` (`@`)
    /// since `@-@` is longer.
    #[parse(prefix, bp = 12)]
    PathLength(#[tok(ATMINUSAT, this)] Box<Self>),
    /// User-defined prefix: `@#@ expr` (e.g. factorial).
    #[parse(prefix, bp = 12)]
    AtHashAtPrefix(#[tok(ATHASHAT, this)] Box<Self>),
    /// Geometric point-count: `# path` — number of points in a path.
    #[parse(prefix, bp = 12)]
    PointCount(#[tok(POUND, this)] Box<Self>),
    /// Absolute value: `@ expr` (Postgres unary `@` operator).
    #[parse(prefix, bp = 12)]
    Abs(#[tok(ATSIGN, this)] Box<Self>),
    /// User-defined prefix: `!=- expr`.
    #[parse(prefix, bp = 12)]
    BangEqMinusPrefix(#[tok(BANGEQMINUS, this)] Box<Self>),
    /// Square root: `|/ expr` (Postgres unary `|/` operator).
    #[parse(prefix, bp = 12)]
    Sqrt(#[tok(PIPESLASH, this)] Box<Self>),
    /// Cube root: `||/ expr` (Postgres unary `||/` operator).
    #[parse(prefix, bp = 12)]
    Cbrt(#[tok(PIPEPIPESLASH, this)] Box<Self>),

    /// Catch-all prefix: any user-defined prefix operator not matched by a
    /// specific token. Declared LAST among prefixes.
    // A dynamic content-token operator cannot occupy Recursa's fixed-only
    // Pratt prefix slot. As an atom it still has an unambiguous lexical
    // starter and retains the operator text plus its expression operand.
    CustomPrefix(
        literal::CustomOp<'input>,
        #[pretty(break_before = soft)] Box<Self>,
    ),

    // --- Postfix ---
    /// Postgres-style cast: `expr::type`
    #[parse(postfix, bp = 20)]
    Cast(Box<Self>, #[tok(COLONCOLON, this)] Box<CastType<'input>>),
    /// Array index or slice: `expr[idx]`, `expr[low:high]`, `expr[:high]`,
    /// `expr[low:]`, or `expr[:]`.
    #[parse(postfix, bp = 20)]
    Subscript(Box<Self>, BracketSubscript<'input>),
    /// `expr COLLATE "collation"` — collation specifier. Binds tighter than
    /// comparisons (bp 5) but looser than `::` cast (bp 20).
    #[parse(postfix, bp = 18)]
    Collate(
        Box<Self>,
        #[tok(COLLATE, this)] crate::tokens::ColId<'input>,
    ),
    /// `lhs operator {ANY|SOME|ALL} (expr-or-query)`.
    ///
    /// PostgreSQL does not admit the quantified right-hand side as a
    /// standalone expression. Keeping the operator and quantifier in one
    /// Pratt continuation also makes `f(ALL(x))` unambiguously the function
    /// application's ALL-qualified argument production.
    #[parse(postfix, bp = 8)]
    QuantifiedComparison(Box<Self>, QuantifiedComparisonSuffix<'input>),
    /// `expr IS NOT DISTINCT FROM expr`. Declared before `IsDistinctFrom` so
    /// the longer `NOT` prefix wins disambiguation.
    #[parse(infix, lbp = 5, rbp = 6)]
    IsNotDistinctFrom(Box<Self>, #[tok(IS, NOT, DISTINCT, FROM, this)] Box<Self>),
    /// `expr IS DISTINCT FROM expr`.
    #[parse(infix, lbp = 5, rbp = 6)]
    IsDistinctFrom(Box<Self>, #[tok(IS, DISTINCT, FROM, this)] Box<Self>),
    /// `expr IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] [{WITH|WITHOUT}
    /// UNIQUE [KEYS]]` — the SQL/JSON type predicate. Declared before
    /// `BoolTest` (both lead with `IS`); `BoolTest` rejects `JSON` as a
    /// `BoolTestKind`, so order is not load-bearing, only tidy.
    #[parse(postfix, bp = 8)]
    IsJson(Box<Self>, #[tok(IS, this)] IsJsonTail),
    /// `expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` — the Unicode
    /// normalisation predicate (gram.y rules 15198/15205/15212/15220).
    /// Declared before `BoolTest` (both lead with `IS`); `BoolTest` rejects
    /// `NORMALIZED`/`NFx` as a `BoolTestKind`, so order is not load-bearing.
    #[parse(postfix, bp = 8)]
    IsNormalized(Box<Self>, #[tok(IS, this)] IsNormalizedTail),
    /// `expr IS [NOT] DOCUMENT` — the XML document predicate.
    #[parse(postfix, bp = 8)]
    IsDocument(Box<Self>, #[tok(IS, this)] IsDocumentTail),
    /// Boolean test: `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`
    #[parse(postfix, bp = 8)]
    BoolTest(Box<Self>, #[tok(IS, this)] BoolTestKind),
    /// Postgres `expr NOTNULL` postfix null test (synonym for `IS NOT NULL`).
    #[parse(postfix, bp = 8)]
    Notnull(#[tok(this, NOTNULL)] Box<Self>),
    /// Postgres `expr ISNULL` postfix null test (synonym for `IS NULL`).
    #[parse(postfix, bp = 8)]
    Isnull(#[tok(this, ISNULL)] Box<Self>),
    /// `expr AT LOCAL` — convert to session timezone. Listed before
    /// `AtTimeZone` so `AT LOCAL` wins (distinct second token `LOCAL` vs `TIME`).
    #[parse(postfix, bp = 9)]
    AtLocal(#[tok(this, AT, LOCAL)] Box<Self>),
    /// `expr AT TIME ZONE zone_expr` — convert to specified timezone.
    #[parse(infix, lbp = 9, rbp = 10)]
    AtTimeZone(Box<Self>, #[tok(AT, TIME, ZONE, this)] Box<Self>),
    /// NOT IN list: `expr NOT IN (val, ...)`
    #[parse(postfix, bp = 6)]
    NotInExpr(Box<Self>, NotInSuffix<'input>),
    /// `expr NOT ILIKE pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT ILIKE` is tried first (matters only if any rule shares a prefix;
    /// here `NOT ILIKE` vs `NOT LIKE` differ on the second token).
    #[parse(postfix, bp = 5)]
    NotIlike(
        Box<Self>,
        #[tok(NOT, ILIKE, this)] Box<Self>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr NOT SIMILAR TO pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT SIMILAR TO` form wins longest-match-wins disambiguation.
    #[parse(postfix, bp = 5)]
    NotSimilarTo(
        Box<Self>,
        #[tok(NOT, SIMILAR, TO, this)] Box<Self>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr NOT LIKE pattern [ESCAPE char]`. Must come before the `Not` prefix atom so
    /// longest-match-wins prefers the postfix form.
    #[parse(postfix, bp = 5)]
    NotLike(
        Box<Self>,
        #[tok(NOT, LIKE, this)] Box<Self>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr SIMILAR TO pattern [ESCAPE char]` — SQL standard similar-to pattern match.
    #[parse(postfix, bp = 5)]
    SimilarTo(
        Box<Self>,
        #[tok(SIMILAR, TO, this)] Box<Self>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr ILIKE pattern [ESCAPE char]`
    #[parse(postfix, bp = 5)]
    Ilike(
        Box<Self>,
        #[tok(ILIKE, this)] Box<Self>,
        Option<EscapeClause<'input>>,
    ),
    /// `expr LIKE pattern [ESCAPE char]`
    #[parse(postfix, bp = 5)]
    Like(
        Box<Self>,
        #[tok(LIKE, this)] Box<Self>,
        Option<EscapeClause<'input>>,
    ),
    // --- Locale-aware text comparison operators (4-char before 3-char) ---
    /// `expr ~<=~ expr` — locale-aware less-or-equal.
    #[parse(infix, lbp = 5, rbp = 6)]
    TildeLeqTilde(Box<Self>, #[tok(TILDELEQTILDE, this)] Box<Self>),
    /// `expr ~>=~ expr` — locale-aware greater-or-equal.
    #[parse(infix, lbp = 5, rbp = 6)]
    TildeGeqTilde(Box<Self>, #[tok(TILDEGEQTILDE, this)] Box<Self>),
    /// `expr ~<~ expr` — locale-aware less-than.
    #[parse(infix, lbp = 5, rbp = 6)]
    TildeLtTilde(Box<Self>, #[tok(TILDELTTILDE, this)] Box<Self>),
    /// `expr ~>~ expr` — locale-aware greater-than.
    #[parse(infix, lbp = 5, rbp = 6)]
    TildeGtTilde(Box<Self>, #[tok(TILDEGTTILDE, this)] Box<Self>),
    /// `expr !~* pattern` — POSIX case-insensitive negated regex match.
    #[parse(infix, lbp = 5, rbp = 6)]
    RegexNotIMatch(Box<Self>, #[tok(BANGTILDESTAR, this)] Box<Self>),
    /// `expr ~* pattern` — POSIX case-insensitive regex match.
    #[parse(infix, lbp = 5, rbp = 6)]
    RegexIMatch(Box<Self>, #[tok(TILDESTAR, this)] Box<Self>),
    /// `expr !~ pattern` — POSIX negated regex match.
    #[parse(infix, lbp = 5, rbp = 6)]
    RegexNotMatch(Box<Self>, #[tok(BANGTILDE, this)] Box<Self>),
    /// `expr ~= expr` — geometric "same as" operator. Declared before `RegexMatch`
    /// so the longer `~=` wins longest-match.
    #[parse(infix, lbp = 5, rbp = 6)]
    GeomSame(Box<Self>, #[tok(TILDEEQ, this)] Box<Self>),
    /// `expr ~ pattern` — POSIX regex match.
    #[parse(infix, lbp = 5, rbp = 6)]
    RegexMatch(Box<Self>, #[tok(TILDE, this)] Box<Self>),
    /// `expr !~~* pattern` — operator-form `NOT ILIKE` (gram.y 14897).
    /// Declared before `LikeOpINeg` (`!~~`) so the longer `!~~*` wins.
    #[parse(infix, lbp = 5, rbp = 6)]
    LikeOpINeg(Box<Self>, #[tok(BANGTILDETILDESTAR, this)] Box<Self>),
    /// `expr ~~* pattern` — operator-form `ILIKE` (gram.y 14888).
    /// Declared before `LikeOpI` would be (no `~~*` longer prefix).
    #[parse(infix, lbp = 5, rbp = 6)]
    LikeOpI(Box<Self>, #[tok(TILDETILDESTAR, this)] Box<Self>),
    /// `expr !~~ pattern` — operator-form `NOT LIKE` (gram.y 14874).
    #[parse(infix, lbp = 5, rbp = 6)]
    LikeOpNeg(Box<Self>, #[tok(BANGTILDETILDE, this)] Box<Self>),
    /// `expr ~~ pattern` — operator-form `LIKE` (gram.y 14860).
    #[parse(infix, lbp = 5, rbp = 6)]
    LikeOp(Box<Self>, #[tok(TILDETILDE, this)] Box<Self>),
    /// `(start, end) OVERLAPS (start, end)` — SQL time-period overlap test.
    /// Each operand is an ordinary parenthesized expression to the parser.
    #[parse(infix, lbp = 5, rbp = 6)]
    Overlaps(Box<Self>, #[tok(OVERLAPS, this)] Box<Self>),
    /// Record comparison operators: `expr *= expr`, `*<>`, `*<`, `*<=`,
    /// `*>`, `*>=` — compare ROW/composite values field by field.
    #[parse(infix, lbp = 5, rbp = 6)]
    RecordLte(Box<Self>, #[tok(STARLTE, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    RecordGte(Box<Self>, #[tok(STARGTE, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    RecordNeq(Box<Self>, #[tok(STARNEQ, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    RecordLt(Box<Self>, #[tok(STARLT, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    RecordGt(Box<Self>, #[tok(STARGT, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    RecordEq(Box<Self>, #[tok(STAREQ, this)] Box<Self>),
    /// IN list: `expr IN (val, ...)`
    #[parse(postfix, bp = 6)]
    InExpr(Box<Self>, #[tok(IN, this)] InList<'input>),
    /// `expr NOT BETWEEN low AND high`. Declared before `BetweenExpr` so
    /// the longer `NOT BETWEEN` prefix wins disambiguation. Recursive fields
    /// in this postfix tail inherit `bp = 6`, so the low/high operands stop
    /// before the literal `AND` infix at `bp = 2`.
    #[parse(postfix, bp = 6)]
    NotBetweenExpr(
        Box<Self>,
        #[tok(NOT, BETWEEN, this)] Box<Self>,
        #[tok(AND, this)] Box<Self>,
    ),
    /// `expr BETWEEN low AND high`. See `NotBetweenExpr` for the recursive
    /// postfix-tail binding-power rationale.
    #[parse(postfix, bp = 6)]
    BetweenExpr(
        Box<Self>,
        #[tok(BETWEEN, this)] Box<Self>,
        #[tok(AND, this)] Box<Self>,
    ),

    // --- Infix ---
    // Multi-char operators before single-char to avoid partial matching.
    //
    // JSON / JSONB operators are listed FIRST among infix so that their
    // longer tokens are peeked before conflicting shorter ones
    // (e.g. `<@` before `<`, `->` before `-`). These dedicated operators use
    // bp = 10; generic `Op` spellings such as `||` use the lower bp = 8 tier.
    /// JSON path as text: `expr #>> path`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonPathText(Box<Self>, #[tok(HASHARROWARROW, this)] Box<Self>),
    /// JSON path: `expr #> path`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonPath(Box<Self>, #[tok(HASHARROW, this)] Box<Self>),
    /// JSON field as text: `expr ->> field`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonFieldText(Box<Self>, #[tok(ARROWARROW, this)] Box<Self>),
    /// JSON field: `expr -> field`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonField(Box<Self>, #[tok(ARROW, this)] Box<Self>),
    /// Geometric parallel: `a ?|| b`. Must precede `JsonAnyKey` (`?|`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, lbp = 5, rbp = 6)]
    Parallel(Box<Self>, #[tok(QUESTIONPIPEPIPE, this)] Box<Self>),
    /// JSON any-key-exists: `expr ?| keys`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonAnyKey(Box<Self>, #[tok(QUESTIONPIPE, this)] Box<Self>),
    /// JSON all-keys-exist: `expr ?& keys`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonAllKeys(Box<Self>, #[tok(QUESTIONAMP, this)] Box<Self>),
    /// Geometric intersect: `a ?# b`. Must precede `JsonKey` (`?`).
    #[parse(infix, lbp = 5, rbp = 6)]
    Intersect(Box<Self>, #[tok(QUESTIONHASH, this)] Box<Self>),
    /// Geometric perpendicular: `a ?-| b`. Must precede `Horizontal` (`?-`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, lbp = 5, rbp = 6)]
    Perpendicular(Box<Self>, #[tok(QUESTIONDASHPIPE, this)] Box<Self>),
    /// Geometric horizontal: `a ?- b`. Must precede `JsonKey` (`?`).
    #[parse(infix, lbp = 5, rbp = 6)]
    Horizontal(Box<Self>, #[tok(QUESTIONDASH, this)] Box<Self>),
    /// Geometric "is horizontal" prefix: `?- s` — tests whether the
    /// LSEG/LINE `s` is horizontal. PG's geometry.sql uses this in WHERE.
    #[parse(prefix, bp = 12)]
    IsHorizontal(#[tok(QUESTIONDASH, this)] Box<Self>),
    /// Geometric "is vertical" prefix: `?| s`.
    #[parse(prefix, bp = 12)]
    IsVertical(#[tok(QUESTIONPIPE, this)] Box<Self>),
    /// Geometric "below": `a <^ b`.
    #[parse(infix, lbp = 5, rbp = 6)]
    Below(Box<Self>, #[tok(LTCARET, this)] Box<Self>),
    /// Geometric "above": `a >^ b`.
    #[parse(infix, lbp = 5, rbp = 6)]
    Above(Box<Self>, #[tok(GTCARET, this)] Box<Self>),
    /// JSON key-exists: `expr ? key`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonKey(Box<Self>, #[tok(QUESTION, this)] Box<Self>),
    /// JSONB contains: `expr @> expr`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonContains(Box<Self>, #[tok(ATGT, this)] Box<Self>),
    /// JSONB contained-by: `expr <@ expr`
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonContainedBy(Box<Self>, #[tok(LTAT, this)] Box<Self>),

    // --- Postgres text-search / jsonpath / range / geometric 3-char operators ---
    //
    // These must come BEFORE any variant whose infix token is a 2-char prefix
    // (e.g. `<<|` before `<<`, `&<|` before `&<`, `?#` before JsonKey `?`).
    // The scanner is longest-match at the token level, but Pratt operator
    // dispatch chooses variants in declaration order — so a shorter-prefix
    // variant declared first would swallow the `&<` / `<<` / `?` and leave
    // the trailing `|` / `#` dangling.
    /// Text-search / jsonb path match: `expr @@@ expr`.
    #[parse(infix, lbp = 5, rbp = 6)]
    TsMatch3(Box<Self>, #[tok(ATATAT, this)] Box<Self>),
    /// User-defined triple-less-than: `a <<< b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, lbp = 5, rbp = 6)]
    TripleLt(Box<Self>, #[tok(LTLTLT, this)] Box<Self>),
    /// Geometric strictly-below: `a <<| b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, lbp = 5, rbp = 6)]
    StrictlyBelow(Box<Self>, #[tok(LTLTPIPE, this)] Box<Self>),
    /// Inet is-subset-or-equal: `a <<= b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, lbp = 5, rbp = 6)]
    SubsetEq(Box<Self>, #[tok(LTLTEQ, this)] Box<Self>),
    /// Distance: `a <-> b`. Before any `<` variant.
    #[parse(infix, lbp = 10, rbp = 11)]
    Distance(Box<Self>, #[tok(LTMINUSGT, this)] Box<Self>),
    /// User-defined triple-greater-than: `a >>> b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, lbp = 5, rbp = 6)]
    TripleGt(Box<Self>, #[tok(GTGTGT, this)] Box<Self>),
    /// Inet is-superset-or-equal: `a >>= b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, lbp = 5, rbp = 6)]
    SupersetEq(Box<Self>, #[tok(GTGTEQ, this)] Box<Self>),
    /// Range adjacent: `a -|- b`. Before `Sub` (`-`).
    #[parse(infix, lbp = 5, rbp = 6)]
    Adjacent(Box<Self>, #[tok(MINUSPIPEMINUS, this)] Box<Self>),
    /// Geometric strictly-above: `a |>> b`. Before `Concat` (`||`).
    #[parse(infix, lbp = 5, rbp = 6)]
    StrictlyAbove(Box<Self>, #[tok(PIPEGTGT, this)] Box<Self>),
    /// Geometric no-extend-below: `a |&> b`. Before `Concat` (`||`).
    #[parse(infix, lbp = 5, rbp = 6)]
    NoExtendBelow(Box<Self>, #[tok(PIPEAMPGT, this)] Box<Self>),
    /// Geometric no-extend-above: `a &<| b`. Before `NoExtendRight` (`&<`).
    #[parse(infix, lbp = 5, rbp = 6)]
    NoExtendAbove(Box<Self>, #[tok(AMPLTPIPE, this)] Box<Self>),

    // --- 2-char operators ---
    /// Text-search / jsonb path match: `expr @@ expr`.
    #[parse(infix, lbp = 5, rbp = 6)]
    TsMatch(Box<Self>, #[tok(ATAT, this)] Box<Self>),
    /// Jsonpath exists: `expr @? path`.
    #[parse(infix, lbp = 5, rbp = 6)]
    JsonPathExists(Box<Self>, #[tok(ATQUESTION, this)] Box<Self>),
    /// Range / array overlap: `a && b`.
    #[parse(infix, lbp = 10, rbp = 11)]
    Overlap(Box<Self>, #[tok(AMPAMP, this)] Box<Self>),
    /// Range does-not-extend-right: `a &< b`.
    #[parse(infix, lbp = 5, rbp = 6)]
    NoExtendRight(Box<Self>, #[tok(AMPLT, this)] Box<Self>),
    /// Range does-not-extend-left: `a &> b`.
    #[parse(infix, lbp = 5, rbp = 6)]
    NoExtendLeft(Box<Self>, #[tok(AMPGT, this)] Box<Self>),
    /// Range strictly-left-of: `a << b`.
    #[parse(infix, lbp = 5, rbp = 6)]
    StrictlyLeft(Box<Self>, #[tok(LTLT, this)] Box<Self>),
    /// Range strictly-right-of: `a >> b`.
    #[parse(infix, lbp = 5, rbp = 6)]
    StrictlyRight(Box<Self>, #[tok(GTGT, this)] Box<Self>),

    // --- User-defined / custom infix operators ---
    /// `expr === expr` — user-defined triple-equal. Must come before `Eq` (`=`).
    #[parse(infix, lbp = 5, rbp = 6)]
    TripleEq(Box<Self>, #[tok(TRIPLEEQ, this)] Box<Self>),
    /// `expr !== expr` — user-defined not-equal. Must come before `BangEq` (`!=`).
    #[parse(infix, lbp = 5, rbp = 6)]
    BangEqEq(Box<Self>, #[tok(BANGEQEQ, this)] Box<Self>),
    /// `expr ## expr` — geometric closest-point / path intersection.
    /// Must come before `BitXor` (`#`).
    #[parse(infix, lbp = 5, rbp = 6)]
    GeomClosest(Box<Self>, #[tok(HASHHASH, this)] Box<Self>),

    #[parse(infix, lbp = 1, rbp = 2)]
    Or(Box<Self>, #[tok(OR, this)] Box<Self>),
    #[parse(infix, lbp = 2, rbp = 3)]
    And(Box<Self>, #[tok(AND, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    BangEq(Box<Self>, #[tok(BANGEQ, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    Neq(Box<Self>, #[tok(NEQ, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    Lte(Box<Self>, #[tok(LTE, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    Gte(Box<Self>, #[tok(GTE, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    Eq(Box<Self>, #[tok(EQ, this)] Box<Self>),

    /// Text starts-with: `expr ^@ expr` (PostgreSQL `starts_with` operator).
    /// `^@` is a single token (see `punct::CaretAt`); declared before
    /// `CustomInfix` so it wins the declaration-order tiebreak. bp=8 matches
    /// Postgres's generic `Op` precedence.
    #[parse(infix, lbp = 8, rbp = 9)]
    StartsWith(Box<Self>, #[tok(CARETAT, this)] Box<Self>),
    /// JSONB delete-path: `expr #- path` (PostgreSQL jsonb delete-at-path
    /// operator). `#-` is a single token (see `punct::HashMinus`); declared
    /// before `CustomInfix` so it wins the declaration-order tiebreak. bp=10
    /// matches the neighbouring `#>`/`#>>` JSON path operators.
    #[parse(infix, lbp = 10, rbp = 11)]
    JsonDeletePath(Box<Self>, #[tok(HASHMINUS, this)] Box<Self>),

    /// Catch-all infix: any user-defined operator not matched by a specific
    /// token above. Declared BEFORE single-char operators so 2+ char custom
    /// operators like `<%` or `~>` aren't consumed as the single-char prefix
    /// (`<`, `~`) plus garbage. Since `CustomOp` requires 2+ characters, bare
    /// single-char operators still fall through to the variants below.
    /// bp=8 matches Postgres's generic `Op` precedence (between comparison
    /// bp=5 and additive bp=10).
    #[parse(infix, lbp = 8, rbp = 9)]
    CustomInfix(
        Box<Self>,
        #[pretty(break_before = soft, break_after = soft)] literal::CustomOp<'input>,
        Box<Self>,
    ),

    #[parse(infix, lbp = 5, rbp = 6)]
    Lt(Box<Self>, #[tok(LT, this)] Box<Self>),
    #[parse(infix, lbp = 5, rbp = 6)]
    Gt(Box<Self>, #[tok(GT, this)] Box<Self>),
    /// String concatenation: `expr || expr`. PostgreSQL scans `||` as a
    /// generic `Op`, below additive operators in the precedence hierarchy.
    #[parse(infix, lbp = 8, rbp = 9)]
    Concat(Box<Self>, #[tok(CONCAT, this)] Box<Self>),
    /// Bitwise OR: `expr | expr`. Must come after `Concat` (`||`) so the
    /// longer token matches first at the punctuation level.
    #[parse(infix, lbp = 10, rbp = 11)]
    BitOr(Box<Self>, #[tok(PIPE, this)] Box<Self>),
    /// Bitwise AND: `expr & expr`.
    #[parse(infix, lbp = 10, rbp = 11)]
    BitAnd(Box<Self>, #[tok(AMP, this)] Box<Self>),
    /// Bitwise XOR: `expr # expr` (Postgres bit-string / integer operator).
    #[parse(infix, lbp = 10, rbp = 11)]
    BitXor(Box<Self>, #[tok(POUND, this)] Box<Self>),
    #[parse(infix, lbp = 10, rbp = 11)]
    Add(Box<Self>, #[tok(PLUS, this)] Box<Self>),
    #[parse(infix, lbp = 10, rbp = 11)]
    Sub(Box<Self>, #[tok(MINUS, this)] Box<Self>),
    /// Multiplication: `expr * expr`
    #[parse(infix, lbp = 11, rbp = 12)]
    Mul(Box<Self>, #[tok(STAR, this)] Box<Self>),
    /// Division: `expr / expr`
    #[parse(infix, lbp = 11, rbp = 12)]
    Div(Box<Self>, #[tok(SLASH, this)] Box<Self>),
    /// Modulo: `expr % expr`
    #[parse(infix, lbp = 11, rbp = 12)]
    Mod(Box<Self>, #[tok(PERCENT, this)] Box<Self>),
    /// Exponentiation: `expr ^ expr` (Postgres numeric power operator).
    #[parse(infix, lbp = 13, rbp = 14)]
    Pow(Box<Self>, #[tok(CARET, this)] Box<Self>),

    // --- Atoms ---
    /// EXISTS subquery: `EXISTS (SELECT ...)`
    Exists(ExistsExpr<'input>),
    /// ARRAY constructor: `ARRAY[...]` or `ARRAY(...)`
    Array(ArrayExpr<'input>),
    /// ROW constructor: `ROW(...)`
    RowExpr(RowExpr<'input>),
    /// CASE expression: `CASE [expr] WHEN ... THEN ... [ELSE ...] END`
    Case(CaseExpr<'input>),
    /// Unicode string literal: `U&'...'` with optional `UESCAPE 'c'`. Must
    /// come before `CastFunc` and `StringLit` for the same reason as
    /// `EscapeStringLit`.
    UnicodeStringLit(UnicodeStringLitWithEscape<'input>),
    /// Escape string literal: `E'foo\n'`. Must come before `CastFunc` and
    /// `StringLit` — `CastFunc` is `TypeName StringLit` and would match `e`
    /// as a type name followed by the string literal.
    EscapeStringLit(
        #[lex(pattern = r"(?i:E)'(?:[^'\\]|\\.|'')*'")] literal::EscapeStringLit<'input>,
    ),
    /// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`.
    TimestampLit(TimestampLit<'input>),
    /// `TIME [WITH|WITHOUT TIME ZONE] 'string'`.
    TimeLit(TimeLit<'input>),
    /// `INTERVAL 'string' [qualifier]`. Must come before `CastFunc` since
    /// `interval` would otherwise parse as an ident-based TypeName.
    IntervalLit(IntervalLit<'input>),
    /// Function-style type cast: `bool 't'` -- must come before ColumnRef
    /// since type keywords like `bool` overlap with identifiers
    CastFunc(TypeCastFunc<'input>),
    /// `xmlelement(NAME ident [, xmlattributes(...)] [, content])`. Must come
    /// before `Func` so `xmlelement(` is matched as the special form.
    XmlElement(Box<XmlElement<'input>>),
    /// `xmlforest(expr [AS alias], ...)`. Before `Func` for the same reason.
    XmlForest(XmlForest<'input>),
    /// `xmlattributes(expr [AS alias], ...)`. Before `Func`.
    XmlAttributes(XmlAttributes<'input>),
    /// `xmlpi(NAME ident [, content])`. Before `Func`.
    XmlPi(XmlPi<'input>),
    /// `XMLSERIALIZE({DOCUMENT|CONTENT} expr AS type [[NO] INDENT])`. Before `Func`.
    XmlSerialize(Box<XmlSerialize<'input>>),
    /// `XMLPARSE({DOCUMENT|CONTENT} expr)`. Before `Func`.
    XmlParse(Box<XmlParse<'input>>),
    /// `XMLROOT(xml, VERSION ... [, STANDALONE ...])`. Before `Func`.
    XmlRoot(Box<XmlRoot<'input>>),
    /// `XMLEXISTS(xpath PASSING ... doc ...)`. Before `Func`.
    XmlExists(Box<XmlExists<'input>>),
    /// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`. Before `Func`
    /// since `trim` is also a valid function-call identifier.
    Trim(TrimCall<'input>),
    /// `CAST(expr AS type [COLLATE "c"])`. Before `Func`.
    CastCall(CastCall<'input>),
    /// `COLLATION FOR (expr)`. Before `Func`.
    CollationFor(CollationForCall<'input>),
    /// `SUBSTRING(source FROM ... | SIMILAR ...)`. Before `Func`.
    Substring(SubstringCall<'input>),
    /// `POSITION(needle IN haystack)`. Before `Func`.
    Position(PositionCall<'input>),
    /// `OVERLAY(source PLACING new FROM start [FOR len])`. Before `Func`.
    Overlay(OverlayCall<'input>),
    /// `EXTRACT(field FROM source)`. Before `Func`.
    Extract(ExtractCall<'input>),
    /// `JSON(...)` SQL/JSON value constructor. Before `Func`.
    JsonCtor(Box<JsonConstructor<'input>>),
    /// `JSON_SCALAR(...)`. Before `Func`.
    JsonScalar(Box<JsonScalar<'input>>),
    /// `JSON_SERIALIZE(...)`. Before `Func`.
    JsonSerialize(Box<JsonSerialize<'input>>),
    /// `JSON_OBJECT(...)` SQL/JSON object constructor. Before `Func`.
    JsonObject(Box<JsonObject<'input>>),
    /// `JSON_ARRAY(...)` SQL/JSON array constructor. Before `Func`.
    JsonArray(Box<JsonArray<'input>>),
    /// `JSON_EXISTS(...)` SQL/JSON path predicate. Before `Func`.
    JsonExists(Box<JsonExists<'input>>),
    /// `JSON_VALUE(...)` SQL/JSON scalar extraction. Before `Func`.
    JsonValue(Box<JsonValue<'input>>),
    /// `JSON_QUERY(...)` SQL/JSON value extraction. Before `Func`.
    JsonQuery(Box<JsonQuery<'input>>),
    /// `JSON_OBJECTAGG(...)` SQL/JSON object aggregate. Before `Func`.
    JsonObjectAgg(Box<JsonObjectAgg<'input>>),
    /// `JSON_ARRAYAGG(...)` SQL/JSON array aggregate. Before `Func`.
    JsonArrayAgg(Box<JsonArrayAgg<'input>>),
    /// Function call: `func(args)` -- must come before ColumnRef
    Func(Box<FuncCall<'input>>),
    #[tok(USER)]
    /// `USER` — the reserved-keyword spelling of `CURRENT_USER` as a
    /// zero-arg function reference. PG's gram.y `func_expr_common_subexpr`
    /// includes `USER { … }` as a synonym for `CURRENT_USER`. pg-sql keeps
    /// `USER` reserved at the token level (for the `CREATE USER ...`
    /// statement disambiguation), so it cannot lex as an `UnquotedIdent`
    /// the way `current_date`/`session_user` do — model it as its own
    /// atom. Declared before `ColumnRef` for clarity (ColumnRef cannot
    /// match a reserved keyword anyway).
    User,
    /// Qualified wildcard: `table.*` -- must come before QualRef and ColumnRef
    QualWild(QualifiedWildcard<'input>),
    /// Qualified column reference: `table.column` -- must come before ColumnRef
    QualRef(QualifiedRef<'input>),
    /// Parenthesized scalar, row, or subquery, with optional field
    /// indirection. Its singleton expression route supplies Pretty's authored
    /// precedence grouping syntax.
    Parenthesized(ParenthesizedExpr<'input>),
    /// Numeric literal: `77.7` -- must come before IntegerLit for longest match
    NumericLit(literal::NumericLit<'input>),
    /// Integer literal: `42`
    IntegerLit(literal::IntegerLit<'input>),
    /// Dollar-quoted string literal: `$$...$$` or `$tag$...$tag$`.
    /// Listed before `StringLit` since it has a distinct prefix (`$`).
    DollarStringLit(literal::DollarStringLit<'input>),
    /// Bit-string literal: `B'10'`. Must come before `StringLit` (and before
    /// any plain `Ident` / `ColumnRef`) for the same reason as
    /// `EscapeStringLit`: the lexer's longest-match-wins picks
    /// `BitStringLit` over `Ident`+`StringLit` only when the prefixed token
    /// is also declared first at the atom level. Without this ordering, the
    /// formatter would round-trip `B'10'` as `B '10'` (inserted space).
    BitStringLit(#[lex(pattern = r"(?i:B)'[^']*'")] literal::BitStringLit<'input>),
    /// Hex-string literal: `X'1FF'`. Same ordering rationale as
    /// `BitStringLit` — must precede `StringLit` and any plain `Ident`.
    HexStringLit(#[lex(pattern = r"(?i:X)'[^']*'")] literal::HexStringLit<'input>),
    /// String literal sequence: `'hello'` or `'first' 'second' ...` —
    /// Postgres concatenates adjacent string literals into one.
    StringLit(StringLitSeq0<'input>),
    #[tok(TRUE)]
    /// Boolean true
    BoolTrue,
    #[tok(FALSE)]
    /// Boolean false
    BoolFalse,
    #[tok(NULL)]
    /// NULL
    Null,
    #[tok(DEFAULT)]
    /// `DEFAULT` — placeholder usable in INSERT/UPDATE value positions.
    Default,
    /// Positional parameter reference: `$1`, `$2`, etc. Used in function bodies
    /// and prepared statements.
    PositionalParam(#[lex(matcher)] literal::DollarNum<'input>),
    /// Unqualified column reference: `f1` or `"Foo"`
    ColumnRef(crate::tokens::ColId<'input>),
    /// psql client variable substitution: `:foo`, `:'foo'`, `:"foo"`.
    PsqlVar(PsqlVariableExpr<'input>),
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/shared/expr.tests.rs"
));
