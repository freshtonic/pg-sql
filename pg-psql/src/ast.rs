//! The psql document AST.
//!
//! A psql document is a sequence of items: runs of SQL text the client
//! forwards untouched, variable interpolations it rewrites, and the
//! terminators that submit a query buffer. That is the whole of what
//! `vendor/postgres/src/fe_utils/psqlscan.l` distinguishes, and this AST
//! models no more than that: the SQL inside a text run is not parsed here,
//! because psql does not parse it either.

/// One complete psql source document.
///
/// The item list covers the whole source: the repetition ends only at end of
/// input, so an empty document is an empty list.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PsqlDocument<'input> {
    /// Greedy over every kind: this is the root, so nothing follows it and
    /// there is no caller continuation an item could be mistaken for. The
    /// repetition ends at end of input, which `crate::parse` then checks.
    #[greedy(all)]
    pub items: Vec<PsqlItem<'input>>,
}

/// One item of a psql document.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PsqlItem<'input> {
    /// A variable interpolation, which substitution rewrites.
    Interpolation(Interpolation<'input>),
    /// A terminator, which submits the query buffer.
    Terminator(Terminator<'input>),
    /// A run of text psql forwards to the server unchanged.
    Sql(SqlText<'input>),
}

/// A psql variable interpolation.
///
/// Each form is one lexical token, exactly as `psqlscan.l` scans it. That
/// matters: pg-sql previously lexed the colon and the name separately and
/// joined them in the SQL grammar, which made `SELECT int :'x'` ambiguous
/// against the SQL/JSON `int : value` entry and cost 8 LALR conflicts.
///
/// Every form shares one name class, `psqlscan.l:376` `variable_char`,
/// written there as `[A-Za-z\200-\377_0-9]`. The high-byte range covers
/// every non-ASCII byte, which in UTF-8 source is every non-ASCII scalar.
/// Unlike an identifier it has no separate start class, so `:1` is a
/// variable reference.
///
/// An incomplete form falls back to a bare colon followed by ordinary text,
/// which is what `psqlscan.l:775-796` does with `yyless(1)`: `:'a b'` is a
/// colon and a string literal, not an interpolation, because a space is not
/// a `variable_char`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interpolation<'input> {
    /// `:'name'` — psqlscan.l:756. Substituted as a SQL string literal.
    Literal(
        #[lex(pattern = r":'[A-Za-z0-9_\u{0080}-\u{10FFFF}]+'")] LiteralInterpolation<'input>,
    ),
    /// `:"name"` — psqlscan.l:761. Substituted as a quoted identifier.
    Identifier(
        #[lex(pattern = r#":"[A-Za-z0-9_\u{0080}-\u{10FFFF}]+""#)]
        IdentifierInterpolation<'input>,
    ),
    /// `:{?name}` — psqlscan.l:766. Substituted as `TRUE` or `FALSE`
    /// according to whether the variable is set.
    Test(
        #[lex(pattern = r":\{\?[A-Za-z0-9_\u{0080}-\u{10FFFF}]+\}")] TestInterpolation<'input>,
    ),
    /// `:name` — psqlscan.l:710. Substituted as raw text.
    ///
    /// Listed last so the three punctuated forms are tried first; they are
    /// separate token kinds, so the order records intent rather than
    /// resolving a conflict.
    Raw(#[lex(pattern = r":[A-Za-z0-9_\u{0080}-\u{10FFFF}]+")] RawInterpolation<'input>),
}

impl<'input> Interpolation<'input> {
    /// The exact source spelling, punctuation included.
    pub fn text(&self) -> &str {
        match self {
            Self::Literal(token) => token.text(),
            Self::Identifier(token) => token.text(),
            Self::Test(token) => token.text(),
            Self::Raw(token) => token.text(),
        }
    }

    /// The variable name, with the interpolation's punctuation removed.
    ///
    /// The lexical patterns fix each form's shape, and every delimiter is
    /// ASCII, so the byte slices below always land on scalar boundaries.
    pub fn name(&self) -> &str {
        let text = self.text();
        match self {
            // `:'name'` and `:"name"` — two leading bytes, one trailing.
            Self::Literal(_) | Self::Identifier(_) => &text[2..text.len() - 1],
            // `:{?name}` — three leading bytes, one trailing.
            Self::Test(_) => &text[3..text.len() - 1],
            // `:name` — one leading byte.
            Self::Raw(_) => &text[1..],
        }
    }
}

/// A terminator: psql submits the current query buffer and starts a new one.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Terminator<'input> {
    /// `;` — psqlscan.l:681, the only terminator `psqlscan.l` itself
    /// returns. The server sees it unchanged, so rendering copies it.
    ///
    /// psql suppresses the boundary inside parentheses and inside a tracked
    /// `BEGIN ... END` block; this crate does not track either, because it
    /// renders one SQL string for the whole document rather than splitting
    /// it into submissions. The distinction is invisible to the rendered
    /// text, which pg-sql then frames on its own semicolons.
    #[tok(SEMI)]
    Semi,
    /// `\g`, `\gx`, `\gset`, `\gexec`, `\crosstabview` — client syntax the
    /// server never sees, so rendering replaces one with `;`.
    Send(SendCommand<'input>),
}

/// A psql send command.
///
/// These are read by `psqlscanslash.l`, not `psqlscan.l`, which returns
/// `LEXRES_BACKSLASH` and hands off. They are modelled here, and the rest of
/// that scanner is not, because a send command terminates a statement and so
/// changes where SQL text begins and ends; `\set` and its kin do not.
///
/// Each carries an explicit priority so it wins the tie against the
/// [`SqlAtom::MetaCommand`] catch-all, which matches the same text. Where
/// the catch-all matches *more* text it wins on length instead, which is
/// what makes `\gsetfoo` one unknown command rather than `\gset` followed
/// by `foo` — `psqlscanslash.l` reads a whole command name before looking it
/// up.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SendCommand<'input> {
    /// `\crosstabview`
    Crosstabview(
        #[lex(pattern = r"\\crosstabview", priority = 2)] SendCrosstabview<'input>,
    ),
    /// `\gexec`
    Gexec(#[lex(pattern = r"\\gexec", priority = 2)] SendGexec<'input>),
    /// `\gset`
    Gset(#[lex(pattern = r"\\gset", priority = 2)] SendGset<'input>),
    /// `\gx`
    Gx(#[lex(pattern = r"\\gx", priority = 2)] SendGx<'input>),
    /// `\g`
    G(#[lex(pattern = r"\\g", priority = 2)] SendG<'input>),
}

impl<'input> SendCommand<'input> {
    /// The exact source spelling.
    pub fn text(&self) -> &str {
        match self {
            Self::Crosstabview(token) => token.text(),
            Self::Gexec(token) => token.text(),
            Self::Gset(token) => token.text(),
            Self::Gx(token) => token.text(),
            Self::G(token) => token.text(),
        }
    }
}

/// A maximal run of source that psql forwards to the server unchanged.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SqlText<'input> {
    pub atoms: recursa::Vec1<SqlAtom<'input>>,
}

/// One lexical unit of a forwarded text run.
///
/// The quoted and dollar-quoted forms are here for one reason: they are the
/// states in which `psqlscan.l` does *not* recognise an interpolation. A
/// colon inside a string, a dollar-quoted body, a quoted identifier or a
/// comment is ordinary content, and it stays ordinary content because the
/// whole construct is one token. Comments reach the same end by being
/// ignored trivia, which rendering preserves because it copies source.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SqlAtom<'input> {
    /// `'...'` — psqlscan.l:485-491 `xq`, with `''` doubling. psql's
    /// default `standard_conforming_strings` is on, so a backslash here is
    /// an ordinary character.
    String(#[lex(pattern = r"'[^']*(?:''[^']*)*'")] StringText<'input>),
    /// `E'...'` — psqlscan.l:492-495 `xe`, where a backslash escapes.
    EscapeString(#[lex(pattern = r"(?i:E)'(?:[^'\\]|\\.|'')*'")] EscapeStringText<'input>),
    /// `U&'...'` — psqlscan.l `xus`.
    UnicodeString(#[lex(pattern = r"(?i:U)&'[^']*(?:''[^']*)*'")] UnicodeStringText<'input>),
    /// `B'...'` — psqlscan.l `xb`.
    BitString(#[lex(pattern = r"(?i:B)'[^']*'")] BitStringText<'input>),
    /// `X'...'` — psqlscan.l `xh`.
    HexString(#[lex(pattern = r"(?i:X)'[^']*'")] HexStringText<'input>),
    /// `U&"..."` — psqlscan.l `xui`.
    UnicodeIdentifier(
        #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*""#)] UnicodeIdentifierText<'input>,
    ),
    /// `"..."` — psqlscan.l:622-624 `xd`, with `""` doubling.
    QuotedIdentifier(#[lex(pattern = r#""[^"]*(?:""[^"]*)*""#)] QuotedIdentifierText<'input>),
    /// `$tag$...$tag$` — psqlscan.l:564-601 `xdolq`.
    DollarString(#[lex(matcher)] DollarString<'input>),
    /// `$1` — psqlscan.l:352 `param`, which is `\${decdigit}+` and carries
    /// no exclusion: `$1a` is a param and an identifier, and psql forwards
    /// both for the server to object to.
    DollarNumber(#[lex(pattern = r"\$[0-9]+")] DollarNumber<'input>),
    /// `\;` — psqlscan.l:697-702. Not a submission boundary: it contributes
    /// a semicolon to the query buffer, so rendering emits `;` for it.
    BatchSemi(#[lex(pattern = r"\\;")] BatchSemiText<'input>),
    /// An identifier-shaped run, `psqlscan.l:340` `identifier`. `$` is a
    /// continuation character there, so `a$$b$$` is one identifier and not
    /// an identifier followed by a dollar-quoted string.
    Word(
        #[lex(pattern = r"[A-Za-z_\u{0080}-\u{10FFFF}][A-Za-z0-9_$\u{0080}-\u{10FFFF}]*")]
        WordText<'input>,
    ),
    /// A run of digits. psql's numeric rules are finer, but nothing here
    /// interprets a number, and no numeric spelling can contain or start an
    /// interpolation.
    Digits(#[lex(pattern = r"[0-9]+")] DigitsText<'input>),
    /// Any other run of operator and delimiter characters. The excluded
    /// characters are exactly those that can begin a longer token above.
    Punct(#[lex(pattern = r#"[^\s\-/:;'"$\\A-Za-z0-9_\u{0080}-\u{10FFFF}]+"#)] PunctText<'input>),
    #[tok(COLONCOLON)]
    Cast,
    #[tok(COLONEQUALS)]
    ColonEquals,
    #[tok(COLON)]
    Colon,
    #[tok(MINUS)]
    Minus,
    #[tok(SLASH)]
    Slash,
    #[tok(DOLLAR)]
    Dollar,
    /// A backslash command that is not a send command: an unmodelled psql
    /// meta-command such as `\set` or `\getenv`, forwarded verbatim. The
    /// whole name is one token, as `psqlscanslash.l` reads it, so a longer
    /// name always beats a send-command prefix. See freshtonic/pg-sql#11,
    /// #12, #13.
    MetaCommand(#[lex(pattern = r"\\[A-Za-z][A-Za-z0-9_]*")] MetaCommandText<'input>),
    /// A backslash starting no command name at all.
    #[tok(BACKSLASH)]
    Backslash,
}
