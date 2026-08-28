# pg-sql Port Plan

Status: accepted

## Goal

Port the legacy PostgreSQL 17.9 grammar and its supporting behaviour to the
current Recursa implementation in a fresh sibling repository. Preserve
PostgreSQL language coverage, semantic AST information, formatting behaviour,
file-level recovery, oracle-backed conformance, the complete Rust test
inventory, and benchmarks, without requiring Rust source or AST API
compatibility with the legacy crate.

The port is complete only when the full contract below passes. Strict
single-statement parser parity is a named intermediate milestone, not the end
of the port. The legacy implementation remains the immutable reference until
the full completion gates pass; only then does the sibling repository become
canonical.

## Repository contract

- Create `freshtonic/pg-sql`, checked out locally beside Recursa as
  `../pg-sql`, with visibility matching Recursa.
- Make the repository root both the `pg-sql` package and workspace root. Add
  private `pg-oracle` and migration-tool members.
- Keep package `pg-sql`, crate `pg_sql`, version `0.1.0`, MIT, Rust edition
  2024, MSRV 1.88, `publish = false`, and a committed `Cargo.lock`.
- Depend on `../recursa/recursa` and `../recursa/recursa-codegen` by path. Pin
  their shared matching-version checkout with a reviewed
  `.recursa-revision`; CI checks out that revision beside `pg-sql`.
- Run an additional scheduled or on-demand downstream check against Recursa
  `main`. Paired capability changes merge in Recursa before the `pg-sql`
  revision advances.
- Keep the legacy repository completely unchanged.

See [ADR 0001](adr/0001-port-pg-sql-as-a-sibling-consumer.md).

## Immutable inputs and migration

- Capture the pre-port baseline in a disposable clone of the complete legacy
  repository at `1e71421`. Never build, migrate, or generate inside the
  original `recursa-old/pg-sql` checkout.
- Import `pg-sql` and `pg-oracle` from Git objects at legacy Recursa commit
  `1e71421`, not from its dirty working tree, and preserve the untouched
  source import as a commit.
- Recreate `vendor/postgres` at the legacy PostgreSQL 17.9 gitlink. Build its
  FFI oracle out of tree without modifying the submodule.
- Record and verify the legacy commit and subtree identities before and after
  migration.
- Keep an auditable migration tool in the new repository. Inventory mode is
  read-only and reports exact construct counts and unsupported cases. Rewrite
  mode uses span-based edits, preserves comments and ordering, writes only to
  the new repository, and publishes no partial result on failure.
- Keep grammar transformation and repetitive test-call transformation as
  separate passes of the tool. Until migration completes, correct a bad
  transformation in the tool or an explicit reviewed mapping file and rerun
  it rather than editing transformed AST or token declarations manually.
- Do not import the legacy checked-in `first_set.rs`, `__firstset` references,
  generation drift checks, or old workspace codegen. Current Recursa owns its
  matching-version Generation in `OUT_DIR`.
- Permanently test representative migration fixtures. Require one final clean
  end-to-end reproduction, but do not make normal CI check out `recursa-old`.

## Semantic compatibility

- Inventory every legacy semantic type, variant, and non-syntactic field. Map
  each to a ported equivalent or an explicitly reviewed semantic change.
- Preserve existing module organization and semantic names where practical.
  Do not preserve fixed-token fields, obsolete container wrappers, generated
  first-set helpers, or other framework artifacts merely for source
  compatibility.
- Do not require legacy parse-error types or message text. Preserve strict
  rejection, meaningful source locations, file-level continuation, and access
  to an underlying structured failure. Prefer stable Recursa diagnostic codes
  in tests where they describe the required behaviour.

See [ADR 0003](adr/0003-make-the-pg-sql-grammar-port-reproducible.md).

## PostgreSQL oracle and corpus

- Keep the FFI `pg-oracle` based on PostgreSQL 17.9 `raw_parser`; do not
  restore the retired Testcontainers execution harness.
- Establish a named pre-transformation differential baseline from the
  disposable legacy clone. Commit a machine-readable record containing the
  legacy commit, PostgreSQL gitlink, included and excluded files, statement
  counts, and pass, skip, and fail outcomes, with an explicit review command
  for updates. Freeze each suite's existing corpus inclusion rules and
  statement counts.
- Completion permits no new corpus exclusions, skips, or unexplained loss of
  PostgreSQL-accepted statements.

See [ADR 0002](adr/0002-use-postgresql-raw-parser-as-the-pg-sql-oracle.md).

## Recursa capability prerequisites

Do not retain handwritten parser exceptions in `pg-sql`. Resolve general
framework gaps through three dedicated `grill-with-docs` sessions before
implementing their outcomes:

1. PostgreSQL lexical and grammar expressiveness: admissions, custom
   operators and identifiers, callbacks, postconditions, and source-region
   capture.
2. Trivia and formatting: comment ownership, adjacent string literals, and
   formatter round trips.
3. File parsing and recovery: statement segmentation, raw and COPY regions,
   spans, error continuation, and compatibility with
   [Recursa ADR 0002](https://github.com/freshtonic/recursa/blob/main/docs/adr/0002-separate-explorer-recovery-projection.md).

## Tests and benchmarks

- Port every current embedded and integration test and fixture. Retain each
  ignored test's status and account for it explicitly.
- Preserve formatter goldens, idempotence, file parsing and recovery tests,
  stress fixtures, and differential tests.
- Convert the custom benchmark harness to Criterion. Preserve the PostgreSQL
  corpus, all 15 stress workloads, and comparisons among `pg-sql`,
  `sqlparser` 0.52, and PostgreSQL. Check acceptance outside timed iterations,
  expose Criterion filtering, and report byte throughput.
- Port the trend-report command with environment metadata and start fresh
  measurement history, linking to rather than copying legacy reports. Defer
  depth-probe and flamegraph utilities unless needed to investigate a port
  regression.
- Benchmark results are evidence for a later optimization session, not a
  performance gate for this port.

## CI and completion gates

- Require Linux for the full FFI oracle suite. Keep macOS development
  supported and smoke-tested where practical.
- On Rust 1.88 and stable, require formatting, warning-free Clippy, all-feature
  workspace tests, and warning-free Rustdoc with broken intra-doc links
  denied.
- Require the complete PostgreSQL 17.9 differential baseline, with no new
  skips or exclusions, and account for the complete legacy test inventory.
- Compile Criterion benchmarks in CI and record one complete benchmark and
  trend report for the port. Report performance without gating it.
- Verify a clean migration reproduction and the unchanged legacy tree before
  declaring the port complete.

## Dependency and work ownership

- Preserve legacy non-Recursa dependency versions during the port, except for
  adding Criterion and changes strictly required by the standalone layout or
  reproducible PostgreSQL build. General dependency updates follow parity.
- Track general design and capability work in Recursa issues. Track import,
  migration, tests, benchmarks, and parity in the new `pg-sql` repository.
  Cross-link blockers, and advance `.recursa-revision` only after its Recursa
  prerequisite lands.

## Execution phases

1. Capture the disposable legacy baseline; bootstrap the fresh repository and
   exact imports.
2. Build the migration inventory, semantic mapping, representative fixtures,
   and dry-run tool.
3. Run the lexical and grammar-expressiveness design session; implement its
   accepted general Recursa capabilities and advance the pinned revision.
4. Run the one-shot grammar and test-call rewrites; reach strict
   single-statement and PostgreSQL 17.9 differential parity.
5. Run the trivia and formatting design session; implement its accepted
   Recursa capabilities and restore formatter goldens.
6. Run the file parsing and recovery design session; implement its accepted
   Recursa capabilities and restore file-level tests.
7. Port and verify the remaining tests, features, Criterion suite, trend
   reporting, CI, and documentation; perform the final migration reproduction.
8. Audit every completion gate and make the sibling repository canonical.

Use phase-sized `pg-sql` issues with explicit exit checks. Track every Recursa
capability separately, and keep strict parsing, formatting, recovery, benchmark
conversion, and the final audit independently reviewable. A phase checkpoint
requires its scoped checks to be green, its decisions to be documented, no
unexplained generated or submodule changes, and the exact Recursa revision to
be recorded.

If migration reveals another Recursa gap, stop that branch and record a
minimized reproducer and blocker. Run a focused `grill-with-docs` session,
land the accepted general capability in Recursa, and only then advance the
port. Do not use temporary handwritten parsing to conceal the dependency.

## Explicitly outside this port

- PostgreSQL releases other than 17.9.
- Performance optimization or performance pass/fail thresholds.
- Upgrades to `sqlparser` or unrelated dependencies.
- Crates.io publication.
- Exact legacy AST/API or diagnostic-text compatibility.
- A new fuzzing requirement.
- Depth-probe and flamegraph tooling unless needed to diagnose a migration
  regression.

## New repository language

Create a root `CONTEXT.md` during repository bootstrap using these canonical
terms:

- **PostgreSQL statement**: one semantically typed statement accepted by the
  supported PostgreSQL grammar.
- **SQL file item**: one statement, raw or COPY payload region, or recoverable
  failure encountered while processing a SQL file.
- **PostgreSQL oracle**: PostgreSQL 17.9's authoritative raw-parser result.
- **Differential baseline**: the pinned corpus membership and outcome counts
  against the PostgreSQL oracle.
- **Grammar migration**: the reproducible transformation from the immutable
  legacy grammar into current Recursa declarations.

Use “PostgreSQL parser” rather than the broader “SQL parser” when referring to
this grammar.
