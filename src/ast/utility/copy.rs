//! COPY statement.

use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::QualifiedName;
use crate::ast::tcl::prepared::PreparableStmt;
use crate::tokens::literal;

// --- COPY ---

recursa::ast_node! {
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
    #[derive(Debug)]
    pub struct CopyStmt {
        #[tok(COPY, this)]
        pub body: CopyBody,
    }
}

recursa::ast_node! {
    /// Body shape of a COPY statement.
    ///
    /// The leading token uniquely identifies each variant:
    /// - `Query` starts with `(` — `COPY (PreparableStmt) TO ...`.
    /// - `BinaryTable` starts with the `BINARY` keyword — `COPY BINARY t TO ...`.
    /// - `Table` starts with a qualified-name identifier — `COPY t FROM ...`.
    ///
    /// `BinaryTable` is a separate variant (rather than `Option<BINARY>` on
    /// `CopyTableBody`) so the legacy prefix remains an explicit grammar
    /// production alongside the ordinary identifier-led table form.
    #[derive(Debug)]
    pub enum CopyBody {
        Query(CopyQueryBody),
        BinaryTable(CopyBinaryTableBody),
        Table(CopyTableBody),
    }
}

recursa::ast_node! {
    /// Table-form COPY body without the legacy `BINARY` prefix:
    /// `name [(cols)] {FROM|TO} [PROGRAM] target [USING DELIMITERS 'c']
    /// [WITH] [options...] [WHERE expr]`.
    ///
    /// The `where_clause` field is FROM-only by Postgres' semantics, but we accept
    /// it unconditionally and let the server reject `WHERE` with `TO`. This keeps
    /// the grammar context-free.
    #[derive(Debug)]
    pub struct CopyTableBody {
        pub table: QualifiedName,
        pub columns: Option<CopyColumnList>,
        pub direction: CopyDirection,
        #[presence(PROGRAM)]
        pub program: bool,
        pub target: CopyTarget,
        pub delimiter: Option<CopyUsingDelimiters>,
        /// gram.y `opt_with`: the noise word before `copy_options`, present
        /// even when the `copy_opt_list` after it is empty (`COPY t FROM stdin
        /// WITH` is a complete statement).
        #[presence(WITH)]
        pub with: bool,
        /// gram.y `copy_options`; the empty `copy_opt_list` is the absent value.
        pub options: Option<CopyOptions>,
        pub where_clause: Option<CopyWhereClause>,
    }
}

recursa::ast_node! {
    /// Table-form COPY body with the legacy `BINARY` prefix:
    /// `BINARY name [(cols)] {FROM|TO} ...`.
    #[derive(Debug)]
    pub struct CopyBinaryTableBody {
        #[tok(BINARY, this)]
        pub inner: CopyTableBody,
    }
}

recursa::ast_node! {
    /// Query-form COPY body: `(PreparableStmt) TO [PROGRAM] target [WITH] [options]`.
    #[derive(Debug)]
    pub struct CopyQueryBody {
        #[tok(LPAREN, this, RPAREN)]
        pub query: boxed!(PreparableStmt),
        #[tok(TO, this)]
        #[presence(PROGRAM)]
        pub program: bool,
        pub target: CopyTarget,
        /// gram.y `opt_with`: the noise word before `copy_options`, present
        /// even when the `copy_opt_list` after it is empty.
        #[presence(WITH)]
        pub with: bool,
        /// gram.y `copy_options`; the empty `copy_opt_list` is the absent value.
        pub options: Option<CopyOptions>,
    }
}

recursa::ast_node! {
    /// `(col [, ...])` column list on the table-form COPY statement.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CopyColumnList {
        #[sep(COMMA)]
        pub cols: one_or_many!(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `FROM` or `TO` direction marker on the table-form COPY statement.
    #[derive(Debug)]
    pub enum CopyDirection {
        #[tok(FROM)]
        From,
        #[tok(TO)]
        To,
    }
}

recursa::ast_node! {
    /// The source/destination of a COPY: a quoted filename, `STDIN`, or
    /// `STDOUT` — gram.y's `copy_file_name: Sconst | STDIN | STDOUT`.
    ///
    /// The regression corpus spells the filename `:'filename'`, but that is psql
    /// substituting before the server lexes, not a `copy_file_name` production;
    /// render such a script through the `pg-psql` crate first.
    ///
    /// `STDIN` / `STDOUT` are tokenized as their keyword kinds, while a quoted
    /// filename is a string token, so the three alternatives have distinct LR
    /// actions.
    #[derive(Debug)]
    pub enum CopyTarget {
        #[tok(STDIN)]
        Stdin,
        #[tok(STDOUT)]
        Stdout,
        File(literal::StringLit),
    }
}

recursa::ast_node! {
    /// Legacy `[USING] DELIMITERS 'c'` clause — Postgres' `copy_delimiter`
    /// production. `USING` is optional.
    #[derive(Debug)]
    pub struct CopyUsingDelimiters {
        #[tok(optional(USING), DELIMITERS, this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// A string-constant value in a COPY statement — Postgres' `Sconst`. Covers
    /// the plain `'…'` form plus the `E'…'` (escape), `U&'…'` (unicode), and
    /// `B'…'` (bit) prefixed forms used in the regression corpus.
    ///
    /// The lexer assigns the prefixed forms (`U&`, `E`, `B`, `X`) their own token
    /// kinds before the parser sees them.
    #[derive(Debug)]
    pub enum CopySconst {
        Unicode(literal::UnicodeStringLit),
        Escape(literal::EscapeStringLit),
        Bit(literal::BitStringLit),
        Hex(literal::HexStringLit),
        Plain(literal::StringLit),
    }
}

recursa::ast_node! {
    /// The COPY options clause — either the legacy bareword form or the modern
    /// parenthesised name/value form.
    ///
    /// `Generic` begins with `(`. `Legacy` is the bareword form starting with one of
    /// `BINARY`/`FREEZE`/`OIDS`/`DELIMITER`/`NULL`/`CSV`/`HEADER`/`QUOTE`/`ESCAPE`/
    /// `FORCE`/`ENCODING`.
    #[derive(Debug)]
    pub enum CopyOptions {
        Generic(CopyGenericOptions),
        Legacy(CopyLegacyOptions),
    }
}

recursa::ast_node! {
    /// Parenthesised, comma-separated generic options: `(name [arg] [, ...])`.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CopyGenericOptions {
        #[sep(COMMA)]
        pub list: one_or_many!(CopyGenericOption),
    }
}

recursa::ast_node! {
    /// Parenthesized list used as a generic COPY option argument.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct CopyGenericOptionNameList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(literal::AliasName),
    );
}

recursa::ast_node! {
    /// One entry in the parenthesised generic options list: `name [arg]`.
    ///
    /// `name` is `AliasName` so unreserved keywords (e.g. `format`, `freeze`,
    /// `header`) and identifiers are both accepted.
    #[derive(Debug)]
    pub struct CopyGenericOption {
        pub name: literal::AliasName,
        pub arg: Option<CopyGenericOptionArg>,
    }
}

recursa::ast_node! {
    /// Value of a generic option — Postgres' `copy_generic_opt_arg`.
    ///
    /// Variant ordering: keyword `Default` and punctuation `Star` / `ParenList`
    /// before the catch-all `NameOrString`, since they begin with a definite
    /// token and `NameOrString` would otherwise consume a leading bareword.
    /// `Numeric` precedes `NameOrString` so an integer like `42` is not parsed
    /// as an identifier (it would not be — different lex kind — but listing
    /// fixed-shape variants first preserves longest-match-wins semantics).
    #[derive(Debug)]
    pub enum CopyGenericOptionArg {
        // Added in 17: research, PostgreSQL 17, "Changes to existing statements"
        // (REL_17_11 gram.y 3539 `copy_generic_opt_arg`).
        #[cfg(feature = "since-pg17")]
        #[tok(DEFAULT)]
        Default,
        #[tok(STAR)]
        Star,
        ParenList(CopyGenericOptionNameList),
        String(CopySconst),
        Numeric(literal::NumericLit),
        Integer(literal::IntegerLit),
        Name(crate::tokens::NonReservedWord),
    }
}

recursa::ast_node! {
    /// Legacy bareword options: zero-or-more space-separated option items.
    ///
    /// Listed as `Vec` (not `Seq`) because the items are separator-free. The Vec
    /// stops at the first non-option token (typically `WHERE` or end-of-statement).
    #[derive(Debug)]
    pub struct CopyLegacyOptions {
        /// gram.y `copy_opt_list` may be empty, but the empty list is the absent
        /// `options`; one node with no items would leave two derivations for
        /// "no options".
        pub items: one_or_many!(CopyLegacyOptionItem),
    }
}

recursa::ast_node! {
    /// One item in the legacy bareword options list — Postgres' `copy_opt_item`.
    ///
    /// Variant ordering: multi-keyword forms (`FORCE NOT NULL ...`, `FORCE QUOTE ...`,
    /// `FORCE NULL ...`) come before any single-keyword form to avoid ambiguity.
    /// The keyword `FORCE` is a separate token so the multi-keyword forms are not
    /// in conflict with each other (`FORCE NOT NULL` vs `FORCE NULL` vs `FORCE QUOTE`
    /// — the second token disambiguates).
    #[derive(Debug)]
    pub enum CopyLegacyOptionItem {
        ForceNotNull(CopyForceNotNullOpt),
        ForceQuote(CopyForceQuoteOpt),
        ForceNull(CopyForceNullOpt),
        Delimiter(CopyDelimiterOpt),
        NullAs(CopyNullOpt),
        Quote(CopyQuoteOpt),
        Escape(CopyEscapeOpt),
        Encoding(CopyEncodingOpt),
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
}

recursa::ast_node! {
    /// `DELIMITER [AS] 'c'` — legacy delimiter option.
    #[derive(Debug)]
    pub struct CopyDelimiterOpt {
        #[tok(DELIMITER, optional(AS), this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `NULL [AS] 'str'` — legacy null-marker option.
    #[derive(Debug)]
    pub struct CopyNullOpt {
        #[tok(NULL, optional(AS), this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `QUOTE [AS] 'c'` — legacy CSV-quote option.
    #[derive(Debug)]
    pub struct CopyQuoteOpt {
        #[tok(QUOTE, optional(AS), this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `ESCAPE [AS] 'c'` — legacy CSV-escape option.
    #[derive(Debug)]
    pub struct CopyEscapeOpt {
        #[tok(ESCAPE, optional(AS), this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `ENCODING 'name'` — legacy encoding option.
    #[derive(Debug)]
    pub struct CopyEncodingOpt {
        #[tok(ENCODING, this)]
        pub value: CopySconst,
    }
}

recursa::ast_node! {
    /// `FORCE QUOTE { * | columnList }` — legacy force-quote option.
    #[derive(Debug)]
    pub struct CopyForceQuoteOpt {
        #[tok(FORCE, QUOTE, this)]
        pub target: CopyForceTarget,
    }
}

recursa::ast_node! {
    /// `FORCE NOT NULL { * | columnList }` — legacy force-not-null option.
    #[derive(Debug)]
    pub struct CopyForceNotNullOpt {
        // `*` is added in 17: research, PostgreSQL 17, "Changes to existing
        // statements" (REL_17_11 gram.y 3475, 3483). REL_16_15 gram.y 3419
        // takes only a `columnList`.
        #[cfg(feature = "since-pg17")]
        #[tok(FORCE, NOT, NULL, this)]
        pub target: CopyForceTarget,
        #[cfg(not(feature = "since-pg17"))]
        #[tok(FORCE, NOT, NULL, this)]
        pub target: CopyForceColumns,
    }
}

recursa::ast_node! {
    /// `FORCE NULL { * | columnList }` — legacy force-null option.
    #[derive(Debug)]
    pub struct CopyForceNullOpt {
        // `*` is added in 17: research, PostgreSQL 17, "Changes to existing
        // statements" (REL_17_11 gram.y 3475, 3483). REL_16_15 gram.y 3419
        // takes only a `columnList`.
        #[cfg(feature = "since-pg17")]
        #[tok(FORCE, NULL, this)]
        pub target: CopyForceTarget,
        #[cfg(not(feature = "since-pg17"))]
        #[tok(FORCE, NULL, this)]
        pub target: CopyForceColumns,
    }
}

recursa::ast_node! {
    /// Target of a `FORCE QUOTE` / `FORCE NULL` / `FORCE NOT NULL` legacy option:
    /// either `*` (all columns) or a bare `columnList` (no parentheses — note
    /// `columnList` in `gram.y` does not include outer `()`).
    #[derive(Debug)]
    pub enum CopyForceTarget {
        #[tok(STAR)]
        Star,
        Columns(#[sep(COMMA)] one_or_many!(crate::tokens::ColId)),
    }
}

// Before 17, `FORCE NOT NULL` and `FORCE NULL` take a `columnList` and no `*`
// (research, PostgreSQL 17, "Changes to existing statements"; REL_16_15
// gram.y 3419-3424).
#[cfg(not(feature = "since-pg17"))]
recursa::ast_node! {
    /// The `columnList` target of a `FORCE NOT NULL` or `FORCE NULL` legacy
    /// option before 17: a [`CopyForceTarget`] without `*`.
    #[restricts(CopyForceTarget)]
    pub enum CopyForceColumns {
        Columns,
    }
}

recursa::ast_node! {
    /// `WHERE expr` clause on a `COPY ... FROM` (the only direction that accepts
    /// it per Postgres' grammar; the server enforces the FROM-only restriction).
    #[derive(Debug)]
    pub struct CopyWhereClause {
        #[tok(WHERE, this)]
        pub condition: Expr,
    }
}
