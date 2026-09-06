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
        // only the delimiter that opened it can close it.
        DollarString => same_delimiter(opener = r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$"),
        // psqlscan.l:352 `param` — `$1` is a server placeholder, not a
        // dollar quote.
        DollarNumber => next_exclusion(pattern = r"\$[0-9]+", excluded = r"[A-Za-z0-9_]"),
        // The send commands, which submit the query buffer exactly as `;`
        // does. `psqlscanslash.l` reads a whole command name before looking
        // it up, so `\gsetfoo` is not `\gset` followed by `foo`; the
        // exclusion reproduces that, and the run becomes a psql lexical
        // error rather than a silent statement boundary, which is what psql
        // answers with "invalid command" too.
        SendCrosstabview => next_exclusion(pattern = r"\\crosstabview", excluded = r"[A-Za-z0-9_]"),
        SendGexec => next_exclusion(pattern = r"\\gexec", excluded = r"[A-Za-z0-9_]"),
        SendGset => next_exclusion(pattern = r"\\gset", excluded = r"[A-Za-z0-9_]"),
        SendGx => next_exclusion(pattern = r"\\gx", excluded = r"[A-Za-z0-9_]"),
        SendG => next_exclusion(pattern = r"\\g", excluded = r"[A-Za-z0-9_]"),
    }
    ignore {
        // psqlscan.l:163 `comment` — `--` to the end of the physical line.
        LineComment => physical_line(prefix = r"--", priority = 2),
        // psqlscan.l:423-458 `xc` — block comments nest, tracked by
        // `xcdepth`.
        BlockComment => nested(opener = "/*", closer = "*/", priority = 2),
    }
}
