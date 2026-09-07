//! Semantic names retained at the strict-statement milestone, plus the
//! document-framing island for the strict SQL document interface.
//!
//! Strict SQL documents are framed through the generated Recursa
//! document-framing adapter over [`SqlDocumentItem`]. psql's own terminators
//! -- the `\g` family of send commands and the `\;` batch separator -- are
//! client syntax the server never sees, and they live in the `pg-psql`
//! crate's grammar; the names below carry no parser annotations and no
//! generated parsing implementations.

use crate::ast::Statement;

/// One semicolon-separated item of a strict PostgreSQL document.
///
/// The framing island for `framing(island = ast::file::SqlDocumentItem,
/// boundary = SEMI)`. The statement is optional because PostgreSQL's raw
/// parser accepts empty statements (`;`, `;;`, leading and interior
/// semicolons): an empty item remains a source and provenance occurrence in
/// the framed document without entering the semantic statement list.
#[derive(recursa::Node, Debug)]
pub struct SqlDocumentItem<'input> {
    pub statement: Option<Statement<'input>>,
}

/// The terminator of a SQL statement.
///
/// PostgreSQL's raw parser sees exactly these two: a semicolon, or the end
/// of the string. psql's send commands terminate a statement for the
/// *client*, and `pg_psql::Terminator` models those.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatementTerminator {
    /// A plain semicolon.
    Semi,
    /// End of input (unterminated statement at end of file).
    Eof,
}

/// A SQL statement followed by its terminator.
#[derive(Debug)]
pub struct TerminatedStatement<'input> {
    pub stmt: Statement<'input>,
    pub terminator: StatementTerminator,
}
