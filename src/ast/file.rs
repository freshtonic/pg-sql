//! Semantic names retained at the strict-statement milestone.
//!
//! Parsing SQL and psql documents is deliberately deferred to the general
//! Recursa document-framing capability. These values therefore carry no
//! parser annotations or generated parsing implementations.

use crate::ast::Statement;

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
