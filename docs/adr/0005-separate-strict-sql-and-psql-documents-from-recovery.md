# Separate strict SQL and psql documents from recovery

Status: accepted

`pg-sql` will expose separate high-level entry points for PostgreSQL SQL and
psql source documents. Both entry points are strict: success means the complete
input belongs to the requested language and produced a complete authored
document. Invalid input is represented only by a separate source-covering
recovery projection, consistent with Recursa ADR 0002.

## Entry points and languages

`parse_sql(source)` parses PostgreSQL 17.9 SQL documents with the server
language accepted by `RAW_PARSE_DEFAULT`: zero or more semicolon-separated
statements, an optional final semicolon, and no psql-only syntax. Empty
statements remain source/provenance occurrences but do not enter the semantic
statement list. `COPY ... FROM STDIN` is an ordinary SQL statement here; its
following client payload and `\.` marker are not SQL.

`parse_psql(source)` parses a PostgreSQL 17.9 psql source document containing
PostgreSQL SQL. It adds query-buffer operations, known backslash commands,
interpolation, send commands, conditional structure, and COPY payload regions.
It models psql source and scanner behavior but does not execute variables,
conditions, includes, shell commands, connection changes, `\quit`, or server
responses. Operations that could generate or load more input remain explicit
source operations and are not followed recursively.

The psql document represents server submissions explicitly, including their
query-buffer fragments and sending operation. Submissions containing
interpolation remain valid psql syntax even when the resulting server SQL
cannot be parsed before expansion. A derived server payload carries a source
map and can therefore represent non-contiguous transformations.

## psql source behavior

- A top-level SQL semicolon and `\g`, `\gx`, `\gset`, `\gexec`, and
  `\crosstabview` submit the current query buffer according to psql behavior.
  Send commands retain their complete source-backed argument regions.
- `\;` is not a submission boundary. It contributes a semicolon to the query
  buffer and maps the derived byte back to its two-byte source spelling.
- Noninteractive EOF submits a nonempty query buffer and emits nothing for an
  empty buffer.
- Directive boundaries follow the PostgreSQL psql scanner, including chained
  commands, rather than blindly consuming the remainder of every physical
  line. Command heads unknown to PostgreSQL 17.9 are rejected. Arguments may
  remain exact source-backed regions until command-specific semantics are
  required.
- `\if`, `\elif`, `\else`, and `\endif` nesting is validated without
  evaluating conditions or hiding inactive branches. Conditional grouping may
  be exposed as a derived view while physical source operations remain ordered.
- Interpolation forms such as `:name`, `:'name'`, `:"name"`, and `:{?name}`
  are structural psql nodes and are preserved rather than expanded.

The PostgreSQL regression corpus requires `parse_psql` for segmentation. The
PostgreSQL raw parser remains the oracle only for server payloads derived from
psql submissions, never for complete psql source.

## Strict construction and recovery

The public result has these conceptual states:

```text
Result<ParsedDocument, FileParseError>

FileParseError
  = Recovered(RecoveryDocument)
  | Fatal(FatalFileFailure)
```

`Ok` contains a fully successful typed `SqlDocument` or `PsqlDocument` and no
failure islands. Strict parsing is the only path that constructs authored AST
values.

`Err::Recovered` contains the complete source, a nonempty diagnostic sequence,
and a grammar-erased recovery projection. It may identify recognized later
statement or psql regions and expose their schemas and exact spans, but it
cannot contain or convert into `Statement`, `Parsed<T>`, or another authored
document. Fatal failure is reserved for invalid spans, grammar/driver mismatch,
or a violated plan, partition, or progress invariant.

This intentionally removes the legacy `FileItem::ParseError` AST variant.
General `RawLines` and handwritten `RestOfLine` are also removed: they were
temporary grammar-development escape hatches. Valid non-SQL source has a named
psql directive, interpolation, or COPY-payload representation; other input is
a failure in the recovery projection rather than an unparsed-leftovers bucket.

## Segmentation and source ownership

Recursa supplies one construction-free recovery and document-framing kernel
shared with Explorer. It lexes once, retains classified trivia and rejected
ranges, attempts strict parsing transactionally, and restarts recovery from the
candidate's original cursor. Synchronization reuses frozen dispatch, FIRST,
FOLLOW, container, failure, Pratt, terminator, and lexical-region products. It
does not rescan bytes, call authored code, continue from a partially advanced
strict cursor, or guess an ambiguous branch.

Every recovery step consumes input, permanently closes parse state, pops a
frame, or reaches EOF. If no safe next boundary can be proven, the failure
region extends to EOF. Lexical regions such as quoted identifiers, strings,
dollar strings, and nested comments remain atomic during synchronization.

Every document or recovery projection covers its complete valid UTF-8 source
exactly once. Meaningful items expose a content span, while ordered extents and
interstitial gaps form a checked, non-overlapping partition. Inter-item trivia
attaches to the right item; EOF trivia belongs to the document root. Empty,
trivia-only, and comment-only documents are valid and source-preserving.

## COPY regions

In `parse_psql`, a successfully structured table or binary-table
`COPY ... FROM STDIN` enters a deterministic client-payload mode. A structured
`\copy ... FROM STDIN` command uses the same mechanism. This is static source
segmentation, not a prediction that a server would actually enter COPY-IN.
Failed COPY syntax does not guess payload mode.

The payload begins on the following physical line and is preserved exactly.
Only psql's exact `\.\n` or `\.\r\n` control line terminates it. The region
exposes separate data and terminator spans, while its owning extent includes
both. Whitespace-surrounded markers and a final `\.` without a newline remain
data. A missing marker preserves the suffix to EOF and produces a recoverable
diagnostic.

## Rendering and oracle extraction

Successful documents support exact-source rendering and transactional
formatting. SQL regions are Pretty-rendered; psql directives, interpolation
spellings, COPY data, and control regions are copied exactly. The candidate is
re-segmented and reparsed, and submission equivalence is checked before output
is published. A recovery projection supports exact-source rendering only;
Pretty is unavailable.

The regression oracle adapter reads derived server submissions from a
successful or grammar-erased recovered psql projection, removes client send
syntax, interprets `\;` as a server semicolon, preserves source mappings, and
skips submissions requiring unresolved interpolation or execution state. It
sends only server payloads to PostgreSQL's raw parser.

## Acceptance

Permanent tests cover both entry points, strict-success and recovered-error
typing, exact source partitioning, stable diagnostic ordering, fail-closed
ambiguity, hard recovery progress, nested lexical regions, every psql send
form, `\;`, chained directives, conditionals, EOF submission, interpolation,
server and client COPY payloads, CRLF and missing-final-newline cases, exact and
formatted rendering, composed source maps, and unchanged corpus extraction
counts. PostgreSQL raw-parser equality applies only to derived successful server
payloads; pinned psql source behavior and file goldens govern client syntax and
recovery boundaries.
