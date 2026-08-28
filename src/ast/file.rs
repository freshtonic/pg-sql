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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[recursa::parser(rules = SqlRules)]
pub enum PsqlTerminator {
    /// `\crosstabview` — listed first as the longest-prefix variant.
    Crosstabview(punct::PsqlCrosstabview),
    /// `\gexec`
    Gexec(punct::PsqlGexec),
    /// `\gset`
    Gset(punct::PsqlGset),
    /// `\gx`
    Gx(punct::PsqlGx),
    /// `\g`
    G(punct::PsqlG),
}

/// The terminator of a SQL statement: a semicolon or a psql meta-command.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[recursa::parser(rules = SqlRules)]
pub enum StatementTerminator {
    /// A psql meta-command like `\gset`.
    Psql(PsqlTerminator),
    /// `\;` — the psql batch separator (ends a statement mid-line).
    BatchSemi(punct::PsqlBatchSemi),
    /// A plain semicolon.
    Semi(punct::Semi),
    /// End of input (unterminated statement at end of file).
    Eof(recursa::Eof),
}

/// A SQL statement followed by a terminator (`;` or a psql meta-command).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TerminatedStatement<'input> {
    pub stmt: Statement<'input>,
    pub terminator: StatementTerminator,
}

/// A psql directive: backslash followed by the rest of the line.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PsqlDirective<'input> {
    pub backslash: punct::BackSlash,
    pub rest: literal::RestOfLine<'input>,
}

/// A command in a psql input file: either a SQL statement or a psql directive.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PsqlCommand<'input> {
    /// A psql directive (e.g., `\pset null '(null)'`).
    /// Listed first so `\` is checked before statement keywords.
    Directive(PsqlDirective<'input>),
    /// A SQL statement followed by a semicolon.
    Statement(TerminatedStatement<'input>),
}

/// An item in a parsed SQL file.
///
/// One of:
/// - [`FileItem::Command`] — a parsed SQL statement or psql directive.
/// - [`FileItem::RawLines`] — non-SQL text (e.g. COPY FROM stdin data blocks)
///   that the parser preserves verbatim.
/// - [`FileItem::ParseError`] — a single SQL statement that pg-sql could not
///   parse structurally. The parser recovers by byte-walking to the next
///   top-level `;`, records the failed statement's source span, and continues
///   so the rest of the file still parses. PostgreSQL regression `.sql` files
///   intentionally include statements PG also rejects; those land here with
///   parse errors mirroring PG's behaviour.
pub enum FileItem<'input> {
    Command(PsqlCommand<'input>),
    RawLines(::std::borrow::Cow<'input, str>),
    /// A statement that failed to parse. `span` covers the failed statement
    /// (including its terminator, when one was present); `error` is the
    /// underlying [`ParseError`] from the structured parse attempt.
    ParseError {
        span: Range<usize>,
        error: ParseError,
    },
}

/// `true` if `item` is a standalone `\.` psql directive — the inline-COPY-data
/// terminator psql consumes (and never echoes) at the end of a data block.
pub fn is_copy_data_terminator(item: &FileItem<'_>) -> bool {
    matches!(
        item,
        FileItem::Command(PsqlCommand::Directive(d)) if d.rest.0.trim() == "."
    )
}

/// Extract the leading meta-command name of a psql `\directive` from its
/// `rest.0` text. Returns the first whitespace-delimited token (already without
/// its leading `\`, since the `\` is consumed by the directive's `backslash`
/// field). Returns the empty string if `rest.0` is whitespace-only.
fn directive_head(rest: &str) -> &str {
    rest.split_whitespace().next().unwrap_or("")
}

/// `true` if `item` is a psql conditional-block OPEN directive: `\if`.
///
/// psql's `\if`/`\elif`/`\else`/`\endif` is a conditional block. A consumer
/// that tracks conditional nesting bumps a depth counter on every open and
/// decrements it on every close; midbranches do not change depth.
///
/// Matched case-insensitively because psql accepts `\IF` as well as `\if`.
pub fn is_psql_conditional_open(item: &FileItem<'_>) -> bool {
    matches!(
        item,
        FileItem::Command(PsqlCommand::Directive(d))
            if directive_head(&d.rest.0).eq_ignore_ascii_case("if")
    )
}

/// `true` if `item` is a psql conditional-block CLOSE directive: `\endif`.
///
/// Paired with [`is_psql_conditional_open`] — see its doc for the conditional
/// collapse rule. Matched case-insensitively (psql accepts `\ENDIF`).
pub fn is_psql_conditional_close(item: &FileItem<'_>) -> bool {
    matches!(
        item,
        FileItem::Command(PsqlCommand::Directive(d))
            if directive_head(&d.rest.0).eq_ignore_ascii_case("endif")
    )
}

/// `true` if `item` is a psql conditional MIDBRANCH directive: `\elif` or
/// `\else`. Midbranches are *inside* the conditional already, so the depth
/// counter must NOT change on them; they collapse into the outer `\if`'s
/// record alongside the rest of the conditional. Matched case-insensitively.
///
/// Exported for symmetry with [`is_psql_conditional_open`] /
/// [`is_psql_conditional_close`]: a depth-counter walk does not need to call
/// this — `\elif`/`\else` simply fall through (they're neither opens nor
/// closes, so depth stays put). It exists to document the complete
/// conditional taxonomy in one place, so a future call site that needs to
/// recognise midbranches (e.g. for per-branch attribution) has a single
/// source of truth alongside the open/close predicates.
pub fn is_psql_conditional_midbranch(item: &FileItem<'_>) -> bool {
    matches!(
        item,
        FileItem::Command(PsqlCommand::Directive(d)) if {
            let head = directive_head(&d.rest.0);
            head.eq_ignore_ascii_case("elif") || head.eq_ignore_ascii_case("else")
        }
    )
}

/// `true` if `item` is a psql `\quit` directive.
///
/// psql's `\quit` unconditionally exits the session: every input line after
/// it is never executed and never echoed. When `\quit` appears inside an
/// `\if`…`\endif` block, whether it fires depends on which branch psql
/// takes at runtime — the oracle walk cannot statically tell. The
/// `.out`-driven `align` walk decides: if the `.out` carries a `\endif`
/// echo line, pg_regress did NOT take the branch (psql ran the
/// conditional to completion) and the conditional record is kept; if no
/// `\endif` line appears, the branch fired (psql exited at `\quit` and
/// never echoed `\endif`) and the conditional record + everything past it
/// are dropped from the records and the echo stream.
///
/// Top-level `\quit` (not inside a conditional) ALWAYS truncates — psql
/// exits unconditionally there, regardless of the `.out` shape.
///
/// The 5 pg_regress fixtures that motivate this rule
/// (`regress_unicode`, `regress_euc_kr`, `regress_collate_utf8`,
/// `regress_collate_linux_utf8`, `regress_collate_icu_utf8`) carry a
/// `\quit` inside an `\if :skip_test` / `\if :{?icu_version}` guard
/// whose branch DOES fire in our locale=C container (SQL_ASCII
/// encoding, no ICU). Their `_1.out` alternate oracle ends at `\quit`
/// with no `\endif`, which is exactly the shape `align` keys on to
/// trigger the drop.
///
/// Matched case-insensitively because psql accepts `\QUIT` as well as
/// `\quit`.
pub fn is_psql_quit(item: &FileItem<'_>) -> bool {
    matches!(
        item,
        FileItem::Command(PsqlCommand::Directive(d))
            if directive_head(&d.rest.0).eq_ignore_ascii_case("quit")
    )
}

/// Parse a complete SQL file into a list of file items.
///
/// Gracefully handles parse errors and unparseable content (COPY FROM stdin
/// data blocks, etc.) by preserving them as `RawLines`.
///
/// A thin wrapper over [`parse_sql_file_with_spans`] that drops the per-item
/// byte spans.
pub fn parse_sql_file<'input>(
    input: &mut Input<'input>,
) -> Result<Vec<FileItem<'input>>, ParseError> {
    Ok(parse_sql_file_with_spans(input)?
        .into_iter()
        .map(|(item, _)| item)
        .collect())
}

/// Like [`parse_sql_file`], but pairs every [`FileItem`] with its byte span in
/// `input`'s source.
///
/// For `Command` items the span excludes leading ignored whitespace and
/// comments, and includes the statement and its terminator (trailing whitespace
/// the token scanner consumed is trimmed off). For `RawLines` items the span
/// covers the raw region from the first raw line of the run to the end of the
/// last raw line (including that line's trailing newline). Note the span may
/// also cover blank or comment lines that appear *between* raw lines within a
/// run; the joined `RawLines` content normalizes those, so slicing the source
/// by the span is not guaranteed to be byte-for-byte identical to the content
/// when a run straddles such inter-line gaps.
pub fn parse_sql_file_with_spans<'input>(
    input: &mut Input<'input>,
) -> Result<Vec<(FileItem<'input>, Range<usize>)>, ParseError> {
    let source = input.source();
    // Span boundaries are *byte offsets* into `source`. With the logos lexer
    // the cursor is a token index, so `input.byte_offset()` recovers the
    // source byte of the token under the cursor (or `source.len()` at EOF).
    // Trim trailing whitespace so a span covers exactly the statement + its
    // terminator, not the gap before the next token.
    let trim_end = |start: usize, end: usize| -> Range<usize> {
        let trimmed = source[start..end].trim_end();
        start..start + trimmed.len()
    };
    let mut items: Vec<(FileItem<'input>, Range<usize>)> = Vec::new();
    // Owned accumulator: raw lines are not contiguous in the source (the
    // lexer drops whitespace/comments between them), so we can't borrow a
    // slice. Stored as Cow::Owned on flush.
    let mut raw_buf = String::new();
    // Byte offset of the first raw line of the current accumulating run.
    let mut raw_start = 0usize;
    // Byte offset just past the last raw line of the current run.
    let mut raw_end = 0usize;
    loop {
        if input.is_empty() {
            break;
        }
        // Span start: the byte offset of the current token.
        let start = input.byte_offset();
        if !PsqlCommand::peek(input) {
            // Collect unparseable lines (e.g., COPY FROM stdin data blocks).
            // Walk raw lines from the previous line's end, not `byte_offset()`:
            // a blank line between two raw lines produces no tokens, so
            // `byte_offset()` would skip it.
            let line_start = if raw_buf.is_empty() {
                raw_start = start;
                start
            } else {
                raw_end
            };
            let (line, next) = take_line(input, line_start);
            raw_buf.push_str(line);
            raw_buf.push('\n');
            raw_end = next;
            continue;
        }
        // Flush any accumulated raw lines before the next command
        if !raw_buf.is_empty() {
            items.push((
                FileItem::RawLines(::std::borrow::Cow::Owned(std::mem::take(&mut raw_buf))),
                raw_start..raw_end,
            ));
        }
        match PsqlCommand::parse(input) {
            Ok(cmd) => {
                // After a `COPY ... FROM stdin`, the following lines are
                // inline data (not SQL) up to a `\.` terminator — capture
                // them as `RawLines` so they are not mis-parsed as
                // statements (their content is arbitrary).
                let copy_stdin = matches!(
                    &cmd,
                    PsqlCommand::Statement(ts) if copy_from_stdin(&ts.stmt)
                );
                let cmd_span = trim_end(start, input.byte_offset());
                items.push((FileItem::Command(cmd), cmd_span.clone()));
                if copy_stdin {
                    // The data block starts on the line *after* the
                    // `copy ... ;` line. `cmd_span.end` is just past the
                    // terminator; skip to the next line so a blank first
                    // data line is preserved.
                    let data_start = source[cmd_span.end..]
                        .find('\n')
                        .map_or(source.len(), |p| cmd_span.end + p + 1);
                    let (data, data_end) = take_copy_data(input, data_start);
                    if !data.is_empty() {
                        items.push((
                            FileItem::RawLines(::std::borrow::Cow::Owned(data)),
                            data_start..data_end,
                        ));
                    }
                }
            }
            Err(error) => {
                // Structured parse failed: byte-walk to the next top-level
                // `;`, advance the token cursor past every token in that
                // range, and record a [`FileItem::ParseError`] with the
                // failed span. Mirrors PG's own behaviour on a regression
                // `.sql` file that intentionally includes a malformed
                // statement — the rest of the file still parses.
                skip_failed_statement(input);
                // Consume the trailing `;` (if any) so the failed span
                // covers the whole statement-with-terminator, matching
                // the span shape of `Command` items. `?` propagates the
                // (otherwise unreachable, since `peek` returned true)
                // parse error instead of silently dropping it.
                if punct::Semi::peek(input) {
                    punct::Semi::parse(input)?;
                }
                let span = trim_end(start, input.byte_offset());
                items.push((
                    FileItem::ParseError {
                        span: span.clone(),
                        error,
                    },
                    span,
                ));
            }
        }
    }
    // Flush trailing raw lines
    if !raw_buf.is_empty() {
        items.push((
            FileItem::RawLines(::std::borrow::Cow::Owned(raw_buf)),
            raw_start..raw_end,
        ));
    }
    Ok(items)
}

/// `true` if `stmt` is a `COPY ... FROM stdin` — the form followed by an
/// inline data block. `COPY ... TO stdout` and file copies are not.
///
/// Structural check on the modelled `CopyStmt`: only the table form (with or
/// without the legacy `BINARY` prefix) is a candidate. The query form
/// `COPY (SELECT ...) TO ...` is TO-only and never reads from stdin. The
/// direction must be `FROM`, and the target must be the `STDIN` keyword (not
/// a quoted filename and not `STDOUT`).
fn copy_from_stdin(stmt: &Statement<'_>) -> bool {
    let Statement::Copy(copy) = stmt else {
        return false;
    };
    let table = match &copy.body {
        CopyBody::Table(t) => t,
        CopyBody::BinaryTable(b) => &b.inner,
        CopyBody::Query(_) => return false,
    };
    matches!(table.direction, CopyDirection::From(_))
        && matches!(table.target, CopyTarget::Stdin(_))
}

/// Consume an inline COPY data block starting at byte offset `data_start`:
/// every line up to and including the `\.` terminator (or end of input).
/// Returns the verbatim block (for `RawLines`) and the byte offset just past
/// it.
///
/// Walks the source line by line from an explicit byte position rather than
/// from `input.byte_offset()`. COPY data is byte-significant: a blank data
/// row produces no tokens, so a token-offset scan would silently drop it.
fn take_copy_data(input: &mut Input<'_>, data_start: usize) -> (String, usize) {
    let source_len = input.source().len();
    let mut pos = data_start;
    let mut buf = String::new();
    while pos < source_len {
        let (line, next) = take_line(input, pos);
        let is_end = line.trim() == r"\.";
        buf.push_str(line);
        buf.push('\n');
        pos = next;
        if is_end {
            break;
        }
    }
    (buf, pos)
}

/// Byte-walk past a failed statement, advancing the token cursor to the next
/// top-level `;` (or end of input).
///
/// Used by [`parse_sql_file_with_spans`] to recover from a structured parse
/// error: the statement that could not be modelled is skipped, the parser
/// records a [`FileItem::ParseError`] covering the failed span, and the loop
/// continues with the next statement.
///
/// Respects PG's lexical structure: nested parentheses, single-quoted
/// strings (with `''` escape), and dollar-quoted strings (`$tag$ … $tag$`).
/// All delimiters are ASCII so byte-based scanning is correct even on UTF-8
/// source — continuation bytes never have the high bit clear.
fn skip_failed_statement(input: &mut Input<'_>) {
    let scan_start = input.byte_offset();
    let bytes = &input.source().as_bytes()[scan_start..];
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut in_dollar_string = false;
    // ASCII dollar tag (`[a-zA-Z_]*` between `$…$`).
    let mut dollar_tag: &[u8] = &[];
    let mut i: usize = 0;
    let n = bytes.len();

    while i < n {
        let b = bytes[i];

        if in_dollar_string {
            if b == b'$' {
                // Look for `$<tag>$`.
                let end_len = 1 + dollar_tag.len() + 1;
                if i + end_len <= n
                    && bytes[i + 1..i + 1 + dollar_tag.len()] == *dollar_tag
                    && bytes[i + end_len - 1] == b'$'
                {
                    i += end_len;
                    in_dollar_string = false;
                    continue;
                }
            }
            i += 1;
        } else if in_string {
            if b == b'\'' {
                // `''` is an escaped quote inside a string.
                if i + 1 < n && bytes[i + 1] == b'\'' {
                    i += 2;
                } else {
                    in_string = false;
                    i += 1;
                }
            } else {
                i += 1;
            }
        } else if b == b'\'' {
            in_string = true;
            i += 1;
        } else if b == b'$' {
            // Dollar-quoted string: `$tag$...$tag$` or `$$...$$`.
            // Match `$([a-zA-Z_]*)$`.
            let tag_start = i + 1;
            let mut tag_end = tag_start;
            while tag_end < n && (bytes[tag_end].is_ascii_alphabetic() || bytes[tag_end] == b'_') {
                tag_end += 1;
            }
            if tag_end < n && bytes[tag_end] == b'$' {
                dollar_tag = &bytes[tag_start..tag_end];
                i = tag_end + 1;
                in_dollar_string = true;
            } else {
                i += 1;
            }
        } else if b == b'(' {
            depth += 1;
            i += 1;
        } else if b == b')' {
            depth -= 1;
            i += 1;
        } else if b == b';' && depth <= 0 {
            break;
        } else {
            i += 1;
        }
    }

    let stop = scan_start + i;
    // Advance the token cursor past every token starting before `stop`.
    while input
        .current_record()
        .is_some_and(|r| (r.start as usize) < stop)
    {
        input.advance();
    }
}

/// Take the source line beginning at byte offset `start` and advance the
/// token cursor past it.
///
/// Operates on the raw `&str` (psql data / meta-command lines are not lexable
/// SQL). Returns the line (newline excluded) and the byte offset of the next
/// line; the caller threads that offset back in as the next `start`. The
/// start is taken explicitly — not from `input.byte_offset()` — because a
/// blank line produces no tokens, so `byte_offset()` would jump past it to
/// the next token on a later line.
fn take_line<'a>(input: &mut Input<'a>, start: usize) -> (&'a str, usize) {
    let source = input.source();
    let (line, next_offset) = match source[start..].find('\n') {
        Some(pos) => (&source[start..start + pos], start + pos + 1),
        None => (&source[start..], source.len()),
    };
    // Skip every token whose span begins before the next line starts.
    while input
        .current_record()
        .is_some_and(|r| (r.start as usize) < next_offset)
    {
        input.advance();
    }
    (line, next_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `CREATE OPERATOR !=-` (create_operator.sql) — legitimate 3-char
    /// operator name, the `pg_lex` `!=--` post-process must NOT split this
    /// because the next byte after `!=-` is whitespace, not `-`.
    #[test]
    fn parse_bang_eq_minus_real_operator_name() {
        let mut input = crate::tokens::test_input("SELECT 10 !=- ;");
        let _items = parse_sql_file(&mut input).unwrap();
    }

    /// `COPY ... FROM stdin` is followed by an inline data block (arbitrary
    /// text up to `\.`); it must be captured as `RawLines`, not mis-parsed
    /// as statements, and the statement after `\.` must still parse.
    #[test]
    fn parse_copy_from_stdin_data_block() {
        let sql = "copy t from stdin csv;\n\
                   this is junk that would not parse\n\
                   1,a,1\n\
                   \\.\n\
                   select 1;\n";
        let mut input = crate::tokens::test_input(sql);
        let items = parse_sql_file(&mut input).unwrap();
        assert!(
            items.iter().any(|i| matches!(i, FileItem::RawLines(_))),
            "COPY data should be captured as RawLines"
        );
        // The COPY statement and the trailing SELECT both parse structurally.
        let stmts = items
            .iter()
            .filter(|i| matches!(i, FileItem::Command(PsqlCommand::Statement(_))))
            .count();
        assert_eq!(stmts, 2, "COPY and SELECT should both parse");
    }

    /// `parse_sql_file_with_spans` pairs each item with its byte span in the
    /// source; slicing the source by that span yields the verbatim statement
    /// text (terminator included, leading whitespace excluded).
    #[test]
    fn parse_sql_file_spans_cover_each_item() {
        let sql = "SELECT 1;\nSELECT 22;";
        let mut input = crate::tokens::test_input(sql);
        let items = parse_sql_file_with_spans(&mut input).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(&sql[items[0].1.clone()], "SELECT 1;");
        assert_eq!(&sql[items[1].1.clone()], "SELECT 22;");
    }

    /// `parse_sql_file_with_spans` must give `RawLines` items a span that
    /// covers exactly the raw content — not over-extended past trailing
    /// whitespace/comments that the loop's ignored-token skip swallows.
    /// Slicing the source by a `RawLines` span must yield exactly the
    /// `RawLines` `Cow<str>` content.
    #[test]
    fn parse_sql_file_spans_cover_raw_lines() {
        let sql = "copy t from stdin csv;\n1,a\n2,b\n\\.\nselect 1;\n";
        let mut input = crate::tokens::test_input(sql);
        let items = parse_sql_file_with_spans(&mut input).unwrap();
        let (raw_text, raw_span) = items
            .iter()
            .find_map(|(item, span)| match item {
                FileItem::RawLines(text) => Some((text, span.clone())),
                _ => None,
            })
            .expect("COPY data should be captured as RawLines");
        assert_eq!(
            &sql[raw_span],
            raw_text.as_ref(),
            "RawLines span must slice to exactly the RawLines content",
        );
        let (_, select_span) = items
            .iter()
            .rev()
            .find(|(item, _)| matches!(item, FileItem::Command(PsqlCommand::Statement(_))))
            .expect("trailing SELECT should parse as a structured command");
        assert_eq!(&sql[select_span.clone()], "select 1;");
    }

    /// A blank line inside a `COPY FROM stdin` data block must survive
    /// verbatim. COPY data is byte-significant, and blank lines produce no
    /// tokens — so a token-offset-based line scan would silently drop them.
    #[test]
    fn copy_data_preserves_blank_lines() {
        let sql = "copy t from stdin;\nA\n\nB\n\\.\nselect 1;\n";
        let mut input = crate::tokens::test_input(sql);
        let items = parse_sql_file_with_spans(&mut input).unwrap();
        let (raw_text, raw_span) = items
            .iter()
            .find_map(|(item, span)| match item {
                FileItem::RawLines(t) => Some((t, span.clone())),
                _ => None,
            })
            .expect("COPY data should be captured as RawLines");
        assert_eq!(
            raw_text.as_ref(),
            "A\n\nB\n\\.\n",
            "blank line inside COPY data must be preserved verbatim",
        );
        assert_eq!(
            &sql[raw_span],
            raw_text.as_ref(),
            "RawLines span must slice to exactly the RawLines content",
        );
    }

    /// `\;` (the psql batch separator) terminates a statement mid-line, so
    /// `SELECT 1\; SELECT 2\; SELECT 3;` is three separate statements.
    #[test]
    fn parse_backslash_semi_batch() {
        let mut input = crate::tokens::test_input("SELECT 1\\; SELECT 2\\; SELECT 3;");
        let items = parse_sql_file(&mut input).unwrap();
        let stmts = items
            .iter()
            .filter(|i| matches!(i, FileItem::Command(PsqlCommand::Statement(_))))
            .count();
        assert_eq!(stmts, 3, "expected 3 statements split on `\\;`");
    }

    /// Convert a byte offset into `(line, col)` (both 1-based).
    fn line_col(src: &str, byte_offset: usize) -> (usize, usize) {
        let cap = byte_offset.min(src.len());
        let prefix = &src[..cap];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
        let last_nl = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = src[last_nl..cap].chars().count() + 1;
        (line, col)
    }

    /// Like [`parse_fixture`] but also returns the leaked source `&'static
    /// str` and the per-item byte spans, so callers can slice the original
    /// corpus text by span — useful when classifying [`FileItem::ParseError`]
    /// items by leading keyword.
    fn parse_fixture_with_spans(
        name: &str,
    ) -> (
        &'static str,
        Vec<(FileItem<'static>, std::ops::Range<usize>)>,
    ) {
        let path = format!(
            "{}/vendor/postgres/src/test/regress/sql/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let sql = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{path}: cannot read fixture: {e}"));
        let sql: &'static str = Box::leak(sql.into_boxed_str());
        let mut input = crate::tokens::test_input(sql);
        let items = match parse_sql_file_with_spans(&mut input) {
            Ok(items) => items,
            Err(e) => {
                let span = e.span();
                let (line, col) = line_col(sql, span.start);
                let snippet_end = (span.start + 80).min(sql.len());
                let snippet = &sql[span.start..snippet_end];
                panic!(
                    "{path}:{line}:{col}: parse error: {e}\n  near: {}",
                    snippet.replace('\n', "\\n")
                );
            }
        };
        if !input.is_empty() {
            let cursor = input.byte_offset();
            let (line, col) = line_col(sql, cursor);
            let snippet_end = (cursor + 80).min(sql.len());
            let snippet = &sql[cursor..snippet_end];
            panic!(
                "{path}:{line}:{col}: leftover input after parse:\n  near: {}",
                snippet.replace('\n', "\\n")
            );
        }
        (sql, items)
    }

    /// Parse a SQL fixture file, panicking with `path:line:col: …` context on error.
    ///
    /// Parses the whole file; on any parse error or leftover input, computes the
    /// human-readable line/column of the offending byte and includes it in the
    /// panic message alongside a short snippet.
    fn parse_fixture(name: &str) -> Vec<FileItem<'static>> {
        let path = format!(
            "{}/vendor/postgres/src/test/regress/sql/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        let sql = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{path}: cannot read fixture: {e}"));
        // Leak so the returned Vec borrows 'static (test-only convenience).
        let sql: &'static str = Box::leak(sql.into_boxed_str());
        let mut input = crate::tokens::test_input(sql);
        let items = match parse_sql_file(&mut input) {
            Ok(items) => items,
            Err(e) => {
                let span = e.span();
                let (line, col) = line_col(sql, span.start);
                let snippet_end = (span.start + 80).min(sql.len());
                let snippet = &sql[span.start..snippet_end];
                panic!(
                    "{path}:{line}:{col}: parse error: {e}\n  near: {}",
                    snippet.replace('\n', "\\n")
                );
            }
        };
        if !input.is_empty() {
            // `cursor()` is a token index; `byte_offset()` is the source byte
            // of the leftover token — what `line_col`/slicing need.
            let cursor = input.byte_offset();
            let (line, col) = line_col(sql, cursor);
            let snippet_end = (cursor + 80).min(sql.len());
            let snippet = &sql[cursor..snippet_end];
            panic!(
                "{path}:{line}:{col}: leftover input after parse:\n  near: {}",
                snippet.replace('\n', "\\n")
            );
        }
        items
    }

    #[test]
    fn parse_multiple_commands() {
        let sql = "SELECT 1;\n\\pset null '(null)'\nSELECT 2;\n";
        let mut input = crate::tokens::test_input(sql);
        let items = parse_sql_file(&mut input).unwrap();
        assert_eq!(items.len(), 3);
        assert!(matches!(
            items[0],
            FileItem::Command(PsqlCommand::Statement(_))
        ));
        assert!(matches!(
            items[1],
            FileItem::Command(PsqlCommand::Directive(_))
        ));
        assert!(matches!(
            items[2],
            FileItem::Command(PsqlCommand::Statement(_))
        ));
    }

    #[test]
    fn parse_create_drop_sequence() {
        let sql = "CREATE TABLE t (f1 bool);\nDROP TABLE t;\n";
        let mut input = crate::tokens::test_input(sql);
        let commands = parse_sql_file(&mut input).unwrap();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn parse_boolean_sql_fixture() {
        let items = parse_fixture("boolean.sql");
        assert!(
            items.len() > 50,
            "expected >50 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_comments_sql_fixture() {
        let items = parse_fixture("comments.sql");
        assert!(items.len() > 3, "expected >3 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_sql_fixture() {
        let items = parse_fixture("select.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_union_sql_fixture() {
        let items = parse_fixture("union.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_subselect_sql_fixture() {
        let items = parse_fixture("subselect.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_case_sql_fixture() {
        let items = parse_fixture("case.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_delete_sql_fixture() {
        let items = parse_fixture("delete.sql");
        assert!(items.len() > 5, "expected >5 commands, got {}", items.len());
    }

    #[test]
    fn parse_with_sql_fixture() {
        let items = parse_fixture("with.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_select_having_sql_fixture() {
        let items = parse_fixture("select_having.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_implicit_sql_fixture() {
        let items = parse_fixture("select_implicit.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_distinct_sql_fixture() {
        let items = parse_fixture("select_distinct.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_into_sql_fixture() {
        let items = parse_fixture("select_into.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_prepared_xacts_sql_fixture() {
        let items = parse_fixture("prepared_xacts.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_namespace_sql_fixture() {
        let items = parse_fixture("namespace.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_btree_index_sql_fixture() {
        let items = parse_fixture("btree_index.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_hash_index_sql_fixture() {
        let items = parse_fixture("hash_index.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_update_sql_fixture() {
        let items = parse_fixture("update.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_transactions_sql_fixture() {
        let items = parse_fixture("transactions.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_aggregates_sql_fixture() {
        let items = parse_fixture("aggregates.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_arrays_sql_fixture() {
        let items = parse_fixture("arrays.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_join_sql_fixture() {
        let items = parse_fixture("join.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_limit_sql_fixture() {
        let items = parse_fixture("limit.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_returning_sql_fixture() {
        let items = parse_fixture("returning.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_truncate_sql_fixture() {
        let items = parse_fixture("truncate.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_alter_table_sql_fixture() {
        let items = parse_fixture("alter_table.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_table_sql_fixture() {
        let items = parse_fixture("create_table.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_insert_sql_fixture() {
        let items = parse_fixture("insert.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_typed_table_sql_fixture() {
        let items = parse_fixture("typed_table.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_vacuum_sql_fixture() {
        let items = parse_fixture("vacuum.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_drop_if_exists_sql_fixture() {
        let items = parse_fixture("drop_if_exists.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_operator_sql_fixture() {
        let items = parse_fixture("create_operator.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_drop_operator_sql_fixture() {
        let items = parse_fixture("drop_operator.sql");
        assert!(items.len() > 5, "expected >5 commands, got {}", items.len());
    }

    #[test]
    fn parse_alter_operator_sql_fixture() {
        let items = parse_fixture("alter_operator.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_geometry_sql_fixture() {
        let items = parse_fixture("geometry.sql");
        assert!(
            items.len() > 50,
            "expected >50 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_create_index_spgist_sql_fixture() {
        let items = parse_fixture("create_index_spgist.sql");
        assert!(
            items.len() > 10,
            "expected >10 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_partition_prune_sql_fixture() {
        let items = parse_fixture("partition_prune.sql");
        assert!(
            items.len() > 100,
            "expected >100 commands, got {}",
            items.len()
        );
    }

    #[test]
    fn parse_advisory_lock_sql_fixture() {
        let items = parse_fixture("advisory_lock.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_alter_generic_sql_fixture() {
        let items = parse_fixture("alter_generic.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_amutils_sql_fixture() {
        let items = parse_fixture("amutils.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_async_sql_fixture() {
        let items = parse_fixture("async.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_bit_sql_fixture() {
        let items = parse_fixture("bit.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_bitmapops_sql_fixture() {
        let items = parse_fixture("bitmapops.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_box_sql_fixture() {
        let items = parse_fixture("box.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_brin_sql_fixture() {
        let items = parse_fixture("brin.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_brin_bloom_sql_fixture() {
        let items = parse_fixture("brin_bloom.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_brin_multi_sql_fixture() {
        let items = parse_fixture("brin_multi.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_char_sql_fixture() {
        let items = parse_fixture("char.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_circle_sql_fixture() {
        let items = parse_fixture("circle.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_cluster_sql_fixture() {
        let items = parse_fixture("cluster.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_collate_sql_fixture() {
        let items = parse_fixture("collate.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_combocid_sql_fixture() {
        let items = parse_fixture("combocid.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_compression_sql_fixture() {
        let items = parse_fixture("compression.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_constraints_sql_fixture() {
        let items = parse_fixture("constraints.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_conversion_sql_fixture() {
        let items = parse_fixture("conversion.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_copy_sql_fixture() {
        let items = parse_fixture("copy.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_copy2_sql_fixture() {
        let items = parse_fixture("copy2.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_copydml_sql_fixture() {
        let items = parse_fixture("copydml.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_copyselect_sql_fixture() {
        let items = parse_fixture("copyselect.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_aggregate_sql_fixture() {
        let items = parse_fixture("create_aggregate.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_am_sql_fixture() {
        let items = parse_fixture("create_am.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_cast_sql_fixture() {
        let items = parse_fixture("create_cast.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_function_c_sql_fixture() {
        let items = parse_fixture("create_function_c.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_function_sql_sql_fixture() {
        let items = parse_fixture("create_function_sql.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_index_sql_fixture() {
        let items = parse_fixture("create_index.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_misc_sql_fixture() {
        let items = parse_fixture("create_misc.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_procedure_sql_fixture() {
        let items = parse_fixture("create_procedure.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_role_sql_fixture() {
        let items = parse_fixture("create_role.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_schema_sql_fixture() {
        let items = parse_fixture("create_schema.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_table_like_sql_fixture() {
        let items = parse_fixture("create_table_like.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_type_sql_fixture() {
        let items = parse_fixture("create_type.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_create_view_sql_fixture() {
        let items = parse_fixture("create_view.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_database_sql_fixture() {
        let items = parse_fixture("database.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_date_sql_fixture() {
        let items = parse_fixture("date.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_dbsize_sql_fixture() {
        let items = parse_fixture("dbsize.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_dependency_sql_fixture() {
        let items = parse_fixture("dependency.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_domain_sql_fixture() {
        let items = parse_fixture("domain.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_encoding_sql_fixture() {
        let items = parse_fixture("encoding.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_enum_sql_fixture() {
        let items = parse_fixture("enum.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_equivclass_sql_fixture() {
        let items = parse_fixture("equivclass.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_errors_sql_fixture() {
        let items = parse_fixture("errors.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_event_trigger_sql_fixture() {
        let items = parse_fixture("event_trigger.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_explain_sql_fixture() {
        let items = parse_fixture("explain.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_expressions_sql_fixture() {
        let items = parse_fixture("expressions.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_fast_default_sql_fixture() {
        let items = parse_fixture("fast_default.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_float4_sql_fixture() {
        let items = parse_fixture("float4.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_float8_sql_fixture() {
        let items = parse_fixture("float8.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_foreign_data_sql_fixture() {
        let items = parse_fixture("foreign_data.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_foreign_key_sql_fixture() {
        let items = parse_fixture("foreign_key.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_functional_deps_sql_fixture() {
        let items = parse_fixture("functional_deps.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_generated_sql_fixture() {
        let items = parse_fixture("generated.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_gin_sql_fixture() {
        let items = parse_fixture("gin.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_gist_sql_fixture() {
        let items = parse_fixture("gist.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_groupingsets_sql_fixture() {
        let items = parse_fixture("groupingsets.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_guc_sql_fixture() {
        let items = parse_fixture("guc.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_hash_func_sql_fixture() {
        let items = parse_fixture("hash_func.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_hash_part_sql_fixture() {
        let items = parse_fixture("hash_part.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_horology_sql_fixture() {
        let items = parse_fixture("horology.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_identity_sql_fixture() {
        let items = parse_fixture("identity.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_incremental_sort_sql_fixture() {
        let items = parse_fixture("incremental_sort.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_index_including_sql_fixture() {
        let items = parse_fixture("index_including.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_index_including_gist_sql_fixture() {
        let items = parse_fixture("index_including_gist.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_indexing_sql_fixture() {
        let items = parse_fixture("indexing.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_indirect_toast_sql_fixture() {
        let items = parse_fixture("indirect_toast.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_inet_sql_fixture() {
        let items = parse_fixture("inet.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_inherit_sql_fixture() {
        let items = parse_fixture("inherit.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_insert_conflict_sql_fixture() {
        let items = parse_fixture("insert_conflict.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_int2_sql_fixture() {
        let items = parse_fixture("int2.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_int4_sql_fixture() {
        let items = parse_fixture("int4.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_int8_sql_fixture() {
        let items = parse_fixture("int8.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_interval_sql_fixture() {
        let items = parse_fixture("interval.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_join_hash_sql_fixture() {
        let items = parse_fixture("join_hash.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_json_sql_fixture() {
        let items = parse_fixture("json.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_json_encoding_sql_fixture() {
        let items = parse_fixture("json_encoding.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_jsonb_sql_fixture() {
        let items = parse_fixture("jsonb.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_jsonb_jsonpath_sql_fixture() {
        let items = parse_fixture("jsonb_jsonpath.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_jsonpath_sql_fixture() {
        let items = parse_fixture("jsonpath.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_jsonpath_encoding_sql_fixture() {
        let items = parse_fixture("jsonpath_encoding.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_largeobject_sql_fixture() {
        let items = parse_fixture("largeobject.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_line_sql_fixture() {
        let items = parse_fixture("line.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_lock_sql_fixture() {
        let items = parse_fixture("lock.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_lseg_sql_fixture() {
        let items = parse_fixture("lseg.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_macaddr_sql_fixture() {
        let items = parse_fixture("macaddr.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_macaddr8_sql_fixture() {
        let items = parse_fixture("macaddr8.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_matview_sql_fixture() {
        let items = parse_fixture("matview.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_md5_sql_fixture() {
        let items = parse_fixture("md5.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_memoize_sql_fixture() {
        let items = parse_fixture("memoize.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_merge_sql_fixture() {
        let items = parse_fixture("merge.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_misc_sql_fixture() {
        let items = parse_fixture("misc.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_misc_functions_sql_fixture() {
        let items = parse_fixture("misc_functions.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_misc_sanity_sql_fixture() {
        let items = parse_fixture("misc_sanity.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_money_sql_fixture() {
        let items = parse_fixture("money.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_multirangetypes_sql_fixture() {
        let items = parse_fixture("multirangetypes.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_name_sql_fixture() {
        let items = parse_fixture("name.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_numeric_sql_fixture() {
        let items = parse_fixture("numeric.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_numeric_big_sql_fixture() {
        let items = parse_fixture("numeric_big.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_numerology_sql_fixture() {
        let items = parse_fixture("numerology.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_object_address_sql_fixture() {
        let items = parse_fixture("object_address.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_oid_sql_fixture() {
        let items = parse_fixture("oid.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_opr_sanity_sql_fixture() {
        let items = parse_fixture("opr_sanity.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_partition_aggregate_sql_fixture() {
        let items = parse_fixture("partition_aggregate.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_partition_info_sql_fixture() {
        let items = parse_fixture("partition_info.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_partition_join_sql_fixture() {
        let items = parse_fixture("partition_join.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_path_sql_fixture() {
        let items = parse_fixture("path.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_pg_lsn_sql_fixture() {
        let items = parse_fixture("pg_lsn.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_plancache_sql_fixture() {
        let items = parse_fixture("plancache.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_point_sql_fixture() {
        let items = parse_fixture("point.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_polygon_sql_fixture() {
        let items = parse_fixture("polygon.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_polymorphism_sql_fixture() {
        let items = parse_fixture("polymorphism.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_portals_sql_fixture() {
        let items = parse_fixture("portals.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_portals_p2_sql_fixture() {
        let items = parse_fixture("portals_p2.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_predicate_sql_fixture() {
        let items = parse_fixture("predicate.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_prepare_sql_fixture() {
        let items = parse_fixture("prepare.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_privileges_sql_fixture() {
        let items = parse_fixture("privileges.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    /// Every GRANT / REVOKE / ALTER DEFAULT PRIVILEGES statement in the
    /// listed corpora must parse through the structured AST path — never
    /// surface as a [`FileItem::ParseError`]. Regression guard for Batch 6:
    /// a statement that drops to a parse error is invisible to the formatter
    /// and the visitor, so the grammar gap goes unnoticed by the
    /// differential test.
    #[test]
    fn corpus_uses_structured_grant_revoke_adp() {
        // Files known to carry GRANT/REVOKE/ALTER DEFAULT PRIVILEGES. The
        // privileges/dependency/object_address/foreign_data corpora together
        // exercise every privilege_target form the corpus covers.
        // Every corpus file with a leading `GRANT`, `REVOKE`, or
        // `ALTER DEFAULT PRIVILEGES` line. Generated with:
        //   grep -lE "^(GRANT|REVOKE|ALTER DEFAULT PRIVILEGES) " \
        //     pg-sql/vendor/postgres/src/test/regress/sql/*.sql
        let corpora = [
            "alter_generic.sql",
            "cluster.sql",
            "copy2.sql",
            "create_function_sql.sql",
            "create_index.sql",
            "create_operator.sql",
            "create_procedure.sql",
            "create_role.sql",
            "dependency.sql",
            "event_trigger_login.sql",
            "foreign_data.sql",
            "generated.sql",
            "identity.sql",
            "init_privs.sql",
            "largeobject.sql",
            "lock.sql",
            "matview.sql",
            "merge.sql",
            "misc_functions.sql",
            "object_address.sql",
            "privileges.sql",
            "publication.sql",
            "rowsecurity.sql",
            "rules.sql",
            "select_into.sql",
            "select_views.sql",
            "sequence.sql",
            "stats_ext.sql",
            "subscription.sql",
            "tablespace.sql",
            "test_setup.sql",
            "updatable_views.sql",
            "update.sql",
        ];
        let mut raw_priv_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                if upper.starts_with("GRANT ")
                    || upper.starts_with("REVOKE ")
                    || upper.starts_with("ALTER DEFAULT PRIVILEGES")
                {
                    raw_priv_count += 1;
                    if examples.len() < 5 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(80)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_priv_count,
            0,
            "expected GRANT/REVOKE/ALTER DEFAULT PRIVILEGES to parse via the \
             structured AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_priv_count,
            examples.join("\n  "),
        );
    }

    /// Every PG-accepted `COPY` statement in the COPY corpora must parse
    /// through the structured AST path — never surface as a
    /// [`FileItem::ParseError`]. Regression guard for Batch 7: a statement
    /// that drops to a parse error is invisible to the formatter and the
    /// visitor, so the grammar gap goes unnoticed by the differential test.
    ///
    /// PG-rejected COPY forms (e.g. `COPY (query) FROM stdin` or a query
    /// form with a column list) are exempt — they are flagged in the corpus
    /// with `-- This should fail` and a parse error is the correct
    /// behaviour for them.
    #[test]
    fn corpus_uses_structured_copy() {
        // PG-rejected forms intentionally exercised by the corpus: query
        // form with `FROM` (only `TO` is allowed), and query form with a
        // column list (only the table form has one).
        let pg_rejected = |t: &str| {
            let upper = t.to_ascii_uppercase();
            // Trim trailing semicolon/comments before checking suffixes.
            // The ParseError span covers the statement *including* its
            // terminator, so strip any trailing `;` so the FROM-STDIN
            // suffix match still works.
            let head = upper
                .split_once("\n--")
                .map(|p| p.0)
                .unwrap_or(upper.as_str())
                .trim()
                .trim_end_matches(';')
                .trim_end();
            head.starts_with("COPY (") && (head.ends_with(") FROM STDIN") || head.contains(") ("))
        };
        let corpora = ["copy.sql", "copy2.sql", "copydml.sql", "copyselect.sql"];
        let mut raw_copy_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                if upper.starts_with("COPY ") && !pg_rejected(t) {
                    raw_copy_count += 1;
                    if examples.len() < 5 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(80)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_copy_count,
            0,
            "expected PG-accepted COPY statements to parse via the structured \
             AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_copy_count,
            examples.join("\n  "),
        );
    }

    /// Every `CREATE ROLE`, `CREATE USER`, `CREATE GROUP`, `CREATE SCHEMA`,
    /// `CREATE EXTENSION`, `CREATE DATABASE`, `CREATE ACCESS METHOD`, and
    /// `CREATE LANGUAGE` statement in the listed corpora must parse through
    /// the structured AST path — never surface as a [`FileItem::ParseError`].
    /// Regression guard for Batch 8: a statement that drops to a parse error
    /// is invisible to the formatter and the visitor, so the grammar gap goes
    /// unnoticed by the differential test.
    #[test]
    fn corpus_uses_structured_create_role_and_containers() {
        // Files known to carry CREATE ROLE/USER/GROUP/SCHEMA/EXTENSION/
        // DATABASE/ACCESS METHOD/LANGUAGE statements. Generated with:
        //   grep -lE "^CREATE (ROLE|USER|GROUP|SCHEMA|EXTENSION|DATABASE|\
        //     ACCESS METHOD|(OR REPLACE )?(TRUSTED )?(PROCEDURAL )?LANGUAGE) " \
        //     pg-sql/vendor/postgres/src/test/regress/sql/*.sql
        // Includes the ROLE/USER/GROUP-creating corpora and the schema/db/am
        // corpora that exercise the container forms.
        let corpora = [
            "alter_generic.sql",
            "create_am.sql",
            "create_misc.sql",
            "create_role.sql",
            "create_schema.sql",
            "database.sql",
            "dependency.sql",
            "event_trigger.sql",
            "password.sql",
            "privileges.sql",
            "publication.sql",
            "roleattributes.sql",
            "subscription.sql",
        ];
        let mut raw_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                // Exclude `CREATE USER MAPPING ...` (a foreign-data
                // statement, not the role-creation `CREATE USER`).
                let matches_target = upper.starts_with("CREATE ROLE ")
                    || (upper.starts_with("CREATE USER ")
                        && !upper.starts_with("CREATE USER MAPPING"))
                    || upper.starts_with("CREATE GROUP ")
                    || upper.starts_with("CREATE SCHEMA ")
                    || upper.starts_with("CREATE EXTENSION ")
                    || upper.starts_with("CREATE DATABASE ")
                    || upper.starts_with("CREATE ACCESS METHOD ")
                    || upper.starts_with("CREATE LANGUAGE ")
                    || upper.starts_with("CREATE TRUSTED LANGUAGE ")
                    || upper.starts_with("CREATE PROCEDURAL LANGUAGE ")
                    || upper.starts_with("CREATE TRUSTED PROCEDURAL LANGUAGE ")
                    || upper.starts_with("CREATE OR REPLACE LANGUAGE ")
                    || upper.starts_with("CREATE OR REPLACE TRUSTED LANGUAGE ")
                    || upper.starts_with("CREATE OR REPLACE PROCEDURAL LANGUAGE ")
                    || upper.starts_with("CREATE OR REPLACE TRUSTED PROCEDURAL LANGUAGE ");
                if matches_target {
                    raw_count += 1;
                    if examples.len() < 10 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(120)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_count,
            0,
            "expected CREATE roles/containers to parse via the structured \
             AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_count,
            examples.join("\n  "),
        );
    }

    /// Regression guard for Batch 9a: CREATE object statements (types,
    /// sequences, domains, casts, ...) must reach the structured AST rather
    /// than surfacing as a [`FileItem::ParseError`]. A parse error would
    /// silently break FormatTokens and Visit while still passing the
    /// differential test.
    #[test]
    fn corpus_uses_structured_create_objects() {
        // Corpora known to carry the Batch 9a statements.
        let corpora = [
            "collate.sql",
            "conversion.sql",
            "create_aggregate.sql",
            "create_cast.sql",
            "create_misc.sql",
            "create_type.sql",
            "domain.sql",
            "rangetypes.sql",
            "sequence.sql",
            "stats_ext.sql",
            "tsdicts.sql",
            "tsearch.sql",
        ];
        let mut raw_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                let matches_target = upper.starts_with("CREATE CAST ")
                    || upper.starts_with("CREATE COLLATION ")
                    || upper.starts_with("CREATE CONVERSION ")
                    || upper.starts_with("CREATE DEFAULT CONVERSION ")
                    || upper.starts_with("CREATE DOMAIN ")
                    || upper.starts_with("CREATE SEQUENCE ")
                    || upper.starts_with("CREATE TEMP SEQUENCE ")
                    || upper.starts_with("CREATE TEMPORARY SEQUENCE ")
                    || upper.starts_with("CREATE UNLOGGED SEQUENCE ")
                    || upper.starts_with("CREATE TYPE ")
                    || upper.starts_with("CREATE AGGREGATE ")
                    || upper.starts_with("CREATE OR REPLACE AGGREGATE ")
                    || upper.starts_with("CREATE STATISTICS ")
                    || upper.starts_with("CREATE TEXT SEARCH ");
                // Known PG-rejected forms in the corpus. These statements
                // are intentionally invalid syntax (they test PG's error
                // path), so a parse error is correct: the differential
                // test passes when both engines reject.
                let is_pg_rejected_error_fixture = upper
                    .starts_with("CREATE STATISTICS TST ON Y + Z")
                    || upper.starts_with("CREATE STATISTICS TST ON (X, Y)");
                if matches_target && !is_pg_rejected_error_fixture {
                    raw_count += 1;
                    if examples.len() < 10 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(120)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_count,
            0,
            "expected CREATE object statements to parse via the structured \
             AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_count,
            examples.join("\n  "),
        );
    }

    /// Batch 9c: confirm CREATE POLICY / PUBLICATION / SUBSCRIPTION /
    /// FOREIGN / SERVER statements parse via their structured AST and do not
    /// surface as a [`FileItem::ParseError`]. Keeps FormatTokens and Visit
    /// honest — a parse error would still pass the differential test (both
    /// engines reject the same byte-faithful text).
    ///
    /// PG-rejected forms in the corpus (statements that intentionally
    /// exercise PG's error path) are excluded: they are syntactically
    /// invalid, so a parse error is correct because both engines reject
    /// them.
    #[test]
    fn corpus_uses_structured_create_policies_publications_etc() {
        let corpora = [
            "rowsecurity.sql",
            "publication.sql",
            "subscription.sql",
            "foreign_data.sql",
        ];
        let mut raw_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                let matches_target = upper.starts_with("CREATE POLICY ")
                    || upper.starts_with("CREATE PUBLICATION ")
                    || upper.starts_with("CREATE SUBSCRIPTION ")
                    || upper.starts_with("CREATE FOREIGN DATA WRAPPER ")
                    || upper.starts_with("CREATE FOREIGN TABLE ")
                    || upper.starts_with("CREATE SERVER ");
                // PG-rejected error fixtures: PG's parser rejects these
                // syntactically (no production matches), so structured
                // parsing fails — both engines reject and the differential
                // test passes.
                let is_pg_rejected_error_fixture =
                    // CREATE PUBLICATION ... FOR TABLES IN SCHEMA name WHERE (...) —
                    // gram.y rejects WHERE on the schema form.
                    (upper.starts_with("CREATE PUBLICATION ")
                        && upper.contains("TABLES IN SCHEMA")
                        && upper.contains(" WHERE "))
                    // CREATE SUBSCRIPTION requires both CONNECTION and PUBLICATION.
                    || (upper.starts_with("CREATE SUBSCRIPTION ")
                        && (!upper.contains(" CONNECTION ") || !upper.contains(" PUBLICATION ")))
                    // CREATE FOREIGN TABLE with empty columns and no SERVER —
                    // a sigma-shaped fixture testing PG's error path.
                    || upper.starts_with("CREATE FOREIGN TABLE FT1 ()");
                if matches_target && !is_pg_rejected_error_fixture {
                    raw_count += 1;
                    if examples.len() < 10 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(160)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_count,
            0,
            "expected Batch 9c statements to parse via the structured \
             AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_count,
            examples.join("\n  "),
        );
    }

    #[test]
    fn parse_publication_sql_fixture() {
        let items = parse_fixture("publication.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_random_sql_fixture() {
        let items = parse_fixture("random.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_rangefuncs_sql_fixture() {
        let items = parse_fixture("rangefuncs.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_rangetypes_sql_fixture() {
        let items = parse_fixture("rangetypes.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_regex_sql_fixture() {
        let items = parse_fixture("regex.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_regproc_sql_fixture() {
        let items = parse_fixture("regproc.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_reloptions_sql_fixture() {
        let items = parse_fixture("reloptions.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_replica_identity_sql_fixture() {
        let items = parse_fixture("replica_identity.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_roleattributes_sql_fixture() {
        let items = parse_fixture("roleattributes.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_rowsecurity_sql_fixture() {
        let items = parse_fixture("rowsecurity.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_rowtypes_sql_fixture() {
        let items = parse_fixture("rowtypes.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_rules_sql_fixture() {
        let items = parse_fixture("rules.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_sanity_check_sql_fixture() {
        let items = parse_fixture("sanity_check.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_security_label_sql_fixture() {
        let items = parse_fixture("security_label.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_distinct_on_sql_fixture() {
        let items = parse_fixture("select_distinct_on.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_parallel_sql_fixture() {
        let items = parse_fixture("select_parallel.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_select_views_sql_fixture() {
        let items = parse_fixture("select_views.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_sequence_sql_fixture() {
        let items = parse_fixture("sequence.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_spgist_sql_fixture() {
        let items = parse_fixture("spgist.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_sqljson_sql_fixture() {
        let items = parse_fixture("sqljson.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_sqljson_jsontable_sql_fixture() {
        let items = parse_fixture("sqljson_jsontable.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_sqljson_queryfuncs_sql_fixture() {
        let items = parse_fixture("sqljson_queryfuncs.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_strings_sql_fixture() {
        let items = parse_fixture("strings.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_subscription_sql_fixture() {
        let items = parse_fixture("subscription.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tablesample_sql_fixture() {
        let items = parse_fixture("tablesample.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tablespace_sql_fixture() {
        let items = parse_fixture("tablespace.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_temp_sql_fixture() {
        let items = parse_fixture("temp.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_text_sql_fixture() {
        let items = parse_fixture("text.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tid_sql_fixture() {
        let items = parse_fixture("tid.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tidscan_sql_fixture() {
        let items = parse_fixture("tidscan.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tidrangescan_sql_fixture() {
        let items = parse_fixture("tidrangescan.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_time_sql_fixture() {
        let items = parse_fixture("time.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_timestamp_sql_fixture() {
        let items = parse_fixture("timestamp.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_timestamptz_sql_fixture() {
        let items = parse_fixture("timestamptz.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_timetz_sql_fixture() {
        let items = parse_fixture("timetz.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_triggers_sql_fixture() {
        let items = parse_fixture("triggers.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tsdicts_sql_fixture() {
        let items = parse_fixture("tsdicts.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tsearch_sql_fixture() {
        let items = parse_fixture("tsearch.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tsrf_sql_fixture() {
        let items = parse_fixture("tsrf.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tstypes_sql_fixture() {
        let items = parse_fixture("tstypes.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_tuplesort_sql_fixture() {
        let items = parse_fixture("tuplesort.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_type_sanity_sql_fixture() {
        let items = parse_fixture("type_sanity.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_unicode_sql_fixture() {
        let items = parse_fixture("unicode.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_updatable_views_sql_fixture() {
        let items = parse_fixture("updatable_views.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_uuid_sql_fixture() {
        let items = parse_fixture("uuid.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_varchar_sql_fixture() {
        let items = parse_fixture("varchar.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_vacuum_parallel_sql_fixture() {
        let items = parse_fixture("vacuum_parallel.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_window_sql_fixture() {
        let items = parse_fixture("window.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_xid_sql_fixture() {
        let items = parse_fixture("xid.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_xml_sql_fixture() {
        let items = parse_fixture("xml.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_xmlmap_sql_fixture() {
        let items = parse_fixture("xmlmap.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_collate_icu_utf8_sql_fixture() {
        let items = parse_fixture("collate.icu.utf8.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_collate_linux_utf8_sql_fixture() {
        let items = parse_fixture("collate.linux.utf8.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_collate_utf8_sql_fixture() {
        let items = parse_fixture("collate.utf8.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    // collate.windows.win1252.sql and euc_kr.sql are non-UTF-8 encoded files

    #[test]
    fn parse_event_trigger_login_sql_fixture() {
        let items = parse_fixture("event_trigger_login.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_infinite_recurse_sql_fixture() {
        let items = parse_fixture("infinite_recurse.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_init_privs_sql_fixture() {
        let items = parse_fixture("init_privs.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_maintain_every_sql_fixture() {
        let items = parse_fixture("maintain_every.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_mvcc_sql_fixture() {
        let items = parse_fixture("mvcc.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_oidjoins_sql_fixture() {
        let items = parse_fixture("oidjoins.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_password_sql_fixture() {
        let items = parse_fixture("password.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_plpgsql_sql_fixture() {
        let items = parse_fixture("plpgsql.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_psql_sql_fixture() {
        let items = parse_fixture("psql.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_psql_crosstab_sql_fixture() {
        let items = parse_fixture("psql_crosstab.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_reindex_catalog_sql_fixture() {
        let items = parse_fixture("reindex_catalog.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_sysviews_sql_fixture() {
        let items = parse_fixture("sysviews.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_test_setup_sql_fixture() {
        let items = parse_fixture("test_setup.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_txid_sql_fixture() {
        let items = parse_fixture("txid.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    #[test]
    fn parse_write_parallel_sql_fixture() {
        let items = parse_fixture("write_parallel.sql");
        assert!(!items.is_empty(), "expected >0 commands, got {}", items.len());
    }

    /// Regression guard for Batches 13 and 15: every PG-accepted
    /// `CREATE OPERATOR CLASS`, `CREATE OPERATOR FAMILY`,
    /// `ALTER OPERATOR CLASS`, `ALTER OPERATOR FAMILY`,
    /// `DROP OPERATOR CLASS`, and `DROP OPERATOR FAMILY` statement in the
    /// listed corpora must parse via the structured AST and not surface as
    /// a [`FileItem::ParseError`].
    ///
    /// Batch 13 modelled `AlterOperatorFamily` (ADD/DROP/RENAME/OWNER/SET
    /// SCHEMA per `AlterOpFamilyStmt`). Batch 15 added
    /// `AlterOperatorClass` covering its corpus-exercised
    /// RENAME/OWNER/SET-SCHEMA arms — CLASS has no ADD/DROP form in
    /// gram.y.
    #[test]
    fn corpus_uses_structured_operator_classes_and_families() {
        // Every corpus file with a `CREATE/ALTER/DROP OPERATOR CLASS/FAMILY`
        // statement at line start. Generated with:
        //   grep -ilE "^[[:space:]]*(create|alter|drop) operator (class|family)" \
        //     pg-sql/vendor/postgres/src/test/regress/sql/*.sql
        let corpora = [
            "alter_generic.sql",
            "alter_table.sql",
            "create_am.sql",
            "create_table.sql",
            "dependency.sql",
            "drop_if_exists.sql",
            "equivclass.sql",
            "event_trigger.sql",
            "expressions.sql",
            "partition_prune.sql",
            "select_parallel.sql",
            "test_setup.sql",
            "update.sql",
        ];
        let mut raw_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                let matches_target = upper.starts_with("CREATE OPERATOR CLASS ")
                    || upper.starts_with("CREATE OPERATOR FAMILY ")
                    || upper.starts_with("ALTER OPERATOR CLASS ")
                    || upper.starts_with("ALTER OPERATOR FAMILY ")
                    || upper.starts_with("DROP OPERATOR CLASS ")
                    || upper.starts_with("DROP OPERATOR FAMILY ");
                if matches_target {
                    raw_count += 1;
                    if examples.len() < 10 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(160)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_count,
            0,
            "expected CREATE/ALTER/DROP OPERATOR CLASS/FAMILY to parse via the \
             structured AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_count,
            examples.join("\n  "),
        );
    }

    /// Regression guard for Batch 15: every PG-accepted `CREATE USER
    /// MAPPING`, `ALTER USER MAPPING`, and `DROP USER MAPPING` statement
    /// in `foreign_data.sql` must parse via the structured AST and not
    /// surface as a [`FileItem::ParseError`].
    #[test]
    fn corpus_uses_structured_user_mapping() {
        let corpora = [
            "foreign_data.sql",
            "object_address.sql",
            "event_trigger.sql",
        ];
        let mut raw_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                let matches_target = upper.starts_with("CREATE USER MAPPING ")
                    || upper.starts_with("ALTER USER MAPPING ")
                    || upper.starts_with("DROP USER MAPPING ");
                if matches_target {
                    raw_count += 1;
                    if examples.len() < 10 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(160)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_count,
            0,
            "expected CREATE/ALTER/DROP USER MAPPING to parse via the \
             structured AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_count,
            examples.join("\n  "),
        );
    }

    /// Batch 14 oracle: every `ALTER FOREIGN TABLE …` and
    /// `ALTER FOREIGN DATA WRAPPER …` statement in the listed corpora must
    /// parse via the structured AST, not surface as a
    /// [`FileItem::ParseError`].
    ///
    /// The exception list captures PG-rejected error fixtures — corpus
    /// statements PG's *parser* itself rejects (no production matches), so
    /// our parser also bails and the differential test passes via the
    /// rejected-on-both-sides path.
    #[test]
    fn corpus_uses_structured_alter_foreign() {
        let corpora = ["foreign_data.sql", "fast_default.sql"];
        let mut raw_count = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for corpus in corpora {
            let (sql, items) = parse_fixture_with_spans(corpus);
            for (item, span) in &items {
                if !matches!(item, FileItem::ParseError { .. }) {
                    continue;
                }
                let t = sql[span.clone()].trim_start();
                let upper = t.to_ascii_uppercase();
                let matches_target = upper.starts_with("ALTER FOREIGN TABLE ")
                    || upper.starts_with("ALTER FOREIGN DATA WRAPPER ");
                if matches_target {
                    raw_count += 1;
                    if examples.len() < 10 {
                        examples.push(format!(
                            "{corpus}: {}",
                            t[..t.len().min(160)].replace('\n', "\\n")
                        ));
                    }
                }
            }
        }
        assert_eq!(
            raw_count,
            0,
            "expected ALTER FOREIGN TABLE / DATA WRAPPER to parse via the \
             structured AST; {} surfaced as FileItem::ParseError. First examples:\n  {}",
            raw_count,
            examples.join("\n  "),
        );
    }
}
