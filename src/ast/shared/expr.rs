/// SQL expression AST whose Pratt declarations lower to LR precedence rules.
///
/// Handles atoms, prefix (NOT, unary minus), infix (AND, OR, comparisons,
/// arithmetic), and postfix operators (::type cast, IS [NOT] TRUE/FALSE/UNKNOWN/NULL,
/// IN (list)).
use crate::ast::dml::values::Subquery;
use crate::tokens::literal;

recursa::ast_node! {
    /// Required opening delimiter for structurally parenthesized SQL forms.
    #[derive(Debug)]
    pub enum ParenthesizedOpen {
        #[tok(LPAREN)]
        Value,
    }
}

recursa::ast_node! {
    /// Required closing delimiter for structurally parenthesized SQL forms.
    #[derive(Debug)]
    pub enum ParenthesizedClose {
        #[tok(RPAREN)]
        Value,
    }
}

recursa::ast_node! {
    /// One PostgreSQL string value, either a single literal or a scanner-valid
    /// newline-concatenated sequence. The sequence is one lexical token so ignored
    /// block-comment trivia cannot disappear between its fragments.
    #[derive(Debug)]
    pub enum StringLitSeq0 {
        Sequence(literal::StringLitSequence),
        Single(literal::StringLit),
    }
}

recursa::ast_node! {
    /// Content inside IN parentheses: either a subquery or expression list.
    ///
    /// PG's `in_expr` is either `select_with_parens` or `'(' expr_list ')'`.
    /// Both alternatives can begin with arbitrarily nested parentheses, so the
    /// grammar shares the enclosing delimiters and lets the LR state after a
    /// completed inner form decide whether a comma continues an expression list
    /// or a set operator continues a grouped query.
    ///
    /// The bare `Subquery` branch still wins on its non-`(` leading tokens
    /// (`SELECT`, `VALUES`, `TABLE`, `WITH`), where only `Subquery` has an LR
    /// action.
    ///
    /// This keeps both the expression-list form (`IN ((SELECT 1), (SELECT 2))`)
    /// and a grouped set query (`IN ((SELECT 1) UNION SELECT 2)`) reachable
    /// without declaration-order priority or parser-specific source scanning.
    #[derive(Debug)]
    pub enum InContent {
        Exprs(#[sep(COMMA)] one_or_many!(Expr)),
        Subquery(boxed!(Subquery)),
    }
}

recursa::ast_node! {
    /// `IN (expr, ...)` or `IN (subquery)` postfix suffix.
    #[derive(Debug, derive_more :: Deref)]
    pub struct InList(
        #[tok(LPAREN, this, RPAREN)]
        #[deref]
        pub InContent,
    );
}

recursa::ast_node! {
    /// A single typmod argument: an optionally-signed integer literal. Postgres'
    /// gram.y allows `expr_list` here, but the corpus only exercises signed
    /// integers (e.g. `numeric(3, -6)` in numeric.sql), so we model only that
    /// shape. A leading `+` or `-` is permitted to mirror PG's behavior.
    #[derive(Debug, PartialEq, Eq)]
    pub struct TypeModifierArg {
        pub sign: Option<TypeModifierSign>,
        pub value: literal::IntegerLit,
    }
}

recursa::ast_node! {
    /// Leading sign of a typmod argument.
    #[derive(Debug, PartialEq, Eq)]
    pub enum TypeModifierSign {
        #[tok(MINUS)]
        Neg,
        #[tok(PLUS)]
        Pos,
    }
}

recursa::ast_node! {
    /// Parenthesized precision/scale for type names: `(10,2)`, `(3)`, `(3,-6)`.
    #[derive(Debug, PartialEq, Eq, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct TypePrecision(
        #[sep(COMMA)]
        #[deref]
        pub zero_or_many!(TypeModifierArg),
    );
}

recursa::ast_node! {
    /// Type name for casts.
    #[derive(Debug, PartialEq, Eq)]
    pub enum TypeName {
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
        Ident(TypeNameIdent),
    }
}

recursa::ast_node! {
    /// Identifier-spelled type name using the type-name-specific admission set.
    /// Fixed legacy spellings are excluded so they retain their public enum
    /// variants; `json` is included despite its `COL_NAME` keyword category.
    #[derive(Debug)]
    pub struct TypeNameIdent {
        #[sep(DOT)]
        pub parts: one_or_many!(crate::tokens::type_name_ident),
    }
}

impl<'input> TypeNameIdent<'input> {
    pub fn object(&self) -> &str {
        self.parts.last().text()
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

recursa::ast_node! {
    /// Boolean test suffix: the part after `IS` in `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`.
    ///
    /// NOT variants are listed first so the combined peek regex disambiguates
    /// via longest match (e.g., `NOT TRUE` is longer than `TRUE`).
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// Unicode normalisation form keyword — gram.y `unicode_normal_form`.
    /// Used by `expr IS [NOT] [NFx] NORMALIZED` and `NORMALIZE(expr, NFx)`.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// Tail of `expr IS [NOT] [NFx] NORMALIZED` — the `[NOT] [NFx] NORMALIZED`
    /// part after the leading `IS`. Modelled as an enum so the postfix-Pratt
    /// `IsNormalized(_, IS, IsNormalizedTail)` can dispatch on the second token.
    ///
    /// Variant ordering: NOT-leading forms first (longer prefix), and within
    /// each NOT/non-NOT bucket the form-prefixed variants come before the bare
    /// `NORMALIZED` so the peek regex prefers the longer match.
    #[derive(Debug)]
    pub enum IsNormalizedTail {
        NotForm(IsNotFormNormalizedTail),
        #[tok(NOT, NORMALIZED)]
        Not,
        Form(IsFormNormalizedTail),
        #[tok(NORMALIZED)]
        Plain,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct IsFormNormalizedTail {
        #[tok(this, NORMALIZED)]
        pub form: UnicodeNormalForm,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct IsNotFormNormalizedTail {
        #[tok(NOT, this, NORMALIZED)]
        pub form: UnicodeNormalForm,
    }
}

// --- Atom wrapper structs ---

recursa::ast_node! {
    /// Dotted or subscripted column reference: `table.column`,
    /// `schema.table.column`, or `schema.table.*`.
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
    #[derive(Debug)]
    pub struct QualifiedRef {
        pub table: crate::tokens::ColId,
        /// Requiring the first indirection here keeps this branch disjoint from
        /// [`ColumnRef`], while putting fields, wildcards, and subscripts in one
        /// chain mirrors gram.y's `columnref: ColId indirection` for references
        /// of any supported qualification depth.
        pub first: QualifiedRefFirstIndirection,
        pub rest: zero_or_many!(ParenthesizedIndirection),
        /// A dotted function name and a dotted column reference share the same
        /// unbounded prefix. Owning the optional call tail here lets the parser
        /// decide at the first non-indirection token instead of trying to peek
        /// past an arbitrary number of name parts.
        pub call: Option<FunctionCallTail>,
    }
}

recursa::ast_node! {
    /// The first indirection of a qualified reference must start with a dot.
    /// Later elements may also be subscripts.
    #[derive(Debug)]
    pub enum QualifiedRefFirstIndirection {
        Field(IndirectionField),
        Star(ParenthesizedDotStar),
    }
}

recursa::ast_node! {
    /// gram.y `columnref: ColId | ColId indirection` for an unqualified name:
    /// the subscripts (and the field selectors after each) belong to the
    /// column reference. Subscripts are not a postfix operator of every
    /// expression: gram.y attaches `indirection` only to `columnref`,
    /// `PARAM`, `'(' a_expr ')'` and `select_with_parens`, so `x::int[1]` is a
    /// cast to an array type and `f(x)[1]` is rejected, as PostgreSQL has it.
    #[derive(Debug)]
    pub struct ColumnRef {
        pub name: crate::tokens::ColId,
        pub subscripts: zero_or_many!(SubscriptIndirection),
    }
}

recursa::ast_node! {
    /// gram.y `PARAM opt_indirection`.
    #[derive(Debug)]
    pub struct PositionalParam {
        #[lex(matcher)]
        pub param: literal::DollarNum,
        pub subscripts: zero_or_many!(SubscriptIndirection),
    }
}

recursa::ast_node! {
    /// Window specification: `OVER window_name` or `OVER (inline_spec)`.
    #[derive(Debug)]
    #[tok(OVER, this)]
    pub struct WindowSpec {
        #[pretty(break_before = soft)]
        pub body: WindowSpecBody,
    }
}

recursa::ast_node! {
    /// Body of an OVER clause.
    ///
    /// Variant ordering: Inline (starts with `(`) before Named (starts with an
    /// identifier). They start with different tokens so peek disambiguation is
    /// trivial.
    #[derive(Debug)]
    pub enum WindowSpecBody {
        Inline(#[tok(LPAREN, this, RPAREN)] InlineWindowSpec),
        Named(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// Interior of an inline window spec (between the parens).
    ///
    /// The optional `ref_name` is an existing-window reference (e.g.
    /// `WINDOW w2 AS (w1 ORDER BY x)`). It relies on `Option<literal::Ident>`
    /// peek-disambiguating cleanly against `PARTITION`/`ORDER`/`ROWS`/etc.
    /// because keywords are rejected by `literal::Ident`.
    #[derive(Debug)]
    pub struct InlineWindowSpec {
        pub ref_name: Option<literal::WindowRefNameIdent>,
        pub partition_by: Option<WindowPartitionBy>,
        pub order_by: Option<crate::ast::dml::select::OrderByClause>,
        pub frame: Option<WindowFrameClause>,
    }
}

recursa::ast_node! {
    /// PARTITION BY in window: `PARTITION BY expr, ...`
    #[derive(Debug)]
    #[tok(PARTITION, BY, this)]
    pub struct WindowPartitionBy {
        /// `ORDER` is reserved, so a `ColId`-qualified `QualifiedRef` cannot begin
        /// an expression with it and the overlap no longer contains it.
        /// gram.y `opt_partition_clause: PARTITION BY expr_list`: one or more.
        #[sep(COMMA)]
        pub exprs: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// Frame unit: `ROWS | RANGE | GROUPS`.
    #[derive(Debug)]
    pub enum WindowFrameUnit {
        #[tok(ROWS)]
        Rows,
        #[tok(RANGE)]
        Range,
        #[tok(GROUPS)]
        Groups,
    }
}

recursa::ast_node! {
    /// `WINDOW` frame clause: `unit (BETWEEN start AND end | bound) [EXCLUDE ...]`.
    /// The common unit prefix is represented once so the two LR productions part
    /// at `BETWEEN` versus the first bound token.
    #[derive(Debug)]
    pub struct WindowFrameClause {
        pub unit: WindowFrameUnit,
        pub body: WindowFrameBody,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum WindowFrameBody {
        Between(WindowFrameBetween),
        Single(WindowFrameSingle),
    }
}

recursa::ast_node! {
    /// `unit BETWEEN start AND end [EXCLUDE ...]`
    #[derive(Debug)]
    pub struct WindowFrameBetween {
        #[tok(BETWEEN, this)]
        pub start: WindowFrameBound,
        #[tok(AND, this)]
        pub end: WindowFrameBound,
        pub exclude: Option<WindowFrameExclude>,
    }
}

recursa::ast_node! {
    /// `unit start [EXCLUDE ...]`
    #[derive(Debug)]
    pub struct WindowFrameSingle {
        pub bound: WindowFrameBound,
        pub exclude: Option<WindowFrameExclude>,
    }
}

recursa::ast_node! {
    /// A single frame bound.
    ///
    /// `UNBOUNDED` is admitted as an expression word and therefore shares the
    /// ordinary expression-plus-direction representation. `CURRENT ROW` remains
    /// the one fixed form without a direction suffix.
    #[derive(Debug)]
    pub enum WindowFrameBound {
        #[tok(CURRENT, ROW)]
        CurrentRow,
        Offset(WindowFrameOffset),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct WindowFrameOffset {
        pub expr: boxed!(Expr),
        pub direction: WindowFrameDirection,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum WindowFrameDirection {
        #[tok(PRECEDING)]
        Preceding,
        #[tok(FOLLOWING)]
        Following,
    }
}

recursa::ast_node! {
    /// `EXCLUDE { CURRENT ROW | GROUP | TIES | NO OTHERS }` frame exclusion.
    #[derive(Debug)]
    pub struct WindowFrameExclude {
        #[tok(EXCLUDE, this)]
        pub target: WindowFrameExcludeTarget,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// PostgreSQL's `func_arg_expr`: a named or positional function argument.
    ///
    /// `VARIADIC` is deliberately not an argument variant. PostgreSQL admits it
    /// only as the sole argument or after the final comma, so the surrounding
    /// application states own that token and make its cardinality structural.
    #[derive(Debug)]
    pub enum FuncArg {
        Named(NamedFuncArg),
        Plain(boxed!(Expr)),
    }
}

recursa::ast_node! {
    /// `=>` or `:=` — the two named-argument operators PostgreSQL accepts.
    ///
    /// Variant ordering: both are distinct two-character punctuation tokens,
    /// no ambiguity.
    #[derive(Debug)]
    pub enum NamedArgOp {
        #[tok(FATARROW)]
        FatArrow,
        #[tok(COLONEQUALS)]
        ColonEquals,
    }
}

recursa::ast_node! {
    /// Named function argument: `name => value` or `name := value` (Postgres).
    #[derive(Debug)]
    pub struct NamedFuncArg {
        pub name: crate::tokens::type_function_name,
        pub arrow: NamedArgOp,
        pub value: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// The `VARIADIC func_arg_expr` at either legal variadic site.
    #[derive(Debug)]
    pub struct FunctionVariadicArgument {
        #[tok(VARIADIC, this)]
        pub argument: FuncArg,
    }
}

recursa::ast_node! {
    /// One or more ordinary PostgreSQL `func_arg_expr` values.
    ///
    /// The comma belongs to the list, not to a modifier such as `ALL` or
    /// `DISTINCT`. Keeping the repetition in its own node is significant for the
    /// LR lowering: a token attachment on a repeated field is repeated
    /// with that field, whereas these modifiers occur exactly once before the
    /// complete list.
    #[derive(Debug, derive_more :: Deref)]
    #[parse(lr_conflict(
    action = reduce,
    against = ast::shared::expr::FunctionOrdinaryArguments,
    lookahead = { RPAREN_TYPED_LA },
    expect = 1
))]
    pub struct FunctionArgumentList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(FuncArg),
    );
}

recursa::ast_node! {
    /// The sole `VARIADIC func_arg_expr` application form.
    #[derive(Debug)]
    pub struct FunctionLeadingVariadicArguments {
        pub variadic: FunctionVariadicArgument,
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
    }
}

recursa::ast_node! {
    /// The final `, VARIADIC func_arg_expr` of a function application.
    ///
    /// It immediately follows the ordinary comma-separated argument list. The
    /// list lowering recognises that its separator is this suffix's first token,
    /// and that `VARIADIC` cannot start [`FuncArg`], producing the same shared
    /// `func_arg_list ',' VARIADIC func_arg_expr` production as gram.y.
    #[derive(Debug)]
    pub struct FunctionTrailingVariadicArgument {
        #[tok(COMMA, this)]
        pub variadic: FunctionVariadicArgument,
    }
}

recursa::ast_node! {
    /// A plain non-empty argument list, optionally ending in one variadic
    /// argument, followed by the aggregate's optional inner `ORDER BY`.
    #[derive(Debug)]
    #[parse(lr_conflict(
    action = reduce,
    against = ast::shared::expr::FunctionArgumentList,
    lookahead = { RPAREN, RPAREN_SELECT_LA },
    expect = 2
))]
    pub struct FunctionOrdinaryArguments {
        #[sep(COMMA)]
        pub args: one_or_many!(FuncArg),
        pub trailing_variadic: Option<FunctionTrailingVariadicArgument>,
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
    }
}

recursa::ast_node! {
    /// `ALL` followed by a required ordinary argument list and optional order.
    #[derive(Debug)]
    pub struct FunctionAllArguments {
        #[tok(ALL, this)]
        pub args: FunctionArgumentList,
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
    }
}

recursa::ast_node! {
    /// `DISTINCT` followed by a required ordinary argument list and optional
    /// aggregate order.
    #[derive(Debug)]
    pub struct FunctionDistinctArguments {
        #[tok(DISTINCT, this)]
        pub args: FunctionArgumentList,
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
    }
}

recursa::ast_node! {
    /// The dedicated PostgreSQL `func_name '(' '*' ')'` application body.
    #[derive(Debug)]
    pub enum FunctionCallStar {
        #[tok(STAR)]
        Value,
    }
}

recursa::ast_node! {
    /// Non-empty content inside a PostgreSQL `func_application`.
    ///
    /// A wildcard is not an expression in PostgreSQL. Its exclusive alternative
    /// here prevents invalid authored states such as `f(*, 1)` or
    /// `f(DISTINCT *)`.
    #[derive(Debug)]
    pub enum FunctionCallBody {
        Star(FunctionCallStar),
        All(FunctionAllArguments),
        Distinct(FunctionDistinctArguments),
        LeadingVariadic(FunctionLeadingVariadicArguments),
        Args(FunctionOrdinaryArguments),
    }
}

recursa::ast_node! {
    /// A complete PostgreSQL `func_application` after its function name.
    #[derive(Debug)]
    pub struct FunctionCallApplication {
        pub open: FunctionCallOpen,
        pub body: Option<FunctionCallBody>,
        pub close: FunctionCallClose,
    }
}

recursa::ast_node! {
    /// A named `func_application` without aggregate/window suffixes.
    ///
    /// PostgreSQL reuses this exact grammar in function-table positions.
    #[derive(Debug)]
    pub struct FunctionApplicationExpr {
        pub name: FuncCallName,
        pub application: FunctionCallApplication,
    }
}

recursa::ast_node! {
    /// Function name in call position: gram.y `func_name`.
    ///
    /// PostgreSQL's `func_name` admits a `type_function_name` directly, while a
    /// dotted name begins with `ColId`. Keeping those two admission sets here is
    /// important: `QualifiedName` is intentionally broader and would also admit
    /// every `COL_NAME` keyword, making the dedicated XML/JSON expression forms
    /// indistinguishable from an ordinary function call.
    ///
    /// This stays its own node because gram.y writes it as one:
    /// `func_name: type_function_name | ColId indirection`. The dispatch bug
    /// that once forced the shape is gone -- recursa #127 makes enum dispatch
    /// commit where the variants part, so inlining the two names into the call
    /// variants would no longer break `f(a + b)` (see
    /// `parse_create_index_bare_func_with_operator_argument`) -- and the
    /// LR cost of the separate node, `$end` in its FOLLOW set, is now
    /// settled by declaration order without a diagnostic (recursa #125).
    /// Inlining would duplicate the call tail across both name shapes and part
    /// from gram.y, so the node stays.
    ///
    /// Variant ordering: `Qualified` needs a dotted tail, `Name` a single
    /// `type_function_name`; they share their first token and part on the dot.
    #[derive(Debug)]
    pub enum FuncCallName {
        Qualified(FuncCallQualifiedName),
        Name(crate::tokens::type_function_name),
    }
}

recursa::ast_node! {
    /// `WITHIN GROUP (ORDER BY ...)` clause for ordered-set aggregate functions.
    #[derive(Debug)]
    #[tok(WITHIN, GROUP, this)]
    pub struct WithinGroupClause {
        #[tok(LPAREN, this, RPAREN)]
        #[pretty(break_before = soft)]
        pub order_by: boxed!(crate::ast::dml::select::OrderByClause),
    }
}

recursa::ast_node! {
    /// `FILTER (WHERE condition)` clause for filtered aggregates.
    #[derive(Debug)]
    #[tok(FILTER, this)]
    pub struct FilterClause {
        #[tok(LPAREN, this, RPAREN)]
        #[pretty(break_before = soft)]
        pub body: boxed!(crate::ast::dml::select::WhereClause),
    }
}

recursa::ast_node! {
    /// A dotted function name — the `ColId indirection` arm of gram.y's
    /// `func_name`. At least one dotted tail is required so this does not
    /// overlap the unqualified `type_function_name` arm of [`FuncCallName`].
    #[derive(Debug)]
    pub struct FuncCallQualifiedName {
        pub first: crate::tokens::ColId,
        pub tail: one_or_many!(FuncCallNamePart),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct FuncCallNamePart {
        #[tok(DOT, this)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// Required opening delimiter shared by ordinary and quoted function calls.
    #[derive(Debug)]
    pub enum FunctionCallOpen {
        #[tok(LPAREN)]
        Value,
    }
}

recursa::ast_node! {
    /// Required closing delimiter shared by ordinary and quoted function calls.
    #[derive(Debug)]
    pub enum FunctionCallClose {
        #[tok(RPAREN)]
        Value,
    }
}

recursa::ast_node! {
    /// gram.y `func_expr: func_application within_group_clause filter_clause
    /// over_clause`: the argument list and every optional suffix in one shape.
    /// One shape rather than a "plain" and a "within group" tail: the two ended
    /// the argument list on the same `)` and could only be told apart after it,
    /// which a one-token parser cannot do.
    #[derive(Debug)]
    pub struct FunctionCallSuffix {
        pub open: FunctionCallOpen,
        pub body: Option<FunctionCallBody>,
        pub close: FunctionCallClose,
        pub within_group: Option<WithinGroupClause>,
        pub filter: Option<FilterClause>,
        pub window: Option<WindowSpec>,
    }
}

recursa::ast_node! {
    /// gram.y `AexprConst: func_name '(' func_arg_list opt_sort_clause ')'
    /// Sconst`: a typed literal spelled like a call, `char(20) 'x'`. The
    /// argument list is `func_arg_list` and nothing more: `*`, `DISTINCT`,
    /// `ALL`, `VARIADIC`, and `ORDER BY` belong to `func_application` and are
    /// syntax errors here. PostgreSQL's `AexprConst` includes `opt_sort_clause`
    /// only to avoid a bison reduce/reduce conflict, then rejects that clause in
    /// its rule action. pg-sql has no rule actions, so its declarative grammar
    /// must exclude it. Named function arguments are a separate pre-existing
    /// semantic-action gap and remain structurally accepted here. It parts from
    /// [`FunctionCallSuffix`] on the string after `)`.
    #[derive(Debug)]
    pub struct FunctionTypedLiteralTail {
        pub open: FunctionCallOpen,
        /// gram.y `func_arg_list`.
        pub args: FunctionArgumentList,
        pub close: FunctionCallClose,
        pub value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// What follows a function name in call position.
    ///
    /// Variant ordering: both start with `(`; `TypedLiteral` is decided by the
    /// string after the closing parenthesis.
    #[derive(Debug)]
    pub enum FunctionCallTail {
        TypedLiteral(FunctionTypedLiteralTail),
        Call(FunctionCallSuffix),
    }
}

recursa::ast_node! {
    /// Unqualified function expression with a staged, state-valid continuation.
    ///
    /// Dotted function calls are represented by [`QualifiedRef`], which owns the
    /// common unbounded dotted prefix shared with qualified column references.
    #[derive(Debug)]
    pub struct FuncCall {
        pub name: crate::tokens::type_function_name,
        pub tail: FunctionCallTail,
    }
}

recursa::ast_node! {
    /// Single identifier retained for the standalone quoted-call compatibility surface.
    ///
    /// Expression parsing admits quoted names through [`FuncCallName`] and
    /// [`Expr::Func`]; this wrapper remains public for callers that parse or
    /// construct [`QuotedFuncCall`] directly.
    #[derive(Debug)]
    pub enum QuotedFuncName {
        Name(crate::tokens::literal::Ident),
    }
}

recursa::ast_node! {
    /// `"name"(...)` as a standalone quoted-call compatibility surface.
    ///
    /// [`Expr`] routes quoted function names through [`Expr::Func`] and
    /// [`FuncCallName`]. This public type remains available to callers that use
    /// the narrower unqualified quoted-call grammar directly.
    #[derive(Debug)]
    pub struct QuotedFuncCall {
        pub name: QuotedFuncName,
        pub tail: FunctionCallTail,
    }
}

recursa::ast_node! {
    /// Content inside parentheses: either a query or a non-empty,
    /// comma-separated expression list.
    #[derive(Debug)]
    pub enum ParenContent {
        #[parse(prec = UMINUS)]
        #[parse(lr_conflict(
        action = reduce,
        against = ast::dml::values::SelectWithParens,
        lookahead = { RPAREN },
        expect = 1
    ))]
        #[parse(lr_conflict(
        action = shift,
        against = ast::dml::values::SelectWithParens,
        lookahead = { RPAREN_SELECT_LA },
        expect = 1
    ))]
        Subquery(boxed!(Subquery)),
        Exprs(#[sep(COMMA)] one_or_many!(Expr)),
    }
}

recursa::ast_node! {
    /// Terminal dot-star indirection.
    #[derive(Debug)]
    pub enum ParenthesizedDotStar {
        #[tok(DOT, STAR)]
        Value,
    }
}

recursa::ast_node! {
    /// One non-star element in the indirection chain following parenthesized
    /// content.
    #[derive(Debug)]
    pub enum ParenthesizedIndirection {
        Field(IndirectionField),
        Subscript(BracketSubscript),
        Star(ParenthesizedDotStar),
    }
}

recursa::ast_node! {
    /// Parenthesized scalar, row, or subquery content, optionally followed by an
    /// arbitrary field, wildcard, or subscript indirection chain.
    #[derive(Debug)]
    pub struct ParenthesizedExpr {
        pub open: ParenthesizedOpen,
        pub content: ParenContent,
        pub close: ParenthesizedClose,
        pub indirection: zero_or_many!(ParenthesizedIndirection),
    }
}

recursa::ast_node! {
    /// Required `:` plus the optional upper bound of an array slice.
    #[derive(Debug)]
    pub struct SubscriptSliceSuffix {
        pub colon: SubscriptColon,
        pub upper: Option<boxed!(Expr)>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SubscriptColon {
        #[tok(COLON)]
        Value,
    }
}

recursa::ast_node! {
    /// Content between subscript brackets — gram.y `indirection_el`'s
    /// `'[' a_expr ']'` and `'[' opt_slice_bound ':' opt_slice_bound ']'`
    /// (gram.y:16801), where `opt_slice_bound` is `a_expr` or empty
    /// (gram.y:16812).
    ///
    /// The empty lower bound is its own variant, as gram.y has it. It used to
    /// reach the `Bounded` path instead, by reading the colon and its upper
    /// bound as one `PsqlVariableExpr`: that gave `[:2]` and `:'name'` a single
    /// representation and so avoided an overlap between a slice colon and a
    /// psql-variable colon. With psql gone from this grammar there is no
    /// overlap to avoid, and the two bounds are simply optional the way
    /// PostgreSQL writes them.
    ///
    /// Variant ordering: only the lower-unbounded forms can begin with a colon,
    /// and no expression can, so the two are disjoint.
    #[derive(Debug)]
    pub enum BracketSubscriptValue {
        /// `[:]` and `[: upper]` — the lower `opt_slice_bound` is empty.
        LowerUnbounded(SubscriptSliceSuffix),
        /// `[lower]`, `[lower :]` and `[lower : upper]`.
        Bounded(BracketSubscriptBounds),
    }
}

recursa::ast_node! {
    /// `lower [: [upper]]` inside subscript brackets.
    #[derive(Debug)]
    pub struct BracketSubscriptBounds {
        pub lower: boxed!(Expr),
        pub slice: Option<SubscriptSliceSuffix>,
    }
}

recursa::ast_node! {
    /// Shared payload for both an index and a slice.
    #[derive(Debug)]
    pub struct BracketSubscript {
        pub open: SubscriptOpen,
        pub content: BracketSubscriptValue,
        pub close: SubscriptClose,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SubscriptOpen {
        #[tok(LBRACKET)]
        Value,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SubscriptClose {
        #[tok(RBRACKET)]
        Value,
    }
}

recursa::ast_node! {
    /// A subscript followed by the field selectors that continue PostgreSQL's
    /// `opt_indirection` chain: `arr[i]`, `arr[i].field`, `arr[i].f.g`.
    ///
    /// Further subscripts are not repeated here — the owning atom keeps a list
    /// of these, so `a[1].b[2]` is a subscript carrying `.b` followed by a
    /// second subscript. Keeping brackets out of this tail leaves the two forms
    /// with disjoint continuations.
    #[derive(Debug)]
    pub struct SubscriptIndirection {
        pub subscript: BracketSubscript,
        pub fields: zero_or_many!(IndirectionField),
    }
}

recursa::ast_node! {
    /// `.field` accessor in an indirection chain.
    #[derive(Debug)]
    pub struct IndirectionField {
        #[tok(DOT, this)]
        pub name: literal::AliasName,
    }
}

recursa::ast_node! {
    /// One element of an indirection chain on an `INSERT` / `UPDATE` column
    /// target: `[idx]`, `[low:high]`, or `.field` (Postgres `opt_indirection`).
    ///
    /// Variant ordering: `Slice` before `Index` — both open with `[`, the
    /// colon-containing slice form is tried first.
    #[derive(Debug)]
    pub enum IndirectionEl {
        Subscript(BracketSubscript),
        Field(IndirectionField),
    }
}

// Operators of PostgreSQL's `subquery_Op` production, one enum per
// precedence level of pg-sql's `Expr`: gram.y decides the shift before a
// quantified comparison by the operator token's own precedence, so each
// `Expr::QuantifiedComparison*` variant carries the level of the same
// token's infix variant. Together the six enums cover `OperatorName`,
// `OPERATOR(...)` and the LIKE family exactly once.

recursa::ast_node! {
    /// `subquery_Op` at the level of pg-sql's comparison operators (binding
    /// power 5): gram.y `MathOp`'s `< > = <= >= <>` and the operators pg-sql's
    /// `Expr` parses at that level.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `subquery_Op`'s `LIKE | NOT_LA LIKE | ILIKE | NOT_LA ILIKE`, at the level
    /// of `Expr::Like` (binding power 60).
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `subquery_Op` at gram.y's generic `Op` level (binding power 80): `||`,
    /// `^@`, every spelling that is only a prefix operator elsewhere in `Expr`,
    /// the multi-character custom operators, and `OPERATOR(any_operator)`
    /// (`%left Op OPERATOR`).
    #[derive(Debug)]
    pub enum QuantifiedOpOperator {
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
        Custom(literal::CustomOp),
        Decorated(QuantifiedDecoratedOperator),
    }
}

recursa::ast_node! {
    /// `subquery_Op` at the level of `+` and `-` (binding power 100), which in
    /// pg-sql also holds the bitwise and JSON operators.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// `subquery_Op` at the level of `*`, `/` and `%` (binding power 110).
    #[derive(Debug)]
    pub enum QuantifiedMulOperator {
        #[tok(STAR)]
        Star,
        #[tok(SLASH)]
        Slash,
        #[tok(PERCENT)]
        Percent,
    }
}

recursa::ast_node! {
    /// `subquery_Op` at the level of `^` (binding power 130).
    #[derive(Debug)]
    pub enum QuantifiedPowOperator {
        #[tok(CARET)]
        Caret,
    }
}

recursa::ast_node! {
    /// `OPERATOR(any_operator)` in a quantified comparison.
    #[derive(Debug)]
    pub struct QuantifiedDecoratedOperator {
        #[tok(OPERATOR, LPAREN, this, RPAREN)]
        pub name: crate::ast::shared::names::QualifiedOperatorName,
    }
}

recursa::ast_node! {
    /// `ANY`, `SOME`, or `ALL` following a comparison operator.
    #[derive(Debug)]
    pub enum QuantifiedComparisonKind {
        #[tok(ANY)]
        Any,
        #[tok(SOME)]
        Some,
        #[tok(ALL)]
        All,
    }
}

recursa::ast_node! {
    /// The single expression or query inside a quantified comparison.
    #[derive(Debug)]
    pub enum QuantifiedComparisonOperand {
        Subquery(boxed!(Subquery)),
        Expr(boxed!(Expr)),
    }
}

recursa::ast_node! {
    /// `{ANY|SOME|ALL} (expression-or-query)` — gram.y `sub_type '(' a_expr ')'`
    /// and `sub_type select_with_parens`, shared by every quantified variant.
    #[derive(Debug)]
    pub struct QuantifiedComparisonTail {
        pub kind: QuantifiedComparisonKind,
        #[tok(LPAREN, this, RPAREN)]
        pub operand: QuantifiedComparisonOperand,
    }
}

recursa::ast_node! {
    /// `comparison operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
    #[derive(Debug)]
    #[pretty(break_before = soft)]
    pub struct QuantifiedComparisonCmpSuffix {
        pub operator: QuantifiedCmpOperator,
        pub tail: QuantifiedComparisonTail,
    }
}

recursa::ast_node! {
    /// `LIKE-family operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
    #[derive(Debug)]
    #[pretty(break_before = soft)]
    pub struct QuantifiedComparisonLikeSuffix {
        pub operator: QuantifiedLikeOperator,
        pub tail: QuantifiedComparisonTail,
    }
}

recursa::ast_node! {
    /// `generic operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
    #[derive(Debug)]
    #[pretty(break_before = soft)]
    pub struct QuantifiedComparisonOpSuffix {
        pub operator: QuantifiedOpOperator,
        pub tail: QuantifiedComparisonTail,
    }
}

recursa::ast_node! {
    /// `additive operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
    #[derive(Debug)]
    #[pretty(break_before = soft)]
    pub struct QuantifiedComparisonAddSuffix {
        pub operator: QuantifiedAddOperator,
        pub tail: QuantifiedComparisonTail,
    }
}

recursa::ast_node! {
    /// `multiplicative operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
    #[derive(Debug)]
    #[pretty(break_before = soft)]
    pub struct QuantifiedComparisonMulSuffix {
        pub operator: QuantifiedMulOperator,
        pub tail: QuantifiedComparisonTail,
    }
}

recursa::ast_node! {
    /// `exponentiation operator {ANY|SOME|ALL} (expression-or-query)` after a left operand.
    #[derive(Debug)]
    #[pretty(break_before = soft)]
    pub struct QuantifiedComparisonPowSuffix {
        pub operator: QuantifiedPowOperator,
        pub tail: QuantifiedComparisonTail,
    }
}

recursa::ast_node! {
    /// EXISTS subquery: `EXISTS (SELECT ...)`
    #[derive(Debug)]
    pub struct ExistsExpr {
        #[tok(EXISTS, LPAREN, this, RPAREN)]
        pub subquery: boxed!(Subquery),
    }
}

recursa::ast_node! {
    /// One element of an `ARRAY[...]` constructor: either an ordinary
    /// expression or a nested bracketed sub-list (for multi-dimensional
    /// literals like `ARRAY[[1,2],[3,4]]`).
    ///
    /// Variant ordering: `Nested` leads with `[`, which no expression atom
    /// does, so dispatch is unambiguous.
    #[derive(Debug)]
    pub enum ArrayElement {
        Nested(NestedArrayElements),
        Expr(boxed!(Expr)),
    }
}

recursa::ast_node! {
    /// One bracketed sub-list inside a multi-dimensional `ARRAY[...]` literal.
    #[derive(Debug)]
    #[tok(LBRACKET, this, RBRACKET)]
    pub struct NestedArrayElements {
        #[sep(COMMA)]
        pub elements: zero_or_many!(ArrayElement),
    }
}

recursa::ast_node! {
    /// ARRAY bracket constructor: `ARRAY[expr, ...]`, including the
    /// multi-dimensional form `ARRAY[[1,2],[3,4]]` and the empty `ARRAY[]`.
    ///
    /// PostgreSQL's `array_expr` keeps `'[' ']'` as its own alternative, so the
    /// element list is nullable. The `ARRAY` keyword still leads the node, which
    /// keeps the opening bracket visible to FIRST-k analysis, exactly as the
    /// nested `NestedArrayElements` list above already relies on.
    #[derive(Debug)]
    #[tok(ARRAY, LBRACKET, this, RBRACKET)]
    pub struct ArrayBracket {
        #[sep(COMMA)]
        pub elements: zero_or_many!(ArrayElement),
    }
}

recursa::ast_node! {
    /// ARRAY subquery constructor: `ARRAY(subquery)`
    #[derive(Debug)]
    pub struct ArraySubquery {
        #[tok(ARRAY, LPAREN, this, RPAREN)]
        pub subquery: boxed!(Subquery),
    }
}

recursa::ast_node! {
    /// ARRAY constructor: `ARRAY[expr, ...]` or `ARRAY(subquery)`
    ///
    /// Variant ordering: Bracket (`ARRAY[`) has a longer first_pattern than
    /// Subquery (`ARRAY(`) because `[` is a different token than `(`.
    #[derive(Debug)]
    pub enum ArrayExpr {
        Bracket(ArrayBracket),
        Subquery(ArraySubquery),
    }
}

recursa::ast_node! {
    /// `GROUPING(expr, ...)` — the grouping-set membership function.
    ///
    /// gram.y keeps this as `func_expr_common_subexpr: GROUPING '(' expr_list
    /// ')'`. `GROUPING` is a `COL_NAME` keyword, so it is a `ColId` but not a
    /// `type_function_name`: it can be a bare column reference, never an
    /// ordinary function name, and the call form needs its own production.
    #[derive(Debug)]
    #[tok(GROUPING, LPAREN, this, RPAREN)]
    pub struct GroupingCall {
        #[sep(COMMA)]
        pub args: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// ROW constructor: `ROW(expr, ...)` or the empty `ROW()`.
    ///
    /// PostgreSQL's `row` production keeps `ROW '(' expr_list ')'` and
    /// `ROW '(' ')'` as separate alternatives, so the field list is nullable
    /// here. The `ROW` keyword still leads the node, so the empty form does not
    /// hide the opening parenthesis from FIRST-k analysis.
    #[derive(Debug)]
    #[tok(ROW, LPAREN, this, RPAREN)]
    pub struct RowExpr {
        #[sep(COMMA)]
        pub values: Option<one_or_many!(Expr)>,
    }
}

recursa::ast_node! {
    /// `WHEN cond THEN result` arm of a CASE expression.
    #[derive(Debug)]
    pub struct CaseWhenArm {
        #[tok(WHEN, this)]
        pub condition: boxed!(Expr),
        #[tok(THEN, this)]
        pub result: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `ELSE result` clause of a CASE expression.
    #[derive(Debug)]
    pub struct CaseElse {
        #[tok(ELSE, this)]
        pub result: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// Searched CASE body: `WHEN cond THEN result [...] [ELSE result]`.
    #[derive(Debug)]
    pub struct CaseSearched {
        pub first_arm: CaseWhenArm,
        pub rest_arms: zero_or_many!(CaseWhenArm),
        pub else_clause: Option<CaseElse>,
    }
}

recursa::ast_node! {
    /// Simple CASE body: `operand WHEN val THEN result [...] [ELSE result]`.
    #[derive(Debug)]
    pub struct CaseSimple {
        pub operand: boxed!(Expr),
        pub first_arm: CaseWhenArm,
        pub rest_arms: zero_or_many!(CaseWhenArm),
        pub else_clause: Option<CaseElse>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum CaseBody {
        Searched(CaseSearched),
        Simple(CaseSimple),
    }
}

recursa::ast_node! {
    /// CASE expression with its common `CASE` / `END` delimiters factored out.
    #[derive(Debug)]
    pub struct CaseExpr {
        #[tok(CASE, this, END)]
        pub body: CaseBody,
    }
}

recursa::ast_node! {
    /// One `opt_array_bounds` element: `[]` or `[N]`.
    ///
    /// Postgres syntax: `Typename opt_array_bounds` allows arbitrary repetition
    /// of either form (`int4[]`, `int4[1]`, `varchar(4)[2][3]`, …). Variant
    /// ordering: `Sized` (`[N]`, 3 tokens) before `Empty` (`[]`, 2 tokens) so
    /// longest-match-wins picks the longer form when an integer literal is
    /// present between the brackets.
    #[derive(Debug, PartialEq, Eq)]
    pub enum ArraySuffix {
        Sized(ArraySuffixSized),
        Empty(ArraySuffixEmpty),
    }
}

recursa::ast_node! {
    /// `[N]` array bound.
    #[derive(Debug, PartialEq, Eq)]
    pub struct ArraySuffixSized {
        #[tok(LBRACKET, this, RBRACKET)]
        pub bounds: literal::IntegerLit,
    }
}

recursa::ast_node! {
    /// `[]` array suffix (unbounded).
    #[derive(Debug, PartialEq, Eq)]
    pub enum ArraySuffixEmpty {
        #[tok(LBRACKET, RBRACKET)]
        Value,
    }
}

recursa::ast_node! {
    /// Cast type with a base-specific modifier and zero-or-more array suffixes:
    /// `numeric(10,0)`, `timestamp with time zone`, `interval day to minute`,
    /// `integer[]`, `int4[][][]`, `varchar(4)[2][3]`.
    #[derive(Debug, PartialEq, Eq)]
    pub struct CastType {
        pub head: CastTypeHead,
        pub array_suffixes: zero_or_many!(ArraySuffix),
        /// PG gram.y also accepts `SimpleTypename ARRAY` and
        /// `SimpleTypename ARRAY '[' Iconst ']'` — the keyword form for
        /// declaring an array type (e.g. `integer ARRAY[4]`, `text ARRAY`).
        /// In practice this is mutually exclusive with `array_suffixes`, but the
        /// grammar admits the suffix appearing AFTER the keyword form, so the
        /// field is parsed last.
        pub array_kw_suffix: Option<ArrayKwSuffix>,
    }
}

recursa::ast_node! {
    /// The modifier-bearing portion of a cast type.
    ///
    /// PostgreSQL gives date/time and interval types dedicated productions. In
    /// particular, `WITH/WITHOUT TIME ZONE` is not a suffix on an arbitrary type;
    /// keeping it structural prevents a following `WITH UNIQUE KEYS` JSON clause
    /// from being consumed as part of a `json` cast.
    #[derive(Debug, PartialEq, Eq)]
    pub enum CastTypeHead {
        DateTime(DateTimeCastType),
        Interval(IntervalCastType),
        General(GeneralCastType),
    }
}

recursa::ast_node! {
    /// `TIMESTAMP` or `TIME`, with their optional precision and timezone suffix.
    #[derive(Debug, PartialEq, Eq)]
    pub struct DateTimeCastType {
        pub base: DateTimeCastTypeName,
        pub precision: Option<TypePrecision>,
        pub tz: Option<TimeZoneQualifier>,
    }
}

recursa::ast_node! {
    #[derive(Debug, PartialEq, Eq)]
    pub enum DateTimeCastTypeName {
        #[tok(TIMESTAMP)]
        Timestamp,
        #[tok(TIME)]
        Time,
    }
}

recursa::ast_node! {
    /// `INTERVAL`, optionally with either a full-type precision or a field range.
    #[derive(Debug, PartialEq, Eq)]
    #[tok(INTERVAL, this)]
    pub struct IntervalCastType {
        pub modifier: Option<IntervalCastTypeModifier>,
    }
}

recursa::ast_node! {
    #[derive(Debug, PartialEq, Eq)]
    pub enum IntervalCastTypeModifier {
        Precision(TypePrecision),
        Qualifier(IntervalQualifier),
    }
}

recursa::ast_node! {
    /// A type without the date/time- or interval-specific suffix grammar.
    #[derive(Debug, PartialEq, Eq)]
    pub struct GeneralCastType {
        pub base: GeneralCastTypeName,
        #[presence(VARYING)]
        /// `VARYING` modifier (e.g., `BIT VARYING`, `CHARACTER VARYING`).
        /// Always precedes the precision parens.
        pub varying: bool,
        pub precision: Option<TypePrecision>,
    }
}

recursa::ast_node! {
    /// A cast base without `TIME`, `TIMESTAMP`, or `INTERVAL`.
    ///
    /// This mirrors [`TypeName`] for the general PostgreSQL type production while
    /// making the three suffix-bearing families disjoint by construction.
    #[derive(Debug, PartialEq, Eq)]
    pub enum GeneralCastTypeName {
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
        Ident(TypeNameIdent),
    }
}

recursa::ast_node! {
    /// `ARRAY` or `ARRAY[N]` post-type-name array suffix
    /// (PG gram.y: `SimpleTypename ARRAY | SimpleTypename ARRAY '[' Iconst ']'`).
    #[derive(Debug, PartialEq, Eq)]
    #[tok(ARRAY, this)]
    pub struct ArrayKwSuffix {
        pub bound: Option<ArraySuffixSized>,
    }
}

recursa::ast_node! {
    /// NOT IN list: `expr NOT IN (val, ...)` suffix.
    #[derive(Debug)]
    pub struct NotInSuffix {
        #[tok(NOT, IN, this)]
        pub list: InList,
    }
}

recursa::ast_node! {
    /// Fixed-keyword name accepted by PostgreSQL's function-style typed literal
    /// syntax.
    ///
    /// These are `COL_NAME` keywords and therefore cannot also be generic
    /// function names.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// Function-style typed literal for a fixed-keyword type. The optional typmod
    /// list is kept on this same node so `numeric '1'` and
    /// `numeric(10, 2) '1.00'` share their prefix honestly.
    #[derive(Debug)]
    pub struct FixedTypeCastFunc {
        pub type_name: FixedTypeCastFuncName,
        #[presence(VARYING)]
        pub varying: bool,
        pub typmods: Option<TypePrecision>,
        pub value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// Function-style typed literal for an identifier-spelled type without
    /// typmods: `bool 'value'`, `text 'hello'`, or `double precision 'value'`.
    ///
    /// The payload is PostgreSQL's `Sconst` and nothing else, as
    /// `AexprConst: func_name Sconst` has it: nothing stands between the name
    /// and the string, and in particular no colon. pg-sql used to admit psql's
    /// `:'var'` here and in the keyword-named form, which collided with the
    /// SQL/JSON `key : value` entry of `JSON_OBJECT` and `JSON_OBJECTAGG`; psql
    /// now has its own grammar, so a colon after a type name is only ever that
    /// entry separator.
    #[derive(Debug)]
    pub struct NamedTypeCastFunc {
        /// gram.y `AexprConst: func_name Sconst` — nothing stands between the
        /// name and the string. `double precision '1'` is
        /// `FixedTypeCastFuncName::DoublePrecision`; a `PRECISION` admitted
        /// here would follow `type_function_name` where gram.y never has it,
        /// and `precision` is also an operator class after a column in an
        /// index element, which the LR parser could not tell apart.
        pub type_name: crate::tokens::type_function_name,
        pub value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// Function-style typed literal for `json`: `json '{"a": 1}'`.
    ///
    /// `JSON` is a `COL_NAME` keyword with its own `JsonType` production in
    /// gram.y, so it reaches neither `NamedTypeCastFunc` (whose name is a
    /// `type_function_name`) nor the typmod form. `JsonType` takes no type
    /// modifiers, so none are modelled here — which also keeps this node
    /// disjoint from the `JSON ( ... )` SQL/JSON value constructor at the second
    /// token.
    #[derive(Debug)]
    pub struct JsonTypeCastFunc {
        #[tok(JSON, this)]
        pub value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// Function-style typed literal. Fixed-keyword type names can carry typmods
    /// directly; identifier-spelled types with typmods use
    /// [`FunctionCallTail::TypedLiteral`], where the complete call/typmod prefix
    /// is shared.
    ///
    /// Variant ordering is immaterial: `Fixed` leads with one of its own
    /// keywords, `Json` with `JSON`, and `Named` with a `type_function_name`,
    /// which admits neither.
    #[derive(Debug)]
    pub enum TypeCastFunc {
        Fixed(FixedTypeCastFunc),
        Json(JsonTypeCastFunc),
        Named(NamedTypeCastFunc),
    }
}

recursa::ast_node! {
    /// `WITH TIME ZONE` or `WITHOUT TIME ZONE` suffix for `TIMESTAMP`/`TIME`.
    #[derive(Debug, PartialEq, Eq)]
    pub enum TimeZoneQualifier {
        #[tok(WITH, TIME, ZONE)]
        With,
        #[tok(WITHOUT, TIME, ZONE)]
        Without,
    }
}

recursa::ast_node! {
    /// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`.
    #[derive(Debug)]
    #[tok(TIMESTAMP, this)]
    pub struct TimestampLit {
        /// Optional precision, e.g., `timestamp(6)`.
        pub precision: Option<TypePrecision>,
        pub tz: Option<TimeZoneQualifier>,
        pub value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `TIME [WITH|WITHOUT TIME ZONE] 'string'`.
    #[derive(Debug)]
    #[tok(TIME, this)]
    pub struct TimeLit {
        /// Optional precision, e.g., `time(2)`.
        pub precision: Option<TypePrecision>,
        pub tz: Option<TimeZoneQualifier>,
        pub value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `SECOND [(p)]` — the SECOND keyword with optional fractional-second
    /// precision. Used in interval qualifiers like `SECOND(2)` or
    /// `DAY TO SECOND(2)`.
    #[derive(Debug, PartialEq, Eq)]
    #[tok(SECOND, this)]
    pub struct SecondWithPrecision {
        pub precision: Option<TypePrecision>,
    }
}

recursa::ast_node! {
    /// Optional qualifier after `INTERVAL 'str'`.
    ///
    /// Variant ordering: multi-keyword `X TO Y` forms must come before the
    /// single-keyword forms so longest-match-wins picks the fuller qualifier
    /// when available. `*ToSecond` variants use `SecondWithPrecision` which
    /// allows optional `(p)` precision.
    #[derive(Debug, PartialEq, Eq)]
    pub enum IntervalQualifier {
        #[tok(YEAR, TO, MONTH)]
        YearToMonth,
        #[tok(DAY, TO, HOUR)]
        DayToHour,
        #[tok(DAY, TO, MINUTE)]
        DayToMinute,
        DayToSecond(#[tok(DAY, TO, this)] SecondWithPrecision),
        #[tok(HOUR, TO, MINUTE)]
        HourToMinute,
        HourToSecond(#[tok(HOUR, TO, this)] SecondWithPrecision),
        MinuteToSecond(#[tok(MINUTE, TO, this)] SecondWithPrecision),
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
        Second(SecondWithPrecision),
    }
}

recursa::ast_node! {
    /// `INTERVAL 'str' [qualifier]`.
    #[derive(Debug)]
    pub struct IntervalLit {
        pub interval: IntervalKeyword,
        /// Optional precision, e.g. `interval(2)` or `interval(0)`.
        pub precision: Option<TypePrecision>,
        pub value: literal::StringLit,
        pub qualifier: Option<IntervalQualifier>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum IntervalKeyword {
        #[tok(INTERVAL)]
        Value,
    }
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

recursa::ast_node! {
    /// A `name [AS alias]` argument to `xmlattributes` / `xmlforest`.
    #[derive(Debug)]
    pub struct XmlNamedArg {
        pub value: boxed!(Expr),
        pub alias: Option<XmlNamedArgAlias>,
    }
}

recursa::ast_node! {
    /// `AS alias` suffix on an XML named argument.
    #[derive(Debug)]
    pub struct XmlNamedArgAlias {
        #[tok(AS, this)]
        pub name: literal::AliasName,
    }
}

recursa::ast_node! {
    /// `xmlattributes(expr [AS alias], ...)` — used as a positional argument
    /// to `xmlelement`, but also can be parsed standalone.
    #[derive(Debug)]
    #[tok(XMLATTRIBUTES, LPAREN, this, RPAREN)]
    pub struct XmlAttributes {
        #[sep(COMMA)]
        pub args: one_or_many!(XmlNamedArg),
    }
}

recursa::ast_node! {
    /// Optional `, xmlattributes(...) [, content_exprs]` tail of `xmlelement`.
    #[derive(Debug)]
    pub struct XmlElementAttrsTail {
        #[tok(COMMA, this)]
        pub attrs: XmlAttributes,
        pub content: Option<XmlElementContentTail>,
    }
}

recursa::ast_node! {
    /// Optional `, content_exprs` tail of `xmlelement`.
    #[derive(Debug)]
    #[tok(COMMA, this)]
    pub struct XmlElementContentTail {
        #[sep(COMMA)]
        pub exprs: one_or_many!(Expr),
    }
}

/// Body of `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
///
/// Variant ordering: the `WithAttrs` form starts with `, xmlattributes(`
/// (longer match) and must be tried before `WithContent` which starts with
/// just `,`. Both trail an `xmlelement(NAME ident` head.
pub type XmlElementTail<'input> = XmlElementContentTail<'input>;

recursa::ast_node! {
    /// Inner contents of an `xmlelement(...)` call.
    #[derive(Debug)]
    pub struct XmlElementInner {
        #[tok(NAME, this)]
        pub element_name: literal::AliasName,
        pub tail: Option<XmlElementTail>,
    }
}

recursa::ast_node! {
    /// `xmlelement(NAME ident [, xmlattributes(...)] [, content_exprs])`.
    #[derive(Debug)]
    pub struct XmlElement {
        #[tok(XMLELEMENT, LPAREN, this, RPAREN)]
        pub inner: XmlElementInner,
    }
}

recursa::ast_node! {
    /// `xmlforest(expr [AS alias], ...)`.
    #[derive(Debug)]
    #[tok(XMLFOREST, LPAREN, this, RPAREN)]
    pub struct XmlForest {
        #[sep(COMMA)]
        pub args: one_or_many!(XmlNamedArg),
    }
}

recursa::ast_node! {
    /// `xmlpi(NAME ident [, content])`.
    #[derive(Debug)]
    pub struct XmlPi {
        #[tok(XMLPI, LPAREN, this, RPAREN)]
        pub inner: XmlPiInner,
    }
}

recursa::ast_node! {
    /// Inner contents of an `xmlpi(...)` call.
    #[derive(Debug)]
    pub struct XmlPiInner {
        #[tok(NAME, this)]
        pub target: literal::AliasName,
        pub content: Option<XmlPiContentTail>,
    }
}

recursa::ast_node! {
    /// Optional `, content_expr` tail of `xmlpi`.
    #[derive(Debug)]
    pub struct XmlPiContentTail {
        #[tok(COMMA, this)]
        pub expr: boxed!(Expr),
    }
}

// --- More XML function atoms: XMLSERIALIZE / XMLPARSE / XMLROOT / XMLEXISTS ---
//
// Like `xmlelement` etc. these use keyword-laced syntax (`DOCUMENT`/`CONTENT`,
// `VERSION`, `PASSING BY REF`, …) that a plain `FuncCall` cannot express.

recursa::ast_node! {
    /// `DOCUMENT` / `CONTENT` — the XML value category in `XMLSERIALIZE` / `XMLPARSE`.
    #[derive(Debug)]
    pub enum XmlDocOrContent {
        #[tok(DOCUMENT)]
        Document,
        #[tok(CONTENT)]
        Content,
    }
}

recursa::ast_node! {
    /// `INDENT` / `NO INDENT` — output indentation option of `XMLSERIALIZE`.
    ///
    /// Variant ordering: `NoIndent` (`NO INDENT`, two tokens) before `Indent`.
    #[derive(Debug)]
    pub enum XmlIndentOption {
        #[tok(NO, INDENT)]
        NoIndent,
        #[tok(INDENT)]
        Indent,
    }
}

recursa::ast_node! {
    /// Inner of `XMLSERIALIZE ( {DOCUMENT|CONTENT} ‹expr› AS ‹type› [[NO] INDENT] )`.
    #[derive(Debug)]
    pub struct XmlSerializeInner {
        pub which: XmlDocOrContent,
        pub value: boxed!(Expr),
        #[tok(AS, this)]
        pub ty: CastType,
        pub indent: Option<XmlIndentOption>,
    }
}

recursa::ast_node! {
    /// `XMLSERIALIZE ( {DOCUMENT|CONTENT} ‹expr› AS ‹type› [[NO] INDENT] )`.
    #[derive(Debug)]
    pub struct XmlSerialize {
        #[tok(XMLSERIALIZE, LPAREN, this, RPAREN)]
        pub inner: XmlSerializeInner,
    }
}

recursa::ast_node! {
    /// Inner of `XMLPARSE ( {DOCUMENT|CONTENT} ‹expr› )`.
    #[derive(Debug)]
    pub struct XmlParseInner {
        pub which: XmlDocOrContent,
        pub value: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `XMLPARSE ( {DOCUMENT|CONTENT} ‹expr› )`.
    #[derive(Debug)]
    pub struct XmlParse {
        #[tok(XMLPARSE, LPAREN, this, RPAREN)]
        pub inner: XmlParseInner,
    }
}

recursa::ast_node! {
    /// `VERSION {‹expr› | NO VALUE}` — the version argument of `XMLROOT`.
    ///
    /// Variant ordering: `NoValue` (`NO VALUE`) before the catch-all `Expr`.
    #[derive(Debug)]
    pub enum XmlVersionValue {
        #[tok(NO, VALUE)]
        NoValue,
        Expr(boxed!(Expr)),
    }
}

recursa::ast_node! {
    /// `VERSION {…}` clause of `XMLROOT`.
    #[derive(Debug)]
    pub struct XmlRootVersion {
        #[tok(VERSION, this)]
        pub value: XmlVersionValue,
    }
}

recursa::ast_node! {
    /// `STANDALONE {YES | NO [VALUE]}`.
    ///
    /// Variant ordering: `NoValue` (`NO VALUE`) before bare `No`.
    #[derive(Debug)]
    pub enum XmlStandaloneValue {
        #[tok(YES)]
        Yes,
        #[tok(NO, VALUE)]
        NoValue,
        #[tok(NO)]
        No,
    }
}

recursa::ast_node! {
    /// `, STANDALONE {…}` clause of `XMLROOT`.
    #[derive(Debug)]
    pub struct XmlRootStandalone {
        #[tok(COMMA, STANDALONE, this)]
        pub value: XmlStandaloneValue,
    }
}

recursa::ast_node! {
    /// Inner of `XMLROOT ( ‹xml› , VERSION {…} [, STANDALONE {…}] )`.
    #[derive(Debug)]
    pub struct XmlRootInner {
        pub value: boxed!(Expr),
        #[tok(COMMA, this)]
        pub version: XmlRootVersion,
        pub standalone: Option<XmlRootStandalone>,
    }
}

recursa::ast_node! {
    /// `XMLROOT ( ‹xml› , VERSION {…} [, STANDALONE {…}] )`.
    #[derive(Debug)]
    pub struct XmlRoot {
        #[tok(XMLROOT, LPAREN, this, RPAREN)]
        pub inner: XmlRootInner,
    }
}

recursa::ast_node! {
    /// `BY REF` / `BY VALUE` qualifier of an `XMLEXISTS` / `XMLTABLE` PASSING clause.
    #[derive(Debug)]
    pub enum XmlRefOrValue {
        #[tok(REF)]
        Ref,
        #[tok(VALUE)]
        Value,
    }
}

recursa::ast_node! {
    /// `BY {REF|VALUE}` qualifier.
    #[derive(Debug)]
    pub struct XmlPassingBy {
        #[tok(BY, this)]
        pub which: XmlRefOrValue,
    }
}

recursa::ast_node! {
    /// Inner of `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
    #[derive(Debug)]
    pub struct XmlExistsInner {
        pub xpath: boxed!(Expr),
        pub passing: XmlExistsPassing,
    }
}

recursa::ast_node! {
    /// Required `PASSING` clause of `XMLEXISTS`.
    #[derive(Debug)]
    pub struct XmlExistsPassing {
        #[tok(PASSING, this)]
        pub document: XmlExistsDocument,
        pub by_after: Option<XmlPassingBy>,
    }
}

recursa::ast_node! {
    /// The document expression, optionally introduced by `BY REF` / `BY VALUE`.
    #[derive(Debug)]
    pub enum XmlExistsDocument {
        Qualified(XmlExistsQualifiedDocument),
        Plain(boxed!(Expr)),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct XmlExistsQualifiedDocument {
        pub by: XmlPassingBy,
        pub doc: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `XMLEXISTS ( ‹xpath› PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] )`.
    #[derive(Debug)]
    pub struct XmlExists {
        #[tok(XMLEXISTS, LPAREN, this, RPAREN)]
        pub inner: XmlExistsInner,
    }
}

recursa::ast_node! {
    /// The tail of an `IS DOCUMENT` predicate: `[NOT] DOCUMENT`.
    #[derive(Debug)]
    pub struct IsDocumentTail {
        #[tok(this, DOCUMENT)]
        #[presence(NOT)]
        pub not: bool,
    }
}

// --- SQL-standard string function atoms ---
//
// TRIM/SUBSTRING/POSITION/OVERLAY use special syntax with FROM/IN/PLACING/FOR
// separators inside parens that don't fit a comma-separated FuncCall.

recursa::ast_node! {
    /// Trim direction: `LEADING | TRAILING | BOTH`.
    #[derive(Debug)]
    pub enum TrimDir {
        #[tok(LEADING)]
        Leading,
        #[tok(TRAILING)]
        Trailing,
        #[tok(BOTH)]
        Both,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub struct TrimInner {
        pub dir: Option<TrimDir>,
        pub tail: TrimTail,
    }
}

recursa::ast_node! {
    /// Tail of `TRIM(...)` after the optional direction keyword.
    ///
    /// Variant ordering: `FromArgs` first because its leading `FROM` token is
    /// distinct from any `Expr` atom; `WithChars` second because the `[chars]
    /// FROM source` form starts with an Expr; `BareArgs` last as the catch-all
    /// `[expr, ...]` (no `FROM`) form for `trim(LEADING ' foo ')` shapes.
    #[derive(Debug)]
    pub enum TrimTail {
        /// `FROM expr_list` — explicit-FROM, no leading chars.
        FromArgs(TrimFromArgs),
        /// `chars FROM source` — explicit-FROM with leading chars.
        Values(TrimValues),
    }
}

recursa::ast_node! {
    /// `FROM expr_list` tail of `TRIM(...)`.
    #[derive(Debug)]
    #[tok(FROM, this)]
    pub struct TrimFromArgs {
        #[sep(COMMA)]
        pub args: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// `chars FROM source` tail of `TRIM(...)`.
    #[derive(Debug)]
    pub struct TrimWithChars {
        pub chars: boxed!(Expr),
        #[tok(FROM, this)]
        #[sep(COMMA)]
        pub args: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// A value-led TRIM tail. A following `FROM` turns the first value into the
    /// trim character; comma suffixes represent the ordinary function form.
    #[derive(Debug)]
    pub struct TrimValues {
        pub first: boxed!(Expr),
        /// gram.y `trim_list: a_expr FROM expr_list | expr_list`: after the first
        /// expression either `FROM expr_list` or the rest of one `expr_list`,
        /// never both, so a comma after `FROM b` continues that list.
        pub rest: Option<TrimValuesRest>,
    }
}

recursa::ast_node! {
    /// What follows the first expression of a `trim_list`.
    ///
    /// Variant ordering: `From` starts with `FROM`, `More` with a comma.
    #[derive(Debug)]
    pub enum TrimValuesRest {
        From(TrimFromArgs),
        More(TrimMoreArgs),
    }
}

recursa::ast_node! {
    /// `, expr [, expr ...]`.
    #[derive(Debug, derive_more :: Deref)]
    pub struct TrimMoreArgs(#[deref] pub one_or_many!(TrimMoreArg));
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct TrimMoreArg {
        #[tok(COMMA, this)]
        pub value: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`.
    #[derive(Debug)]
    pub struct TrimCall {
        #[tok(TRIM, LPAREN, this, RPAREN)]
        pub inner: TrimInner,
    }
}

recursa::ast_node! {
    /// `FOR len` suffix in `SUBSTRING(... FROM ... FOR ...)` / `OVERLAY(...)`.
    #[derive(Debug)]
    pub struct ForCount {
        #[tok(FOR, this)]
        pub count: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `FROM start [FOR len]` form for SUBSTRING.
    #[derive(Debug)]
    pub struct SubstringFromFor {
        #[tok(FROM, this)]
        pub start: boxed!(Expr),
        pub for_count: Option<ForCount>,
    }
}

recursa::ast_node! {
    /// `SIMILAR pattern ESCAPE escape` form for SUBSTRING.
    #[derive(Debug)]
    pub struct SubstringSimilar {
        #[tok(SIMILAR, this)]
        pub pattern: boxed!(Expr),
        #[tok(ESCAPE, this)]
        pub escape: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// Tail of a SUBSTRING call after the source expression.
    ///
    /// Variant ordering: `Similar` (`SIMILAR`) before `FromFor` (`FROM`) — distinct
    /// first tokens, so order is not strictly required, but listed by length.
    /// One `, arg` of the ordinary function-call spelling of `SUBSTRING`.
    #[derive(Debug)]
    pub struct SubstringMoreArg {
        #[tok(COMMA, this)]
        pub value: boxed!(Expr),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum SubstringTail {
        Similar(SubstringSimilar),
        FromFor(SubstringFromFor),
        For(ForCount),
        Args(one_or_many!(SubstringMoreArg)),
    }
}

recursa::ast_node! {
    /// Inner of `SUBSTRING(...)`: `source` followed by FROM/SIMILAR tail.
    #[derive(Debug)]
    pub struct SubstringInner {
        pub source: boxed!(Expr),
        pub tail: SubstringTail,
    }
}

recursa::ast_node! {
    /// `COLLATION FOR (expr)` — SQL-standard collation introspection.
    #[derive(Debug)]
    pub struct CollationForCall {
        #[tok(COLLATION, FOR, LPAREN, this, RPAREN)]
        pub arg: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `expr AS cast_type [COLLATE "c"]` — inner of `CAST(...)`.
    #[derive(Debug)]
    pub struct CastAsInner {
        pub value: boxed!(Expr),
        #[tok(AS, this)]
        pub target: CastType,
        pub collate: Option<CollateSuffix>,
    }
}

recursa::ast_node! {
    /// `COLLATE "name"` suffix appearing after a cast target type.
    #[derive(Debug)]
    pub struct CollateSuffix {
        #[tok(COLLATE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `CAST(expr AS type [COLLATE "c"])` — SQL-standard cast form.
    #[derive(Debug)]
    pub struct CastCall {
        #[tok(CAST, LPAREN, this, RPAREN)]
        pub inner: CastAsInner,
    }
}

recursa::ast_node! {
    /// `SUBSTRING(source FROM start [FOR len])` /
    /// `SUBSTRING(source SIMILAR pattern ESCAPE escape)`.
    #[derive(Debug)]
    pub struct SubstringCall {
        #[tok(SUBSTRING, LPAREN, this, RPAREN)]
        pub inner: SubstringInner,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub struct PositionInner {
        #[parse(pratt(exclude(
        // gram.y's `b_expr` reaches no `DEFAULT`: the keyword is not a
        // `c_expr`, only pg-sql's `INSERT`/`UPDATE` value placeholder.
        // recursa #122 lets an exclusion name an atom variant. Every other
        // `b_expr` operand excludes it too, citing this site.
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
        pub needle: boxed!(Expr),
        #[tok(IN, this)]
        pub haystack: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `POSITION(needle IN haystack)`.
    #[derive(Debug)]
    pub struct PositionCall {
        #[tok(POSITION, LPAREN, this, RPAREN)]
        pub inner: PositionInner,
    }
}

recursa::ast_node! {
    /// Inner of `OVERLAY(source PLACING new FROM start [FOR len])`.
    #[derive(Debug)]
    pub struct OverlayInner {
        pub source: boxed!(Expr),
        #[tok(PLACING, this)]
        pub new: boxed!(Expr),
        #[tok(FROM, this)]
        pub start: boxed!(Expr),
        pub for_count: Option<ForCount>,
    }
}

recursa::ast_node! {
    /// `OVERLAY(source PLACING new FROM start [FOR len])`.
    #[derive(Debug)]
    pub struct OverlayCall {
        #[tok(OVERLAY, LPAREN, this, RPAREN)]
        pub inner: OverlayInner,
    }
}

recursa::ast_node! {
    /// Field argument of `EXTRACT(field FROM source)`.
    ///
    /// Variant ordering: `StringLit` before `Ident` — string literal has a
    /// distinct first token (`'`) so order is not strictly required; listed
    /// first to match the Postgres docs ordering.
    #[derive(Debug)]
    pub enum ExtractField {
        StringLit(StringLitSeq0),
        Ident(literal::AliasName),
    }
}

recursa::ast_node! {
    /// Inner of `EXTRACT(field FROM source)`.
    #[derive(Debug)]
    pub struct ExtractInner {
        pub field: ExtractField,
        #[tok(FROM, this)]
        pub source: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `EXTRACT(field FROM source)` — Postgres-specific function syntax.
    #[derive(Debug)]
    pub struct ExtractCall {
        #[tok(EXTRACT, LPAREN, this, RPAREN)]
        pub inner: ExtractInner,
    }
}

recursa::ast_node! {
    /// `UESCAPE 'c'` suffix that may follow a `U&'...'` literal.
    #[derive(Debug)]
    pub struct UescapeSuffix {
        #[tok(UESCAPE, this)]
        pub escape_char: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `U&'...'` unicode string literal with optional `UESCAPE 'c'` suffix.
    #[derive(Debug)]
    pub struct UnicodeStringLitWithEscape {
        #[lex(pattern = r"(?i:U)&'(?:[^']|'')*'")]
        pub lit: literal::UnicodeStringLit,
        pub uescape: Option<UescapeSuffix>,
    }
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

recursa::ast_node! {
    /// `ENCODING ‹name›` suffix of a `FORMAT JSON` clause (e.g. `ENCODING UTF8`).
    #[derive(Debug)]
    pub struct JsonEncoding {
        #[tok(ENCODING, this)]
        pub name: literal::AliasName,
    }
}

recursa::ast_node! {
    /// `FORMAT JSON [ENCODING ‹name›]` — SQL/JSON input/output format specifier.
    #[derive(Debug)]
    #[tok(FORMAT, JSON, this)]
    pub struct JsonFormat {
        pub encoding: Option<JsonEncoding>,
    }
}

recursa::ast_node! {
    /// `RETURNING ‹data_type› [FORMAT JSON [ENCODING ...]]` — output type clause.
    #[derive(Debug)]
    pub struct JsonReturning {
        #[tok(RETURNING, this)]
        pub ty: CastType,
        pub format: Option<JsonFormat>,
    }
}

recursa::ast_node! {
    /// `WITH` / `WITHOUT` lead-in of a `UNIQUE KEYS` constraint.
    #[derive(Debug)]
    pub enum WithOrWithout {
        #[tok(WITH)]
        With,
        #[tok(WITHOUT)]
        Without,
    }
}

recursa::ast_node! {
    /// `{WITH|WITHOUT} UNIQUE [KEYS]` — duplicate-key handling for `JSON()` /
    /// `JSON_OBJECT()`.
    ///
    /// gram.y `json_key_uniqueness_constraint_opt`: "KEYS is a noise word here.
    /// To avoid shift/reduce conflicts, assign the KEYS-less productions a
    /// precedence less than IDENT (i.e., less than KEYS). This prevents reducing
    /// them when the next token is KEYS." `UNBOUNDED` (gram.y:886) is that level.
    #[derive(Debug)]
    #[parse(prec = UNBOUNDED)]
    pub struct JsonUniqueKeys {
        #[tok(this, UNIQUE)]
        pub with_or_without: WithOrWithout,
        /// Whether the optional `KEYS` noise word occurred, preserved for
        /// round-trip rendering.
        #[presence(KEYS)]
        pub keys: bool,
    }
}

recursa::ast_node! {
    /// `NULL` / `ABSENT` lead-in of an `ON NULL` clause.
    #[derive(Debug)]
    pub enum NullOrAbsent {
        #[tok(NULL)]
        Null,
        #[tok(ABSENT)]
        Absent,
    }
}

recursa::ast_node! {
    /// `{NULL|ABSENT} ON NULL` — null-input handling for `JSON_OBJECT()` /
    /// `JSON_ARRAY()`.
    #[derive(Debug)]
    pub struct JsonOnNull {
        #[tok(this, ON, NULL)]
        pub which: NullOrAbsent,
    }
}

recursa::ast_node! {
    /// Inner contents of `JSON ( ‹expr› [FORMAT JSON ...] [{WITH|WITHOUT} UNIQUE [KEYS]] )`.
    #[derive(Debug)]
    pub struct JsonConstructorInner {
        pub value: boxed!(Expr),
        pub format: Option<JsonFormat>,
        pub unique: Option<JsonUniqueKeys>,
    }
}

recursa::ast_node! {
    /// `JSON ( ‹expr› [FORMAT JSON ...] [{WITH|WITHOUT} UNIQUE [KEYS]] )`.
    #[derive(Debug)]
    pub struct JsonConstructor {
        #[tok(JSON, LPAREN, this, RPAREN)]
        pub inner: JsonConstructorInner,
    }
}

recursa::ast_node! {
    /// `JSON_SCALAR ( ‹expr› )`.
    #[derive(Debug)]
    pub struct JsonScalar {
        #[tok(JSON_SCALAR, LPAREN, this, RPAREN)]
        pub inner: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// Inner contents of `JSON_SERIALIZE ( ‹expr› [FORMAT JSON ...] [RETURNING ...] )`.
    #[derive(Debug)]
    pub struct JsonSerializeInner {
        pub value: boxed!(Expr),
        pub format: Option<JsonFormat>,
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `JSON_SERIALIZE ( ‹expr› [FORMAT JSON ...] [RETURNING ‹type› ...] )`.
    #[derive(Debug)]
    pub struct JsonSerialize {
        #[tok(JSON_SERIALIZE, LPAREN, this, RPAREN)]
        pub inner: JsonSerializeInner,
    }
}

recursa::ast_node! {
    /// Key/value separator inside a `JSON_OBJECT` entry: `:` or the `VALUE` keyword.
    #[derive(Debug)]
    pub enum JsonKeyValueSep {
        #[tok(COLON)]
        Colon,
        #[tok(VALUE)]
        Value,
    }
}

recursa::ast_node! {
    /// The `{: | VALUE} ‹value› [FORMAT JSON ...]` part of a `JSON_OBJECT` item.
    #[derive(Debug)]
    pub struct JsonObjectEntryValue {
        pub sep: JsonKeyValueSep,
        pub value: boxed!(Expr),
        pub format: Option<JsonFormat>,
    }
}

recursa::ast_node! {
    /// One item of a `JSON_OBJECT` list: either a SQL/JSON
    /// `[KEY] ‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry, or one
    /// argument of PostgreSQL's legacy `json_object(text[])` /
    /// `json_object(text[], text[])` function form.
    ///
    /// gram.y keeps those as separate productions — `JSON_OBJECT` is a
    /// `COL_NAME` keyword and can never be an ordinary `FuncCall` name, so the
    /// legacy call gets its own `JSON_OBJECT '(' func_arg_list ')'` rule. Both
    /// are comma-separated and expression-led, with an arbitrarily long shared
    /// prefix. They therefore share this node, and the presence of the key/value
    /// separator is what tells the two forms apart.
    ///
    /// PostgreSQL's `func_arg_list` also admits the `name => value` spelling.
    /// That form is not modelled here: no `json_object` call uses it, and a
    /// named argument is not a legal SQL/JSON entry key.
    #[derive(Debug)]
    pub struct JsonObjectEntry {
        pub key: boxed!(Expr),
        pub value: Option<JsonObjectEntryValue>,
    }
}

recursa::ast_node! {
    /// One `‹key› {: | VALUE} ‹value› [FORMAT JSON ...]` entry of
    /// `JSON_OBJECTAGG`, whose gram.y production admits only the SQL/JSON
    /// spelling and therefore requires the value.
    #[derive(Debug)]
    pub struct JsonObjectAggEntry {
        pub key: boxed!(Expr),
        pub value: JsonObjectEntryValue,
    }
}

recursa::ast_node! {
    /// Non-empty item list of `JSON_OBJECT`, followed by the optional `ON NULL`,
    /// `UNIQUE` and `RETURNING` clauses. Those clauses belong to the SQL/JSON
    /// entry production only; the legacy function form never carries them.
    #[derive(Debug)]
    pub struct JsonObjectArgs {
        #[sep(COMMA)]
        pub entries: one_or_many!(JsonObjectEntry),
        pub on_null: Option<JsonOnNull>,
        pub unique: Option<JsonUniqueKeys>,
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `JSON_OBJECT` has distinct PostgreSQL productions for a non-empty item
    /// list and for the empty/returning-only form. Keeping those paths distinct
    /// prevents the expression-led entry parser from claiming the reserved
    /// `RETURNING` token as an entry key.
    #[derive(Debug)]
    pub enum JsonObject {
        Entries(#[tok(JSON_OBJECT, LPAREN, this, RPAREN)] JsonObjectArgs),
        Returning(#[tok(JSON_OBJECT, LPAREN, this, RPAREN)] JsonReturning),
        #[tok(JSON_OBJECT, LPAREN, RPAREN)]
        Empty,
    }
}

recursa::ast_node! {
    /// One `‹expr› [FORMAT JSON ...]` element of a `JSON_ARRAY` element list.
    #[derive(Debug)]
    pub struct JsonArrayElement {
        pub value: boxed!(Expr),
        pub format: Option<JsonFormat>,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum JsonArrayArgs {
        Query(JsonArrayQueryArgs),
        Elements(JsonArrayElementsArgs),
        Empty(JsonArrayEmptyArgs),
    }
}

recursa::ast_node! {
    /// `select_no_parens json_format_clause_opt json_returning_clause_opt`: the
    /// query form has no `ON NULL` clause, so the query ends where its own
    /// syntax ends. The `FORMAT JSON` option is not admitted: `FORMAT` is also
    /// a table alias in the query's FROM list, which the `FORMAT_LA` token filter
    /// distinguishes in the LR grammar.
    #[derive(Debug)]
    pub struct JsonArrayQueryArgs {
        pub query: boxed!(Subquery),
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `json_value_expr_list json_array_constructor_null_clause_opt
    /// json_returning_clause_opt`.
    #[derive(Debug)]
    pub struct JsonArrayElementsArgs {
        #[sep(COMMA)]
        pub elements: one_or_many!(JsonArrayElement),
        pub on_null: Option<JsonOnNull>,
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `json_returning_clause_opt` alone.
    #[derive(Debug)]
    pub struct JsonArrayEmptyArgs {
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `JSON_ARRAY ( ... )` — element-list or query form.
    #[derive(Debug)]
    pub struct JsonArray {
        #[tok(JSON_ARRAY, LPAREN, this, RPAREN)]
        pub args: JsonArrayArgs,
    }
}

// --- SQL/JSON query function atoms ---
//
// `JSON_EXISTS()`, `JSON_VALUE()` and `JSON_QUERY()` test/extract values from
// a JSON context item using a jsonpath. Like the constructors they are
// grammar constructs with `PASSING`, `RETURNING`, wrapper/quotes and
// `ON EMPTY`/`ON ERROR` behavior clauses that no function-argument list can
// express. Modeled as dedicated atoms before `Func`.

recursa::ast_node! {
    /// One `‹value› AS ‹name›` binding of a `PASSING` clause.
    #[derive(Debug)]
    pub struct JsonPassingArg {
        pub value: boxed!(Expr),
        #[tok(AS, this)]
        pub name: literal::AliasName,
    }
}

recursa::ast_node! {
    /// `PASSING ‹value› AS ‹name› [, ...]` — jsonpath variable bindings.
    #[derive(Debug)]
    #[tok(PASSING, this)]
    pub struct JsonPassing {
        #[sep(COMMA)]
        pub args: one_or_many!(JsonPassingArg),
    }
}

recursa::ast_node! {
    /// `DEFAULT ‹expr›` — the default-value form of an `ON EMPTY`/`ON ERROR` behavior.
    #[derive(Debug)]
    pub struct JsonDefault {
        #[tok(DEFAULT, this)]
        pub value: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// The behavior of an `ON EMPTY` / `ON ERROR` clause — the union of every
    /// query function's accepted behaviors (`JSON_EXISTS` uses the boolean
    /// forms, `JSON_VALUE`/`JSON_QUERY` the rest). Parsed permissively; which
    /// behaviors are valid for which function is Postgres's concern.
    ///
    /// Variant ordering: the two-keyword `EMPTY ARRAY`/`EMPTY OBJECT` forms
    /// before bare `Empty` so longest-match-wins picks them.
    #[derive(Debug)]
    pub enum JsonBehavior {
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
        Default(JsonDefault),
    }
}

recursa::ast_node! {
    /// `EMPTY` or `ERROR` — the trigger of an `ON` behavior clause.
    #[derive(Debug)]
    pub enum EmptyOrError {
        #[tok(EMPTY)]
        Empty,
        #[tok(ERROR)]
        Error,
    }
}

recursa::ast_node! {
    /// `‹behavior› ON {EMPTY|ERROR}` clause.
    #[derive(Debug)]
    pub struct JsonOnBehavior {
        pub behavior: JsonBehavior,
        #[tok(ON, this)]
        pub trigger: EmptyOrError,
    }
}

recursa::ast_node! {
    /// gram.y `json_behavior_clause_opt`: `json_behavior ON EMPTY`,
    /// `json_behavior ON ERROR`, or both. Each `JsonOnBehavior` names its own
    /// trigger, so the pair is order-independent and the second slot is simply
    /// optional; two independent optional slots would leave a single clause
    /// ambiguous between them.
    #[derive(Debug)]
    pub struct JsonBehaviorClause {
        pub first: JsonOnBehavior,
        pub second: Option<JsonOnBehavior>,
    }
}

recursa::ast_node! {
    /// `CONDITIONAL` / `UNCONDITIONAL` modifier of a `WITH ... WRAPPER` clause.
    #[derive(Debug)]
    pub enum WrapperBehavior {
        #[tok(CONDITIONAL)]
        Conditional,
        #[tok(UNCONDITIONAL)]
        Unconditional,
    }
}

recursa::ast_node! {
    /// `{WITH [CONDITIONAL|UNCONDITIONAL] | WITHOUT} [ARRAY] WRAPPER` — the
    /// `JSON_QUERY` array-wrapper clause.
    #[derive(Debug)]
    pub struct JsonWrapper {
        pub with_or_without: WithOrWithout,
        pub behavior: Option<WrapperBehavior>,
        #[tok(this, WRAPPER)]
        #[presence(ARRAY)]
        pub array: bool,
    }
}

recursa::ast_node! {
    /// `ON SCALAR STRING` suffix of a `JSON_QUERY` quotes clause.
    #[derive(Debug)]
    pub enum JsonQuotesOnScalar {
        #[tok(ON, SCALAR, STRING)]
        Value,
    }
}

recursa::ast_node! {
    /// `KEEP` / `OMIT` lead-in of a `JSON_QUERY` quotes clause.
    #[derive(Debug)]
    pub enum KeepOrOmit {
        #[tok(KEEP)]
        Keep,
        #[tok(OMIT)]
        Omit,
    }
}

recursa::ast_node! {
    /// `{KEEP|OMIT} QUOTES [ON SCALAR STRING]` — the `JSON_QUERY` quotes clause.
    #[derive(Debug)]
    pub struct JsonQuotes {
        pub keep_or_omit: KeepOrOmit,
        pub quotes: JsonQuotesKeyword,
        pub on_scalar: Option<JsonQuotesOnScalar>,
    }
}

recursa::ast_node! {
    /// Required `QUOTES` keyword in a `JSON_QUERY` quotes clause.
    #[derive(Debug)]
    pub enum JsonQuotesKeyword {
        #[tok(QUOTES)]
        Value,
    }
}

recursa::ast_node! {
    /// Inner contents of `JSON_EXISTS ( ‹context› , ‹path› [PASSING ...] [‹behavior› ON ERROR] )`.
    #[derive(Debug)]
    pub struct JsonExistsInner {
        pub context: boxed!(Expr),
        pub context_format: Option<JsonFormat>,
        #[tok(COMMA, this)]
        pub path: boxed!(Expr),
        pub passing: Option<JsonPassing>,
        pub on_error: Option<JsonOnBehavior>,
    }
}

recursa::ast_node! {
    /// `JSON_EXISTS ( ... )` — tests whether a jsonpath matches.
    #[derive(Debug)]
    pub struct JsonExists {
        #[tok(JSON_EXISTS, LPAREN, this, RPAREN)]
        pub inner: JsonExistsInner,
    }
}

recursa::ast_node! {
    /// Inner contents of `JSON_VALUE`.
    #[derive(Debug)]
    pub struct JsonValueInner {
        pub context: boxed!(Expr),
        pub context_format: Option<JsonFormat>,
        #[tok(COMMA, this)]
        pub path: boxed!(Expr),
        pub passing: Option<JsonPassing>,
        pub returning: Option<JsonReturning>,
        /// gram.y `json_behavior_clause_opt`.
        pub on_behavior: Option<JsonBehaviorClause>,
    }
}

recursa::ast_node! {
    /// `JSON_VALUE ( ... )` — extracts a scalar SQL value via a jsonpath.
    #[derive(Debug)]
    pub struct JsonValue {
        #[tok(JSON_VALUE, LPAREN, this, RPAREN)]
        pub inner: JsonValueInner,
    }
}

recursa::ast_node! {
    /// Inner contents of `JSON_QUERY`.
    #[derive(Debug)]
    pub struct JsonQueryInner {
        pub context: boxed!(Expr),
        pub context_format: Option<JsonFormat>,
        #[tok(COMMA, this)]
        pub path: boxed!(Expr),
        pub passing: Option<JsonPassing>,
        pub returning: Option<JsonReturning>,
        pub wrapper: Option<JsonWrapper>,
        pub quotes: Option<JsonQuotes>,
        /// gram.y `json_behavior_clause_opt`.
        pub on_behavior: Option<JsonBehaviorClause>,
    }
}

recursa::ast_node! {
    /// `JSON_QUERY ( ... )` — extracts a JSON value via a jsonpath.
    #[derive(Debug)]
    pub struct JsonQuery {
        #[tok(JSON_QUERY, LPAREN, this, RPAREN)]
        pub inner: JsonQueryInner,
    }
}

// --- SQL/JSON aggregate atoms ---
//
// `JSON_OBJECTAGG()` and `JSON_ARRAYAGG()` aggregate rows into a JSON object
// or array. They are grammar constructs (the object form takes a `key :
// value` entry, the array form an `ORDER BY`) and, being aggregates, accept
// the ordinary `FILTER (WHERE ...)` and `OVER (...)` suffixes.

recursa::ast_node! {
    /// Inner contents of `JSON_OBJECTAGG`.
    #[derive(Debug)]
    pub struct JsonObjectAggInner {
        pub entry: JsonObjectAggEntry,
        pub on_null: Option<JsonOnNull>,
        pub unique: Option<JsonUniqueKeys>,
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `JSON_OBJECTAGG ( ‹key› {: | VALUE} ‹value› ... ) [FILTER (...)] [OVER (...)]`.
    #[derive(Debug)]
    pub struct JsonObjectAgg {
        #[tok(JSON_OBJECTAGG, LPAREN, this, RPAREN)]
        pub inner: JsonObjectAggInner,
        pub filter: Option<FilterClause>,
        pub window: Option<WindowSpec>,
    }
}

recursa::ast_node! {
    /// Inner contents of `JSON_ARRAYAGG`.
    #[derive(Debug)]
    pub struct JsonArrayAggInner {
        pub value: boxed!(Expr),
        pub format: Option<JsonFormat>,
        pub order_by: Option<boxed!(crate::ast::dml::select::OrderByClause)>,
        pub on_null: Option<JsonOnNull>,
        pub returning: Option<JsonReturning>,
    }
}

recursa::ast_node! {
    /// `JSON_ARRAYAGG ( ‹value› [ORDER BY ...] ... ) [FILTER (...)] [OVER (...)]`.
    #[derive(Debug)]
    pub struct JsonArrayAgg {
        #[tok(JSON_ARRAYAGG, LPAREN, this, RPAREN)]
        pub inner: JsonArrayAggInner,
        pub filter: Option<FilterClause>,
        pub window: Option<WindowSpec>,
    }
}

// --- `IS JSON` predicate ---

recursa::ast_node! {
    /// The JSON item type tested by an `IS JSON` predicate.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// The tail of an `IS JSON` predicate: `[NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}]
    /// [{WITH|WITHOUT} UNIQUE [KEYS]]`.
    ///
    /// gram.y `json_predicate_type_constraint: JSON %prec UNBOUNDED` and
    /// `json_key_uniqueness_constraint_opt: /* EMPTY */ %prec UNBOUNDED`: the
    /// tail-less productions sit below the `IDENT` level that carries `VALUE`,
    /// `SCALAR`, `OBJECT`, `WITH`, `WITHOUT` and `KEYS`, so the parser shifts the
    /// tail rather than ending the predicate.
    #[derive(Debug)]
    #[parse(prec = UNBOUNDED)]
    pub struct IsJsonTail {
        #[tok(this, JSON)]
        #[presence(NOT)]
        pub not: bool,
        pub type_kind: Option<JsonTypeKind>,
        pub unique: Option<JsonUniqueKeys>,
    }
}

recursa::ast_node! {
    /// Any value-producing SQL/JSON function — the constructors and query
    /// functions grouped into one peekable type. Each variant leads with a
    /// distinct soft keyword, so this peeks `true` only for a JSON function.
    /// Lets non-Pratt contexts (e.g. a `CREATE INDEX` expression element)
    /// accept the whole family. Aggregates and `JSON_TABLE` are excluded:
    /// neither is a plain value expression usable as an index element.
    #[derive(Debug)]
    pub enum JsonFuncExpr {
        Ctor(boxed!(JsonConstructor)),
        Scalar(boxed!(JsonScalar)),
        Serialize(boxed!(JsonSerialize)),
        Object(boxed!(JsonObject)),
        Array(boxed!(JsonArray)),
        Exists(boxed!(JsonExists)),
        Value(boxed!(JsonValue)),
        Query(boxed!(JsonQuery)),
    }
}

// --- Pratt expression enum ---
//
// Binding powers on this enum are gram.y's precedence order times ten. The
// scale is shared with the `precedence { ... }` block in `crate::tokens`,
// which needs room for the two keyword levels gram.y puts between `ESCAPE`
// and `Op`; that block names the gram.y line each level comes from. A left
// associative infix variant is `lbp = N, rbp = N + 1`, which is the only
// shape a rule precedence reproduces (`RCA0402`).

recursa::ast_node! {
    /// SQL expression whose Pratt declarations lower into LR productions.
    #[derive(Debug)]
    #[pratt]
    pub enum Expr {
        // --- Prefix ---
        #[parse(prefix, bp = 150)]
        Not(#[tok(NOT, this)] boxed!(Self)),
        #[parse(prefix, bp = 120)]
        Neg(#[tok(MINUS, this)] boxed!(Self)),
        /// Unary plus: `+expr` — identity operator on numeric types.
        #[parse(prefix, bp = 120)]
        Pos(#[tok(PLUS, this)] boxed!(Self)),
        /// Unary geometric "center point": `@@ expr`. Postgres uses `@@` as
        /// a prefix operator on box / polygon / etc. (in addition to the
        /// text-search infix form).
        #[parse(prefix, bp = 120)]
        GeomCenter(#[tok(ATAT, this)] boxed!(Self)),
        /// Bitwise NOT: `~ expr` (e.g. inet / bit / int bitwise complement).
        /// Must come before any infix `~` variant so the prefix form wins when
        /// `~` appears at the start of an operand.
        #[parse(prefix, bp = 120)]
        BitNot(#[tok(TILDE, this)] boxed!(Self)),
        /// Geometric path/lseg length: `@-@ expr`. Must come before `Abs` (`@`)
        /// since `@-@` is longer.
        #[parse(prefix, bp = 120)]
        PathLength(#[tok(ATMINUSAT, this)] boxed!(Self)),
        /// User-defined prefix: `@#@ expr` (e.g. factorial).
        #[parse(prefix, bp = 120)]
        AtHashAtPrefix(#[tok(ATHASHAT, this)] boxed!(Self)),
        /// Geometric point-count: `# path` — number of points in a path.
        #[parse(prefix, bp = 120)]
        PointCount(#[tok(POUND, this)] boxed!(Self)),
        /// Absolute value: `@ expr` (Postgres unary `@` operator).
        #[parse(prefix, bp = 120)]
        Abs(#[tok(ATSIGN, this)] boxed!(Self)),
        /// User-defined prefix: `!=- expr`.
        #[parse(prefix, bp = 120)]
        BangEqMinusPrefix(#[tok(BANGEQMINUS, this)] boxed!(Self)),
        /// Square root: `|/ expr` (Postgres unary `|/` operator).
        #[parse(prefix, bp = 120)]
        Sqrt(#[tok(PIPESLASH, this)] boxed!(Self)),
        /// Cube root: `||/ expr` (Postgres unary `||/` operator).
        #[parse(prefix, bp = 120)]
        Cbrt(#[tok(PIPEPIPESLASH, this)] boxed!(Self)),

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
            literal::CustomOp,
            #[pretty(break_before = soft)] boxed!(Self),
        ),

        // --- Postfix ---
        /// Postgres-style cast: `expr::type`
        #[parse(postfix, bp = 200)]
        Cast(boxed!(Self), #[tok(COLONCOLON, this)] boxed!(CastType)),
        /// `expr COLLATE "collation"` — collation specifier. Binds tighter than
        /// comparisons (bp 5) but looser than `::` cast (bp 20).
        #[parse(postfix, bp = 180)]
        Collate(boxed!(Self), #[tok(COLLATE, this)] crate::tokens::ColId),
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
        QuantifiedComparisonCmp(boxed!(Self), QuantifiedComparisonCmpSuffix),
        /// `subquery_Op`'s `LIKE` family, at the level of `Like`.
        #[parse(postfix, bp = 60)]
        QuantifiedComparisonLike(boxed!(Self), QuantifiedComparisonLikeSuffix),
        /// `subquery_Op` at gram.y's generic `Op` level.
        #[parse(postfix, bp = 80)]
        QuantifiedComparisonOp(boxed!(Self), QuantifiedComparisonOpSuffix),
        /// `subquery_Op` at the level of `Add` / `Sub`.
        #[parse(postfix, bp = 100)]
        QuantifiedComparisonAdd(boxed!(Self), QuantifiedComparisonAddSuffix),
        /// `subquery_Op` at the level of `Mul` / `Div` / `Mod`.
        #[parse(postfix, bp = 110)]
        QuantifiedComparisonMul(boxed!(Self), QuantifiedComparisonMulSuffix),
        /// `subquery_Op` at the level of `Pow`.
        #[parse(postfix, bp = 130)]
        QuantifiedComparisonPow(boxed!(Self), QuantifiedComparisonPowSuffix),
        // The `IS` family sits at one level below the comparison operators, gram.y
        // `%nonassoc IS ISNULL NOTNULL` (`a = b IS NULL` is `(a = b) IS NULL`);
        // one level for every `IS` form is also what lets the LR parser
        // settle `IS` by precedence. Binding power 4.
        /// `expr IS NOT DISTINCT FROM expr`. Declared before `IsDistinctFrom` so
        /// the longer `NOT` prefix wins disambiguation.
        #[parse(infix, lbp = 40, rbp = 41)]
        IsNotDistinctFrom(
            boxed!(Self),
            #[tok(IS, NOT, DISTINCT, FROM, this)] boxed!(Self),
        ),
        /// `expr IS DISTINCT FROM expr`.
        #[parse(infix, lbp = 40, rbp = 41)]
        IsDistinctFrom(boxed!(Self), #[tok(IS, DISTINCT, FROM, this)] boxed!(Self)),
        /// `expr IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] [{WITH|WITHOUT}
        /// UNIQUE [KEYS]]` — the SQL/JSON type predicate. Declared before
        /// `BoolTest` (both lead with `IS`); `BoolTest` rejects `JSON` as a
        /// `BoolTestKind`, so order is not load-bearing, only tidy.
        #[parse(postfix, bp = 40)]
        IsJson(boxed!(Self), #[tok(IS, this)] IsJsonTail),
        /// `expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` — the Unicode
        /// normalisation predicate (gram.y rules 15198/15205/15212/15220).
        /// Declared before `BoolTest` (both lead with `IS`); `BoolTest` rejects
        /// `NORMALIZED`/`NFx` as a `BoolTestKind`, so order is not load-bearing.
        #[parse(postfix, bp = 40)]
        IsNormalized(boxed!(Self), #[tok(IS, this)] IsNormalizedTail),
        /// `expr IS [NOT] DOCUMENT` — the XML document predicate.
        #[parse(postfix, bp = 40)]
        IsDocument(boxed!(Self), #[tok(IS, this)] IsDocumentTail),
        /// Boolean test: `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`
        #[parse(postfix, bp = 40)]
        BoolTest(boxed!(Self), #[tok(IS, this)] BoolTestKind),
        /// Postgres `expr NOTNULL` postfix null test (synonym for `IS NOT NULL`).
        #[parse(postfix, bp = 40)]
        Notnull(#[tok(this, NOTNULL)] boxed!(Self)),
        /// Postgres `expr ISNULL` postfix null test (synonym for `IS NULL`).
        #[parse(postfix, bp = 40)]
        Isnull(#[tok(this, ISNULL)] boxed!(Self)),
        /// `expr AT LOCAL` — convert to session timezone. Listed before
        /// `AtTimeZone` so `AT LOCAL` wins (distinct second token `LOCAL` vs `TIME`).
        #[parse(postfix, bp = 90)]
        AtLocal(#[tok(this, AT, LOCAL)] boxed!(Self)),
        /// `expr AT TIME ZONE zone_expr` — convert to specified timezone.
        #[parse(infix, lbp = 90, rbp = 91)]
        AtTimeZone(boxed!(Self), #[tok(AT, TIME, ZONE, this)] boxed!(Self)),
        /// NOT IN list: `expr NOT IN (val, ...)`
        #[parse(postfix, bp = 60)]
        NotInExpr(boxed!(Self), NotInSuffix),
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
        // precedence into the generated parser: it takes every operator above
        // `ESCAPE` (`'$'::bytea`, `'$' || 'x'`) and stops at `LIKE`'s level
        // and below. The first pass had to spell this as a separate
        // `EscapeClause` struct with an exclusion list, which no rule
        // precedence reproduced.
        //
        // Each `ESCAPE` operand accepts `all`. It keeps extending on every
        // operator above `ESCAPE`'s level, and every one of those may also
        // follow the whole LIKE expression, so the overlap is that entire set.
        // Naming it as a list would say no more and would need an edit for each
        // operator added.
        /// `expr NOT ILIKE pattern [ESCAPE char]`. Declared before `NotLike` so the longer
        /// `NOT ILIKE` is tried first (matters only if any rule shares a prefix;
        /// here `NOT ILIKE` vs `NOT LIKE` differ on the second token).
        #[parse(infix, lbp = 60, rbp = 61)]
        NotIlike(
            boxed!(Self),
            #[tok(NOT, ILIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr NOT SIMILAR TO pattern [ESCAPE char]`. Declared before `NotLike` so the longer
        /// `NOT SIMILAR TO` form wins longest-match-wins disambiguation.
        #[parse(infix, lbp = 60, rbp = 61)]
        NotSimilarTo(
            boxed!(Self),
            #[tok(NOT, SIMILAR, TO, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr NOT LIKE pattern [ESCAPE char]`. Must come before the `Not` prefix atom so
        /// longest-match-wins prefers the postfix form.
        #[parse(infix, lbp = 60, rbp = 61)]
        NotLike(
            boxed!(Self),
            #[tok(NOT, LIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr SIMILAR TO pattern [ESCAPE char]` — SQL standard similar-to pattern match.
        #[parse(infix, lbp = 60, rbp = 61)]
        SimilarTo(
            boxed!(Self),
            #[tok(SIMILAR, TO, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr ILIKE pattern [ESCAPE char]`
        #[parse(infix, lbp = 60, rbp = 61)]
        Ilike(
            boxed!(Self),
            #[tok(ILIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr LIKE pattern [ESCAPE char]`
        #[parse(infix, lbp = 60, rbp = 61)]
        Like(
            boxed!(Self),
            #[tok(LIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        // --- Locale-aware text comparison operators (4-char before 3-char) ---
        /// `expr ~<=~ expr` — locale-aware less-or-equal.
        #[parse(infix, lbp = 50, rbp = 51)]
        TildeLeqTilde(boxed!(Self), #[tok(TILDELEQTILDE, this)] boxed!(Self)),
        /// `expr ~>=~ expr` — locale-aware greater-or-equal.
        #[parse(infix, lbp = 50, rbp = 51)]
        TildeGeqTilde(boxed!(Self), #[tok(TILDEGEQTILDE, this)] boxed!(Self)),
        /// `expr ~<~ expr` — locale-aware less-than.
        #[parse(infix, lbp = 50, rbp = 51)]
        TildeLtTilde(boxed!(Self), #[tok(TILDELTTILDE, this)] boxed!(Self)),
        /// `expr ~>~ expr` — locale-aware greater-than.
        #[parse(infix, lbp = 50, rbp = 51)]
        TildeGtTilde(boxed!(Self), #[tok(TILDEGTTILDE, this)] boxed!(Self)),
        /// `expr !~* pattern` — POSIX case-insensitive negated regex match.
        #[parse(infix, lbp = 50, rbp = 51)]
        RegexNotIMatch(boxed!(Self), #[tok(BANGTILDESTAR, this)] boxed!(Self)),
        /// `expr ~* pattern` — POSIX case-insensitive regex match.
        #[parse(infix, lbp = 50, rbp = 51)]
        RegexIMatch(boxed!(Self), #[tok(TILDESTAR, this)] boxed!(Self)),
        /// `expr !~ pattern` — POSIX negated regex match.
        #[parse(infix, lbp = 50, rbp = 51)]
        RegexNotMatch(boxed!(Self), #[tok(BANGTILDE, this)] boxed!(Self)),
        /// `expr ~= expr` — geometric "same as" operator. Declared before `RegexMatch`
        /// so the longer `~=` wins longest-match.
        #[parse(infix, lbp = 50, rbp = 51)]
        GeomSame(boxed!(Self), #[tok(TILDEEQ, this)] boxed!(Self)),
        /// `expr ~ pattern` — POSIX regex match.
        #[parse(infix, lbp = 50, rbp = 51)]
        RegexMatch(boxed!(Self), #[tok(TILDE, this)] boxed!(Self)),
        /// `expr !~~* pattern` — operator-form `NOT ILIKE` (gram.y 14897).
        /// Declared before `LikeOpINeg` (`!~~`) so the longer `!~~*` wins.
        #[parse(infix, lbp = 50, rbp = 51)]
        LikeOpINeg(boxed!(Self), #[tok(BANGTILDETILDESTAR, this)] boxed!(Self)),
        /// `expr ~~* pattern` — operator-form `ILIKE` (gram.y 14888).
        /// Declared before `LikeOpI` would be (no `~~*` longer prefix).
        #[parse(infix, lbp = 50, rbp = 51)]
        LikeOpI(boxed!(Self), #[tok(TILDETILDESTAR, this)] boxed!(Self)),
        /// `expr !~~ pattern` — operator-form `NOT LIKE` (gram.y 14874).
        #[parse(infix, lbp = 50, rbp = 51)]
        LikeOpNeg(boxed!(Self), #[tok(BANGTILDETILDE, this)] boxed!(Self)),
        /// `expr ~~ pattern` — operator-form `LIKE` (gram.y 14860).
        #[parse(infix, lbp = 50, rbp = 51)]
        LikeOp(boxed!(Self), #[tok(TILDETILDE, this)] boxed!(Self)),
        /// `(start, end) OVERLAPS (start, end)` — SQL time-period overlap test.
        /// Each operand is an ordinary parenthesized expression to the parser.
        #[parse(infix, lbp = 50, rbp = 51)]
        Overlaps(boxed!(Self), #[tok(OVERLAPS, this)] boxed!(Self)),
        /// Record comparison operators: `expr *= expr`, `*<>`, `*<`, `*<=`,
        /// `*>`, `*>=` — compare ROW/composite values field by field.
        #[parse(infix, lbp = 50, rbp = 51)]
        RecordLte(boxed!(Self), #[tok(STARLTE, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        RecordGte(boxed!(Self), #[tok(STARGTE, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        RecordNeq(boxed!(Self), #[tok(STARNEQ, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        RecordLt(boxed!(Self), #[tok(STARLT, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        RecordGt(boxed!(Self), #[tok(STARGT, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        RecordEq(boxed!(Self), #[tok(STAREQ, this)] boxed!(Self)),
        /// IN list: `expr IN (val, ...)`
        #[parse(postfix, bp = 60)]
        InExpr(boxed!(Self), #[tok(IN, this)] InList),
        /// `expr NOT BETWEEN low AND high`. Declared before `BetweenExpr` so
        /// the longer `NOT BETWEEN` prefix wins disambiguation. Recursive fields
        /// in this postfix tail inherit `bp = 60`, so the low/high operands stop
        /// before the literal `AND` infix at `bp = 20`.
        #[parse(postfix, bp = 60)]
        NotBetweenExpr(
            boxed!(Self),
            #[tok(NOT, BETWEEN, this)] boxed!(Self),
            #[tok(AND, this)] boxed!(Self),
        ),
        /// `expr BETWEEN low AND high`. See `NotBetweenExpr` for the recursive
        /// postfix-tail binding-power rationale.
        #[parse(postfix, bp = 60)]
        BetweenExpr(
            boxed!(Self),
            #[tok(BETWEEN, this)] boxed!(Self),
            #[tok(AND, this)] boxed!(Self),
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
        JsonPathText(boxed!(Self), #[tok(HASHARROWARROW, this)] boxed!(Self)),
        /// JSON path: `expr #> path`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonPath(boxed!(Self), #[tok(HASHARROW, this)] boxed!(Self)),
        /// JSON field as text: `expr ->> field`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonFieldText(boxed!(Self), #[tok(ARROWARROW, this)] boxed!(Self)),
        /// JSON field: `expr -> field`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonField(boxed!(Self), #[tok(ARROW, this)] boxed!(Self)),
        /// Geometric parallel: `a ?|| b`. Must precede `JsonAnyKey` (`?|`)
        /// so the 3-char token wins over the 2-char token.
        #[parse(infix, lbp = 50, rbp = 51)]
        Parallel(boxed!(Self), #[tok(QUESTIONPIPEPIPE, this)] boxed!(Self)),
        /// JSON any-key-exists: `expr ?| keys`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonAnyKey(boxed!(Self), #[tok(QUESTIONPIPE, this)] boxed!(Self)),
        /// JSON all-keys-exist: `expr ?& keys`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonAllKeys(boxed!(Self), #[tok(QUESTIONAMP, this)] boxed!(Self)),
        /// Geometric intersect: `a ?# b`. Must precede `JsonKey` (`?`).
        #[parse(infix, lbp = 50, rbp = 51)]
        Intersect(boxed!(Self), #[tok(QUESTIONHASH, this)] boxed!(Self)),
        /// Geometric perpendicular: `a ?-| b`. Must precede `Horizontal` (`?-`)
        /// so the 3-char token wins over the 2-char token.
        #[parse(infix, lbp = 50, rbp = 51)]
        Perpendicular(boxed!(Self), #[tok(QUESTIONDASHPIPE, this)] boxed!(Self)),
        /// Geometric horizontal: `a ?- b`. Must precede `JsonKey` (`?`).
        #[parse(infix, lbp = 50, rbp = 51)]
        Horizontal(boxed!(Self), #[tok(QUESTIONDASH, this)] boxed!(Self)),
        /// Geometric "is horizontal" prefix: `?- s` — tests whether the
        /// LSEG/LINE `s` is horizontal. PG's geometry.sql uses this in WHERE.
        #[parse(prefix, bp = 120)]
        IsHorizontal(#[tok(QUESTIONDASH, this)] boxed!(Self)),
        /// Geometric "is vertical" prefix: `?| s`.
        #[parse(prefix, bp = 120)]
        IsVertical(#[tok(QUESTIONPIPE, this)] boxed!(Self)),
        /// Geometric "below": `a <^ b`.
        #[parse(infix, lbp = 50, rbp = 51)]
        Below(boxed!(Self), #[tok(LTCARET, this)] boxed!(Self)),
        /// Geometric "above": `a >^ b`.
        #[parse(infix, lbp = 50, rbp = 51)]
        Above(boxed!(Self), #[tok(GTCARET, this)] boxed!(Self)),
        /// JSON key-exists: `expr ? key`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonKey(boxed!(Self), #[tok(QUESTION, this)] boxed!(Self)),
        /// JSONB contains: `expr @> expr`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonContains(boxed!(Self), #[tok(ATGT, this)] boxed!(Self)),
        /// JSONB contained-by: `expr <@ expr`
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonContainedBy(boxed!(Self), #[tok(LTAT, this)] boxed!(Self)),

        // --- Postgres text-search / jsonpath / range / geometric 3-char operators ---
        //
        // These must come BEFORE any variant whose infix token is a 2-char prefix
        // (e.g. `<<|` before `<<`, `&<|` before `&<`, `?#` before JsonKey `?`).
        // The scanner is longest-match at the token level and assigns each of
        // these spellings its own token kind. Keeping the longer spellings first
        // mirrors the lexical declaration and makes the precedence table easier
        // to compare with it.
        /// Text-search / jsonb path match: `expr @@@ expr`.
        #[parse(infix, lbp = 50, rbp = 51)]
        TsMatch3(boxed!(Self), #[tok(ATATAT, this)] boxed!(Self)),
        /// User-defined triple-less-than: `a <<< b`. Before `StrictlyLeft` (`<<`).
        #[parse(infix, lbp = 50, rbp = 51)]
        TripleLt(boxed!(Self), #[tok(LTLTLT, this)] boxed!(Self)),
        /// Geometric strictly-below: `a <<| b`. Before `StrictlyLeft` (`<<`).
        #[parse(infix, lbp = 50, rbp = 51)]
        StrictlyBelow(boxed!(Self), #[tok(LTLTPIPE, this)] boxed!(Self)),
        /// Inet is-subset-or-equal: `a <<= b`. Before `StrictlyLeft` (`<<`).
        #[parse(infix, lbp = 50, rbp = 51)]
        SubsetEq(boxed!(Self), #[tok(LTLTEQ, this)] boxed!(Self)),
        /// Distance: `a <-> b`. Before any `<` variant.
        #[parse(infix, lbp = 100, rbp = 101)]
        Distance(boxed!(Self), #[tok(LTMINUSGT, this)] boxed!(Self)),
        /// User-defined triple-greater-than: `a >>> b`. Before `StrictlyRight` (`>>`).
        #[parse(infix, lbp = 50, rbp = 51)]
        TripleGt(boxed!(Self), #[tok(GTGTGT, this)] boxed!(Self)),
        /// Inet is-superset-or-equal: `a >>= b`. Before `StrictlyRight` (`>>`).
        #[parse(infix, lbp = 50, rbp = 51)]
        SupersetEq(boxed!(Self), #[tok(GTGTEQ, this)] boxed!(Self)),
        /// Range adjacent: `a -|- b`. Before `Sub` (`-`).
        #[parse(infix, lbp = 50, rbp = 51)]
        Adjacent(boxed!(Self), #[tok(MINUSPIPEMINUS, this)] boxed!(Self)),
        /// Geometric strictly-above: `a |>> b`. Before `Concat` (`||`).
        #[parse(infix, lbp = 50, rbp = 51)]
        StrictlyAbove(boxed!(Self), #[tok(PIPEGTGT, this)] boxed!(Self)),
        /// Geometric no-extend-below: `a |&> b`. Before `Concat` (`||`).
        #[parse(infix, lbp = 50, rbp = 51)]
        NoExtendBelow(boxed!(Self), #[tok(PIPEAMPGT, this)] boxed!(Self)),
        /// Geometric no-extend-above: `a &<| b`. Before `NoExtendRight` (`&<`).
        #[parse(infix, lbp = 50, rbp = 51)]
        NoExtendAbove(boxed!(Self), #[tok(AMPLTPIPE, this)] boxed!(Self)),

        // --- 2-char operators ---
        /// Text-search / jsonb path match: `expr @@ expr`.
        #[parse(infix, lbp = 50, rbp = 51)]
        TsMatch(boxed!(Self), #[tok(ATAT, this)] boxed!(Self)),
        /// Jsonpath exists: `expr @? path`.
        #[parse(infix, lbp = 50, rbp = 51)]
        JsonPathExists(boxed!(Self), #[tok(ATQUESTION, this)] boxed!(Self)),
        /// Range / array overlap: `a && b`.
        #[parse(infix, lbp = 100, rbp = 101)]
        Overlap(boxed!(Self), #[tok(AMPAMP, this)] boxed!(Self)),
        /// Range does-not-extend-right: `a &< b`.
        #[parse(infix, lbp = 50, rbp = 51)]
        NoExtendRight(boxed!(Self), #[tok(AMPLT, this)] boxed!(Self)),
        /// Range does-not-extend-left: `a &> b`.
        #[parse(infix, lbp = 50, rbp = 51)]
        NoExtendLeft(boxed!(Self), #[tok(AMPGT, this)] boxed!(Self)),
        /// Range strictly-left-of: `a << b`.
        #[parse(infix, lbp = 50, rbp = 51)]
        StrictlyLeft(boxed!(Self), #[tok(LTLT, this)] boxed!(Self)),
        /// Range strictly-right-of: `a >> b`.
        #[parse(infix, lbp = 50, rbp = 51)]
        StrictlyRight(boxed!(Self), #[tok(GTGT, this)] boxed!(Self)),

        // --- User-defined / custom infix operators ---
        /// `expr === expr` — user-defined triple-equal. Must come before `Eq` (`=`).
        #[parse(infix, lbp = 50, rbp = 51)]
        TripleEq(boxed!(Self), #[tok(TRIPLEEQ, this)] boxed!(Self)),
        /// `expr !== expr` — user-defined not-equal. Must come before `BangEq` (`!=`).
        #[parse(infix, lbp = 50, rbp = 51)]
        BangEqEq(boxed!(Self), #[tok(BANGEQEQ, this)] boxed!(Self)),
        /// `expr ## expr` — geometric closest-point / path intersection.
        /// Must come before `BitXor` (`#`).
        #[parse(infix, lbp = 50, rbp = 51)]
        GeomClosest(boxed!(Self), #[tok(HASHHASH, this)] boxed!(Self)),

        #[parse(infix, lbp = 10, rbp = 11)]
        Or(boxed!(Self), #[tok(OR, this)] boxed!(Self)),
        #[parse(infix, lbp = 20, rbp = 21)]
        And(boxed!(Self), #[tok(AND, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        BangEq(boxed!(Self), #[tok(BANGEQ, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        Neq(boxed!(Self), #[tok(NEQ, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        Lte(boxed!(Self), #[tok(LTE, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        Gte(boxed!(Self), #[tok(GTE, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        Eq(boxed!(Self), #[tok(EQ, this)] boxed!(Self)),

        /// Text starts-with: `expr ^@ expr` (PostgreSQL `starts_with` operator).
        /// `^@` is a single token (see `punct::CaretAt`); declared before
        /// `CustomInfix` so it wins the declaration-order tiebreak. bp=8 matches
        /// Postgres's generic `Op` precedence.
        #[parse(infix, lbp = 80, rbp = 81)]
        StartsWith(boxed!(Self), #[tok(CARETAT, this)] boxed!(Self)),
        /// JSONB delete-path: `expr #- path` (PostgreSQL jsonb delete-at-path
        /// operator). `#-` is a single token (see `punct::HashMinus`); declared
        /// before `CustomInfix` so it wins the declaration-order tiebreak. bp=10
        /// matches the neighbouring `#>`/`#>>` JSON path operators.
        #[parse(infix, lbp = 100, rbp = 101)]
        JsonDeletePath(boxed!(Self), #[tok(HASHMINUS, this)] boxed!(Self)),

        /// Catch-all infix: any user-defined operator not matched by a specific
        /// token above. Declared BEFORE single-char operators so 2+ char custom
        /// operators like `<%` or `~>` aren't consumed as the single-char prefix
        /// (`<`, `~`) plus garbage. Since `CustomOp` requires 2+ characters, bare
        /// single-char operators still fall through to the variants below.
        /// bp=8 matches Postgres's generic `Op` precedence (between comparison
        /// bp=5 and additive bp=10).
        #[parse(infix, lbp = 80, rbp = 81)]
        CustomInfix(
            boxed!(Self),
            #[pretty(break_before = soft, break_after = soft)] literal::CustomOp,
            boxed!(Self),
        ),

        #[parse(infix, lbp = 50, rbp = 51)]
        Lt(boxed!(Self), #[tok(LT, this)] boxed!(Self)),
        #[parse(infix, lbp = 50, rbp = 51)]
        Gt(boxed!(Self), #[tok(GT, this)] boxed!(Self)),
        /// String concatenation: `expr || expr`. PostgreSQL scans `||` as a
        /// generic `Op`, below additive operators in the precedence hierarchy.
        #[parse(infix, lbp = 80, rbp = 81)]
        Concat(boxed!(Self), #[tok(CONCAT, this)] boxed!(Self)),
        /// Bitwise OR: `expr | expr`. Must come after `Concat` (`||`) so the
        /// longer token matches first at the punctuation level.
        #[parse(infix, lbp = 100, rbp = 101)]
        BitOr(boxed!(Self), #[tok(PIPE, this)] boxed!(Self)),
        /// Bitwise AND: `expr & expr`.
        #[parse(infix, lbp = 100, rbp = 101)]
        BitAnd(boxed!(Self), #[tok(AMP, this)] boxed!(Self)),
        /// Bitwise XOR: `expr # expr` (Postgres bit-string / integer operator).
        #[parse(infix, lbp = 100, rbp = 101)]
        BitXor(boxed!(Self), #[tok(POUND, this)] boxed!(Self)),
        #[parse(infix, lbp = 100, rbp = 101)]
        Add(boxed!(Self), #[tok(PLUS, this)] boxed!(Self)),
        #[parse(infix, lbp = 100, rbp = 101)]
        Sub(boxed!(Self), #[tok(MINUS, this)] boxed!(Self)),
        /// Multiplication: `expr * expr`
        #[parse(infix, lbp = 110, rbp = 111)]
        Mul(boxed!(Self), #[tok(STAR, this)] boxed!(Self)),
        /// Division: `expr / expr`
        #[parse(infix, lbp = 110, rbp = 111)]
        Div(boxed!(Self), #[tok(SLASH, this)] boxed!(Self)),
        /// Modulo: `expr % expr`
        #[parse(infix, lbp = 110, rbp = 111)]
        Mod(boxed!(Self), #[tok(PERCENT, this)] boxed!(Self)),
        /// Exponentiation: `expr ^ expr` (Postgres numeric power operator).
        #[parse(infix, lbp = 130, rbp = 131)]
        Pow(boxed!(Self), #[tok(CARET, this)] boxed!(Self)),

        // --- Atoms ---
        /// EXISTS subquery: `EXISTS (SELECT ...)`
        Exists(ExistsExpr),
        /// ARRAY constructor: `ARRAY[...]` or `ARRAY(...)`
        Array(ArrayExpr),
        /// ROW constructor: `ROW(...)`
        RowExpr(RowExpr),
        /// `GROUPING(...)` grouping-set membership function. Declared before
        /// `ColumnRef`, which would otherwise claim the bare `GROUPING` keyword
        /// and leave the argument list unparsed.
        Grouping(GroupingCall),
        /// CASE expression: `CASE [expr] WHEN ... THEN ... [ELSE ...] END`
        Case(CaseExpr),
        /// Unicode string literal: `U&'...'` with optional `UESCAPE 'c'`. Must
        /// come before `CastFunc` and `StringLit` for the same reason as
        /// `EscapeStringLit`.
        UnicodeStringLit(UnicodeStringLitWithEscape),
        /// Escape string literal: `E'foo\n'`. Must come before `CastFunc` and
        /// `StringLit` — `CastFunc` is `TypeName StringLit` and would match `e`
        /// as a type name followed by the string literal.
        EscapeStringLit(#[lex(pattern = r"(?i:E)'(?:[^'\\]|\\.|'')*'")] literal::EscapeStringLit),
        /// `TIMESTAMP [WITH|WITHOUT TIME ZONE] 'string'`.
        TimestampLit(TimestampLit),
        /// `TIME [WITH|WITHOUT TIME ZONE] 'string'`.
        TimeLit(TimeLit),
        /// `INTERVAL 'string' [qualifier]`. Must come before `CastFunc` since
        /// `interval` would otherwise parse as an ident-based TypeName.
        IntervalLit(IntervalLit),
        /// Function-style type cast: `bool 't'` -- must come before ColumnRef
        /// since type keywords like `bool` overlap with identifiers
        CastFunc(TypeCastFunc),
        /// `xmlelement(NAME ident [, xmlattributes(...)] [, content])`. Must come
        /// before `Func` so `xmlelement(` is matched as the special form.
        XmlElement(boxed!(XmlElement)),
        /// `xmlforest(expr [AS alias], ...)`. Before `Func` for the same reason.
        XmlForest(XmlForest),
        /// `xmlattributes(expr [AS alias], ...)`. Before `Func`.
        XmlAttributes(XmlAttributes),
        /// `xmlpi(NAME ident [, content])`. Before `Func`.
        XmlPi(XmlPi),
        /// `XMLSERIALIZE({DOCUMENT|CONTENT} expr AS type [[NO] INDENT])`. Before `Func`.
        XmlSerialize(boxed!(XmlSerialize)),
        /// `XMLPARSE({DOCUMENT|CONTENT} expr)`. Before `Func`.
        XmlParse(boxed!(XmlParse)),
        /// `XMLROOT(xml, VERSION ... [, STANDALONE ...])`. Before `Func`.
        XmlRoot(boxed!(XmlRoot)),
        /// `XMLEXISTS(xpath PASSING ... doc ...)`. Before `Func`.
        XmlExists(boxed!(XmlExists)),
        /// `TRIM([LEADING|TRAILING|BOTH] [chars] FROM source)`. Before `Func`
        /// since `trim` is also a valid function-call identifier.
        Trim(TrimCall),
        /// `CAST(expr AS type [COLLATE "c"])`. Before `Func`.
        CastCall(CastCall),
        /// `COLLATION FOR (expr)`. Before `Func`.
        CollationFor(CollationForCall),
        /// `SUBSTRING(source FROM ... | SIMILAR ...)`. Before `Func`.
        Substring(SubstringCall),
        /// `POSITION(needle IN haystack)`. Before `Func`.
        Position(PositionCall),
        /// `OVERLAY(source PLACING new FROM start [FOR len])`. Before `Func`.
        Overlay(OverlayCall),
        /// `EXTRACT(field FROM source)`. Before `Func`.
        Extract(ExtractCall),
        /// `JSON(...)` SQL/JSON value constructor. Before `Func`.
        JsonCtor(boxed!(JsonConstructor)),
        /// `JSON_SCALAR(...)`. Before `Func`.
        JsonScalar(boxed!(JsonScalar)),
        /// `JSON_SERIALIZE(...)`. Before `Func`.
        JsonSerialize(boxed!(JsonSerialize)),
        /// `JSON_OBJECT(...)` SQL/JSON object constructor. Before `Func`.
        JsonObject(boxed!(JsonObject)),
        /// `JSON_ARRAY(...)` SQL/JSON array constructor. Before `Func`.
        JsonArray(boxed!(JsonArray)),
        /// `JSON_EXISTS(...)` SQL/JSON path predicate. Before `Func`.
        JsonExists(boxed!(JsonExists)),
        /// `JSON_VALUE(...)` SQL/JSON scalar extraction. Before `Func`.
        JsonValue(boxed!(JsonValue)),
        /// `JSON_QUERY(...)` SQL/JSON value extraction. Before `Func`.
        JsonQuery(boxed!(JsonQuery)),
        /// `JSON_OBJECTAGG(...)` SQL/JSON object aggregate. Before `Func`.
        JsonObjectAgg(boxed!(JsonObjectAgg)),
        /// `JSON_ARRAYAGG(...)` SQL/JSON array aggregate. Before `Func`.
        JsonArrayAgg(boxed!(JsonArrayAgg)),
        /// Function call: `func(args)` -- must come before ColumnRef
        Func(boxed!(FuncCall)),
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
        /// Qualified reference: `table.column`, `schema.table.column`, or
        /// `schema.table.*` -- must come before ColumnRef.
        QualRef(QualifiedRef),
        /// Parenthesized scalar, row, or subquery, with optional field
        /// indirection. Its singleton expression route supplies Pretty's authored
        /// precedence grouping syntax.
        Parenthesized(ParenthesizedExpr),
        /// Numeric literal: `77.7` -- must come before IntegerLit for longest match
        NumericLit(literal::NumericLit),
        /// Integer literal: `42`
        IntegerLit(literal::IntegerLit),
        /// Dollar-quoted string literal: `$$...$$` or `$tag$...$tag$`.
        /// Listed before `StringLit` since it has a distinct prefix (`$`).
        DollarStringLit(literal::DollarStringLit),
        /// Bit-string literal: `B'10'`. Must come before `StringLit` (and before
        /// any plain `Ident` / `ColumnRef`) for the same reason as
        /// `EscapeStringLit`: the lexer's longest-match-wins picks
        /// `BitStringLit` over `Ident`+`StringLit` only when the prefixed token
        /// is also declared first at the atom level. Without this ordering, the
        /// formatter would round-trip `B'10'` as `B '10'` (inserted space).
        BitStringLit(#[lex(pattern = r"(?i:B)'[^']*'")] literal::BitStringLit),
        /// Hex-string literal: `X'1FF'`. Same ordering rationale as
        /// `BitStringLit` — must precede `StringLit` and any plain `Ident`.
        HexStringLit(#[lex(pattern = r"(?i:X)'[^']*'")] literal::HexStringLit),
        /// String literal sequence: `'hello'` or `'first' 'second' ...` —
        /// Postgres concatenates adjacent string literals into one.
        StringLit(StringLitSeq0),
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
        PositionalParam(PositionalParam),
        /// Unqualified column reference: `f1` or `"Foo"`, with its subscripts
        ColumnRef(ColumnRef),
    }
}
