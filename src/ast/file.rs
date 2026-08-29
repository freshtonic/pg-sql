//! File-driver AST types and helpers: psql terminators, the top-level
//! `FileItem` enum, and the `parse_sql_file` / `parse_sql_file_with_spans`
//! drivers along with the psql-directive predicates they expose.
//!
//! Per §4 of the destination map every type and helper concerned with
//! the file-driver layer lives here, leaving `ast/mod.rs` to host only
//! the `Statement` enum and its direct-parse tests.

use std::ops::Range;

use recursa::{FormatTokens, Input, Parse, ParseError, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::Statement;
use crate::ast::utility::copy::{CopyBody, CopyDirection, CopyTarget};
use crate::tokens::{literal, punct};

/// A psql meta-command that terminates a SQL statement in place of `;`.
///
/// Psql accepts `\gset`, `\gexec`, `\g`, `\gx`, and `\crosstabview` as
/// statement terminators: e.g. `SELECT oid FROM pg_database \gset` sends the
/// query and binds the results to psql variables, ending the statement just
/// like `;` would.
#[derive(Debug, Clone, FormatTokens, Visit, Transform, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PsqlTerminator {
    #[tok(PSQLCROSSTABVIEW)] /// `\crosstabview` — listed first as the longest-prefix variant.
    Crosstabview,
    #[tok(PSQLGEXEC)] /// `\gexec`
    Gexec,
    #[tok(PSQLGSET)] /// `\gset`
    Gset,
    #[tok(PSQLGX)] /// `\gx`
    Gx,
    #[tok(PSQLG)] /// `\g`
    G,
}

/// The terminator of a SQL statement: a semicolon or a psql meta-command.
#[derive(Debug, Clone, FormatTokens, Visit, Transform, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StatementTerminator {
    /// A psql meta-command like `\gset`.
    Psql(PsqlTerminator),
    #[tok(PSQLBATCHSEMI)] /// `\;` — the psql batch separator (ends a statement mid-line).
    BatchSemi,
    #[tok(SEMI)] /// A plain semicolon.
    Semi,
    /// End of input (unterminated statement at end of file).
    Eof(recursa::Eof),
}

/// A SQL statement followed by a terminator (`;` or a psql meta-command).
#[derive(Debug, FormatTokens, Visit, Transform)]
pub struct TerminatedStatement<'input> {
    pub stmt: Statement<'input>,
    pub terminator: StatementTerminator,
}
