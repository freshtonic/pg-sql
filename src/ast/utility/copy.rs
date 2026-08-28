//! COPY statement.

use recursa::seq::Seq1;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::ast::tcl::prepared::PreparableStmt;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::{
    CSV, DELIMITER, DELIMITERS, ENCODING, FORCE, FREEZE, HEADER, PROGRAM, QUOTE, STDIN, STDOUT,
};
use crate::tokens::{literal, punct};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct CopyStmt<'input> {
    pub copy: COPY,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyTableBody<'input> {
    pub table: QualifiedName<'input>,
    pub columns: Option<CopyColumnList<'input>>,
    pub direction: CopyDirection,
    pub program: Option<PROGRAM>,
    pub target: CopyTarget<'input>,
    pub delimiter: Option<CopyUsingDelimiters<'input>>,
    pub with: Option<WITH>,
    pub options: Option<CopyOptions<'input>>,
    pub where_clause: Option<CopyWhereClause<'input>>,
}

/// Table-form COPY body with the legacy `BINARY` prefix:
/// `BINARY name [(cols)] {FROM|TO} ...`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyBinaryTableBody<'input> {
    pub binary: BINARY,
    pub inner: CopyTableBody<'input>,
}

/// Query-form COPY body: `(PreparableStmt) TO [PROGRAM] target [WITH] [options]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyQueryBody<'input> {
    pub query: Surrounded<punct::LParen, Box<PreparableStmt<'input>>, punct::RParen>,
    pub to: TO,
    pub program: Option<PROGRAM>,
    pub target: CopyTarget<'input>,
    pub with: Option<WITH>,
    pub options: Option<CopyOptions<'input>>,
}

/// `(col [, ...])` column list on the table-form COPY statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyColumnList<'input> {
    pub cols:
        Surrounded<punct::LParen, Seq1<crate::tokens::ColId<'input>, punct::Comma>, punct::RParen>,
}

/// `FROM` or `TO` direction marker on the table-form COPY statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CopyDirection {
    From(FROM),
    To(TO),
}

/// The source/destination of a COPY: a quoted filename, a psql `:'var'`
/// variable substitution (common in the regression corpus), `STDIN`, or
/// `STDOUT`.
///
/// Variant ordering: keyword forms first so they win over the otherwise-
/// matching string/PsqlVar rules (`STDIN` / `STDOUT` are soft keywords that
/// the scanner could equally well classify as identifiers).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CopyTarget<'input> {
    Stdin(STDIN),
    Stdout(STDOUT),
    File(literal::StringLit<'input>),
    PsqlVar(literal::PsqlVar<'input>),
}

/// Legacy `[USING] DELIMITERS 'c'` clause — Postgres' `copy_delimiter`
/// production. `USING` is optional.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyUsingDelimiters<'input> {
    pub using: Option<USING>,
    pub delimiters: DELIMITERS,
    pub value: CopySconst<'input>,
}

/// A string-constant value in a COPY statement — Postgres' `Sconst`. Covers
/// the plain `'…'` form plus the `E'…'` (escape), `U&'…'` (unicode), and
/// `B'…'` (bit) prefixed forms used in the regression corpus.
///
/// Variant ordering: the prefixed forms (`U&`, `E`, `B`, `X`) come before the
/// bare `StringLit` so the lexer's longest-match-wins picks the prefixed
/// kind first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CopyOptions<'input> {
    Generic(CopyGenericOptions<'input>),
    Legacy(CopyLegacyOptions<'input>),
}

/// Parenthesised, comma-separated generic options: `(name [arg] [, ...])`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyGenericOptions<'input> {
    pub list:
        Surrounded<punct::LParen, Seq1<CopyGenericOption<'input>, punct::Comma>, punct::RParen>,
}

/// One entry in the parenthesised generic options list: `name [arg]`.
///
/// `name` is `AliasName` so unreserved keywords (e.g. `format`, `freeze`,
/// `header`) and identifiers are both accepted.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CopyGenericOptionArg<'input> {
    Default(DEFAULT),
    Star(punct::Star),
    ParenList(
        Surrounded<punct::LParen, Seq1<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    ),
    String(CopySconst<'input>),
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    Name(literal::AliasName<'input>),
}

/// Legacy bareword options: zero-or-more space-separated option items.
///
/// Listed as `Vec` (not `Seq`) because the items are separator-free. The Vec
/// stops at the first non-option token (typically `WHERE` or end-of-statement).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyLegacyOptions<'input> {
    pub items: Vec<CopyLegacyOptionItem<'input>>,
}

/// One item in the legacy bareword options list — Postgres' `copy_opt_item`.
///
/// Variant ordering: multi-keyword forms (`FORCE NOT NULL ...`, `FORCE QUOTE ...`,
/// `FORCE NULL ...`) come before any single-keyword form to avoid ambiguity.
/// The keyword `FORCE` is a separate token so the multi-keyword forms are not
/// in conflict with each other (`FORCE NOT NULL` vs `FORCE NULL` vs `FORCE QUOTE`
/// — the second token disambiguates).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CopyLegacyOptionItem<'input> {
    ForceNotNull(CopyForceNotNullOpt<'input>),
    ForceQuote(CopyForceQuoteOpt<'input>),
    ForceNull(CopyForceNullOpt<'input>),
    Delimiter(CopyDelimiterOpt<'input>),
    NullAs(CopyNullOpt<'input>),
    Quote(CopyQuoteOpt<'input>),
    Escape(CopyEscapeOpt<'input>),
    Encoding(CopyEncodingOpt<'input>),
    Binary(BINARY),
    Freeze(FREEZE),
    Oids(OIDS),
    Csv(CSV),
    Header(HEADER),
}

/// `DELIMITER [AS] 'c'` — legacy delimiter option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyDelimiterOpt<'input> {
    pub delimiter: DELIMITER,
    pub r#as: Option<AS>,
    pub value: CopySconst<'input>,
}

/// `NULL [AS] 'str'` — legacy null-marker option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyNullOpt<'input> {
    pub null: NULL,
    pub r#as: Option<AS>,
    pub value: CopySconst<'input>,
}

/// `QUOTE [AS] 'c'` — legacy CSV-quote option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyQuoteOpt<'input> {
    pub quote: QUOTE,
    pub r#as: Option<AS>,
    pub value: CopySconst<'input>,
}

/// `ESCAPE [AS] 'c'` — legacy CSV-escape option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyEscapeOpt<'input> {
    pub escape: ESCAPE,
    pub r#as: Option<AS>,
    pub value: CopySconst<'input>,
}

/// `ENCODING 'name'` — legacy encoding option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyEncodingOpt<'input> {
    pub encoding: ENCODING,
    pub value: CopySconst<'input>,
}

/// `FORCE QUOTE { * | columnList }` — legacy force-quote option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyForceQuoteOpt<'input> {
    pub force: FORCE,
    pub quote: QUOTE,
    pub target: CopyForceTarget<'input>,
}

/// `FORCE NOT NULL { * | columnList }` — legacy force-not-null option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyForceNotNullOpt<'input> {
    pub force: FORCE,
    pub not: NOT,
    pub null: NULL,
    pub target: CopyForceTarget<'input>,
}

/// `FORCE NULL { * | columnList }` — legacy force-null option.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyForceNullOpt<'input> {
    pub force: FORCE,
    pub null: NULL,
    pub target: CopyForceTarget<'input>,
}

/// Target of a `FORCE QUOTE` / `FORCE NULL` / `FORCE NOT NULL` legacy option:
/// either `*` (all columns) or a bare `columnList` (no parentheses — note
/// `columnList` in `gram.y` does not include outer `()`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CopyForceTarget<'input> {
    Star(punct::Star),
    Columns(Seq1<crate::tokens::ColId<'input>, punct::Comma>),
}

/// `WHERE expr` clause on a `COPY ... FROM` (the only direction that accepts
/// it per Postgres' grammar; the server enforces the FROM-only restriction).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CopyWhereClause<'input> {
    pub r#where: WHERE,
    pub condition: Expr<'input>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn copy_table_from_stdin_bare() {
        let stmt: CopyStmt = parse_stmt("COPY t FROM STDIN");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(matches!(table.direction, CopyDirection::From(_)));
        assert!(matches!(table.target, CopyTarget::Stdin(_)));
        assert!(table.columns.is_none());
        assert!(table.program.is_none());
        assert!(table.delimiter.is_none());
        assert!(table.with.is_none());
        assert!(table.options.is_none());
        assert!(table.where_clause.is_none());
        reparse_stable::<CopyStmt>("COPY t FROM STDIN");
    }

    #[test]
    fn copy_table_to_stdout() {
        let stmt: CopyStmt = parse_stmt("COPY t TO STDOUT");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(matches!(table.direction, CopyDirection::To(_)));
        assert!(matches!(table.target, CopyTarget::Stdout(_)));
        reparse_stable::<CopyStmt>("COPY t TO STDOUT");
    }

    #[test]
    fn copy_table_with_columns_from_stdin() {
        let stmt: CopyStmt = parse_stmt("COPY t (a, b, c) FROM STDIN");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(table.columns.is_some());
        reparse_stable::<CopyStmt>("COPY t (a, b, c) FROM STDIN");
    }

    #[test]
    fn copy_table_to_file() {
        reparse_stable::<CopyStmt>("COPY t TO 'foo.csv'");
    }

    #[test]
    fn copy_table_from_file() {
        let stmt: CopyStmt = parse_stmt("COPY t FROM 'foo.csv'");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(matches!(table.target, CopyTarget::File(_)));
    }

    #[test]
    fn copy_table_to_stdout_csv_legacy() {
        // Legacy `csv` option without WITH or parens.
        reparse_stable::<CopyStmt>("COPY t TO STDOUT CSV");
    }

    #[test]
    fn copy_table_from_stdin_csv_header_legacy() {
        // Two consecutive legacy options.
        reparse_stable::<CopyStmt>("COPY t FROM STDIN CSV HEADER");
    }

    #[test]
    fn copy_table_to_file_csv_quote_escape_legacy() {
        // Legacy options that carry string arguments.
        reparse_stable::<CopyStmt>("COPY t TO 'f.csv' CSV QUOTE '|' ESCAPE '\\'");
    }

    #[test]
    fn copy_table_with_legacy_delimiter_null_as() {
        // `WITH` followed by legacy form.
        reparse_stable::<CopyStmt>("COPY x FROM STDIN WITH DELIMITER AS ';' NULL AS ''");
    }

    #[test]
    fn copy_table_using_delimiters_legacy() {
        // `USING DELIMITERS 'c'` precedes the option list.
        reparse_stable::<CopyStmt>("COPY t FROM 'f' USING DELIMITERS '|'");
    }

    #[test]
    fn copy_table_binary_legacy() {
        // `COPY BINARY t TO file` legacy binary option — `CopyBody::BinaryTable`.
        let stmt: CopyStmt = parse_stmt("COPY BINARY t TO 'f'");
        let CopyBody::BinaryTable(body) = &stmt.body else {
            panic!("expected binary-table body");
        };
        assert!(matches!(body.inner.direction, CopyDirection::To(_)));
        reparse_stable::<CopyStmt>("COPY BINARY t TO 'f'");
    }

    #[test]
    fn copy_table_from_program() {
        reparse_stable::<CopyStmt>("COPY t FROM PROGRAM 'cat foo.csv'");
    }

    #[test]
    fn copy_table_generic_options_single() {
        // `(freeze)` — a single generic option with no value.
        reparse_stable::<CopyStmt>("COPY t FROM 'f' (FREEZE)");
    }

    #[test]
    fn copy_table_with_generic_options() {
        // `WITH (header match, format csv)` — generic options after WITH.
        reparse_stable::<CopyStmt>("COPY t FROM STDIN WITH (HEADER MATCH, FORMAT CSV)");
    }

    #[test]
    fn copy_table_generic_option_star() {
        reparse_stable::<CopyStmt>("COPY t FROM STDIN (FORCE_QUOTE *)");
    }

    #[test]
    fn copy_table_generic_option_paren_list() {
        reparse_stable::<CopyStmt>("COPY t FROM STDIN (FORCE_QUOTE (a, b))");
    }

    #[test]
    fn copy_table_from_where_expr() {
        // `WHERE expr` after the option list (FROM-only).
        reparse_stable::<CopyStmt>("COPY x FROM STDIN WHERE a = 1");
    }

    #[test]
    fn copy_query_to_stdout() {
        let stmt: CopyStmt = parse_stmt("COPY (SELECT * FROM t) TO STDOUT");
        assert!(matches!(stmt.body, CopyBody::Query(_)));
        reparse_stable::<CopyStmt>("COPY (SELECT * FROM t) TO STDOUT");
    }

    #[test]
    fn copy_query_to_file() {
        reparse_stable::<CopyStmt>("COPY (SELECT 1) TO 'f'");
    }

    #[test]
    fn copy_query_with_generic_options() {
        reparse_stable::<CopyStmt>("COPY (SELECT 1) TO STDOUT WITH (DEFAULT '\\D')");
    }

    #[test]
    fn copy_query_insert_returning() {
        // Query body must accept INSERT ... RETURNING (PreparableStmt).
        reparse_stable::<CopyStmt>("COPY (INSERT INTO t (a) VALUES (1) RETURNING id) TO STDOUT");
    }

    #[test]
    fn copy_table_with_encoding_legacy() {
        reparse_stable::<CopyStmt>("COPY t FROM STDIN WITH ENCODING 'sql_ascii'");
    }

    #[test]
    fn copy_table_legacy_force_quote_star() {
        // Three-keyword legacy item.
        reparse_stable::<CopyStmt>("COPY t TO 'f' CSV FORCE QUOTE *");
    }

    #[test]
    fn copy_table_legacy_force_not_null_list() {
        reparse_stable::<CopyStmt>("COPY t FROM 'f' CSV FORCE NOT NULL a, b");
    }

    #[test]
    fn copy_table_legacy_with_null_as() {
        // Corpus regression case: `WITH ... NULL AS '...'` chained options.
        reparse_stable::<CopyStmt>("COPY t TO STDOUT WITH NULL AS E'\\0'");
    }

    #[test]
    fn copy_table_legacy_chained_delimiter_null_encoding() {
        // Three chained legacy options after WITH.
        reparse_stable::<CopyStmt>(
            "COPY x FROM STDIN WITH DELIMITER AS ':' NULL AS E'\\X' ENCODING 'sql_ascii'",
        );
    }

    #[test]
    fn copy_table_psql_var_target() {
        reparse_stable::<CopyStmt>("COPY t TO :'filename' CSV");
    }
}
