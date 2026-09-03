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
//! Success is a [`SqlDocument`]: a fully strict, provenance-bearing
//! partition of the complete source. Ordinary invalid input is rejected with
//! a [`SqlRejection`]: the first failing statement island, its strict parse
//! diagnostics, and the framing cause when the statement itself parsed. A
//! rejection cannot contain or convert into a PostgreSQL statement. Fatal
//! failure is limited to violated framing invariants ([`FrameError`]).

use std::fmt;
use std::ops::ControlFlow;

use recursa::{NodeView, Parsed, Span, Visit, VisitBreak, Visitor};

use crate::ast::Statement;
use crate::ast::file::SqlDocumentItem;
use crate::tokens::literal::PsqlVariableValue;
use crate::{CompleteFrame, CompletePart, FrameDiagnostic, FrameError, FrameFailure, FrameRejection};

/// Parses one strict PostgreSQL document.
///
/// The generated document-framing adapter partitions the source into
/// semicolon-bounded statement islands and right-owned trivia gaps, parses
/// every island strictly, and fails closed on any ordinary failure. A final
/// statement does not need a trailing semicolon. Psql-only syntax is
/// rejected: directives, send commands, query-buffer escapes, and COPY
/// payload text are not SQL and select [`SqlParseError::Rejected`], while
/// psql interpolation (`:name`, `:'name'`, `:"name"`) parses lexically but
/// is rejected as [`SqlParseError::Psql`].
pub fn parse_sql(source: &str) -> Result<SqlDocument<'_>, SqlParseError<'_>> {
    match SqlDocumentItem::frame(source) {
        Err(FrameFailure::Fatal(fatal)) => Err(SqlParseError::Fatal(fatal)),
        Err(FrameFailure::Rejected(rejection)) => {
            Err(SqlParseError::Rejected(SqlRejection(rejection)))
        }
        Ok(frame) => match first_psql_use(&frame) {
            Some(rejection) => Err(SqlParseError::Psql(rejection)),
            None => {
                // The generated view accessors do not declare precise
                // `use<..>` capture, so under edition 2024 a borrowed
                // statement view cannot be returned across a closure
                // boundary; the semantic list is cloned once instead.
                let statements = frame
                    .typed_islands()
                    .filter_map(|item| item.root().value().statement.clone())
                    .collect();
                Ok(SqlDocument { frame, statements })
            }
        },
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
    frame: CompleteFrame<'input, SqlDocumentItem<'input>>,
    /// Semantic statement list projected once from the strict islands.
    statements: Vec<Statement<'input>>,
}

impl<'input> SqlDocument<'input> {
    /// Iterates every statement item in source order, including empty
    /// statements, with island-bounded occurrence provenance.
    pub fn items(&self) -> impl Iterator<Item = &Parsed<'input, SqlDocumentItem<'input>>> {
        self.frame.typed_islands()
    }

    /// Returns the semantic statement list in source order.
    ///
    /// Empty statements stay out of this list; use [`SqlDocument::items`]
    /// for their source and provenance occurrences.
    pub fn statements(&self) -> &[Statement<'input>] {
        &self.statements
    }

    /// Iterates the exact source-ownership partition spans in order.
    pub fn part_spans(&self) -> impl Iterator<Item = Span> + '_ {
        self.frame.parts().map(CompletePart::span)
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
            .field("statements", &self.statements)
            .finish_non_exhaustive()
    }
}

/// Rejection of one strict-parse request.
pub enum SqlParseError<'input> {
    /// Ordinary invalid input: the first failing statement island with its
    /// strict diagnostics and optional framing cause.
    Rejected(SqlRejection<'input>),
    /// Psql interpolation inside an otherwise-parseable document.
    Psql(PsqlSyntaxError),
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
            Self::Psql(psql) => psql.fmt(formatter),
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
            Self::Psql(psql) => psql.fmt(formatter),
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

/// Psql interpolation found inside an otherwise-parseable document.
///
/// The strict interface accepts PostgreSQL `RAW_PARSE_DEFAULT` input only.
/// `:name`, `:'name'`, and `:"name"` are psql client-side substitutions, not
/// server SQL, so a document containing one is rejected even though the
/// permissive statement grammar (which also serves the psql-flavored
/// regression corpus) can parse it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PsqlSyntaxError {
    /// Extent of the statement island containing the interpolation.
    span: Span,
}

impl PsqlSyntaxError {
    /// Returns the extent of the statement island containing the
    /// interpolation, in absolute document offsets.
    pub const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PsqlSyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "psql variable interpolation is not PostgreSQL server SQL (statement at {}..{})",
            self.span.start(),
            self.span.end()
        )
    }
}

impl std::error::Error for PsqlSyntaxError {}

/// Breaks traversal at the first psql client-side substitution.
///
/// `PsqlVariableValue` is the one shared leaf of every interpolation
/// spelling: the expression atom (`SELECT :name`), the typed-literal cast
/// (`bigint :'name'`), function bodies (`AS :'lib'`), and COPY targets. The
/// lower-unbounded array-slice forms (`[:2]`, `[:]`, `[:(expr)]`) that reuse
/// the same colon atom carry non-psql values and stay accepted.
#[derive(recursa::TotalVisitor)]
#[total_visitor(
    dispatch = [crate::tokens::literal::PsqlVariableValue<'static>],
    error = (),
    event = crate::__RecursaVisitEvent,
)]
struct PsqlUseScan;

impl<'input> Visitor<PsqlVariableValue<'input>> for PsqlUseScan {
    type Error = ();

    fn enter(&mut self, _node: &PsqlVariableValue<'input>) -> ControlFlow<VisitBreak<()>> {
        ControlFlow::Break(VisitBreak::Error(()))
    }
}

/// Finds the first island containing psql interpolation, if any.
fn first_psql_use(frame: &CompleteFrame<'_, SqlDocumentItem<'_>>) -> Option<PsqlSyntaxError> {
    frame.parts().find_map(|part| match part {
        CompletePart::Island(island) => match island.parsed().visit(&mut PsqlUseScan) {
            ControlFlow::Break(VisitBreak::Error(())) => Some(PsqlSyntaxError {
                span: island.span(),
            }),
            ControlFlow::Break(VisitBreak::SkipChildren | VisitBreak::Finished)
            | ControlFlow::Continue(()) => None,
        },
        CompletePart::Gap(_)
        | CompletePart::Line(_)
        | CompletePart::Delimited(_)
        | CompletePart::Payload(_) => None,
    })
}
