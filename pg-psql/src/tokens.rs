//! The psql client lexer.
//!
//! Single-declaration token site for the psql grammar. Every entry mirrors a
//! rule of PostgreSQL's own client scanner,
//! `vendor/postgres/src/fe_utils/psqlscan.l`; the citations name the rule
//! each one stands for.
//!
//! The interpolation forms are declared at their AST sites in
//! [`crate::ast`] as single tokens, because psql recognises `:'name'` as one
//! lexical unit. pg-sql used to lex the colon and the name separately and
//! join them in the grammar, which is precisely what made `SELECT int :'x'`
//! ambiguous against the SQL/JSON `int : value` entry.

recursa::tokens! {
    // psqlscan.l:163-165 folds `--` line comments into `{whitespace}`. Both
    // are passed through to the server untouched, and neither can contain an
    // interpolation, so both are ignored here: rendering copies the source
    // between interpolations verbatim, so ignored trivia survives exactly.
    ignore = r"[ \t\r\n\f\x0b]+",
    punctuation {
        // psqlscan.l:288 `typecast` and psqlscan.l:290 `colon_equals` are
        // matched ahead of a bare colon, which is why `a::b` and `a := b`
        // are never interpolations. Longest-match keeps that order here.
        COLONCOLON => "::",
        COLONEQUALS => ":=",
        // psqlscan.l:316 `{self}` includes `:`; a colon that starts no
        // interpolation is ordinary passed-through text.
        COLON => ":",
        // psqlscan.l:681 — the only terminator psqlscan.l itself returns.
        SEMI => ";",
        // Not scanned by psqlscan.l: `psqlscanslash.l` owns backslash
        // commands. A backslash that starts none of the send commands is
        // ordinary text, so that unmodelled meta-commands render verbatim
        // instead of failing to lex.
        BACKSLASH => "\\",
        // Characters excluded from the `Punct` run below because a longer
        // token can start with them.
        MINUS => "-",
        SLASH => "/",
        DOLLAR => "$",
    }
    matchers {
        // psqlscan.l:564-601 `xdolq`: a dollar-quoted body is opaque, and
        // only the delimiter that opened it can close it. The tag classes
        // are psqlscan.l:235-236's `dolq_start [A-Za-z\200-\377_]` and
        // `dolq_cont [A-Za-z\200-\377_0-9]`; the high-byte range is every
        // non-ASCII scalar in UTF-8 source, and leaving it out would end the
        // opaque body early at a tag like `$tagé$` and expose a colon inside
        // it to the interpolation rules.
        DollarString => same_delimiter(
            opener = r"\$(?:[A-Za-z_\u{0080}-\u{10FFFF}][A-Za-z0-9_\u{0080}-\u{10FFFF}]*)?\$"
        ),
        // No `next_exclusion` appears in this lexer, and that is deliberate.
        // It reports a lexical error when the excluded character follows,
        // which is what a *SQL* lexer wants for `$1a` or `123abc`. psql
        // validates nothing: it forwards the bytes and lets the server
        // object. Erroring here would refuse whole psql documents over text
        // psql is happy to send, so every token below simply matches what it
        // matches and longest-match settles the rest.
    }
    ignore {
        // psqlscan.l:163 `comment` — `--` to the end of the physical line.
        LineComment => physical_line(prefix = r"--", priority = 2),
        // psqlscan.l:423-458 `xc` — block comments nest, tracked by
        // `xcdepth`.
        BlockComment => nested(opener = "/*", closer = "*/", priority = 2),
    }
}
