//! SQL expression AST: gram.y's `a_expr` as an ordinary left-recursive enum,
//! plus `b_expr` as the restricted expression `BExpr`.
//!
//! Holds atoms, prefix operators (`NOT`, unary minus, the prefix operator
//! spellings), infix operators (`AND`, `OR`, comparisons, arithmetic) and
//! postfix operators (`::` cast, `IS [NOT] TRUE/FALSE/UNKNOWN/NULL`,
//! `IN (list)`). Which of two operators binds tighter is decided only by the
//! `precedence { ... }` block of [`crate::tokens`], which mirrors
//! `gram.y:829-908`.
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
    /// part after the leading `IS`. Modelled as an enum so that
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
    #[flat(pool)]
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
    #[flat(pool)]
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

// Operators of PostgreSQL's `subquery_Op` production, split into six enums.
// gram.y writes one production, `a_expr subquery_Op sub_type ... %prec Op`
// (gram.y:15152, 15164), and every `Expr::QuantifiedComparison*` variant
// takes that same `Op` level through `#[parse(prec = Op)]`. The split is an
// AST commitment, not a precedence one: each suffix names the operator family
// its variant holds, and collapsing them would change the public AST. Together
// the six enums cover `OperatorName`, `OPERATOR(...)` and the LIKE family
// exactly once.

recursa::ast_node! {
    /// `subquery_Op`'s comparison spellings: gram.y `MathOp`'s
    /// `< > = <= >= <>` and the operator spellings pg-sql parses as
    /// comparisons.
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
    /// `subquery_Op`'s `LIKE | NOT_LA LIKE | ILIKE | NOT_LA ILIKE`.
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
    /// `subquery_Op`'s generic operator spellings: `||`, `^@`, every spelling
    /// that is only a prefix operator elsewhere in `Expr`, the multi-character
    /// custom operators, and `OPERATOR(any_operator)` (gram.y:889
    /// `%left Op OPERATOR`).
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
    /// `subquery_Op`'s `+` and `-`, and the bitwise and JSON operator
    /// spellings PostgreSQL's scanner returns as `Op`.
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
    /// `subquery_Op`'s `*`, `/` and `%`.
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
    /// `subquery_Op`'s `^`.
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
    /// `COALESCE(expr, ...)`: gram.y:15848 `func_expr_common_subexpr: COALESCE
    /// '(' expr_list ')'`, PostgreSQL's `CoalesceExpr`.
    ///
    /// `COALESCE` is a `COL_NAME` keyword, so it is a `ColId` but not a
    /// `type_function_name`: a bare `coalesce` is a column reference and
    /// `coalesce(...)` is only ever this production. `expr_list` has neither
    /// `*`, `DISTINCT`, `VARIADIC` nor named arguments, and cannot be empty.
    #[derive(Debug)]
    #[tok(COALESCE, LPAREN, this, RPAREN)]
    pub struct CoalesceExpr {
        #[sep(COMMA)]
        pub args: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// `GREATEST(expr, ...)`: gram.y:15856 `func_expr_common_subexpr: GREATEST
    /// '(' expr_list ')'`, PostgreSQL's `MinMaxExpr` with `IS_GREATEST`.
    /// `GREATEST` is a `COL_NAME` keyword, as [`CoalesceExpr`] describes.
    #[derive(Debug)]
    #[tok(GREATEST, LPAREN, this, RPAREN)]
    pub struct GreatestExpr {
        #[sep(COMMA)]
        pub args: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// `LEAST(expr, ...)`: gram.y:15865 `func_expr_common_subexpr: LEAST '('
    /// expr_list ')'`, PostgreSQL's `MinMaxExpr` with `IS_LEAST`. `LEAST` is a
    /// `COL_NAME` keyword, as [`CoalesceExpr`] describes.
    #[derive(Debug)]
    #[tok(LEAST, LPAREN, this, RPAREN)]
    pub struct LeastExpr {
        #[sep(COMMA)]
        pub args: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// `NULLIF(left, right)`: gram.y:15844 `func_expr_common_subexpr: NULLIF
    /// '(' a_expr ',' a_expr ')'`, PostgreSQL's `A_Expr` of kind
    /// `AEXPR_NULLIF`. It takes exactly two arguments. `NULLIF` is a
    /// `COL_NAME` keyword, as [`CoalesceExpr`] describes.
    #[derive(Debug)]
    #[tok(NULLIF, LPAREN, this, RPAREN)]
    pub struct NullIfExpr {
        pub left: boxed!(Expr),
        #[tok(COMMA, this)]
        pub right: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// The `COALESCE`, `GREATEST`, `LEAST` and `NULLIF` forms where gram.y
    /// writes `func_expr_windowless` (gram.y:15637) and not an expression: a
    /// `func_table`, a `rowsfrom_item`, an `index_elem` and a `part_elem`.
    /// `func_expr_windowless` is `func_application | func_expr_common_subexpr
    /// | ...`, and a `COL_NAME` keyword is never the name of a
    /// `func_application`, so each position names these forms itself.
    /// [`Expr`] holds the same four nodes as variants of its own.
    #[derive(Debug)]
    pub enum CommonSubexprCall {
        Coalesce(CoalesceExpr),
        Greatest(GreatestExpr),
        Least(LeastExpr),
        NullIf(NullIfExpr),
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
        // gram.y `position_list: b_expr IN_P b_expr`: the restricted
        // expression stops the needle before `IN`, so `IN` can only be the
        // delimiter here.
        pub needle: boxed!(BExpr),
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
// dedicated `Expr` atom declared before `Func`.
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
    /// Lets a context outside `Expr` (for instance a `CREATE INDEX`
    /// expression element) accept the whole family. Aggregates and `JSON_TABLE` are excluded:
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

// --- The expression enum ---
//
// `Expr` is an ordinary left-recursive enum, as gram.y's `a_expr` is an
// ordinary left-recursive nonterminal. Every operator token attaches to the
// operand field it precedes or follows, and the shape of a variant is the
// shape of the gram.y production it mirrors.
//
// Operator conflicts are decided by one model: the `precedence { ... }` block
// in `crate::tokens`, which mirrors `gram.y:829-908` level for level. A rule
// takes the level of its last terminal that has one; a
// `#[parse(prec = NAME)]` on the variant overrides that, exactly as bison
// reads `%prec`.
//
// The overrides below are the ones gram.y needs, for the reason gram.y states
// at gram.y:14774-14778: a production with more than one terminal takes the
// level of its *last* terminal, which is almost never the one you want. Every
// override cites its gram.y line.
//
// gram.y also writes `%prec Op` on `a_expr qual_Op a_expr` (gram.y:14844) and
// on `b_expr qual_Op b_expr` (gram.y:15322) because `qual_Op` is a
// nonterminal, so those rules hold no terminal of their own. pg-sql spells
// each operator as its own token, so an infix operator variant already ends
// in a terminal at gram.y's `Op` level and needs no override; the prefix
// forms carry one anyway, because gram.y annotates `qual_Op a_expr`
// (gram.y:14846) and the annotation is where a reader looks for it. The
// variants that genuinely have no terminal -- the `QuantifiedComparison*`
// family and `NotInExpr`, whose operators live in a suffix Node -- carry the
// override because nothing else can give them a level.

recursa::ast_node! {
    /// SQL expression: gram.y's `a_expr`, with `c_expr`'s atoms inlined as
    /// variants. Its operator conflicts are decided by the declared precedence
    /// of `crate::tokens`.
    #[derive(Debug)]
    // `FlatExpr` measures 16 bytes (a discriminant word plus three handle
    // words); the bound is that size, so any widening of a variant's payload
    // fails to compile rather than silently growing the hottest typed pool.
    // Raising it is a measured decision, not a reflex.
    #[flat(pool, max_size = 16)]
    pub enum Expr {
        // --- Prefix ---
        /// gram.y:14853 `NOT a_expr` and gram.y:14855 `NOT_LA a_expr %prec NOT`:
        /// the `NOT_LA` twin sits at `BETWEEN`'s level as a token, so the override
        /// is what keeps both spellings at `NOT`'s own level.
        #[parse(prec = NOT)]
        Not(#[tok(NOT, this)] boxed!(Self)),
        /// gram.y:14817 `'-' a_expr %prec UMINUS`.
        #[parse(prec = UMINUS)]
        Neg(#[tok(MINUS, this)] boxed!(Self)),
        /// Unary plus: `+expr` — identity operator on numeric types.
        /// gram.y:14815 `'+' a_expr %prec UMINUS`.
        #[parse(prec = UMINUS)]
        Pos(#[tok(PLUS, this)] boxed!(Self)),
        /// Unary geometric "center point": `@@ expr`. Postgres uses `@@` as
        /// a prefix operator on box / polygon / etc. (in addition to the
        /// text-search infix form).
        #[parse(prec = Op)]
        GeomCenter(#[tok(ATAT, this)] boxed!(Self)),
        /// Bitwise NOT: `~ expr` (e.g. inet / bit / int bitwise complement).
        /// Must come before any infix `~` variant so the prefix form wins when
        /// `~` appears at the start of an operand.
        #[parse(prec = Op)]
        BitNot(#[tok(TILDE, this)] boxed!(Self)),
        /// Geometric path/lseg length: `@-@ expr`. Must come before `Abs` (`@`)
        /// since `@-@` is longer.
        #[parse(prec = Op)]
        PathLength(#[tok(ATMINUSAT, this)] boxed!(Self)),
        /// User-defined prefix: `@#@ expr` (e.g. factorial).
        #[parse(prec = Op)]
        AtHashAtPrefix(#[tok(ATHASHAT, this)] boxed!(Self)),
        /// Geometric point-count: `# path` — number of points in a path.
        #[parse(prec = Op)]
        PointCount(#[tok(POUND, this)] boxed!(Self)),
        /// Absolute value: `@ expr` (Postgres unary `@` operator).
        #[parse(prec = Op)]
        Abs(#[tok(ATSIGN, this)] boxed!(Self)),
        /// User-defined prefix: `!=- expr`.
        #[parse(prec = Op)]
        BangEqMinusPrefix(#[tok(BANGEQMINUS, this)] boxed!(Self)),
        /// Square root: `|/ expr` (Postgres unary `|/` operator).
        #[parse(prec = Op)]
        Sqrt(#[tok(PIPESLASH, this)] boxed!(Self)),
        /// Cube root: `||/ expr` (Postgres unary `||/` operator).
        #[parse(prec = Op)]
        Cbrt(#[tok(PIPEPIPESLASH, this)] boxed!(Self)),

        /// Catch-all prefix: any user-defined prefix operator not matched by a
        /// specific token. Declared LAST among prefixes.
        ///
        /// gram.y:14846 `qual_Op a_expr %prec Op` (gram.y:889
        /// `%left Op OPERATOR`): the operand extends over the operators above
        /// `Op` (`@# a + b` is `@# (a + b)`) and stops at `Op` and below
        /// (`@# a = b` is `(@# a) = b`, `@# a @# b` is `(@# a) @# b`).
        /// `CustomOp` is the content token gram.y calls `qual_Op`, and like
        /// gram.y's nonterminal it gives the rule no level of its own, so the
        /// override is what carries `Op` here.
        #[parse(prec = Op)]
        CustomPrefix(
            literal::CustomOp,
            #[pretty(break_before = soft)] boxed!(Self),
        ),

        // --- Postfix ---
        /// Postgres-style cast: `expr::type`
        Cast(boxed!(Self), #[tok(COLONCOLON, this)] boxed!(CastType)),
        /// `expr COLLATE "collation"` — collation specifier. Binds tighter than
        /// comparisons (bp 5) but looser than `::` cast (bp 20).
        Collate(boxed!(Self), #[tok(COLLATE, this)] crate::tokens::ColId),
        /// `lhs operator {ANY|SOME|ALL} (expr-or-query)`: gram.y `a_expr
        /// subquery_Op sub_type '(' a_expr ')' %prec Op` and its
        /// `select_with_parens` twin. gram.y decides the shift before the
        /// gram.y gives every such production one level, `%prec Op`, and so
        /// does pg-sql: the six variants differ only in which operator family
        /// their suffix holds, which is an AST commitment, not a precedence
        /// one.
        ///
        /// PostgreSQL does not admit the quantified right-hand side as a
        /// standalone expression. Keeping the operator and quantifier in one
        /// suffix also makes `f(ALL(x))` unambiguously the function
        /// application's ALL-qualified argument production.
        #[parse(prec = Op)]
        QuantifiedComparisonCmp(boxed!(Self), QuantifiedComparisonCmpSuffix),
        /// `subquery_Op`'s `LIKE` family.
        #[parse(prec = Op)]
        QuantifiedComparisonLike(boxed!(Self), QuantifiedComparisonLikeSuffix),
        /// `subquery_Op`'s generic operator spellings.
        #[parse(prec = Op)]
        QuantifiedComparisonOp(boxed!(Self), QuantifiedComparisonOpSuffix),
        /// `subquery_Op`'s `+` and `-` and the operator spellings beside them.
        #[parse(prec = Op)]
        QuantifiedComparisonAdd(boxed!(Self), QuantifiedComparisonAddSuffix),
        /// `subquery_Op`'s `*`, `/` and `%`.
        #[parse(prec = Op)]
        QuantifiedComparisonMul(boxed!(Self), QuantifiedComparisonMulSuffix),
        /// `subquery_Op`'s `^`.
        #[parse(prec = Op)]
        QuantifiedComparisonPow(boxed!(Self), QuantifiedComparisonPowSuffix),
        // The `IS` family sits one level below the comparison operators,
        // gram.y:835 `%nonassoc IS ISNULL NOTNULL` (`a = b IS NULL` is
        // `(a = b) IS NULL`). Every `IS` form whose rule ends in `IS` takes
        // that level from the token; the two `DISTINCT FROM` forms end in
        // `FROM` and carry `%prec IS` as gram.y does.
        /// `expr IS NOT DISTINCT FROM expr`. Declared before `IsDistinctFrom` so
        /// the longer `NOT` prefix wins disambiguation.
        /// gram.y:15072 `a_expr IS NOT DISTINCT FROM a_expr %prec IS`: the rule
        /// ends in `FROM`, which has no level of its own.
        #[parse(prec = IS)]
        IsNotDistinctFrom(
            boxed!(Self),
            #[tok(IS, NOT, DISTINCT, FROM, this)] boxed!(Self),
        ),
        /// `expr IS DISTINCT FROM expr`.
        /// gram.y:15068 `a_expr IS DISTINCT FROM a_expr %prec IS`.
        #[parse(prec = IS)]
        IsDistinctFrom(boxed!(Self), #[tok(IS, DISTINCT, FROM, this)] boxed!(Self)),
        /// `expr IS [NOT] JSON [{VALUE|SCALAR|ARRAY|OBJECT}] [{WITH|WITHOUT}
        /// UNIQUE [KEYS]]` — the SQL/JSON type predicate. Declared before
        /// `BoolTest` (both lead with `IS`); `BoolTest` rejects `JSON` as a
        /// `BoolTestKind`, so order is not load-bearing, only tidy.
        IsJson(boxed!(Self), #[tok(IS, this)] IsJsonTail),
        /// `expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` — the Unicode
        /// normalisation predicate (gram.y rules 15198/15205/15212/15220).
        /// Declared before `BoolTest` (both lead with `IS`); `BoolTest` rejects
        /// `NORMALIZED`/`NFx` as a `BoolTestKind`, so order is not load-bearing.
        IsNormalized(boxed!(Self), #[tok(IS, this)] IsNormalizedTail),
        /// `expr IS [NOT] DOCUMENT` — the XML document predicate.
        IsDocument(boxed!(Self), #[tok(IS, this)] IsDocumentTail),
        /// Boolean test: `expr IS [NOT] TRUE/FALSE/UNKNOWN/NULL`
        BoolTest(boxed!(Self), #[tok(IS, this)] BoolTestKind),
        /// Postgres `expr NOTNULL` postfix null test (synonym for `IS NOT NULL`).
        Notnull(#[tok(this, NOTNULL)] boxed!(Self)),
        /// Postgres `expr ISNULL` postfix null test (synonym for `IS NULL`).
        Isnull(#[tok(this, ISNULL)] boxed!(Self)),
        /// `expr AT LOCAL` — convert to session timezone. Listed before
        /// `AtTimeZone` so `AT LOCAL` wins (distinct second token `LOCAL` vs `TIME`).
        /// gram.y:14799 `a_expr AT LOCAL %prec AT`.
        #[parse(prec = AT)]
        AtLocal(#[tok(this, AT, LOCAL)] boxed!(Self)),
        /// `expr AT TIME ZONE zone_expr` — convert to specified timezone.
        /// gram.y:14792 `a_expr AT TIME ZONE a_expr %prec AT`.
        #[parse(prec = AT)]
        AtTimeZone(boxed!(Self), #[tok(AT, TIME, ZONE, this)] boxed!(Self)),
        /// NOT IN list: `expr NOT IN (val, ...)`
        /// gram.y:15129 `a_expr NOT_LA IN_P in_expr %prec NOT_LA`: the suffix is
        /// its own Node, so the rule has no terminal to take a level from.
        #[parse(prec = NOT_LA)]
        NotInExpr(boxed!(Self), NotInSuffix),
        // The LIKE family shares one level with `BETWEEN` and `IN`, gram.y:837
        // `%nonassoc BETWEEN IN_P LIKE ILIKE SIMILAR NOT_LA`, one step above
        // the comparison operators: `a = b LIKE c` is `a = (b LIKE c)`. The
        // level is non-associative, so a LIKE never nests in a LIKE's pattern
        // and an `ESCAPE` belongs to the LIKE it follows.
        //
        // gram.y writes each `ESCAPE` form as its own production,
        // `a_expr LIKE a_expr ESCAPE a_expr %prec LIKE` (gram.y:14863). pg-sql
        // writes one variant with an attached optional operand, which lowers to
        // the same pair of rules; `%nonassoc ESCAPE` one level above `LIKE`
        // (gram.y:838) is what makes the parser shift the clause rather than
        // end the expression, so the escape operand takes every operator above
        // `ESCAPE` (`'$'::bytea`, `'$' || 'x'`) and stops at `LIKE`'s level and
        // below.
        /// `expr NOT ILIKE pattern [ESCAPE char]`. Declared before `NotLike` so the longer
        /// `NOT ILIKE` is tried first (matters only if any rule shares a prefix;
        /// here `NOT ILIKE` vs `NOT LIKE` differ on the second token).
        /// gram.y:14900 `a_expr NOT_LA ILIKE a_expr %prec NOT_LA` and gram.y:14905
        /// with `ESCAPE`.
        #[parse(prec = NOT_LA)]
        NotIlike(
            boxed!(Self),
            #[tok(NOT, ILIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr NOT SIMILAR TO pattern [ESCAPE char]`. Declared before `NotLike` so the longer
        /// `NOT SIMILAR TO` form wins longest-match-wins disambiguation.
        /// gram.y:14933 `a_expr NOT_LA SIMILAR TO a_expr %prec NOT_LA` and
        /// gram.y:14942 with `ESCAPE`: the rule ends in `TO`, which has no level.
        #[parse(prec = NOT_LA)]
        NotSimilarTo(
            boxed!(Self),
            #[tok(NOT, SIMILAR, TO, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr NOT LIKE pattern [ESCAPE char]`. Must come before the `Not` prefix atom so
        /// longest-match-wins prefers the postfix form.
        /// gram.y:14872 `a_expr NOT_LA LIKE a_expr %prec NOT_LA` and gram.y:14877
        /// with `ESCAPE`.
        #[parse(prec = NOT_LA)]
        NotLike(
            boxed!(Self),
            #[tok(NOT, LIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr SIMILAR TO pattern [ESCAPE char]` — SQL standard similar-to pattern match.
        /// gram.y:14915 `a_expr SIMILAR TO a_expr %prec SIMILAR` and gram.y:14924
        /// with `ESCAPE`: the rule ends in `TO`, which has no level.
        #[parse(prec = SIMILAR)]
        SimilarTo(
            boxed!(Self),
            #[tok(SIMILAR, TO, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr ILIKE pattern [ESCAPE char]`
        /// gram.y:14891 `a_expr ILIKE a_expr ESCAPE a_expr %prec ILIKE`.
        #[parse(prec = ILIKE)]
        Ilike(
            boxed!(Self),
            #[tok(ILIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        /// `expr LIKE pattern [ESCAPE char]`
        /// gram.y:14863 `a_expr LIKE a_expr ESCAPE a_expr %prec LIKE`.
        #[parse(prec = LIKE)]
        Like(
            boxed!(Self),
            #[tok(LIKE, this)] boxed!(Self),
            #[tok(ESCAPE, this)] Option<boxed!(Self)>,
        ),
        // --- Locale-aware text comparison operators (4-char before 3-char) ---
        /// `expr ~<=~ expr` — locale-aware less-or-equal.
        TildeLeqTilde(boxed!(Self), #[tok(TILDELEQTILDE, this)] boxed!(Self)),
        /// `expr ~>=~ expr` — locale-aware greater-or-equal.
        TildeGeqTilde(boxed!(Self), #[tok(TILDEGEQTILDE, this)] boxed!(Self)),
        /// `expr ~<~ expr` — locale-aware less-than.
        TildeLtTilde(boxed!(Self), #[tok(TILDELTTILDE, this)] boxed!(Self)),
        /// `expr ~>~ expr` — locale-aware greater-than.
        TildeGtTilde(boxed!(Self), #[tok(TILDEGTTILDE, this)] boxed!(Self)),
        /// `expr !~* pattern` — POSIX case-insensitive negated regex match.
        RegexNotIMatch(boxed!(Self), #[tok(BANGTILDESTAR, this)] boxed!(Self)),
        /// `expr ~* pattern` — POSIX case-insensitive regex match.
        RegexIMatch(boxed!(Self), #[tok(TILDESTAR, this)] boxed!(Self)),
        /// `expr !~ pattern` — POSIX negated regex match.
        RegexNotMatch(boxed!(Self), #[tok(BANGTILDE, this)] boxed!(Self)),
        /// `expr ~= expr` — geometric "same as" operator. Declared before `RegexMatch`
        /// so the longer `~=` wins longest-match.
        GeomSame(boxed!(Self), #[tok(TILDEEQ, this)] boxed!(Self)),
        /// `expr ~ pattern` — POSIX regex match.
        RegexMatch(boxed!(Self), #[tok(TILDE, this)] boxed!(Self)),
        /// `expr !~~* pattern` — operator-form `NOT ILIKE` (gram.y 14897).
        /// Declared before `LikeOpINeg` (`!~~`) so the longer `!~~*` wins.
        LikeOpINeg(boxed!(Self), #[tok(BANGTILDETILDESTAR, this)] boxed!(Self)),
        /// `expr ~~* pattern` — operator-form `ILIKE` (gram.y 14888).
        /// Declared before `LikeOpI` would be (no `~~*` longer prefix).
        LikeOpI(boxed!(Self), #[tok(TILDETILDESTAR, this)] boxed!(Self)),
        /// `expr !~~ pattern` — operator-form `NOT LIKE` (gram.y 14874).
        LikeOpNeg(boxed!(Self), #[tok(BANGTILDETILDE, this)] boxed!(Self)),
        /// `expr ~~ pattern` — operator-form `LIKE` (gram.y 14860).
        LikeOp(boxed!(Self), #[tok(TILDETILDE, this)] boxed!(Self)),
        /// `(start, end) OVERLAPS (start, end)` — SQL time-period overlap test.
        /// Each operand is an ordinary parenthesized expression to the parser.
        Overlaps(boxed!(Self), #[tok(OVERLAPS, this)] boxed!(Self)),
        /// Record comparison operators: `expr *= expr`, `*<>`, `*<`, `*<=`,
        /// `*>`, `*>=` — compare ROW/composite values field by field.
        RecordLte(boxed!(Self), #[tok(STARLTE, this)] boxed!(Self)),
        RecordGte(boxed!(Self), #[tok(STARGTE, this)] boxed!(Self)),
        RecordNeq(boxed!(Self), #[tok(STARNEQ, this)] boxed!(Self)),
        RecordLt(boxed!(Self), #[tok(STARLT, this)] boxed!(Self)),
        RecordGt(boxed!(Self), #[tok(STARGT, this)] boxed!(Self)),
        RecordEq(boxed!(Self), #[tok(STAREQ, this)] boxed!(Self)),
        /// IN list: `expr IN (val, ...)`
        InExpr(boxed!(Self), #[tok(IN, this)] InList),
        /// `expr NOT BETWEEN low AND high`. The low operand is the restricted
        /// expression `BExpr`, which is how gram.y stops it before the `AND`
        /// that closes the clause.
        ///
        /// gram.y:15084 `a_expr NOT_LA BETWEEN opt_asymmetric b_expr AND a_expr
        /// %prec NOT_LA`: the rule ends in `AND`, whose own level would make the
        /// whole form bind looser than a boolean operand.
        #[parse(prec = NOT_LA)]
        NotBetweenExpr(
            boxed!(Self),
            #[tok(NOT, BETWEEN, this)] boxed!(BExpr),
            #[tok(AND, this)] boxed!(Self),
        ),
        /// `expr BETWEEN low AND high`. The low operand is the restricted
        /// expression `BExpr`, as in `NotBetweenExpr`.
        ///
        /// gram.y:15076 `a_expr BETWEEN opt_asymmetric b_expr AND a_expr
        /// %prec BETWEEN`: the rule ends in `AND`, as above.
        #[parse(prec = BETWEEN)]
        BetweenExpr(
            boxed!(Self),
            #[tok(BETWEEN, this)] boxed!(BExpr),
            #[tok(AND, this)] boxed!(Self),
        ),

        // --- Infix ---
        // Multi-char operators before single-char to avoid partial matching.
        //
        // JSON / JSONB operators are listed FIRST among infix so that their
        // longer tokens are peeked before conflicting shorter ones
        // (e.g. `<@` before `<`, `->` before `-`). Every one of them is a
        // spelling PostgreSQL's scanner returns as `Op`, so they all sit at
        // gram.y's single `Op` level (gram.y:889).
        /// JSON path as text: `expr #>> path`
        JsonPathText(boxed!(Self), #[tok(HASHARROWARROW, this)] boxed!(Self)),
        /// JSON path: `expr #> path`
        JsonPath(boxed!(Self), #[tok(HASHARROW, this)] boxed!(Self)),
        /// JSON field as text: `expr ->> field`
        JsonFieldText(boxed!(Self), #[tok(ARROWARROW, this)] boxed!(Self)),
        /// JSON field: `expr -> field`
        JsonField(boxed!(Self), #[tok(ARROW, this)] boxed!(Self)),
        /// Geometric parallel: `a ?|| b`. Must precede `JsonAnyKey` (`?|`)
        /// so the 3-char token wins over the 2-char token.
        Parallel(boxed!(Self), #[tok(QUESTIONPIPEPIPE, this)] boxed!(Self)),
        /// JSON any-key-exists: `expr ?| keys`
        JsonAnyKey(boxed!(Self), #[tok(QUESTIONPIPE, this)] boxed!(Self)),
        /// JSON all-keys-exist: `expr ?& keys`
        JsonAllKeys(boxed!(Self), #[tok(QUESTIONAMP, this)] boxed!(Self)),
        /// Geometric intersect: `a ?# b`. Must precede `JsonKey` (`?`).
        Intersect(boxed!(Self), #[tok(QUESTIONHASH, this)] boxed!(Self)),
        /// Geometric perpendicular: `a ?-| b`. Must precede `Horizontal` (`?-`)
        /// so the 3-char token wins over the 2-char token.
        Perpendicular(boxed!(Self), #[tok(QUESTIONDASHPIPE, this)] boxed!(Self)),
        /// Geometric horizontal: `a ?- b`. Must precede `JsonKey` (`?`).
        Horizontal(boxed!(Self), #[tok(QUESTIONDASH, this)] boxed!(Self)),
        /// Geometric "is horizontal" prefix: `?- s` — tests whether the
        /// LSEG/LINE `s` is horizontal. PG's geometry.sql uses this in WHERE.
        #[parse(prec = Op)]
        IsHorizontal(#[tok(QUESTIONDASH, this)] boxed!(Self)),
        /// Geometric "is vertical" prefix: `?| s`.
        #[parse(prec = Op)]
        IsVertical(#[tok(QUESTIONPIPE, this)] boxed!(Self)),
        /// Geometric "below": `a <^ b`.
        Below(boxed!(Self), #[tok(LTCARET, this)] boxed!(Self)),
        /// Geometric "above": `a >^ b`.
        Above(boxed!(Self), #[tok(GTCARET, this)] boxed!(Self)),
        /// JSON key-exists: `expr ? key`
        JsonKey(boxed!(Self), #[tok(QUESTION, this)] boxed!(Self)),
        /// JSONB contains: `expr @> expr`
        JsonContains(boxed!(Self), #[tok(ATGT, this)] boxed!(Self)),
        /// JSONB contained-by: `expr <@ expr`
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
        TsMatch3(boxed!(Self), #[tok(ATATAT, this)] boxed!(Self)),
        /// User-defined triple-less-than: `a <<< b`. Before `StrictlyLeft` (`<<`).
        TripleLt(boxed!(Self), #[tok(LTLTLT, this)] boxed!(Self)),
        /// Geometric strictly-below: `a <<| b`. Before `StrictlyLeft` (`<<`).
        StrictlyBelow(boxed!(Self), #[tok(LTLTPIPE, this)] boxed!(Self)),
        /// Inet is-subset-or-equal: `a <<= b`. Before `StrictlyLeft` (`<<`).
        SubsetEq(boxed!(Self), #[tok(LTLTEQ, this)] boxed!(Self)),
        /// Distance: `a <-> b`. Before any `<` variant.
        Distance(boxed!(Self), #[tok(LTMINUSGT, this)] boxed!(Self)),
        /// User-defined triple-greater-than: `a >>> b`. Before `StrictlyRight` (`>>`).
        TripleGt(boxed!(Self), #[tok(GTGTGT, this)] boxed!(Self)),
        /// Inet is-superset-or-equal: `a >>= b`. Before `StrictlyRight` (`>>`).
        SupersetEq(boxed!(Self), #[tok(GTGTEQ, this)] boxed!(Self)),
        /// Range adjacent: `a -|- b`. Before `Sub` (`-`).
        Adjacent(boxed!(Self), #[tok(MINUSPIPEMINUS, this)] boxed!(Self)),
        /// Geometric strictly-above: `a |>> b`. Before `Concat` (`||`).
        StrictlyAbove(boxed!(Self), #[tok(PIPEGTGT, this)] boxed!(Self)),
        /// Geometric no-extend-below: `a |&> b`. Before `Concat` (`||`).
        NoExtendBelow(boxed!(Self), #[tok(PIPEAMPGT, this)] boxed!(Self)),
        /// Geometric no-extend-above: `a &<| b`. Before `NoExtendRight` (`&<`).
        NoExtendAbove(boxed!(Self), #[tok(AMPLTPIPE, this)] boxed!(Self)),

        // --- 2-char operators ---
        /// Text-search / jsonb path match: `expr @@ expr`.
        TsMatch(boxed!(Self), #[tok(ATAT, this)] boxed!(Self)),
        /// Jsonpath exists: `expr @? path`.
        JsonPathExists(boxed!(Self), #[tok(ATQUESTION, this)] boxed!(Self)),
        /// Range / array overlap: `a && b`.
        Overlap(boxed!(Self), #[tok(AMPAMP, this)] boxed!(Self)),
        /// Range does-not-extend-right: `a &< b`.
        NoExtendRight(boxed!(Self), #[tok(AMPLT, this)] boxed!(Self)),
        /// Range does-not-extend-left: `a &> b`.
        NoExtendLeft(boxed!(Self), #[tok(AMPGT, this)] boxed!(Self)),
        /// Range strictly-left-of: `a << b`.
        StrictlyLeft(boxed!(Self), #[tok(LTLT, this)] boxed!(Self)),
        /// Range strictly-right-of: `a >> b`.
        StrictlyRight(boxed!(Self), #[tok(GTGT, this)] boxed!(Self)),

        // --- User-defined / custom infix operators ---
        /// `expr === expr` — user-defined triple-equal. Must come before `Eq` (`=`).
        TripleEq(boxed!(Self), #[tok(TRIPLEEQ, this)] boxed!(Self)),
        /// `expr !== expr` — user-defined not-equal. Must come before `BangEq` (`!=`).
        BangEqEq(boxed!(Self), #[tok(BANGEQEQ, this)] boxed!(Self)),
        /// `expr ## expr` — geometric closest-point / path intersection.
        /// Must come before `BitXor` (`#`).
        GeomClosest(boxed!(Self), #[tok(HASHHASH, this)] boxed!(Self)),

        Or(boxed!(Self), #[tok(OR, this)] boxed!(Self)),
        And(boxed!(Self), #[tok(AND, this)] boxed!(Self)),
        BangEq(boxed!(Self), #[tok(BANGEQ, this)] boxed!(Self)),
        Neq(boxed!(Self), #[tok(NEQ, this)] boxed!(Self)),
        Lte(boxed!(Self), #[tok(LTE, this)] boxed!(Self)),
        Gte(boxed!(Self), #[tok(GTE, this)] boxed!(Self)),
        Eq(boxed!(Self), #[tok(EQ, this)] boxed!(Self)),

        /// Text starts-with: `expr ^@ expr` (PostgreSQL `starts_with` operator).
        /// `^@` is a single token (see `punct::CaretAt`); declared before
        /// `CustomInfix` so it wins the declaration-order tiebreak. bp=8 matches
        /// Postgres's generic `Op` precedence.
        StartsWith(boxed!(Self), #[tok(CARETAT, this)] boxed!(Self)),
        /// JSONB delete-path: `expr #- path` (PostgreSQL jsonb delete-at-path
        /// operator). `#-` is a single token (see `punct::HashMinus`); declared
        /// before `CustomInfix` so it wins the declaration-order tiebreak. bp=10
        /// matches the neighbouring `#>`/`#>>` JSON path operators.
        JsonDeletePath(boxed!(Self), #[tok(HASHMINUS, this)] boxed!(Self)),

        /// Catch-all infix: any user-defined operator not matched by a specific
        /// token above. Declared BEFORE single-char operators so 2+ char custom
        /// operators like `<%` or `~>` aren't consumed as the single-char prefix
        /// (`<`, `~`) plus garbage. Since `CustomOp` requires 2+ characters, bare
        /// single-char operators still fall through to the variants below.
        /// bp=8 matches Postgres's generic `Op` precedence (between comparison
        /// bp=5 and additive bp=10).
        #[parse(prec = Op)]
        CustomInfix(
            boxed!(Self),
            #[pretty(break_before = soft, break_after = soft)] literal::CustomOp,
            boxed!(Self),
        ),

        Lt(boxed!(Self), #[tok(LT, this)] boxed!(Self)),
        Gt(boxed!(Self), #[tok(GT, this)] boxed!(Self)),
        /// String concatenation: `expr || expr`. PostgreSQL scans `||` as a
        /// generic `Op`, below additive operators in the precedence hierarchy.
        Concat(boxed!(Self), #[tok(CONCAT, this)] boxed!(Self)),
        /// Bitwise OR: `expr | expr`. Must come after `Concat` (`||`) so the
        /// longer token matches first at the punctuation level.
        BitOr(boxed!(Self), #[tok(PIPE, this)] boxed!(Self)),
        /// Bitwise AND: `expr & expr`.
        BitAnd(boxed!(Self), #[tok(AMP, this)] boxed!(Self)),
        /// Bitwise XOR: `expr # expr` (Postgres bit-string / integer operator).
        BitXor(boxed!(Self), #[tok(POUND, this)] boxed!(Self)),
        Add(boxed!(Self), #[tok(PLUS, this)] boxed!(Self)),
        Sub(boxed!(Self), #[tok(MINUS, this)] boxed!(Self)),
        /// Multiplication: `expr * expr`
        Mul(boxed!(Self), #[tok(STAR, this)] boxed!(Self)),
        /// Division: `expr / expr`
        Div(boxed!(Self), #[tok(SLASH, this)] boxed!(Self)),
        /// Modulo: `expr % expr`
        Mod(boxed!(Self), #[tok(PERCENT, this)] boxed!(Self)),
        /// Exponentiation: `expr ^ expr` (Postgres numeric power operator).
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
        /// `COALESCE(expr, ...)`. `COALESCE`, `GREATEST`, `LEAST` and `NULLIF`
        /// are `COL_NAME` keywords, so `Func` never sees them as a name; they
        /// stand before `ColumnRef` for the reason `Grouping` does.
        Coalesce(CoalesceExpr),
        /// `GREATEST(expr, ...)`.
        Greatest(GreatestExpr),
        /// `LEAST(expr, ...)`.
        Least(LeastExpr),
        /// `NULLIF(left, right)`.
        NullIf(NullIfExpr),
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

recursa::ast_node! {
    /// gram.y `b_expr` (gram.y:15290-15345): `a_expr` without the
    /// boolean-keyword productions.
    ///
    /// gram.y:15285-15288 states why it exists: "AND, NOT, IS, and IN are the
    /// a_expr keywords that would cause trouble in the places where b_expr is
    /// used. For simplicity, we just eliminate all the boolean-keyword-operator
    /// productions from b_expr." Declared precedence cannot express that, and
    /// gram.y does not try; the restriction is written by hand here, variant by
    /// variant, against gram.y's own production list.
    ///
    /// A restricted expression is not an AST type: every rule it lowers builds
    /// an [`Expr`], so a field typed `BExpr` holds an `Expr`. Its rules are the
    /// admitted [`Expr`] variants with one substitution -- a recursive operand
    /// that is the first or the last symbol of the rule becomes a `BExpr`,
    /// every other operand stays an `Expr`. That is gram.y's own shape:
    /// `b_expr '+' b_expr`, but `'(' a_expr ')'` through the parenthesized
    /// atom (gram.y:14768-14769).
    ///
    /// The admitted list below is gram.y's `b_expr` production list:
    ///
    /// - `c_expr` (gram.y:15290, 15355+): every atom of [`Expr`] except
    ///   `Default`, which gram.y reaches only through `a_expr: DEFAULT`
    ///   (gram.y:15264).
    /// - `b_expr TYPECAST Typename` (gram.y:15292): `Cast`.
    /// - `'+' b_expr` and `'-' b_expr` (gram.y:15294-15296): `Pos`, `Neg`.
    /// - `b_expr '+' '-' '*' '/' '%' '^' b_expr` (gram.y:15298-15308).
    /// - `b_expr '<' '>' '=' LESS_EQUALS GREATER_EQUALS NOT_EQUALS b_expr`
    ///   (gram.y:15310-15320).
    /// - `b_expr qual_Op b_expr` and `qual_Op b_expr` (gram.y:15322-15324):
    ///   every operator spelling PostgreSQL's scanner returns as `Op`, which
    ///   pg-sql names one token at a time, in both its infix and its prefix
    ///   form.
    /// - `b_expr IS [NOT] DISTINCT FROM b_expr` (gram.y:15326-15330).
    /// - `b_expr IS [NOT] DOCUMENT_P` (gram.y:15334-15339): `IsDocument`.
    ///
    /// Everything else `a_expr` has is absent: `AND`, `OR`, `NOT`, the `LIKE`
    /// family, `BETWEEN`, `IN`, the rest of the `IS` family, `ISNULL`,
    /// `NOTNULL`, `OVERLAPS`, `AT TIME ZONE`, `AT LOCAL`, `COLLATE`, the
    /// quantified comparisons and `DEFAULT`.
    #[restricts(Expr)]
    pub enum BExpr {
        Neg,
        Pos,
        GeomCenter,
        BitNot,
        PathLength,
        AtHashAtPrefix,
        PointCount,
        Abs,
        BangEqMinusPrefix,
        Sqrt,
        Cbrt,
        CustomPrefix,
        Cast,
        IsNotDistinctFrom,
        IsDistinctFrom,
        IsDocument,
        TildeLeqTilde,
        TildeGeqTilde,
        TildeLtTilde,
        TildeGtTilde,
        RegexNotIMatch,
        RegexIMatch,
        RegexNotMatch,
        GeomSame,
        RegexMatch,
        LikeOpINeg,
        LikeOpI,
        LikeOpNeg,
        LikeOp,
        RecordLte,
        RecordGte,
        RecordNeq,
        RecordLt,
        RecordGt,
        RecordEq,
        JsonPathText,
        JsonPath,
        JsonFieldText,
        JsonField,
        Parallel,
        JsonAnyKey,
        JsonAllKeys,
        Intersect,
        Perpendicular,
        Horizontal,
        IsHorizontal,
        IsVertical,
        Below,
        Above,
        JsonKey,
        JsonContains,
        JsonContainedBy,
        TsMatch3,
        TripleLt,
        StrictlyBelow,
        SubsetEq,
        Distance,
        TripleGt,
        SupersetEq,
        Adjacent,
        StrictlyAbove,
        NoExtendBelow,
        NoExtendAbove,
        TsMatch,
        JsonPathExists,
        Overlap,
        NoExtendRight,
        NoExtendLeft,
        StrictlyLeft,
        StrictlyRight,
        TripleEq,
        BangEqEq,
        GeomClosest,
        BangEq,
        Neq,
        Lte,
        Gte,
        Eq,
        StartsWith,
        JsonDeletePath,
        CustomInfix,
        Lt,
        Gt,
        Concat,
        BitOr,
        BitAnd,
        BitXor,
        Add,
        Sub,
        Mul,
        Div,
        Mod,
        Pow,
        Exists,
        Array,
        RowExpr,
        Grouping,
        Coalesce,
        Greatest,
        Least,
        NullIf,
        Case,
        UnicodeStringLit,
        EscapeStringLit,
        TimestampLit,
        TimeLit,
        IntervalLit,
        CastFunc,
        XmlElement,
        XmlForest,
        XmlAttributes,
        XmlPi,
        XmlSerialize,
        XmlParse,
        XmlRoot,
        XmlExists,
        Trim,
        CastCall,
        CollationFor,
        Substring,
        Position,
        Overlay,
        Extract,
        JsonCtor,
        JsonScalar,
        JsonSerialize,
        JsonObject,
        JsonArray,
        JsonExists,
        JsonValue,
        JsonQuery,
        JsonObjectAgg,
        JsonArrayAgg,
        Func,
        User,
        QualRef,
        Parenthesized,
        NumericLit,
        IntegerLit,
        DollarStringLit,
        BitStringLit,
        HexStringLit,
        StringLit,
        BoolTrue,
        BoolFalse,
        Null,
        PositionalParam,
        ColumnRef,
    }
}
