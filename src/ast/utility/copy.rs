//! COPY statement.

use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::ast::tcl::prepared::PreparableStmt;
use crate::tokens::literal;

// --- COPY ---

/// ```sql
/// COPY [BINARY] qualified_name [(col, ...)] {FROM|TO} [PROGRAM] target
///      [USING DELIMITERS 'c'] [WITH] [option ...] [WHERE expr]
/// COPY (PreparableStmt) TO [PROGRAM] target [WITH] [option ...]
/// ```
///
/// Two distinct body shapes — the query form (`COPY (SelectStmt) TO ...`) and
/// the table form (`COPY qualified_name [(cols)] {FROM|TO} ...`) — drive the
/// `CopyBody` enum. The query form is selected by the `(` lookahead immediately
/// after `COPY`, before the table form's optional `BINARY` keyword.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyStmt<'input> {
    #[tok(COPY, this)]
    pub body: CopyBody<'input>,
}

/// Body shape of a COPY statement.
///
/// Variant ordering matters for disambiguation. The leading token uniquely
/// identifies each variant:
/// - `Query` starts with `(` — `COPY (PreparableStmt) TO ...`.
/// - `BinaryTable` starts with the `BINARY` keyword — `COPY BINARY t TO ...`.
/// - `Table` starts with a qualified-name identifier — `COPY t FROM ...`.
///
/// `BinaryTable` is a separate variant (rather than `Option<BINARY>` on
/// `CopyTableBody`) so the derived first-set computation for `CopyBody`
/// correctly lists both `BINARY` and the identifier first-set. With
/// `Option<BINARY>` as the leading field of `CopyTableBody`, the codegen
/// first-set was `{ LPAREN, BINARY }` and missed the identifier branch.
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyBody<'input> {
    Query(CopyQueryBody<'input>),
    BinaryTable(CopyBinaryTableBody<'input>),
    Table(CopyTableBody<'input>),
}

/// Table-form COPY body without the legacy `BINARY` prefix:
/// `name [(cols)] {FROM|TO} [PROGRAM] target [USING DELIMITERS 'c']
/// [WITH] [options...] [WHERE expr]`.
///
/// The `where_clause` field is FROM-only by Postgres' semantics, but we accept
/// it unconditionally and let the server reject `WHERE` with `TO`. This keeps
/// the grammar context-free.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyTableBody<'input> {
    pub table: QualifiedName<'input>,
    pub columns: Option<CopyColumnList<'input>>,
    pub direction: CopyDirection,
    #[presence(PROGRAM)]
    pub program: bool,
    pub target: CopyTarget<'input>,
    pub delimiter: Option<CopyUsingDelimiters<'input>>,
    #[tok(optional(WITH), this)]
    pub options: Option<CopyOptions<'input>>,
    pub where_clause: Option<CopyWhereClause<'input>>,
}

/// Table-form COPY body with the legacy `BINARY` prefix:
/// `BINARY name [(cols)] {FROM|TO} ...`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyBinaryTableBody<'input> {
    #[tok(BINARY, this)]
    pub inner: CopyTableBody<'input>,
}

/// Query-form COPY body: `(PreparableStmt) TO [PROGRAM] target [WITH] [options]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyQueryBody<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub query: Box<PreparableStmt<'input>>,
    #[tok(TO, this)]
    #[presence(PROGRAM)]
    pub program: bool,
    pub target: CopyTarget<'input>,
    #[tok(optional(WITH), this)]
    pub options: Option<CopyOptions<'input>>,
}

/// `(col [, ...])` column list on the table-form COPY statement.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct CopyColumnList<'input> {
    #[sep(COMMA)]
    pub cols: recursa::Vec1<crate::tokens::ColId<'input>>,
}

/// `FROM` or `TO` direction marker on the table-form COPY statement.
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyDirection {
    #[tok(FROM)]
    From,
    #[tok(TO)]
    To,
}

/// The source/destination of a COPY: a quoted filename, a psql `:'var'`
/// variable substitution (common in the regression corpus), `STDIN`, or
/// `STDOUT`.
///
/// Variant ordering: keyword forms first so they win over the otherwise-
/// matching string/PsqlVar rules (`STDIN` / `STDOUT` are soft keywords that
/// the scanner could equally well classify as identifiers).
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyTarget<'input> {
    #[tok(STDIN)]
    Stdin,
    #[tok(STDOUT)]
    Stdout,
    File(literal::StringLit<'input>),
    PsqlVar(literal::PsqlVariable<'input>),
}

/// Legacy `[USING] DELIMITERS 'c'` clause — Postgres' `copy_delimiter`
/// production. `USING` is optional.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyUsingDelimiters<'input> {
    #[tok(optional(USING), DELIMITERS, this)]
    pub value: CopySconst<'input>,
}

/// A string-constant value in a COPY statement — Postgres' `Sconst`. Covers
/// the plain `'…'` form plus the `E'…'` (escape), `U&'…'` (unicode), and
/// `B'…'` (bit) prefixed forms used in the regression corpus.
///
/// Variant ordering: the prefixed forms (`U&`, `E`, `B`, `X`) come before the
/// bare `StringLit` so the lexer's longest-match-wins picks the prefixed
/// kind first.
#[derive(recursa::Node, Debug, Clone)]
pub enum CopySconst<'input> {
    Unicode(literal::UnicodeStringLit<'input>),
    Escape(literal::EscapeStringLit<'input>),
    Bit(literal::BitStringLit<'input>),
    Hex(literal::HexStringLit<'input>),
    Plain(literal::StringLit<'input>),
}

/// The COPY options clause — either the legacy bareword form or the modern
/// parenthesised name/value form.
///
/// Variant ordering: `Generic` first because it begins with `(` (a single
/// unambiguous lookahead). `Legacy` is the bareword form starting with one of
/// `BINARY`/`FREEZE`/`OIDS`/`DELIMITER`/`NULL`/`CSV`/`HEADER`/`QUOTE`/`ESCAPE`/
/// `FORCE`/`ENCODING`.
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyOptions<'input> {
    Generic(CopyGenericOptions<'input>),
    Legacy(CopyLegacyOptions<'input>),
}

/// Parenthesised, comma-separated generic options: `(name [arg] [, ...])`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct CopyGenericOptions<'input> {
    #[sep(COMMA)]
    pub list: recursa::Vec1<CopyGenericOption<'input>>,
}

/// Parenthesized list used as a generic COPY option argument.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct CopyGenericOptionNameList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<literal::AliasName<'input>>,
);

/// One entry in the parenthesised generic options list: `name [arg]`.
///
/// `name` is `AliasName` so unreserved keywords (e.g. `format`, `freeze`,
/// `header`) and identifiers are both accepted.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyGenericOption<'input> {
    pub name: literal::AliasName<'input>,
    pub arg: Option<CopyGenericOptionArg<'input>>,
}

/// Value of a generic option — Postgres' `copy_generic_opt_arg`.
///
/// Variant ordering: keyword `Default` and punctuation `Star` / `ParenList`
/// before the catch-all `NameOrString`, since they begin with a definite
/// token and `NameOrString` would otherwise consume a leading bareword.
/// `Numeric` precedes `NameOrString` so an integer like `42` is not parsed
/// as an identifier (it would not be — different lex kind — but listing
/// fixed-shape variants first preserves longest-match-wins semantics).
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyGenericOptionArg<'input> {
    #[tok(DEFAULT)]
    Default,
    #[tok(STAR)]
    Star,
    ParenList(CopyGenericOptionNameList<'input>),
    String(CopySconst<'input>),
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    Name(crate::tokens::NonReservedWord<'input>),
}

/// Legacy bareword options: zero-or-more space-separated option items.
///
/// Listed as `Vec` (not `Seq`) because the items are separator-free. The Vec
/// stops at the first non-option token (typically `WHERE` or end-of-statement).
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyLegacyOptions<'input> {
    /// Greedy: a leading token from any of 11 kinds starts this element instead of ending `CopyLegacyOptions` (bison shift preference).
    #[greedy(
        BINARY, CSV, DELIMITER, ENCODING, ESCAPE, FORCE, FREEZE, HEADER, NULL, OIDS, QUOTE
    )]
    pub items: Vec<CopyLegacyOptionItem<'input>>,
}

/// One item in the legacy bareword options list — Postgres' `copy_opt_item`.
///
/// Variant ordering: multi-keyword forms (`FORCE NOT NULL ...`, `FORCE QUOTE ...`,
/// `FORCE NULL ...`) come before any single-keyword form to avoid ambiguity.
/// The keyword `FORCE` is a separate token so the multi-keyword forms are not
/// in conflict with each other (`FORCE NOT NULL` vs `FORCE NULL` vs `FORCE QUOTE`
/// — the second token disambiguates).
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyLegacyOptionItem<'input> {
    ForceNotNull(CopyForceNotNullOpt<'input>),
    ForceQuote(CopyForceQuoteOpt<'input>),
    ForceNull(CopyForceNullOpt<'input>),
    Delimiter(CopyDelimiterOpt<'input>),
    NullAs(CopyNullOpt<'input>),
    Quote(CopyQuoteOpt<'input>),
    Escape(CopyEscapeOpt<'input>),
    Encoding(CopyEncodingOpt<'input>),
    #[tok(BINARY)]
    Binary,
    #[tok(FREEZE)]
    Freeze,
    #[tok(OIDS)]
    Oids,
    #[tok(CSV)]
    Csv,
    #[tok(HEADER)]
    Header,
}

/// `DELIMITER [AS] 'c'` — legacy delimiter option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyDelimiterOpt<'input> {
    #[tok(DELIMITER, optional(AS), this)]
    pub value: CopySconst<'input>,
}

/// `NULL [AS] 'str'` — legacy null-marker option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyNullOpt<'input> {
    #[tok(NULL, optional(AS), this)]
    pub value: CopySconst<'input>,
}

/// `QUOTE [AS] 'c'` — legacy CSV-quote option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyQuoteOpt<'input> {
    #[tok(QUOTE, optional(AS), this)]
    pub value: CopySconst<'input>,
}

/// `ESCAPE [AS] 'c'` — legacy CSV-escape option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyEscapeOpt<'input> {
    #[tok(ESCAPE, optional(AS), this)]
    pub value: CopySconst<'input>,
}

/// `ENCODING 'name'` — legacy encoding option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyEncodingOpt<'input> {
    #[tok(ENCODING, this)]
    pub value: CopySconst<'input>,
}

/// `FORCE QUOTE { * | columnList }` — legacy force-quote option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyForceQuoteOpt<'input> {
    #[tok(FORCE, QUOTE, this)]
    pub target: CopyForceTarget<'input>,
}

/// `FORCE NOT NULL { * | columnList }` — legacy force-not-null option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyForceNotNullOpt<'input> {
    #[tok(FORCE, NOT, NULL, this)]
    pub target: CopyForceTarget<'input>,
}

/// `FORCE NULL { * | columnList }` — legacy force-null option.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyForceNullOpt<'input> {
    #[tok(FORCE, NULL, this)]
    pub target: CopyForceTarget<'input>,
}

/// Target of a `FORCE QUOTE` / `FORCE NULL` / `FORCE NOT NULL` legacy option:
/// either `*` (all columns) or a bare `columnList` (no parentheses — note
/// `columnList` in `gram.y` does not include outer `()`).
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyForceTarget<'input> {
    #[tok(STAR)]
    Star,
    Columns(#[sep(COMMA)] recursa::Vec1<crate::tokens::ColId<'input>>),
}

/// `WHERE expr` clause on a `COPY ... FROM` (the only direction that accepts
/// it per Postgres' grammar; the server enforces the FROM-only restriction).
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyWhereClause<'input> {
    #[tok(WHERE, this)]
    pub condition: Expr<'input>,
}
