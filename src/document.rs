//! The strict PostgreSQL server-language document interface.
//!
//! [`parse_sql`] accepts zero or more semicolon-separated PostgreSQL
//! statements — PostgreSQL 17.9 `RAW_PARSE_DEFAULT` input — with an optional
//! final semicolon and no psql-only syntax. Empty statements remain source
//! and provenance occurrences without entering the semantic statement list.
//! A COPY FROM STDIN header is ordinary SQL; the client payload and control
//! line that follow it in a psql script are not, and they are rejected like
//! every other non-SQL region.
//!
//! psql input is rejected here the way any other invalid input is. This
//! grammar mirrors `gram.y`, which contains no psql construct, so there is
//! nothing left in it that could recognise one: a psql script is text the
//! SQL grammar does not accept, and it selects [`SqlParseError::Rejected`]
//! with the failing statement named. To parse one, render it through the
//! `pg-psql` crate first — that is what psql itself does before the server
//! ever sees the text.
//!
//! Success is a [`SqlDocument`]: a fully strict, provenance-bearing
//! partition of the complete source. Ordinary invalid input is rejected with
//! a [`SqlRejection`]: the first failing statement island, its strict parse
//! diagnostics, and the framing cause when the statement itself parsed. A
//! rejection cannot contain or convert into a PostgreSQL statement. Fatal
//! failure is limited to violated framing invariants ([`FrameError`]).

use std::fmt;

use recursa::{ArenaParsed, Span};

use crate::ast::Statement;
use crate::ast::file::SqlDocumentItem;
use crate::{CompletePart, FrameDiagnostic, FrameError, FrameFailure, FrameRejection};

type SqlDocumentItemFamily =
    <SqlDocumentItem<'static> as recursa::__private::ArenaGeneratedParse<'static>>::Family;

/// Parses one strict PostgreSQL document.
///
/// The generated document-framing adapter partitions the source into
/// semicolon-bounded statement islands and right-owned trivia gaps, parses
/// every island strictly, and fails closed on any ordinary failure. A final
/// statement does not need a trailing semicolon. Psql-only syntax — a
/// directive, a send command, a query-buffer escape, COPY payload text, or
/// an interpolation such as `:name` — is not SQL and selects
/// [`SqlParseError::Rejected`]; use the `pg-psql` crate to render it to SQL
/// first.
pub fn parse_sql(source: &str) -> Result<SqlDocument<'_>, SqlParseError<'_>> {
    match SqlDocumentItem::frame(source) {
        Err(FrameFailure::Fatal(fatal)) => Err(SqlParseError::Fatal(fatal)),
        Err(FrameFailure::Rejected(rejection)) => {
            Err(SqlParseError::Rejected(SqlRejection(rejection)))
        }
        Ok(frame) => Ok(SqlDocument { frame }),
    }
}

/// A complete, provenance-bearing strict PostgreSQL document.
///
/// Wraps the exact-source [`CompleteFrame`] partition: every UTF-8 source
/// byte is owned exactly once by an island extent or a right-owned gap, and
/// [`CompleteFrame::render_exact`] reproduces the complete source.
#[derive(derive_more::Deref)]
pub struct SqlDocument<'input> {
    /// Exact-source strict partition; [`std::ops::Deref`] target.
    #[deref]
    frame: recursa::framing::ArenaCompleteFrame<'input, SqlDocumentItemFamily>,
}

impl<'input> SqlDocument<'input> {
    /// Iterates every statement item in source order, including empty
    /// statements, with island-bounded occurrence provenance.
    pub fn items(&self) -> impl Iterator<Item = &ArenaParsed<'input, SqlDocumentItemFamily>> {
        self.frame.typed_islands()
    }

    /// Returns the semantic statement list in source order.
    ///
    /// Empty statements stay out of this list; use [`SqlDocument::items`]
    /// for their source and provenance occurrences.
    pub fn statements(&self) -> Vec<&Statement<'_>> {
        self.frame
            .typed_islands()
            .filter_map(|item| item.ast().statement.as_ref())
            .collect()
    }

    /// Iterates the exact source-ownership partition spans in order.
    pub fn part_spans(&self) -> impl Iterator<Item = Span> + '_ {
        self.frame.parts().map(|part| part.span())
    }

    /// Returns end-of-file trivia owned by the document root, if any.
    ///
    /// An empty document owns one zero-width gap; that is not trivia.
    pub fn eof_trivia(&self) -> Option<&'input str> {
        match self.frame.parts().last()? {
            CompletePart::Gap(gap) if !gap.text().is_empty() => Some(gap.text()),
            CompletePart::Gap(_)
            | CompletePart::Island(_)
            | CompletePart::Line(_)
            | CompletePart::Delimited(_)
            | CompletePart::Payload(_) => None,
        }
    }
}

impl fmt::Debug for SqlDocument<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SqlDocument")
            .field("source", &self.frame.source())
            .field("statements", &self.statements())
            .finish_non_exhaustive()
    }
}

/// Rejection of one strict-parse request.
pub enum SqlParseError<'input> {
    /// Ordinary invalid input: the first failing statement island with its
    /// strict diagnostics and optional framing cause.
    Rejected(SqlRejection<'input>),
    /// Violated grammar, framing, partition, plan, or progress invariants.
    Fatal(FrameError<'input>),
}

impl fmt::Display for SqlParseError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejection) => {
                write!(
                    formatter,
                    "invalid SQL document (statement at {}..{}): {}",
                    rejection.island().start(),
                    rejection.island().end(),
                    rejection.0,
                )
            }
            Self::Fatal(fatal) => fatal.fmt(formatter),
        }
    }
}

impl fmt::Debug for SqlParseError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejection) => formatter
                .debug_struct("Rejected")
                .field("island", &rejection.island())
                .field("diagnostics", &rejection.diagnostics().len())
                .field("framing", &rejection.framing().map(FrameDiagnostic::code))
                .finish_non_exhaustive(),
            Self::Fatal(fatal) => fatal.fmt(formatter),
        }
    }
}

impl std::error::Error for SqlParseError<'_> {}

/// The strict rejection of one document at its first failing statement.
///
/// Wraps the [`FrameRejection`]: the rejected statement extent, that
/// statement's strict parse diagnostics with their stable codes, and the
/// framing cause (`RCA5001` missing COPY terminator, `RCA5002` missing `;`)
/// when the statement itself parsed. No authored value is retained, so a
/// rejection cannot contain or convert into a PostgreSQL statement, parsed
/// statement, or authored document.
#[derive(derive_more::Deref)]
pub struct SqlRejection<'input>(FrameRejection<'input>);
