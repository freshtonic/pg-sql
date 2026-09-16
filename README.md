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
- [x] Support "super flat" ASTs https://jhwlr.io/super-flat-ast/ — see
      [Flat AST](#flat-ast)

## Toolchain

`rust-toolchain.toml` pins a dated nightly. The dev loop compiles the
generated grammar with the parallel rustc front end (`-Z threads` in
`.cargo/config.toml`), which stable rustc does not accept. If your shell
exports `RUSTUP_TOOLCHAIN`, it overrides the file; unset it in this
workspace.

## Flat AST

The grammar root selects Recursa's `flat` flag, so every `ast_node!`
declaration has a second concrete representation beside the nested arena AST:
one contiguous, handle-addressed store per parse. `Statement::parse_flat` and
`Statement::parse_flat_without_spans` sit beside `parse` and
`parse_without_spans`, run the same lexer and the same LR automaton, and
return a `FlatAst` that renders exactly what the detached nested AST renders.

`Expr`, `ColumnRef`, `FuncCall`, `SelectStmt` and `Statement` carry
`#[flat(pool)]`, which gives each its own typed pool; every other declaration
reaches the shared word pool. `tests/flat_parity.rs` proves nested/flat
recognition and rendering parity over every statement the differential
baseline pins, with no PostgreSQL oracle needed:

```bash
cargo test -p pg-sql --test flat_parity
cargo test -p pg-sql --features spans --test flat_parity
```

The design lives in Recursa's `docs/flat-ast-design.md`.

## Benchmarks

The `parse` bench measures pg-sql parser throughput and compares it against
its own Flat AST (the `pg-sql-flat` engine), `sqlparser-rs`
(`PostgreSqlDialect`), and PostgreSQL 17.9's raw parser. `pg-sql` and
`pg-sql-flat` share the lex pass and the automaton, so the pair isolates the
representation; the harness refuses to report timings unless the two accept
exactly the same statements.

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
