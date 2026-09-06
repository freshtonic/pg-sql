//! The psql client grammar: what `psqlscan.l` recognises, and nothing else.
//!
//! psql is a client program, not a dialect. It scans its input, substitutes
//! variable interpolations textually, and sends the resulting string to the
//! server; PostgreSQL's `gram.y` therefore contains no psql construct at
//! all. That is why this is a separate crate from `pg-sql` rather than an
//! extension of it: admitting `:'name'` into an expression grammar puts a
//! client rewrite where the server grammar has a type cast or a SQL/JSON
//! key/value separator, and no lookahead settles the difference.
//!
//! The pipeline is three steps, in this order:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut variables = pg_psql::Variables::new();
//! variables.set("x", "42");
//!
//! // 1. Parse the psql document.  2. Substitute and render server SQL.
//! let rendered = pg_psql::render("SELECT int :'x';", &variables)?;
//! assert_eq!(rendered.sql(), "SELECT int '42';");
//!
//! // 3. Only now parse the result as PostgreSQL SQL.
//! let document = rendered.parse_sql().map_err(|error| error.to_string())?;
//! assert_eq!(document.statements().len(), 1);
//! # Ok(()) }
//! ```
//!
//! Rendering keeps a [`SourceMap`] so a diagnostic over the rendered SQL can
//! still name a byte of the user's psql source. Without it, substitution
//! would destroy the tie between a diagnostic and the text the user wrote,
//! which is the whole point of pg-sql's occurrence model.
//!
//! # What this crate does not model
//!
//! psql meta-commands (`\set`, `\d`, `\if`, `\copy`, ...) are out of scope
//! and tracked as freshtonic/pg-sql#11, #12 and #13. `psqlscan.l` does not
//! scan them either — it returns `LEXRES_BACKSLASH` and hands off to
//! `psqlscanslash.l` — so a backslash command that is not one of the send
//! commands stays inside an ordinary [`SqlText`] run and is rendered
//! verbatim. A document using one therefore renders to SQL the server
//! cannot parse, which is a diagnosable condition rather than a silent
//! rewrite.

recursa::grammar! {
    module = crate,
    derives(Pretty, Visit),
    keyword_matching = sensitive,
    max_lookahead = 1,
    diagrams,
}

pub mod ast;
pub mod substitution;
pub mod tokens;

pub use ast::{Interpolation, PsqlDocument, PsqlItem, SendCommand, SqlAtom, SqlText, Terminator};
pub use substitution::{Origin, Rendered, SourceMap, Unbound, Variables};

use std::fmt;

/// Why a psql document could not be parsed.
#[derive(Debug)]
pub enum PsqlError {
    /// One or more source ranges were not recognised by the psql lexer.
    Lexical(Vec<recursa::Span>),
    /// The recognised tokens did not form a psql document.
    Syntax(recursa::ParseError),
    /// A document was parsed but input remained, which the item repetition
    /// covering the whole source makes a grammar defect rather than a user
    /// error.
    TrailingInput,
}

impl fmt::Display for PsqlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexical(spans) => {
                write!(formatter, "{} psql lexical error(s)", spans.len())
            }
            Self::Syntax(error) => error.fmt(formatter),
            Self::TrailingInput => formatter.write_str("trailing input after the psql document"),
        }
    }
}

impl std::error::Error for PsqlError {}

/// Parses `source` as psql, substitutes `variables`, and renders server SQL.
///
/// This is the whole pipeline up to the SQL parse; call
/// [`Rendered::parse_sql`] for the last step.
pub fn render(source: &str, variables: &Variables) -> Result<Rendered, PsqlError> {
    let document = parse(source)?;
    Ok(substitution::render_document(&document, source, variables))
}

/// Parses one complete psql source document.
///
/// The result is the item sequence `psqlscan.l` produces: runs of SQL text,
/// variable interpolations, and the terminators that submit a query buffer.
/// Nothing is substituted — use [`render`] for that.
pub fn parse(source: &str) -> Result<PsqlDocument<'_>, PsqlError> {
    let lexed = lex(source);
    let lexical = lexed.errors().map(|error| error.span()).collect::<Vec<_>>();
    if !lexical.is_empty() {
        return Err(PsqlError::Lexical(lexical));
    }
    let mut input = lexed.input();
    let parsed = PsqlDocument::parse(&mut input).map_err(PsqlError::Syntax)?;
    if !input.is_eof() {
        return Err(PsqlError::TrailingInput);
    }
    Ok(parsed.into_ast())
}
