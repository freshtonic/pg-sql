# pg-sql Domain

`pg-sql` is a PostgreSQL parser built with Recursa. Each build targets one
PostgreSQL version, PostgreSQL 17 by default. The port preserves language coverage
and semantic information without promising compatibility with the legacy Rust
API.

## Canonical terms

- **Target version**: the one PostgreSQL major version whose raw parser a
  pg-sql build reproduces, and whose psql client language a pg-psql build
  reproduces. It is pinned to one exact release: the latest minor release of
  that major, or a named commit for a pre-release major. A build has exactly
  one target version, and the psql document and the SQL in it always come
  from that same release.
- **Version feature**: the Cargo feature that selects the target version:
  `pg14`, `pg15`, `pg16`, `pg17`, `pg18` or `pg19-beta`. They are mutually
  exclusive, and `pg17` is the default. `pg19-beta` becomes `pg19` when
  PostgreSQL 19.0 is tagged.
- **Version parity**: a build accepts exactly what its target version's raw
  parser (`gram.y` and `scan.l`) accepts, and rejects exactly what it
  rejects. Changes that only execution, the catalog or parse analysis enforce
  are not part of it. Neither is the `standard_conforming_strings = off`
  lexing mode: every target version lexes as if the setting is on. For a
  pg-psql build, version parity covers only the psql language that pg-psql
  models: `psqlscan.l` lexing and the send commands. Meta-commands that
  pg-psql keeps as unparsed text have no version gate.
- **Version gate**: a statement that one grammar element (a statement,
  clause, field, lexer rule, keyword or keyword category) exists only from a
  target version ("added in N"), or only before one ("removed in N"). Every
  version gate cites the research entry or the `gram.y`/`scan.l` change that
  justifies it.
- **PostgreSQL statement**: one semantically typed statement accepted by the
  supported PostgreSQL grammar.
- **SQL file item**: one statement, raw or COPY payload region, or the first
  strict rejection encountered while processing a SQL file.
- **psql document**: one client-side source document in psql's language --
  runs of SQL text, variable interpolations, and query-buffer terminators.
  It belongs to the `pg-psql` crate, never to this grammar: `gram.y` has no
  psql construct (ADR 0007).
- **Substitution**: the `pg-psql` step that turns a psql document into
  server SQL text plus a source map. It runs before any SQL parse, exactly
  as psql runs before the server.
- **PostgreSQL oracle**: the target version's authoritative raw-parser result.
- **Differential baseline**: the pinned corpus membership and outcome counts
  against the PostgreSQL oracle.
- **Grammar migration**: the reproducible transformation from the immutable
  legacy grammar into current Recursa declarations.
- **Parity gate**: geomean of per-benchmark pg-sql/sqlparser medians at or
  below 1.0 on the statement-level benchmark (ADR 0006).
- **Parser gate**: pg-sql's Recursa parser must preserve the pinned
  differential outcomes, parsed values, failing tokens, and, when the
  `spans` feature is enabled, provenance.
- **Spans feature**: the opt-in Cargo feature that makes ordinary generated
  parse entry points retain occurrence provenance. The always-available
  `parse_without_spans` entry point is used by performance benchmarks for
  comparison with PostgreSQL's raw parser.
- **Quick profiling suite**: the three canonical profiling targets - corpus
  head-to-head, select_list_10000, bool_chain - used for fast before/after
  checks.
- **Discovery profiling suite**: the broader pg-sql/PostgreSQL corpus and its
  statement-family partitions, structural and lexical stress fixtures, and
  rejected-input paths. Each member is profiled independently; results are
  never blended into one flamegraph.
- **Perf journal**: `docs/notes/perf.md`, appended per profile/change pair;
  diagnostic result sections read newest-last.

Use **PostgreSQL parser**, not the broader **SQL parser**, when referring to
this grammar.
