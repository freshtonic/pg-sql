# pg-psql

Postgres-flavoured SQL parser based on `recursa`.

## Roadmap

- [ ] 100% accuracy with Postgres SQL syntax (as of version 17)
- [ ] Passes Postgres's SQL parser regression tests
- [x] Generate accurate railroad diagrams from the syntax tree
- [ ] Impossible to represent invalid SQL with the AST
- [x] Support quickcheck-style testing: optional Arbitrary impl feature
- [ ] PGPLSQL support behind feature flag
- [x] Benchmark (runnable in CI to catch regressions)
- [ ] Support "super flat" ASTs https://jhwlr.io/super-flat-ast/

## Benchmarks

The `parse` Criterion bench measures pg-sql parser throughput and compares
against `sqlparser-rs` (`PostgreSqlDialect`).

Run everything:

```bash
cargo bench -p pg-sql --features postgres-oracle
```

Filter to one group:

```bash
cargo bench -p pg-sql --features postgres-oracle -- corpus/head-to-head
cargo bench -p pg-sql --features postgres-oracle -- stress/insert_values
```

Groups:

- `corpus/pg-sql-full` — parse every file under `fixtures/sql/`, reports MB/s.
- `corpus/head-to-head` — parse the set of files accepted by *both* parsers;
  compares pg-sql vs sqlparser directly.
- `stress/<shape>` — per-shape scaling curves over generated stress inputs
  (`insert_values`, `bool_chain`, `select_list`, `nested_subquery`, `in_list`).
- `stress/aggregate` — single regression-check measurement across all stress
  files.

Every group is capped at 1 s of measurement time and 10 samples so a single
slow case can't stall the suite.

### Regenerating stress fixtures

Stress fixtures live in `fixtures/stress/` and are deterministic. Regenerate
with:

```bash
cargo run --bin gen-stress -p pg-sql
```
