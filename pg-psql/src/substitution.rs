//! Substitution: a psql document becomes server SQL text plus a source map.
//!
//! Rendering copies the source verbatim and rewrites only the regions psql
//! itself rewrites. That is what keeps the map exact: everything outside a
//! recorded region is the user's own bytes at a known shift, so a diagnostic
//! over the rendered SQL can always be carried back to the psql source.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::ast::{Interpolation, PsqlDocument, PsqlItem, SqlAtom, Terminator};

/// psql variable bindings.
///
/// Names are case-sensitive, as psql's are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Variables(BTreeMap<String, String>);

impl Variables {
    /// An empty binding set: every interpolation will be left verbatim.
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds `name`, returning the previous value if there was one.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.0.insert(name.into(), value.into())
    }

    /// Unbinds `name`, returning the previous value if there was one.
    pub fn unset(&mut self, name: &str) -> Option<String> {
        self.0.remove(name)
    }

    /// The value bound to `name`, if any.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    /// Whether `name` is bound. This is what `:{?name}` tests.
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for Variables {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(bindings: I) -> Self {
        Self(
            bindings
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        )
    }
}

/// An interpolation left in the rendered SQL because its variable is unbound.
///
/// psql copies such a token through unchanged (`psqlscan.l:744-751` for
/// `:name`, `psqlscan.l:1561-1565` for the quoted forms), and so does this
/// crate: substitution never invents a value. The SQL parse then fails on
/// the surviving colon, which makes an unbound variable a diagnosable
/// condition rather than a silent rewrite — and this list names it directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unbound {
    /// The variable name, without the interpolation's punctuation.
    pub name: String,
    /// The interpolation's extent in the psql source.
    pub source: Range<usize>,
}

/// Where a byte of the rendered SQL came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The byte is the user's own text, at this psql-source offset.
    Verbatim(usize),
    /// The byte is substituted text. A substituted region has no byte-level
    /// correspondence to the source — the value never appeared there — so
    /// the extent of the interpolation that produced it is the whole truth
    /// available.
    Substituted { start: usize, end: usize },
}

/// Translates offsets in the rendered SQL back to the psql source.
///
/// Outside the recorded regions the two texts differ only by an accumulated
/// shift, so a verbatim offset is exact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceMap {
    /// Rewritten regions, disjoint and in ascending rendered order.
    regions: Vec<Region>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Region {
    rendered: Range<usize>,
    source: Range<usize>,
}

impl SourceMap {
    /// Where the byte at `rendered` came from.
    ///
    /// An offset past the end of the rendered SQL continues the final
    /// verbatim run, so `origin(sql.len())` names the end of the source.
    pub fn origin(&self, rendered: usize) -> Origin {
        // The first region that has not already ended at `rendered`.
        let next = self
            .regions
            .partition_point(|region| region.rendered.end <= rendered);
        if let Some(region) = self.regions.get(next)
            && region.rendered.start <= rendered
        {
            return Origin::Substituted {
                start: region.source.start,
                end: region.source.end,
            };
        }
        // Between regions the texts run in step, so the offset is the end of
        // the previous region plus the distance travelled since.
        let (rendered_base, source_base) = match next.checked_sub(1) {
            Some(previous) => {
                let previous = &self.regions[previous];
                (previous.rendered.end, previous.source.end)
            }
            None => (0, 0),
        };
        Origin::Verbatim(source_base + (rendered - rendered_base))
    }

    /// The rewritten regions as `(rendered, source)` pairs, in order.
    pub fn regions(&self) -> impl Iterator<Item = (Range<usize>, Range<usize>)> + '_ {
        self.regions
            .iter()
            .map(|region| (region.rendered.clone(), region.source.clone()))
    }
}

/// Server SQL rendered from a psql document, with the map back to its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    sql: String,
    map: SourceMap,
    unbound: Vec<Unbound>,
}

impl Rendered {
    /// The rendered server SQL.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// The map from rendered offsets back to the psql source.
    pub fn map(&self) -> &SourceMap {
        &self.map
    }

    /// Interpolations left verbatim because their variables are unbound.
    pub fn unbound(&self) -> &[Unbound] {
        &self.unbound
    }

    /// Parses the rendered SQL as a strict PostgreSQL document.
    ///
    /// This is the last step of the pipeline, and the only place pg-sql is
    /// asked to read anything: the SQL grammar never sees psql syntax.
    pub fn parse_sql(&self) -> Result<pg_sql::SqlDocument<'_>, pg_sql::SqlParseError<'_>> {
        pg_sql::document::parse_sql(&self.sql)
    }
}

/// One rewritten span of the source and the text that replaces it.
struct Rewrite {
    source: Range<usize>,
    text: String,
}

pub(crate) fn render_document(
    document: &PsqlDocument<'_>,
    source: &str,
    variables: &Variables,
) -> Rendered {
    let mut rewrites = Vec::new();
    let mut unbound = Vec::new();

    for item in &document.items {
        match item {
            PsqlItem::Interpolation(interpolation) => {
                let span = span_of(source, interpolation.text());
                match substitute(interpolation, variables) {
                    Some(text) => rewrites.push(Rewrite { source: span, text }),
                    // Left verbatim: no rewrite, so the token stays part of
                    // the surrounding copied run.
                    None => unbound.push(Unbound {
                        name: interpolation.name().to_owned(),
                        source: span,
                    }),
                }
            }
            // A send command submits the buffer but is client syntax the
            // server never sees, so it renders as the boundary it is.
            PsqlItem::Terminator(Terminator::Send(send)) => rewrites.push(Rewrite {
                source: span_of(source, send.text()),
                text: ";".to_owned(),
            }),
            PsqlItem::Terminator(Terminator::Semi) => {}
            PsqlItem::Sql(text) => {
                for atom in text.atoms.iter() {
                    // `\;` and `\:` each force their second character into
                    // the query buffer (psqlscan.l:697-702); the server
                    // receives that one byte, not the two the user wrote.
                    if let SqlAtom::Escaped(token) = atom {
                        let escaped = token.text();
                        rewrites.push(Rewrite {
                            source: span_of(source, escaped),
                            text: escaped[1..].to_owned(),
                        });
                    }
                }
            }
        }
    }

    // Items are parsed in source order and each rewrite lies inside its own
    // item, so the rewrites are already ascending and disjoint.
    let mut sql = String::with_capacity(source.len());
    let mut regions = Vec::with_capacity(rewrites.len());
    let mut cursor = 0;
    for rewrite in rewrites {
        sql.push_str(&source[cursor..rewrite.source.start]);
        let start = sql.len();
        sql.push_str(&rewrite.text);
        regions.push(Region {
            rendered: start..sql.len(),
            source: rewrite.source.clone(),
        });
        cursor = rewrite.source.end;
    }
    sql.push_str(&source[cursor..]);

    Rendered {
        sql,
        map: SourceMap { regions },
        unbound,
    }
}

/// The text a bound interpolation renders to, or `None` when it is unbound.
fn substitute(interpolation: &Interpolation<'_>, variables: &Variables) -> Option<String> {
    let name = interpolation.name();
    match interpolation {
        Interpolation::Literal(_) => variables.get(name).map(quote_literal),
        Interpolation::Identifier(_) => variables.get(name).map(quote_identifier),
        Interpolation::Raw(_) => variables.get(name).map(str::to_owned),
        // The existence test is the one form with no unbound case: psql
        // answers it rather than passing it through (psqlscan.l:1568-1591).
        Interpolation::Test(_) => Some(
            if variables.contains(name) {
                "TRUE"
            } else {
                "FALSE"
            }
            .to_owned(),
        ),
    }
}

/// Quotes a value as a SQL string literal, as libpq's `PQescapeLiteral` does.
///
/// `fe-exec.c:4371-4375` doubles every `'` and every `\`. When the value
/// contains a backslash, `fe-exec.c:4334-4345` emits ` E'...'` instead of
/// `'...'`, so the literal means the same under either setting of
/// `standard_conforming_strings`; the leading space guards against the
/// result landing immediately after an identifier.
fn quote_literal(value: &str) -> String {
    let backslashes = value.bytes().filter(|byte| *byte == b'\\').count();
    let mut quoted = String::with_capacity(value.len() + backslashes + 4);
    if backslashes > 0 {
        quoted.push_str(" E");
    }
    quoted.push('\'');
    for character in value.chars() {
        if character == '\'' || character == '\\' {
            quoted.push(character);
        }
        quoted.push(character);
    }
    quoted.push('\'');
    quoted
}

/// Quotes a value as a quoted identifier, as libpq's `PQescapeIdentifier`
/// does: `fe-exec.c:4371-4375` doubles every `"`, and a backslash is an
/// ordinary character inside an identifier.
fn quote_identifier(value: &str) -> String {
    let quotes = value.bytes().filter(|byte| *byte == b'"').count();
    let mut quoted = String::with_capacity(value.len() + quotes + 2);
    quoted.push('"');
    for character in value.chars() {
        if character == '"' {
            quoted.push(character);
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

/// The extent of `part` inside `source`.
///
/// Every token this crate reads is a subslice of the parsed source, so the
/// distance between the two pointers is the offset. recursa's own span
/// capture is per node and reached through generated views; a token's source
/// slice carries the same fact directly and without a traversal, and the
/// assertion keeps the two honest.
fn span_of(source: &str, part: &str) -> Range<usize> {
    let base = source.as_ptr() as usize;
    let start = part.as_ptr() as usize;
    assert!(
        start >= base && start + part.len() <= base + source.len(),
        "token text is not a subslice of the parsed source",
    );
    (start - base)..(start - base + part.len())
}
