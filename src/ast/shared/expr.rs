/// SQL expression AST with derived Pratt parsing for operator precedence.
///
/// Handles atoms, prefix (NOT, unary minus), infix (AND, OR, comparisons,
/// arithmetic), and postfix operators (::type cast, IS [NOT] TRUE/FALSE/UNKNOWN/NULL,
/// IN (list)).
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
/// The query inside a parenthesized subquery position: gram.y
/// `select_with_parens: '(' select_no_parens ')'` without the parentheses
/// the position supplies, so the `with_clause`, the set-operation chain and
/// the ORDER BY / LIMIT / FOR UPDATE tail of `select_no_parens` all live
/// here. It differs from [`Subquery`] in one way: a parenthesized left
/// operand must be followed by a set operation
/// ([`DirectSelectClause::ParenthesizedSet`]); requiring the continuation
/// keeps a plain `(SELECT ...)` expression on the ordinary
/// parenthesized-expression path.
#[derive(recursa::Node, Debug, Clone)]
pub struct DirectSubquery<'input> {
    pub with: Option<crate::ast::shared::with_clause::WithClause<'input>>,
    pub clause: DirectSelectClause<'input>,
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub limit_offset: Option<Box<crate::ast::dml::select::LimitOffsetClause<'input>>>,
    pub for_update: Option<Box<crate::ast::dml::select::ForUpdateClause<'input>>>,
}

/// `select_clause` inside a parenthesized subquery position, see
/// [`DirectSubquery`].
#[derive(recursa::Node, Debug, Clone)]
pub enum DirectSelectClause<'input> {
    ParenthesizedSet(DirectParenthesizedSet<'input>),
    Table(TableStmt<'input>),
    Body(crate::ast::dml::values::CompoundBody<'input>),
}

/// A query whose left operand is parenthesized and whose set-operation
/// continuation is required, such as `(SELECT 1) UNION SELECT 2`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DirectParenthesizedSet<'input> {
    pub open: ParenthesizedOpen,
    pub left: Box<Subquery<'input>>,
    pub close: ParenthesizedClose,
    pub set_op: SetOpCombiner<'input>,
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
/// gram.y spells this `columnref: ColId indirection`, so the qualifier is a
/// `ColId`: unreserved and column-name keywords (`EXCLUDED`, `NEW`, `OLD`, ...)
/// but never a reserved one. The attribute after the dot stays `AliasName`,
/// matching gram.y's `attr_name: ColLabel`.
///
/// The qualifier's admission set is load-bearing beyond this node. `Expr` is
/// the head of every SELECT target list, so FIRST(`Expr`) is the selector that
/// decides whether a `SelectStmt` has a target list at all. A qualifier that
/// admitted every keyword would put `UNION`, `EXCEPT`, `INTERSECT` and every
/// other reserved word into that selector and make a targetless `SELECT`
/// unusable as a set-operation operand (issue #55).
#[derive(recursa::Node, Debug, Clone)]
pub struct QualifiedRef<'input> {
    pub table: crate::tokens::ColId<'input>,
    #[tok(DOT, this)]
    pub column: literal::AliasName<'input>,
    /// The rest of gram.y `columnref: ColId indirection`: subscripts, each
    /// with the field selectors that may follow it.
    /// Greedy: a leading LBRACKET starts this element instead of ending `QualifiedRef` (bison shift preference).
    #[greedy(LBRACKET)]
    pub subscripts: Vec<SubscriptIndirection<'input>>,
}

/// gram.y `columnref: ColId | ColId indirection` for an unqualified name:
/// the subscripts (and the field selectors after each) belong to the
/// column reference. Subscripts are not a postfix operator of every
/// expression: gram.y attaches `indirection` only to `columnref`,
/// `PARAM`, `'(' a_expr ')'` and `select_with_parens`, so `x::int[1]` is a
/// cast to an array type and `f(x)[1]` is rejected, as PostgreSQL has it.
#[derive(recursa::Node, Debug, Clone)]
pub struct ColumnRef<'input> {
    pub name: crate::tokens::ColId<'input>,
    /// Greedy: a leading LBRACKET starts this element instead of ending `ColumnRef` (bison shift preference).
    #[greedy(LBRACKET)]
    pub subscripts: Vec<SubscriptIndirection<'input>>,
}

/// gram.y `PARAM opt_indirection`.
#[derive(recursa::Node, Debug, Clone)]
pub struct PositionalParam<'input> {
    #[lex(matcher)]
    pub param: literal::DollarNum<'input>,
    /// Greedy: a leading LBRACKET starts this element instead of ending `PositionalParam` (bison shift preference).
    #[greedy(LBRACKET)]
    pub subscripts: Vec<SubscriptIndirection<'input>>,
}

/// Qualified wildcard: `table.*`
#[derive(recursa::Node, Debug, Clone)]
pub struct QualifiedWildcard<'input> {
    #[tok(this, DOT, STAR)]
    pub table: crate::tokens::ColId<'input>,
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
    /// Greedy: a leading GROUPS, RANGE, ROWS starts this element instead of ending `WindowPartitionBy` (bison shift preference).
    /// `ORDER` is reserved, so a `ColId`-qualified `QualifiedRef` cannot begin
    /// an expression with it and the overlap no longer contains it.
    /// gram.y `opt_partition_clause: PARTITION BY expr_list`: one or more.
    #[sep(COMMA)]
    pub exprs: recursa::Vec1<Expr<'input>>,
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

/// Function name in call position: gram.y `func_name`.
///
/// PostgreSQL's `func_name` admits a `type_function_name` directly, while a
/// dotted name begins with `ColId`. Keeping those two admission sets here is
/// important: `QualifiedName` is intentionally broader and would also admit
/// every `COL_NAME` keyword, making the dedicated XML/JSON expression forms
/// indistinguishable from an ordinary function call.
///
/// This stays its own node: inlining the two names into the call variants
/// (each carrying the call tail) makes recursa's predictive dispatch for the
/// call walk into the first argument, where an operator has no edge, so
/// `f(a + b)` fails before `Expr` runs. The table-driven lowering pays for
/// the separate node with `$end` in its FOLLOW set (recursa #125).
///
/// Variant ordering: `Qualified` needs a dotted tail, `Name` a single
/// `type_function_name`; they share their first token and part on the dot.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncCallName<'input> {
    Qualified(FuncCallQualifiedName<'input>),
    Name(crate::tokens::type_function_name<'input>),
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

/// A dotted function name — the `ColId indirection` arm of gram.y's
/// `func_name`. At least one dotted tail is required so this does not
/// overlap the unqualified `type_function_name` arm of [`FuncCallName`].
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

/// gram.y `func_expr: func_application within_group_clause filter_clause
/// over_clause`: the argument list and every optional suffix in one shape.
/// One shape rather than a "plain" and a "within group" tail: the two ended
/// the argument list on the same `)` and could only be told apart after it,
/// which a one-token parser cannot do.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionCallSuffix<'input> {
    pub open: FunctionCallOpen,
    pub body: Option<FunctionCallBody<'input>>,
    pub close: FunctionCallClose,
    pub within_group: Option<WithinGroupClause<'input>>,
    pub filter: Option<FilterClause<'input>>,
    pub window: Option<WindowSpec<'input>>,
}

/// gram.y `AexprConst: func_name '(' func_arg_list opt_sort_clause ')'
/// Sconst`: a typed literal spelled like a call, `char(20) 'x'`. The
/// argument list is `func_arg_list opt_sort_clause` and nothing more: `*`,
/// `DISTINCT`, `ALL` and `VARIADIC` belong to `func_application` and are
/// syntax errors here. A named argument or the sort clause is grammatical
/// and gram.y rejects it in the rule's action ("type modifier cannot have
/// parameter name" / "... ORDER BY"). It parts from [`FunctionCallSuffix`]
/// on the string after `)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FunctionTypedLiteralTail<'input> {
    pub open: FunctionCallOpen,
    /// gram.y `func_arg_list`.
    pub args: FunctionArgumentSequence<'input>,
    /// gram.y `opt_sort_clause`.
    pub order_by: Option<Box<crate::ast::dml::select::OrderByClause<'input>>>,
    pub close: FunctionCallClose,
    pub value: TypeCastValue<'input>,
}

/// What follows a function name in call position.
///
/// Variant ordering: both start with `(`; `TypedLiteral` is decided by the
/// string after the closing parenthesis.
#[derive(recursa::Node, Debug, Clone)]
pub enum FunctionCallTail<'input> {
    TypedLiteral(FunctionTypedLiteralTail<'input>),
    Call(FunctionCallSuffix<'input>),
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
    /// gram.y `c_expr: select_with_parens %prec UMINUS`: with a query on the
    /// stack the parser reduces rather than shifting the query's own
    /// `ORDER BY` / `LIMIT` / `FETCH` / `FOR` tail.
    #[parse(prec = UMINUS)]
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
    /// Greedy: a leading DOT, LBRACKET starts this element instead of ending `ParenthesizedExpr` (bison shift preference).
    #[greedy(DOT, LBRACKET)]
    pub indirection: Vec<ParenthesizedIndirection<'input>>,
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
/// lower-unbounded slice form with an upper bound (`[:2]`) one expression
/// representation, avoiding an exact grammar overlap between a slice colon
/// and a psql-variable colon; the bare `[:]` is
/// [`BracketSubscriptValue::Unbounded`]. The value is required: a colon on
/// its own is not an expression, so nothing decides between "the value
/// follows" and "the expression has ended".
#[derive(recursa::Node, Debug, Clone)]
pub struct PsqlVariableExpr<'input> {
    #[tok(COLON, this)]
    pub value: PsqlVariableExprValue<'input>,
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

/// Content between subscript brackets — gram.y `indirection_el`'s
/// `'[' a_expr ']'` and `'[' opt_slice_bound ':' opt_slice_bound ']'`.
///
/// Variant ordering: `Unbounded` (`[:]`) first; a colon followed by a value
/// is a `PsqlVariableExpr` lower bound and takes the `Bounded` path.
#[derive(recursa::Node, Debug, Clone)]
pub enum BracketSubscriptValue<'input> {
    /// `[:]` — both slice bounds absent.
    #[tok(COLON)]
    Unbounded,
    Bounded(BracketSubscriptBounds<'input>),
}

/// `lower [: [upper]]` inside subscript brackets.
#[derive(recursa::Node, Debug, Clone)]
pub struct BracketSubscriptBounds<'input> {
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

/// A subscript followed by the field selectors that continue PostgreSQL's
/// `opt_indirection` chain: `arr[i]`, `arr[i].field`, `arr[i].f.g`.
///
/// Further subscripts are not repeated here — the owning atom keeps a list
/// of these, so `a[1].b[2]` is a subscript carrying `.b` followed by a
/// second subscript. Keeping brackets out of this tail leaves the two forms
/// with disjoint continuations.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubscriptIndirection<'input> {
    pub subscript: BracketSubscript<'input>,
    /// Greedy: a leading DOT starts this element instead of ending
    /// `SubscriptIndirection` (bison shift preference). PostgreSQL's
    /// `indirection_el` is left-recursive on the subscripted value, so a `.`
    /// after a subscript is always the next chain element — no other
    /// production resumes on `.` at that point.
    #[greedy(DOT)]
    pub fields: Vec<IndirectionField<'input>>,
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

// Operators of PostgreSQL's `subquery_Op` production, one enum per
// precedence level of pg-sql's `Expr`: gram.y decides the shift before a
// quantified comparison by the operator token's own precedence, so each
// `Expr::QuantifiedComparison*` variant carries the level of the same
// token's infix variant. Together the six enums cover `OperatorName`,
// `OPERATOR(...)` and the LIKE family exactly once.

/// `subquery_Op` at the level of pg-sql's comparison operators (binding
/// power 5): gram.y `MathOp`'s `< > = <= >= <>` and the operators pg-sql's
/// `Expr` parses at that level.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedCmpOperator {
    #[tok(STARLTE)]
    StarLte,
    #[tok(STARGTE)]
    StarGte,
    #[tok(STARNEQ)]
    StarNeq,
    #[tok(STARLT)]
    StarLt,
    #[tok(STARGT)]
    StarGt,
    #[tok(STAREQ)]
    StarEq,
    #[tok(TRIPLEEQ)]
    TripleEq,
    #[tok(BANGEQEQ)]
    BangEqEq,
    #[tok(BANGEQ)]
    BangEq,
    #[tok(LTLTLT)]
    LtLtLt,
    #[tok(LTLTEQ)]
    LtLtEq,
    #[tok(LTLTPIPE)]
    LtLtPipe,
    #[tok(LTLT)]
    LtLt,
    #[tok(LTCARET)]
    LtCaret,
    #[tok(GTGTGT)]
    GtGtGt,
    #[tok(GTGTEQ)]
    GtGtEq,
    #[tok(GTGT)]
    GtGt,
    #[tok(GTCARET)]
    GtCaret,
    #[tok(HASHHASH)]
    HashHash,
    #[tok(MINUSPIPEMINUS)]
    MinusPipeMinus,
    #[tok(PIPEGTGT)]
    PipeGtGt,
    #[tok(PIPEAMPGT)]
    PipeAmpGt,
    #[tok(QUESTIONPIPEPIPE)]
    QuestionPipePipe,
    #[tok(QUESTIONDASHPIPE)]
    QuestionDashPipe,
    #[tok(QUESTIONHASH)]
    QuestionHash,
    #[tok(QUESTIONDASH)]
    QuestionDash,
    #[tok(ATATAT)]
    AtAtAt,
    #[tok(ATAT)]
    AtAt,
    #[tok(ATQUESTION)]
    AtQuestion,
    #[tok(AMPLTPIPE)]
    AmpLtPipe,
    #[tok(AMPLT)]
    AmpLt,
    #[tok(AMPGT)]
    AmpGt,
    #[tok(TILDELEQTILDE)]
    TildeLeqTilde,
    #[tok(TILDEGEQTILDE)]
    TildeGeqTilde,
    #[tok(TILDELTTILDE)]
    TildeLtTilde,
    #[tok(TILDEGTTILDE)]
    TildeGtTilde,
    #[tok(BANGTILDETILDESTAR)]
    BangTildeTildeStar,
    #[tok(TILDETILDESTAR)]
    TildeTildeStar,
    #[tok(BANGTILDETILDE)]
    BangTildeTilde,
    #[tok(TILDETILDE)]
    TildeTilde,
    #[tok(BANGTILDESTAR)]
    BangTildeStar,
    #[tok(TILDESTAR)]
    TildeStar,
    #[tok(BANGTILDE)]
    BangTilde,
    #[tok(TILDEEQ)]
    TildeEq,
    #[tok(LTE)]
    Lte,
    #[tok(GTE)]
    Gte,
    #[tok(NEQ)]
    Neq,
    #[tok(LT)]
    Lt,
    #[tok(GT)]
    Gt,
    #[tok(EQ)]
    Eq,
    #[tok(TILDE)]
    Tilde,
}

/// `subquery_Op`'s `LIKE | NOT_LA LIKE | ILIKE | NOT_LA ILIKE`, at the level
/// of `Expr::Like` (binding power 60).
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedLikeOperator {
    #[tok(NOT, LIKE)]
    NotLike,
    #[tok(NOT, ILIKE)]
    NotIlike,
    #[tok(LIKE)]
    Like,
    #[tok(ILIKE)]
    Ilike,
}

/// `subquery_Op` at gram.y's generic `Op` level (binding power 80): `||`,
/// `^@`, every spelling that is only a prefix operator elsewhere in `Expr`,
/// the multi-character custom operators, and `OPERATOR(any_operator)`
/// (`%left Op OPERATOR`).
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedOpOperator<'input> {
    #[tok(CONCAT)]
    Concat,
    #[tok(CARETAT)]
    CaretAt,
    #[tok(BANGEQMINUS)]
    BangEqMinus,
    #[tok(PIPEPIPESLASH)]
    PipePipeSlash,
    #[tok(PIPESLASH)]
    PipeSlash,
    #[tok(ATMINUSAT)]
    AtMinusAt,
    #[tok(ATHASHAT)]
    AtHashAt,
    #[tok(ATPLUSAT)]
    AtPlusAt,
    #[tok(ATSIGN)]
    At,
    Custom(literal::CustomOp<'input>),
    Decorated(QuantifiedDecoratedOperator<'input>),
}

/// `subquery_Op` at the level of `+` and `-` (binding power 100), which in
/// pg-sql also holds the bitwise and JSON operators.
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedAddOperator {
    #[tok(LTMINUSGT)]
    LtMinusGt,
    #[tok(LTAT)]
    LtAt,
    #[tok(HASHARROWARROW)]
    HashArrowArrow,
    #[tok(HASHARROW)]
    HashArrow,
    #[tok(HASHMINUS)]
    HashMinus,
    #[tok(ARROWARROW)]
    ArrowArrow,
    #[tok(ARROW)]
    Arrow,
    #[tok(QUESTIONPIPE)]
    QuestionPipe,
    #[tok(QUESTIONAMP)]
    QuestionAmp,
    #[tok(ATGT)]
    AtGt,
    #[tok(AMPAMP)]
    AmpAmp,
    #[tok(PLUS)]
    Plus,
    #[tok(MINUS)]
    Minus,
    #[tok(POUND)]
    Pound,
    #[tok(AMP)]
    Amp,
    #[tok(PIPE)]
    Pipe,
    #[tok(QUESTION)]
    Question,
}

/// `subquery_Op` at the level of `*`, `/` and `%` (binding power 110).
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedMulOperator {
    #[tok(STAR)]
    Star,
    #[tok(SLASH)]
    Slash,
    #[tok(PERCENT)]
    Percent,
}

/// `subquery_Op` at the level of `^` (binding power 130).
#[derive(recursa::Node, Debug, Clone)]
pub enum QuantifiedPowOperator {
    #[tok(CARET)]
    Caret,
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

/// `{ANY|SOME|ALL} (expression-or-query)` — gram.y `sub_type '(' a_expr ')'`
/// and `sub_type select_with_parens`, shared by every quantified variant.
#[derive(recursa::Node, Debug, Clone)]
pub struct QuantifiedComparisonTail<'input> {
    pub kind: QuantifiedComparisonKind,
    #[tok(LPAREN, this, RPAREN)]
    pub operand: QuantifiedComparisonOperand<'input>,
}

/// `comparison operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonCmpSuffix<'input> {
    pub operator: QuantifiedCmpOperator,
    pub tail: QuantifiedComparisonTail<'input>,
}

/// `LIKE-family operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonLikeSuffix<'input> {
    pub operator: QuantifiedLikeOperator,
    pub tail: QuantifiedComparisonTail<'input>,
}

/// `generic operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonOpSuffix<'input> {
    pub operator: QuantifiedOpOperator<'input>,
    pub tail: QuantifiedComparisonTail<'input>,
}

/// `additive operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonAddSuffix<'input> {
    pub operator: QuantifiedAddOperator,
    pub tail: QuantifiedComparisonTail<'input>,
}

/// `multiplicative operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonMulSuffix<'input> {
    pub operator: QuantifiedMulOperator,
    pub tail: QuantifiedComparisonTail<'input>,
}

/// `exponentiation operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(break_before = soft)]
pub struct QuantifiedComparisonPowSuffix<'input> {
    pub operator: QuantifiedPowOperator,
    pub tail: QuantifiedComparisonTail<'input>,
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
/// multi-dimensional form `ARRAY[[1,2],[3,4]]` and the empty `ARRAY[]`.
///
/// PostgreSQL's `array_expr` keeps `'[' ']'` as its own alternative, so the
/// element list is nullable. The `ARRAY` keyword still leads the node, which
/// keeps the opening bracket visible to FIRST-k analysis, exactly as the
/// nested `NestedArrayElements` list above already relies on.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ARRAY, LBRACKET, this, RBRACKET)]
pub struct ArrayBracket<'input> {
    #[sep(COMMA)]
    pub elements: Vec<ArrayElement<'input>>,
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

/// `GROUPING(expr, ...)` — the grouping-set membership function.
///
/// gram.y keeps this as `func_expr_common_subexpr: GROUPING '(' expr_list
/// ')'`. `GROUPING` is a `COL_NAME` keyword, so it is a `ColId` but not a
/// `type_function_name`: it can be a bare column reference, never an
/// ordinary function name, and the call form needs its own production.
#[derive(recursa::Node, Debug, Clone)]
#[tok(GROUPING, LPAREN, this, RPAREN)]
pub struct GroupingCall<'input> {
    #[sep(COMMA)]
    pub args: recursa::Vec1<Expr<'input>>,
}

/// ROW constructor: `ROW(expr, ...)` or the empty `ROW()`.
///
/// PostgreSQL's `row` production keeps `ROW '(' expr_list ')'` and
/// `ROW '(' ')'` as separate alternatives, so the field list is nullable
/// here. The `ROW` keyword still leads the node, so the empty form does not
/// hide the opening parenthesis from FIRST-k analysis.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ROW, LPAREN, this, RPAREN)]
pub struct RowExpr<'input> {
    #[sep(COMMA)]
    pub values: Option<recursa::Vec1<Expr<'input>>>,
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
    /// Greedy: a leading WHEN starts this element instead of ending `CaseSearched` (bison shift preference).
    #[greedy(WHEN)]
    pub rest_arms: Vec<CaseWhenArm<'input>>,
    pub else_clause: Option<CaseElse<'input>>,
}

/// Simple CASE body: `operand WHEN val THEN result [...] [ELSE result]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CaseSimple<'input> {
    pub operand: Box<Expr<'input>>,
    pub first_arm: CaseWhenArm<'input>,
    /// Greedy: a leading WHEN starts this element instead of ending `CaseSimple` (bison shift preference).
    #[greedy(WHEN)]
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
    /// Greedy: a leading LBRACKET starts this element instead of ending `CastType` (bison shift preference).
    #[greedy(LBRACKET)]
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
/// (`numeric :'txid_current'`).
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
    /// gram.y `Numeric: DOUBLE_P PRECISION`; `double precision '1'` is a
    /// `ConstTypename Sconst` literal. Listed first: the two-token spelling
    /// must win over `double` as an identifier-spelled type name.
    #[tok(DOUBLE, PRECISION)]
    DoublePrecision,
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
/// typmods: `bool 'value'`, `text 'hello'`, or `double precision 'value'`.
///
/// The payload is PostgreSQL's `Sconst` and nothing else. pg-sql admits
/// psql's `:'var'` interpolation as a stand-in for a string constant
/// elsewhere, but not after a bare identifier: `ident : …` is also the
/// SQL/JSON `key : value` entry of `JSON_OBJECT` and `JSON_OBJECTAGG`, and
/// the atom dispatcher commits on the identifier and colon alone. Since
/// PostgreSQL's own `AexprConst: func_name Sconst` has no colon at all, the
/// pg-sql-only spelling is the one that yields. The keyword-named form
/// (`numeric :'var'`) and the typmod form (`name(10) :'var'`) keep it —
/// neither can be mistaken for an unquoted column name.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedTypeCastFunc<'input> {
    /// gram.y `AexprConst: func_name Sconst` — nothing stands between the
    /// name and the string. `double precision '1'` is
    /// `FixedTypeCastFuncName::DoublePrecision`; a `PRECISION` admitted
    /// here would follow `type_function_name` where gram.y never has it,
    /// and `precision` is also an operator class after a column in an
    /// index element, which a table-driven parser could not tell apart.
    pub type_name: crate::tokens::type_function_name<'input>,
    pub value: literal::StringLit<'input>,
}

/// Function-style typed literal for `json`: `json '{"a": 1}'`.
///
/// `JSON` is a `COL_NAME` keyword with its own `JsonType` production in
/// gram.y, so it reaches neither `NamedTypeCastFunc` (whose name is a
/// `type_function_name`) nor the typmod form. `JsonType` takes no type
/// modifiers, so none are modelled here — which also keeps this node
/// disjoint from the `JSON ( ... )` SQL/JSON value constructor at the second
/// token.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTypeCastFunc<'input> {
    #[tok(JSON, this)]
    pub value: TypeCastValue<'input>,
}

/// Function-style typed literal. Fixed-keyword type names can carry typmods
/// directly; identifier-spelled types with typmods use
/// [`FunctionCallTail::TypedLiteral`], where the complete call/typmod prefix
/// is shared.
///
/// Variant ordering is immaterial: `Fixed` leads with one of its own
/// keywords, `Json` with `JSON`, and `Named` with a `type_function_name`,
/// which admits neither.
#[derive(recursa::Node, Debug, Clone)]
pub enum TypeCastFunc<'input> {
    Fixed(FixedTypeCastFunc<'input>),
    Json(JsonTypeCastFunc<'input>),
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
    /// gram.y `trim_list: a_expr FROM expr_list | expr_list`: after the first
    /// expression either `FROM expr_list` or the rest of one `expr_list`,
    /// never both, so a comma after `FROM b` continues that list.
    pub rest: Option<TrimValuesRest<'input>>,
}

/// What follows the first expression of a `trim_list`.
///
/// Variant ordering: `From` starts with `FROM`, `More` with a comma.
#[derive(recursa::Node, Debug, Clone)]
pub enum TrimValuesRest<'input> {
    From(TrimFromArgs<'input>),
    More(TrimMoreArgs<'input>),
}

/// `, expr [, expr ...]`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
pub struct TrimMoreArgs<'input>(#[deref] pub recursa::Vec1<TrimMoreArg<'input>>);

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
/// One `, arg` of the ordinary function-call spelling of `SUBSTRING`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringMoreArg<'input> {
    #[tok(COMMA, this)]
    pub value: Box<Expr<'input>>,
}

/// The tail of `SUBSTRING(...)` after its first argument.
///
/// Besides the SQL-standard FROM/FOR and SIMILAR forms, gram.y keeps
/// `SUBSTRING '(' func_arg_list_opt ')'` so that a function named
/// `substring` can be called without the special syntax — the
/// `substring(x, 3, 1)` spelling. `SUBSTRING` is a `COL_NAME` keyword and
/// can never be an ordinary `FuncCall` name, so that form belongs here.
///
/// Variant ordering is immaterial: the four alternatives lead with SIMILAR,
/// FROM, FOR and COMMA respectively.
#[derive(recursa::Node, Debug, Clone)]
pub enum SubstringTail<'input> {
    Similar(SubstringSimilar<'input>),
    FromFor(SubstringFromFor<'input>),
    For(ForCount<'input>),
    Args(recursa::Vec1<SubstringMoreArg<'input>>),
}

/// Inner of `SUBSTRING(...)`: `source` followed by FROM/SIMILAR tail.
#[derive(recursa::Node, Debug, Clone)]
pub struct SubstringInner<'input> {
    /// Greedy: the expression keeps extending on SIMILAR instead of yielding to what may follow `SubstringInner`.
    #[greedy(SIMILAR)]
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
        // gram.y's `b_expr` reaches no `DEFAULT`: the keyword is not a
        // `c_expr`, only pg-sql's `INSERT`/`UPDATE` value placeholder.
        // recursa #122 lets an exclusion name an atom variant.
        Default,
        Collate,
        QuantifiedComparisonCmp,
        QuantifiedComparisonLike,
        QuantifiedComparisonOp,
        QuantifiedComparisonAdd,
        QuantifiedComparisonMul,
        QuantifiedComparisonPow,
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
    // No acceptance: the exclusion list already stops this operand before
    // `IN`, so analysis finds no overlap here (recursa #126).
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
///
/// gram.y `json_key_uniqueness_constraint_opt`: "KEYS is a noise word here.
/// To avoid shift/reduce conflicts, assign the KEYS-less productions a
/// precedence less than IDENT (i.e., less than KEYS). This prevents reducing
/// them when the next token is KEYS." `UNBOUNDED` (gram.y:886) is that level.
#[derive(recursa::Node, Debug, Clone)]
#[parse(prec = UNBOUNDED)]
pub struct JsonUniqueKeys {
    #[tok(this, UNIQUE)]
    pub with_or_without: WithOrWithout,
    /// Greedy: a leading KEYS starts this element instead of ending `JsonUniqueKeys` (bison shift preference).
    #[greedy(KEYS)]
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

/// The `{: | VALUE} ‹value› [FORMAT JSON ...]` part of a `JSON_OBJECT` item.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectEntryValue<'input> {
    pub sep: JsonKeyValueSep,
    pub value: Box<Expr<'input>>,
    pub format: Option<JsonFormat<'input>>,
}

/// One item of a `JSON_OBJECT` list: either a SQL/JSON
/// `[KEY] ‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry, or one
/// argument of PostgreSQL's legacy `json_object(text[])` /
/// `json_object(text[], text[])` function form.
///
/// gram.y keeps those as separate productions — `JSON_OBJECT` is a
/// `COL_NAME` keyword and can never be an ordinary `FuncCall` name, so the
/// legacy call gets its own `JSON_OBJECT '(' func_arg_list ')'` rule. Both
/// are comma-separated and expression-led, and no bounded lookahead splits
/// them: one `U&'...' UESCAPE '...'` key already spans five tokens. They
/// therefore share this node, and the presence of the key/value separator
/// is what tells the two forms apart.
///
/// PostgreSQL's `func_arg_list` also admits the `name => value` spelling.
/// That form is not modelled here: no `json_object` call uses it, and a
/// named argument is not a legal SQL/JSON entry key.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectEntry<'input> {
    pub key: Box<Expr<'input>>,
    pub value: Option<JsonObjectEntryValue<'input>>,
}

/// One `‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry of
/// `JSON_OBJECTAGG`, whose gram.y production admits only the SQL/JSON
/// spelling and therefore requires the value.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectAggEntry<'input> {
    pub key: Box<Expr<'input>>,
    pub value: JsonObjectEntryValue<'input>,
}

/// Non-empty item list of `JSON_OBJECT`, followed by the optional `ON NULL`,
/// `UNIQUE` and `RETURNING` clauses. Those clauses belong to the SQL/JSON
/// entry production only; the legacy function form never carries them.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonObjectArgs<'input> {
    #[sep(COMMA)]
    pub entries: recursa::Vec1<JsonObjectEntry<'input>>,
    pub on_null: Option<JsonOnNull>,
    pub unique: Option<JsonUniqueKeys>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `JSON_OBJECT` has distinct PostgreSQL productions for a non-empty item
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
/// gram.y `json_array_constructor`'s three argument forms.
///
/// Variant ordering: `Query` starts with a query keyword or `(`,
/// `Elements` with an expression, `Empty` with `RETURNING` or nothing.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonArrayArgs<'input> {
    Query(JsonArrayQueryArgs<'input>),
    Elements(JsonArrayElementsArgs<'input>),
    Empty(JsonArrayEmptyArgs<'input>),
}

/// `select_no_parens json_format_clause_opt json_returning_clause_opt`: the
/// query form has no `ON NULL` clause, so the query ends where its own
/// syntax ends. The `FORMAT JSON` option is not admitted: `FORMAT` is also
/// a table alias in the query's FROM list, which gram.y tells apart only
/// through the `FORMAT_LA` token filter that recursive descent lacks.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayQueryArgs<'input> {
    pub query: Box<DirectSubquery<'input>>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `json_value_expr_list json_array_constructor_null_clause_opt
/// json_returning_clause_opt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayElementsArgs<'input> {
    #[sep(COMMA)]
    pub elements: recursa::Vec1<JsonArrayElement<'input>>,
    pub on_null: Option<JsonOnNull>,
    pub returning: Option<JsonReturning<'input>>,
}

/// `json_returning_clause_opt` alone.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonArrayEmptyArgs<'input> {
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

/// gram.y `json_behavior_clause_opt`: `json_behavior ON EMPTY`,
/// `json_behavior ON ERROR`, or both. Each `JsonOnBehavior` names its own
/// trigger, so the pair is order-independent and the second slot is simply
/// optional; two independent optional slots would leave a single clause
/// ambiguous between them.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonBehaviorClause<'input> {
    pub first: JsonOnBehavior<'input>,
    pub second: Option<JsonOnBehavior<'input>>,
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
    /// gram.y `json_behavior_clause_opt`.
    pub on_behavior: Option<JsonBehaviorClause<'input>>,
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
    /// gram.y `json_behavior_clause_opt`.
    pub on_behavior: Option<JsonBehaviorClause<'input>>,
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
    pub entry: JsonObjectAggEntry<'input>,
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
///
/// gram.y `json_predicate_type_constraint: JSON %prec UNBOUNDED` and
/// `json_key_uniqueness_constraint_opt: /* EMPTY */ %prec UNBOUNDED`: the
/// tail-less productions sit below the `IDENT` level that carries `VALUE`,
/// `SCALAR`, `OBJECT`, `WITH`, `WITHOUT` and `KEYS`, so the parser shifts the
/// tail rather than ending the predicate.
#[derive(recursa::Node, Debug, Clone)]
#[parse(prec = UNBOUNDED)]
pub struct IsJsonTail {
    #[tok(this, JSON)]
    #[presence(NOT)]
    pub not: bool,
    /// Greedy: a leading OBJECT, SCALAR, VALUE starts this element instead of ending `IsJsonTail` (bison shift preference).
    #[greedy(OBJECT, SCALAR, VALUE)]
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
/// Greedy: this enum-level acceptance covers every left-denotation operand
/// inside `Expr` (the right operands of infix and postfix variants and the
/// operands of prefix forms). An operand keeps extending on a shared
/// extender instead of yielding to whatever may follow the enclosing
/// expression, which is PostgreSQL's precedence resolution. The optional
/// `ESCAPE` tails of the LIKE family carry their own `#[greedy(ESCAPE)]`.
///
/// The overlap here is every extender of `Expr` — each infix and postfix
/// operator in this enum both continues an operand and may follow the
/// enclosing expression — so the acceptance is `all` rather than a
/// hand-copied list of every operator kind, which would say no more and
/// would need editing for each operator added.
#[greedy(all)]
#[pratt]
pub enum Expr<'input> {
    // --- Prefix ---
    #[parse(prefix, bp = 150)]
    Not(#[tok(NOT, this)] Box<Self>),
    #[parse(prefix, bp = 120)]
    Neg(#[tok(MINUS, this)] Box<Self>),
    /// Unary plus: `+expr` — identity operator on numeric types.
    #[parse(prefix, bp = 120)]
    Pos(#[tok(PLUS, this)] Box<Self>),
    /// Unary geometric "center point": `@@ expr`. Postgres uses `@@` as
    /// a prefix operator on box / polygon / etc. (in addition to the
    /// text-search infix form).
    #[parse(prefix, bp = 120)]
    GeomCenter(#[tok(ATAT, this)] Box<Self>),
    /// Bitwise NOT: `~ expr` (e.g. inet / bit / int bitwise complement).
    /// Must come before any infix `~` variant so the prefix form wins when
    /// `~` appears at the start of an operand.
    #[parse(prefix, bp = 120)]
    BitNot(#[tok(TILDE, this)] Box<Self>),
    /// Geometric path/lseg length: `@-@ expr`. Must come before `Abs` (`@`)
    /// since `@-@` is longer.
    #[parse(prefix, bp = 120)]
    PathLength(#[tok(ATMINUSAT, this)] Box<Self>),
    /// User-defined prefix: `@#@ expr` (e.g. factorial).
    #[parse(prefix, bp = 120)]
    AtHashAtPrefix(#[tok(ATHASHAT, this)] Box<Self>),
    /// Geometric point-count: `# path` — number of points in a path.
    #[parse(prefix, bp = 120)]
    PointCount(#[tok(POUND, this)] Box<Self>),
    /// Absolute value: `@ expr` (Postgres unary `@` operator).
    #[parse(prefix, bp = 120)]
    Abs(#[tok(ATSIGN, this)] Box<Self>),
    /// User-defined prefix: `!=- expr`.
    #[parse(prefix, bp = 120)]
    BangEqMinusPrefix(#[tok(BANGEQMINUS, this)] Box<Self>),
    /// Square root: `|/ expr` (Postgres unary `|/` operator).
    #[parse(prefix, bp = 120)]
    Sqrt(#[tok(PIPESLASH, this)] Box<Self>),
    /// Cube root: `||/ expr` (Postgres unary `||/` operator).
    #[parse(prefix, bp = 120)]
    Cbrt(#[tok(PIPEPIPESLASH, this)] Box<Self>),

    /// Catch-all prefix: any user-defined prefix operator not matched by a
    /// specific token. Declared LAST among prefixes.
    ///
    /// gram.y `qual_Op a_expr %prec Op` (gram.y:889 `%left Op OPERATOR`):
    /// the operand extends over the operators above `Op` (`@# a + b` is
    /// `@# (a + b)`) and stops at `Op` and below (`@# a = b` is
    /// `(@# a) = b`, `@# a @# b` is `(@# a) @# b`), which is the binding
    /// power one step above `Op`'s level. recursa #124 admits a content
    /// token as a Pratt prefix operator, so this is a real prefix variant
    /// rather than an atom with a hand-written exclusion list.
    #[parse(prefix, bp = 81)]
    CustomPrefix(
        literal::CustomOp<'input>,
        #[pretty(break_before = soft)] Box<Self>,
    ),

    // --- Postfix ---
    /// Postgres-style cast: `expr::type`
    #[parse(postfix, bp = 200)]
    Cast(Box<Self>, #[tok(COLONCOLON, this)] Box<CastType<'input>>),
    /// `expr COLLATE "collation"` — collation specifier. Binds tighter than
    /// comparisons (bp 5) but looser than `::` cast (bp 20).
    #[parse(postfix, bp = 180)]
    Collate(
        Box<Self>,
        #[tok(COLLATE, this)] crate::tokens::ColId<'input>,
    ),
    /// `lhs operator {ANY|SOME|ALL} (expr-or-query)`: gram.y `a_expr
    /// subquery_Op sub_type '(' a_expr ')' %prec Op` and its
    /// `select_with_parens` twin. gram.y decides the shift before the
    /// operator by the operator token's own precedence, so `1 + 2 / ANY (x)`
    /// is `1 + (2 / ANY (x))` and `a = b + ANY (x)` is `a = (b + ANY (x))`;
    /// one postfix variant per precedence level of `subquery_Op`, each at
    /// the level of the same token's infix variant, reproduces that.
    ///
    /// PostgreSQL does not admit the quantified right-hand side as a
    /// standalone expression. Keeping the operator and quantifier in one
    /// Pratt continuation also makes `f(ALL(x))` unambiguously the function
    /// application's ALL-qualified argument production.
    #[parse(postfix, bp = 50)]
    QuantifiedComparisonCmp(Box<Self>, QuantifiedComparisonCmpSuffix<'input>),
    /// `subquery_Op`'s `LIKE` family, at the level of `Like`.
    #[parse(postfix, bp = 60)]
    QuantifiedComparisonLike(Box<Self>, QuantifiedComparisonLikeSuffix<'input>),
    /// `subquery_Op` at gram.y's generic `Op` level.
    #[parse(postfix, bp = 80)]
    QuantifiedComparisonOp(Box<Self>, QuantifiedComparisonOpSuffix<'input>),
    /// `subquery_Op` at the level of `Add` / `Sub`.
    #[parse(postfix, bp = 100)]
    QuantifiedComparisonAdd(Box<Self>, QuantifiedComparisonAddSuffix<'input>),
    /// `subquery_Op` at the level of `Mul` / `Div` / `Mod`.
    #[parse(postfix, bp = 110)]
    QuantifiedComparisonMul(Box<Self>, QuantifiedComparisonMulSuffix<'input>),
    /// `subquery_Op` at the level of `Pow`.
    #[parse(postfix, bp = 130)]
    QuantifiedComparisonPow(Box<Self>, QuantifiedComparisonPowSuffix<'input>),
    // The `IS` family sits at one level below the comparison operators, gram.y
    // `%nonassoc IS ISNULL NOTNULL` (`a = b IS NULL` is `(a = b) IS NULL`);
    // one level for every `IS` form is also what lets a table-driven parser
    // settle `IS` by precedence. Binding power 4.
    /// `expr IS NOT DISTINCT FROM expr`. Declared before `IsDistinctFrom` so
    /// the longer `NOT` prefix wins disambiguation.
    #[parse(infix, lbp = 40, rbp = 41)]
    IsNotDistinctFrom(Box<Self>, #[tok(IS, NOT, DISTINCT, FROM, this)] Box<Self>),
    /// `expr IS DISTINCT FROM expr`.
    #[parse(infix, lbp = 40, rbp = 41)]
    IsDistinctFrom(Box<Self>, #[tok(IS, DISTINCT, FROM, this)] Box<Self>),
    /// `expr IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] [{WITH|WITHOUT}
    /// UNIQUE [KEYS]]` — the SQL/JSON type predicate. Declared before
    /// `BoolTest` (both lead with `IS`); `BoolTest` rejects `JSON` as a
    /// `BoolTestKind`, so order is not load-bearing, only tidy.
    #[parse(postfix, bp = 40)]
    IsJson(Box<Self>, #[tok(IS, this)] IsJsonTail),
    /// `expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` — the Unicode
    /// normalisation predicate (gram.y rules 15198/15205/15212/15220).
    /// Declared before `BoolTest` (both lead with `IS`); `BoolTest` rejects
    /// `NORMALIZED`/`NFx` as a `BoolTestKind`, so order is not load-bearing.
    #[parse(postfix, bp = 40)]
    IsNormalized(Box<Self>, #[tok(IS, this)] IsNormalizedTail),
    /// `expr IS [NOT] DOCUMENT` — the XML document predicate.
    #[parse(postfix, bp = 40)]
    IsDocument(Box<Self>, #[tok(IS, this)] IsDocumentTail),
    /// Boolean test: `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`
    #[parse(postfix, bp = 40)]
    BoolTest(Box<Self>, #[tok(IS, this)] BoolTestKind),
    /// Postgres `expr NOTNULL` postfix null test (synonym for `IS NOT NULL`).
    #[parse(postfix, bp = 40)]
    Notnull(#[tok(this, NOTNULL)] Box<Self>),
    /// Postgres `expr ISNULL` postfix null test (synonym for `IS NULL`).
    #[parse(postfix, bp = 40)]
    Isnull(#[tok(this, ISNULL)] Box<Self>),
    /// `expr AT LOCAL` — convert to session timezone. Listed before
    /// `AtTimeZone` so `AT LOCAL` wins (distinct second token `LOCAL` vs `TIME`).
    #[parse(postfix, bp = 90)]
    AtLocal(#[tok(this, AT, LOCAL)] Box<Self>),
    /// `expr AT TIME ZONE zone_expr` — convert to specified timezone.
    #[parse(infix, lbp = 90, rbp = 91)]
    AtTimeZone(Box<Self>, #[tok(AT, TIME, ZONE, this)] Box<Self>),
    /// NOT IN list: `expr NOT IN (val, ...)`
    #[parse(postfix, bp = 60)]
    NotInExpr(Box<Self>, NotInSuffix<'input>),
    // The LIKE family shares one level with `BETWEEN` and `IN`, gram.y
    // `%nonassoc BETWEEN IN_P LIKE ILIKE SIMILAR NOT_LA`, one step above the
    // comparison operators: `a = b LIKE c` is `a = (b LIKE c)`. Binding
    // power 6; the pattern operand is the infix right operand at 7, so a
    // LIKE never nests in a LIKE's pattern (gram.y makes the level
    // non-associative) and an `ESCAPE` belongs to the LIKE it follows.
    //
    // gram.y `a_expr LIKE a_expr ESCAPE a_expr %prec LIKE` with
    // `%nonassoc ESCAPE` one level above `LIKE`. The `ESCAPE` operand is
    // an attached optional operand on the variant itself (recursa #123),
    // so it is a Pratt right operand at `ESCAPE`'s level and carries that
    // precedence into both parsers: it takes every operator above
    // `ESCAPE` (`'$'::bytea`, `'$' || 'x'`) and stops at `LIKE`'s level
    // and below. The first pass had to spell this as a separate
    // `EscapeClause` struct with an exclusion list, which no rule
    // precedence reproduced.
    /// `expr NOT ILIKE pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT ILIKE` is tried first (matters only if any rule shares a prefix;
    /// here `NOT ILIKE` vs `NOT LIKE` differ on the second token).
    #[parse(infix, lbp = 60, rbp = 61)]
    NotIlike(
        Box<Self>,
        #[tok(NOT, ILIKE, this)] Box<Self>,
        // Greedy: the `ESCAPE` operand is a Pratt right operand at
        // `ESCAPE`'s level, so it keeps extending on every operator above
        // that level -- each of which may also follow the whole LIKE
        // expression. The overlap is that entire set, so the acceptance is
        // `all` rather than a hand-copied list of every operator kind.
        #[greedy(all)]
        #[tok(ESCAPE, this)]
        Option<Box<Self>>,
    ),
    /// `expr NOT SIMILAR TO pattern [ESCAPE char]`. Declared before `NotLike` so the longer
    /// `NOT SIMILAR TO` form wins longest-match-wins disambiguation.
    #[parse(infix, lbp = 60, rbp = 61)]
    NotSimilarTo(
        Box<Self>,
        #[tok(NOT, SIMILAR, TO, this)] Box<Self>,
        // Greedy: the `ESCAPE` operand is a Pratt right operand at
        // `ESCAPE`'s level, so it keeps extending on every operator above
        // that level -- each of which may also follow the whole LIKE
        // expression. The overlap is that entire set, so the acceptance is
        // `all` rather than a hand-copied list of every operator kind.
        #[greedy(all)]
        #[tok(ESCAPE, this)]
        Option<Box<Self>>,
    ),
    /// `expr NOT LIKE pattern [ESCAPE char]`. Must come before the `Not` prefix atom so
    /// longest-match-wins prefers the postfix form.
    #[parse(infix, lbp = 60, rbp = 61)]
    NotLike(
        Box<Self>,
        #[tok(NOT, LIKE, this)] Box<Self>,
        // Greedy: the `ESCAPE` operand is a Pratt right operand at
        // `ESCAPE`'s level, so it keeps extending on every operator above
        // that level -- each of which may also follow the whole LIKE
        // expression. The overlap is that entire set, so the acceptance is
        // `all` rather than a hand-copied list of every operator kind.
        #[greedy(all)]
        #[tok(ESCAPE, this)]
        Option<Box<Self>>,
    ),
    /// `expr SIMILAR TO pattern [ESCAPE char]` — SQL standard similar-to pattern match.
    #[parse(infix, lbp = 60, rbp = 61)]
    SimilarTo(
        Box<Self>,
        #[tok(SIMILAR, TO, this)] Box<Self>,
        // Greedy: the `ESCAPE` operand is a Pratt right operand at
        // `ESCAPE`'s level, so it keeps extending on every operator above
        // that level -- each of which may also follow the whole LIKE
        // expression. The overlap is that entire set, so the acceptance is
        // `all` rather than a hand-copied list of every operator kind.
        #[greedy(all)]
        #[tok(ESCAPE, this)]
        Option<Box<Self>>,
    ),
    /// `expr ILIKE pattern [ESCAPE char]`
    #[parse(infix, lbp = 60, rbp = 61)]
    Ilike(
        Box<Self>,
        #[tok(ILIKE, this)] Box<Self>,
        // Greedy: the `ESCAPE` operand is a Pratt right operand at
        // `ESCAPE`'s level, so it keeps extending on every operator above
        // that level -- each of which may also follow the whole LIKE
        // expression. The overlap is that entire set, so the acceptance is
        // `all` rather than a hand-copied list of every operator kind.
        #[greedy(all)]
        #[tok(ESCAPE, this)]
        Option<Box<Self>>,
    ),
    /// `expr LIKE pattern [ESCAPE char]`
    #[parse(infix, lbp = 60, rbp = 61)]
    Like(
        Box<Self>,
        #[tok(LIKE, this)] Box<Self>,
        // Greedy: the `ESCAPE` operand is a Pratt right operand at
        // `ESCAPE`'s level, so it keeps extending on every operator above
        // that level -- each of which may also follow the whole LIKE
        // expression. The overlap is that entire set, so the acceptance is
        // `all` rather than a hand-copied list of every operator kind.
        #[greedy(all)]
        #[tok(ESCAPE, this)]
        Option<Box<Self>>,
    ),
    // --- Locale-aware text comparison operators (4-char before 3-char) ---
    /// `expr ~<=~ expr` — locale-aware less-or-equal.
    #[parse(infix, lbp = 50, rbp = 51)]
    TildeLeqTilde(Box<Self>, #[tok(TILDELEQTILDE, this)] Box<Self>),
    /// `expr ~>=~ expr` — locale-aware greater-or-equal.
    #[parse(infix, lbp = 50, rbp = 51)]
    TildeGeqTilde(Box<Self>, #[tok(TILDEGEQTILDE, this)] Box<Self>),
    /// `expr ~<~ expr` — locale-aware less-than.
    #[parse(infix, lbp = 50, rbp = 51)]
    TildeLtTilde(Box<Self>, #[tok(TILDELTTILDE, this)] Box<Self>),
    /// `expr ~>~ expr` — locale-aware greater-than.
    #[parse(infix, lbp = 50, rbp = 51)]
    TildeGtTilde(Box<Self>, #[tok(TILDEGTTILDE, this)] Box<Self>),
    /// `expr !~* pattern` — POSIX case-insensitive negated regex match.
    #[parse(infix, lbp = 50, rbp = 51)]
    RegexNotIMatch(Box<Self>, #[tok(BANGTILDESTAR, this)] Box<Self>),
    /// `expr ~* pattern` — POSIX case-insensitive regex match.
    #[parse(infix, lbp = 50, rbp = 51)]
    RegexIMatch(Box<Self>, #[tok(TILDESTAR, this)] Box<Self>),
    /// `expr !~ pattern` — POSIX negated regex match.
    #[parse(infix, lbp = 50, rbp = 51)]
    RegexNotMatch(Box<Self>, #[tok(BANGTILDE, this)] Box<Self>),
    /// `expr ~= expr` — geometric "same as" operator. Declared before `RegexMatch`
    /// so the longer `~=` wins longest-match.
    #[parse(infix, lbp = 50, rbp = 51)]
    GeomSame(Box<Self>, #[tok(TILDEEQ, this)] Box<Self>),
    /// `expr ~ pattern` — POSIX regex match.
    #[parse(infix, lbp = 50, rbp = 51)]
    RegexMatch(Box<Self>, #[tok(TILDE, this)] Box<Self>),
    /// `expr !~~* pattern` — operator-form `NOT ILIKE` (gram.y 14897).
    /// Declared before `LikeOpINeg` (`!~~`) so the longer `!~~*` wins.
    #[parse(infix, lbp = 50, rbp = 51)]
    LikeOpINeg(Box<Self>, #[tok(BANGTILDETILDESTAR, this)] Box<Self>),
    /// `expr ~~* pattern` — operator-form `ILIKE` (gram.y 14888).
    /// Declared before `LikeOpI` would be (no `~~*` longer prefix).
    #[parse(infix, lbp = 50, rbp = 51)]
    LikeOpI(Box<Self>, #[tok(TILDETILDESTAR, this)] Box<Self>),
    /// `expr !~~ pattern` — operator-form `NOT LIKE` (gram.y 14874).
    #[parse(infix, lbp = 50, rbp = 51)]
    LikeOpNeg(Box<Self>, #[tok(BANGTILDETILDE, this)] Box<Self>),
    /// `expr ~~ pattern` — operator-form `LIKE` (gram.y 14860).
    #[parse(infix, lbp = 50, rbp = 51)]
    LikeOp(Box<Self>, #[tok(TILDETILDE, this)] Box<Self>),
    /// `(start, end) OVERLAPS (start, end)` — SQL time-period overlap test.
    /// Each operand is an ordinary parenthesized expression to the parser.
    #[parse(infix, lbp = 50, rbp = 51)]
    Overlaps(Box<Self>, #[tok(OVERLAPS, this)] Box<Self>),
    /// Record comparison operators: `expr *= expr`, `*<>`, `*<`, `*<=`,
    /// `*>`, `*>=` — compare ROW/composite values field by field.
    #[parse(infix, lbp = 50, rbp = 51)]
    RecordLte(Box<Self>, #[tok(STARLTE, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    RecordGte(Box<Self>, #[tok(STARGTE, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    RecordNeq(Box<Self>, #[tok(STARNEQ, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    RecordLt(Box<Self>, #[tok(STARLT, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    RecordGt(Box<Self>, #[tok(STARGT, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    RecordEq(Box<Self>, #[tok(STAREQ, this)] Box<Self>),
    /// IN list: `expr IN (val, ...)`
    #[parse(postfix, bp = 60)]
    InExpr(Box<Self>, #[tok(IN, this)] InList<'input>),
    /// `expr NOT BETWEEN low AND high`. Declared before `BetweenExpr` so
    /// the longer `NOT BETWEEN` prefix wins disambiguation. Recursive fields
    /// in this postfix tail inherit `bp = 60`, so the low/high operands stop
    /// before the literal `AND` infix at `bp = 20`.
    #[parse(postfix, bp = 60)]
    NotBetweenExpr(
        Box<Self>,
        #[tok(NOT, BETWEEN, this)] Box<Self>,
        #[tok(AND, this)] Box<Self>,
    ),
    /// `expr BETWEEN low AND high`. See `NotBetweenExpr` for the recursive
    /// postfix-tail binding-power rationale.
    #[parse(postfix, bp = 60)]
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
    // bp = 100; generic `Op` spellings such as `||` use the lower bp = 80 tier.
    /// JSON path as text: `expr #>> path`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonPathText(Box<Self>, #[tok(HASHARROWARROW, this)] Box<Self>),
    /// JSON path: `expr #> path`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonPath(Box<Self>, #[tok(HASHARROW, this)] Box<Self>),
    /// JSON field as text: `expr ->> field`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonFieldText(Box<Self>, #[tok(ARROWARROW, this)] Box<Self>),
    /// JSON field: `expr -> field`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonField(Box<Self>, #[tok(ARROW, this)] Box<Self>),
    /// Geometric parallel: `a ?|| b`. Must precede `JsonAnyKey` (`?|`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, lbp = 50, rbp = 51)]
    Parallel(Box<Self>, #[tok(QUESTIONPIPEPIPE, this)] Box<Self>),
    /// JSON any-key-exists: `expr ?| keys`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonAnyKey(Box<Self>, #[tok(QUESTIONPIPE, this)] Box<Self>),
    /// JSON all-keys-exist: `expr ?& keys`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonAllKeys(Box<Self>, #[tok(QUESTIONAMP, this)] Box<Self>),
    /// Geometric intersect: `a ?# b`. Must precede `JsonKey` (`?`).
    #[parse(infix, lbp = 50, rbp = 51)]
    Intersect(Box<Self>, #[tok(QUESTIONHASH, this)] Box<Self>),
    /// Geometric perpendicular: `a ?-| b`. Must precede `Horizontal` (`?-`)
    /// so the 3-char token wins over the 2-char token.
    #[parse(infix, lbp = 50, rbp = 51)]
    Perpendicular(Box<Self>, #[tok(QUESTIONDASHPIPE, this)] Box<Self>),
    /// Geometric horizontal: `a ?- b`. Must precede `JsonKey` (`?`).
    #[parse(infix, lbp = 50, rbp = 51)]
    Horizontal(Box<Self>, #[tok(QUESTIONDASH, this)] Box<Self>),
    /// Geometric "is horizontal" prefix: `?- s` — tests whether the
    /// LSEG/LINE `s` is horizontal. PG's geometry.sql uses this in WHERE.
    #[parse(prefix, bp = 120)]
    IsHorizontal(#[tok(QUESTIONDASH, this)] Box<Self>),
    /// Geometric "is vertical" prefix: `?| s`.
    #[parse(prefix, bp = 120)]
    IsVertical(#[tok(QUESTIONPIPE, this)] Box<Self>),
    /// Geometric "below": `a <^ b`.
    #[parse(infix, lbp = 50, rbp = 51)]
    Below(Box<Self>, #[tok(LTCARET, this)] Box<Self>),
    /// Geometric "above": `a >^ b`.
    #[parse(infix, lbp = 50, rbp = 51)]
    Above(Box<Self>, #[tok(GTCARET, this)] Box<Self>),
    /// JSON key-exists: `expr ? key`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonKey(Box<Self>, #[tok(QUESTION, this)] Box<Self>),
    /// JSONB contains: `expr @> expr`
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonContains(Box<Self>, #[tok(ATGT, this)] Box<Self>),
    /// JSONB contained-by: `expr <@ expr`
    #[parse(infix, lbp = 100, rbp = 101)]
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
    #[parse(infix, lbp = 50, rbp = 51)]
    TsMatch3(Box<Self>, #[tok(ATATAT, this)] Box<Self>),
    /// User-defined triple-less-than: `a <<< b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, lbp = 50, rbp = 51)]
    TripleLt(Box<Self>, #[tok(LTLTLT, this)] Box<Self>),
    /// Geometric strictly-below: `a <<| b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, lbp = 50, rbp = 51)]
    StrictlyBelow(Box<Self>, #[tok(LTLTPIPE, this)] Box<Self>),
    /// Inet is-subset-or-equal: `a <<= b`. Before `StrictlyLeft` (`<<`).
    #[parse(infix, lbp = 50, rbp = 51)]
    SubsetEq(Box<Self>, #[tok(LTLTEQ, this)] Box<Self>),
    /// Distance: `a <-> b`. Before any `<` variant.
    #[parse(infix, lbp = 100, rbp = 101)]
    Distance(Box<Self>, #[tok(LTMINUSGT, this)] Box<Self>),
    /// User-defined triple-greater-than: `a >>> b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, lbp = 50, rbp = 51)]
    TripleGt(Box<Self>, #[tok(GTGTGT, this)] Box<Self>),
    /// Inet is-superset-or-equal: `a >>= b`. Before `StrictlyRight` (`>>`).
    #[parse(infix, lbp = 50, rbp = 51)]
    SupersetEq(Box<Self>, #[tok(GTGTEQ, this)] Box<Self>),
    /// Range adjacent: `a -|- b`. Before `Sub` (`-`).
    #[parse(infix, lbp = 50, rbp = 51)]
    Adjacent(Box<Self>, #[tok(MINUSPIPEMINUS, this)] Box<Self>),
    /// Geometric strictly-above: `a |>> b`. Before `Concat` (`||`).
    #[parse(infix, lbp = 50, rbp = 51)]
    StrictlyAbove(Box<Self>, #[tok(PIPEGTGT, this)] Box<Self>),
    /// Geometric no-extend-below: `a |&> b`. Before `Concat` (`||`).
    #[parse(infix, lbp = 50, rbp = 51)]
    NoExtendBelow(Box<Self>, #[tok(PIPEAMPGT, this)] Box<Self>),
    /// Geometric no-extend-above: `a &<| b`. Before `NoExtendRight` (`&<`).
    #[parse(infix, lbp = 50, rbp = 51)]
    NoExtendAbove(Box<Self>, #[tok(AMPLTPIPE, this)] Box<Self>),

    // --- 2-char operators ---
    /// Text-search / jsonb path match: `expr @@ expr`.
    #[parse(infix, lbp = 50, rbp = 51)]
    TsMatch(Box<Self>, #[tok(ATAT, this)] Box<Self>),
    /// Jsonpath exists: `expr @? path`.
    #[parse(infix, lbp = 50, rbp = 51)]
    JsonPathExists(Box<Self>, #[tok(ATQUESTION, this)] Box<Self>),
    /// Range / array overlap: `a && b`.
    #[parse(infix, lbp = 100, rbp = 101)]
    Overlap(Box<Self>, #[tok(AMPAMP, this)] Box<Self>),
    /// Range does-not-extend-right: `a &< b`.
    #[parse(infix, lbp = 50, rbp = 51)]
    NoExtendRight(Box<Self>, #[tok(AMPLT, this)] Box<Self>),
    /// Range does-not-extend-left: `a &> b`.
    #[parse(infix, lbp = 50, rbp = 51)]
    NoExtendLeft(Box<Self>, #[tok(AMPGT, this)] Box<Self>),
    /// Range strictly-left-of: `a << b`.
    #[parse(infix, lbp = 50, rbp = 51)]
    StrictlyLeft(Box<Self>, #[tok(LTLT, this)] Box<Self>),
    /// Range strictly-right-of: `a >> b`.
    #[parse(infix, lbp = 50, rbp = 51)]
    StrictlyRight(Box<Self>, #[tok(GTGT, this)] Box<Self>),

    // --- User-defined / custom infix operators ---
    /// `expr === expr` — user-defined triple-equal. Must come before `Eq` (`=`).
    #[parse(infix, lbp = 50, rbp = 51)]
    TripleEq(Box<Self>, #[tok(TRIPLEEQ, this)] Box<Self>),
    /// `expr !== expr` — user-defined not-equal. Must come before `BangEq` (`!=`).
    #[parse(infix, lbp = 50, rbp = 51)]
    BangEqEq(Box<Self>, #[tok(BANGEQEQ, this)] Box<Self>),
    /// `expr ## expr` — geometric closest-point / path intersection.
    /// Must come before `BitXor` (`#`).
    #[parse(infix, lbp = 50, rbp = 51)]
    GeomClosest(Box<Self>, #[tok(HASHHASH, this)] Box<Self>),

    #[parse(infix, lbp = 10, rbp = 11)]
    Or(Box<Self>, #[tok(OR, this)] Box<Self>),
    #[parse(infix, lbp = 20, rbp = 21)]
    And(Box<Self>, #[tok(AND, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    BangEq(Box<Self>, #[tok(BANGEQ, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    Neq(Box<Self>, #[tok(NEQ, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    Lte(Box<Self>, #[tok(LTE, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    Gte(Box<Self>, #[tok(GTE, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    Eq(Box<Self>, #[tok(EQ, this)] Box<Self>),

    /// Text starts-with: `expr ^@ expr` (PostgreSQL `starts_with` operator).
    /// `^@` is a single token (see `punct::CaretAt`); declared before
    /// `CustomInfix` so it wins the declaration-order tiebreak. bp=8 matches
    /// Postgres's generic `Op` precedence.
    #[parse(infix, lbp = 80, rbp = 81)]
    StartsWith(Box<Self>, #[tok(CARETAT, this)] Box<Self>),
    /// JSONB delete-path: `expr #- path` (PostgreSQL jsonb delete-at-path
    /// operator). `#-` is a single token (see `punct::HashMinus`); declared
    /// before `CustomInfix` so it wins the declaration-order tiebreak. bp=10
    /// matches the neighbouring `#>`/`#>>` JSON path operators.
    #[parse(infix, lbp = 100, rbp = 101)]
    JsonDeletePath(Box<Self>, #[tok(HASHMINUS, this)] Box<Self>),

    /// Catch-all infix: any user-defined operator not matched by a specific
    /// token above. Declared BEFORE single-char operators so 2+ char custom
    /// operators like `<%` or `~>` aren't consumed as the single-char prefix
    /// (`<`, `~`) plus garbage. Since `CustomOp` requires 2+ characters, bare
    /// single-char operators still fall through to the variants below.
    /// bp=8 matches Postgres's generic `Op` precedence (between comparison
    /// bp=5 and additive bp=10).
    #[parse(infix, lbp = 80, rbp = 81)]
    CustomInfix(
        Box<Self>,
        #[pretty(break_before = soft, break_after = soft)] literal::CustomOp<'input>,
        Box<Self>,
    ),

    #[parse(infix, lbp = 50, rbp = 51)]
    Lt(Box<Self>, #[tok(LT, this)] Box<Self>),
    #[parse(infix, lbp = 50, rbp = 51)]
    Gt(Box<Self>, #[tok(GT, this)] Box<Self>),
    /// String concatenation: `expr || expr`. PostgreSQL scans `||` as a
    /// generic `Op`, below additive operators in the precedence hierarchy.
    #[parse(infix, lbp = 80, rbp = 81)]
    Concat(Box<Self>, #[tok(CONCAT, this)] Box<Self>),
    /// Bitwise OR: `expr | expr`. Must come after `Concat` (`||`) so the
    /// longer token matches first at the punctuation level.
    #[parse(infix, lbp = 100, rbp = 101)]
    BitOr(Box<Self>, #[tok(PIPE, this)] Box<Self>),
    /// Bitwise AND: `expr & expr`.
    #[parse(infix, lbp = 100, rbp = 101)]
    BitAnd(Box<Self>, #[tok(AMP, this)] Box<Self>),
    /// Bitwise XOR: `expr # expr` (Postgres bit-string / integer operator).
    #[parse(infix, lbp = 100, rbp = 101)]
    BitXor(Box<Self>, #[tok(POUND, this)] Box<Self>),
    #[parse(infix, lbp = 100, rbp = 101)]
    Add(Box<Self>, #[tok(PLUS, this)] Box<Self>),
    #[parse(infix, lbp = 100, rbp = 101)]
    Sub(Box<Self>, #[tok(MINUS, this)] Box<Self>),
    /// Multiplication: `expr * expr`
    #[parse(infix, lbp = 110, rbp = 111)]
    Mul(Box<Self>, #[tok(STAR, this)] Box<Self>),
    /// Division: `expr / expr`
    #[parse(infix, lbp = 110, rbp = 111)]
    Div(Box<Self>, #[tok(SLASH, this)] Box<Self>),
    /// Modulo: `expr % expr`
    #[parse(infix, lbp = 110, rbp = 111)]
    Mod(Box<Self>, #[tok(PERCENT, this)] Box<Self>),
    /// Exponentiation: `expr ^ expr` (Postgres numeric power operator).
    #[parse(infix, lbp = 130, rbp = 131)]
    Pow(Box<Self>, #[tok(CARET, this)] Box<Self>),

    // --- Atoms ---
    /// EXISTS subquery: `EXISTS (SELECT ...)`
    Exists(ExistsExpr<'input>),
    /// ARRAY constructor: `ARRAY[...]` or `ARRAY(...)`
    Array(ArrayExpr<'input>),
    /// ROW constructor: `ROW(...)`
    RowExpr(RowExpr<'input>),
    /// `GROUPING(...)` grouping-set membership function. Declared before
    /// `ColumnRef`, which would otherwise claim the bare `GROUPING` keyword
    /// and leave the argument list unparsed.
    Grouping(GroupingCall<'input>),
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
    PositionalParam(PositionalParam<'input>),
    /// Unqualified column reference: `f1` or `"Foo"`, with its subscripts
    ColumnRef(ColumnRef<'input>),
    /// psql client variable substitution: `:foo`, `:'foo'`, `:"foo"`.
    PsqlVar(PsqlVariableExpr<'input>),
}
