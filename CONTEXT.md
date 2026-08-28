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

Use **PostgreSQL parser**, not the broader **SQL parser**, when referring to
this grammar.
