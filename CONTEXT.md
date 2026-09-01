# pg-sql Domain

`pg-sql` is a PostgreSQL parser built with Recursa. The port targets
PostgreSQL 17.9 and preserves language coverage and semantic information
without promising compatibility with the legacy Rust API.

## Canonical terms

- **PostgreSQL statement**: one semantically typed statement accepted by the
  supported PostgreSQL grammar.
- **SQL file item**: one statement, raw or COPY payload region, or recoverable
  failure encountered while processing a SQL file.
- **PostgreSQL oracle**: PostgreSQL 17.9's authoritative raw-parser result.
- **Differential baseline**: the pinned corpus membership and outcome counts
  against the PostgreSQL oracle.
- **Grammar migration**: the reproducible transformation from the immutable
  legacy grammar into current Recursa declarations.
- **Parity gate**: geomean of per-benchmark pg-sql/sqlparser medians at or
  below 1.0 on the statement-level benchmark (ADR 0006).
- **Canonical workloads**: the three profiling targets - corpus head-to-head,
  select_list_10000, bool_chain - every profile and journal entry names one.
- **Perf journal**: `docs/notes/perf.md`, appended per profile/change pair;
  diagnostic result sections read newest-last.

Use **PostgreSQL parser**, not the broader **SQL parser**, when referring to
this grammar.
