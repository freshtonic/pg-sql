# Use declarative lexing and classified trivia

Status: accepted

The PostgreSQL parser will express its lexical and trivia rules through focused,
closed Recursa mechanisms. PostgreSQL policy remains in `pg-sql`; Recursa owns
the reusable matching, provenance, validation, and rendering machinery.

## Lexical contract

- `pg-sql` selects Recursa's ASCII-insensitive text-equivalence mode for
  keywords. Recursa continues to support both exact and ASCII-insensitive
  matching; it does not make Unicode case folding or normalization intrinsic.
- `pg-sql` selects generated frozen FIRST-k dispatch with `max_lookahead = 5`.
  This is a consumer setting, not an intrinsic Recursa limit.
- Keyword metadata has one category and zero or more orthogonal flags. Named
  token admissions use an acyclic union-and-subtraction algebra compiled to
  canonical bitsets. Duplicate union membership warns; an exclusion that
  removes nothing is an error.
- Nominal admission targets and content matchers are declared centrally with
  the token grammar. Every emitted content matcher generates an exact,
  source-backed parse type.
- Dollar strings, nested block comments, operator runs, and next-character
  boundaries use closed generated matchers. Fixed-operator classification is
  inferred from fixed-token declarations. Matchers must make monotonic progress
  with linear work; checked nesting depth is permitted, arbitrary fuel is not.
- Lexical arbitration uses longest final byte extent, then exact fixed spelling
  over content, then explicit priority. Declaration order is not a tie-breaker;
  unresolved ambiguity is a build error.
- Authored lexer callbacks, post-lex hooks, and parser postconditions are
  removed without a deprecation period. Existing syntax fails explicitly.
- Lexical failures use stable mechanism-specific diagnostic codes, an exact
  error region, and a primary anchor. Unterminated block comments and dollar
  strings span from their opener to EOF and anchor at the opener.
- `LexBuilder` remains a supported alternative-lexer adapter. It validates
  token and trivia spans, ordering, kinds, UTF-8 boundaries, and non-overlap;
  it does not run callbacks or infer trivia by rescanning gaps.

## PostgreSQL-specific declarations

- `WindowRefName` is `ColId - { ROWS, RANGE, GROUPS }`.
- `PsqlVariableName` is `AllWordKinds - { NULL, TRUE, FALSE }` and psql
  variables remain structural compositions of `Colon` and ordinary tokens.
- PostgreSQL custom operators use a closed `operator_run` matcher whose trivia
  fences and suffix rules are declared by `pg-sql`.

## Trivia and formatting contract

`pg-sql` opts into retained `Whitespace`, `LineComment`, and `BlockComment`
trivia kinds. The parser still consumes only significant tokens. Recursa stores
trivia as exact, classified records in immutable token-to-token gaps, including
BOF and EOF gaps, and exposes a read-only view without adding trivia fields to
the semantic AST.

Named gap predicates are restricted regular languages over ordered trivia kinds
and properties such as newline presence. They are compiled data, may guard
repetition or optional admission, cannot consume input, and cannot select
between grammar alternatives.

Each gap has one deterministic attachment view: it belongs to the finest
right-hand occurrence anchor, while EOF trivia belongs to the parsed root. A
comment before a comma is therefore emitted before and owned by the comma
anchor. Every retained comment is emitted exactly once and never crosses its
anchor.

Provenance-aware Pretty preserves exact comment text and source order while
canonically regenerating ordinary whitespace with LF line endings. Line
comments force a hard line. A layout conflict is a structured error rather
than permission to move, duplicate, or discard trivia. Comment-preserving
Pretty requires parsed provenance; detached and synthetic ASTs format
canonically without comments, and `into_ast()` is an explicit lossy boundary.
Shape-changing transformations must reject incompatible provenance until a
separately designed typed editing interface exists.

Leading, trailing, and comment-only input is retained through BOF and EOF
gaps. Raw regions, COPY payloads, parse-error islands, and their file-level
ownership remain part of the separate file-recovery design.

## Adjacent string literals

The PostgreSQL 17.9 scanner is authoritative. String fragments remain separate
source-backed AST occurrences and concatenate only across the scanner's
newline-bearing continuation language:

- a newline is required, so same-line `'a' 'b'` is rejected;
- a line comment followed by its newline can participate in continuation;
- a block comment does not itself qualify as continuation whitespace;
- a block comment following the only newline prevents continuation unless a
  later newline satisfies the scanner rule.

Legacy tests and migration fixtures that state the reverse are corrected rather
than preserved.

## Acceptance

Permanent fixtures cover token and trivia classification, ownership around
separators and optional nodes, adjacent strings, leading and trailing comments,
comment-only input, wide and narrow formatting, structured layout conflicts,
exact comment source maps, reparsing, semantic equivalence, and byte-stable
parse-format-format output. Unsupported formatter settings such as leading
commas or lowercase keywords are rejected explicitly and tracked separately.
