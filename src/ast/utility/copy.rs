//! COPY statement.

use recursa::seq::Seq1;
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
    pub query:  Box<PreparableStmt<'input>> ,
    #[tok(TO, this)]
    #[presence(PROGRAM)]
    pub program: bool,
    pub target: CopyTarget<'input>,
    #[tok(optional(WITH), this)]
    pub options: Option<CopyOptions<'input>>,
}

/// `(col [, ...])` column list on the table-form COPY statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyColumnList<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub cols:
         recursa::Vec1<crate::tokens::ColId<'input> > ,
}

/// `FROM` or `TO` direction marker on the table-form COPY statement.
#[derive(recursa::Node, Debug, Clone)]
pub enum CopyDirection {
    #[tok(FROM)] From,
    #[tok(TO)] To,
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
    #[tok(STDIN)] Stdin,
    #[tok(STDOUT)] Stdout,
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
pub struct CopyGenericOptions<'input> {
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub list:
         recursa::Vec1<CopyGenericOption<'input> > ,
}

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
    #[tok(DEFAULT)] Default,
    #[tok(STAR)] Star,
    ParenList(
        #[tok(LPAREN, this, RPAREN)]
        #[sep(COMMA)]
         recursa::Vec1<literal::AliasName<'input> > ,
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
#[derive(recursa::Node, Debug, Clone)]
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
    #[tok(BINARY)] Binary,
    #[tok(FREEZE)] Freeze,
    #[tok(OIDS)] Oids,
    #[tok(CSV)] Csv,
    #[tok(HEADER)] Header,
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
    #[tok(STAR)] Star,
    Columns(#[sep(COMMA)] recursa::Vec1<crate::tokens::ColId<'input> >),
}

/// `WHERE expr` clause on a `COPY ... FROM` (the only direction that accepts
/// it per Postgres' grammar; the server enforces the FROM-only restriction).
#[derive(recursa::Node, Debug, Clone)]
pub struct CopyWhereClause<'input> {
    #[tok(WHERE, this)]
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
