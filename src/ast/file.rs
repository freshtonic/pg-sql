//! Semantic names retained at the strict-statement milestone, plus the
//! document-framing island for the strict SQL document interface.
//!
//! Parsing psql documents is deliberately deferred; the psql terminator
//! values below therefore carry no parser annotations or generated parsing
//! implementations. Strict SQL documents are framed through the generated
//! Recursa document-framing adapter over [`SqlDocumentItem`].

use crate::ast::Statement;

/// One semicolon-separated item of a strict PostgreSQL document.
///
/// The framing island for `framing(island = ast::file::SqlDocumentItem,
/// boundary = SEMI)`. The statement is optional because PostgreSQL's raw
/// parser accepts empty statements (`;`, `;;`, leading and interior
/// semicolons): an empty item remains a source and provenance occurrence in
/// the framed document without entering the semantic statement list.
#[derive(recursa::Node, Debug, Clone)]
pub struct SqlDocumentItem<'input> {
    pub statement: Option<Statement<'input>>,
}

/// A psql meta-command that terminates a SQL statement in place of `;`.
///
/// Psql accepts `\gset`, `\gexec`, `\g`, `\gx`, and `\crosstabview` as
/// statement terminators: e.g. `SELECT oid FROM pg_database \gset` sends the
/// query and binds the results to psql variables, ending the statement just
/// like `;` would.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PsqlTerminator {
    /// `\crosstabview` — listed first as the longest-prefix variant.
    Crosstabview,
    /// `\gexec`
    Gexec,
    /// `\gset`
    Gset,
    /// `\gx`
    Gx,
    /// `\g`
    G,
}

/// The terminator of a SQL statement: a semicolon or a psql meta-command.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatementTerminator {
    /// A psql meta-command like `\gset`.
    Psql(PsqlTerminator),
    /// `\;` — the psql batch separator (ends a statement mid-line).
    BatchSemi,
    /// A plain semicolon.
    Semi,
    /// End of input (unterminated statement at end of file).
    Eof,
}

/// A SQL statement followed by a terminator (`;` or a psql meta-command).
#[derive(Debug)]
pub struct TerminatedStatement<'input> {
    pub stmt: Statement<'input>,
    pub terminator: StatementTerminator,
}
