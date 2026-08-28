# PostgreSQL 17.9 differential baseline

`baselines/postgresql-17.9.json` is the named, machine-readable floor for
PostgreSQL statement parity. It records the immutable legacy Git identities,
the PostgreSQL 17.9 gitlink, all 226 regression SQL files, the legacy suite's
222 included files and four explained exclusions, and per-file statement and
pass/skip/fail counts.

The inclusion rule is the exact `corpus_tests!` declaration at legacy commit
`1e71421d66baac15c8c5264e8f29b5f80122f50e`. The only excluded files are the
four `collate.*` fixtures whose dotted names could not become macro-generated
Rust test identifiers. Discovery fails if a future valid identifier is absent
from the declaration, so an undeclared fixture cannot silently become a new
exclusion.

The frozen statement rules are:

- psql backslash directives and COPY-from-stdin payload regions are non-SQL;
- statements containing psql variable interpolation are not standalone SQL;
- a whole-file legacy parse failure yields no extracted statements; and
- a PostgreSQL-accepted statement that the legacy parser cannot model is a
  skip, while formatter/tree differences and over-permissive parses are fails.

The 18 captured skips are frozen by fixture: `amutils` 1, `create_index` 1,
`create_view` 2, `join` 6, `returning` 2, `rules` 1, `select` 1, and `with` 4.
Every other included fixture has zero skips.

Capture uses detached, no-hardlink disposable clones of both local source
repositories. PostgreSQL is generated and built only inside that disposable
clone, under Cargo's `OUT_DIR`, using the pinned capture-only build plumbing in
`migration-tool/fixtures/baseline/`. The legacy parser, formatter, oracle C
sources, and differential tests are unchanged. Capture fails if the disposable
PostgreSQL source worktree changes during the build. It also checks that the
original legacy and PostgreSQL repositories have the same HEAD and worktree
state before and after the run.

The machine-readable baseline records the exact SHA-256 of both capture-only
build fixtures. For the reviewed capture they are
`b97860f909c6bb11c453281c021235fa2ab9c19560d393faa887fe742bcb54d7` for
`build-pg.sh` and
`f208ffc91be7ad270b827e9fb6fb38ed8433542bc81e20480794087629617d2f` for
`pg-oracle-build.rs`. Changing either fixture is therefore an explicit
baseline update even when statement outcomes happen to remain equal.

Review the committed bytes by repeating the full capture:

```sh
cargo run --locked -p pg-sql-migrate -- baseline review --legacy-repository ../recursa-old --postgres-repository vendor/postgres --baseline baselines/postgresql-17.9.json
```

After deliberately reviewing a changed input or rule, update the record with:

```sh
cargo run --locked -p pg-sql-migrate -- baseline capture --legacy-repository ../recursa-old --postgres-repository vendor/postgres --output baselines/postgresql-17.9.json
```
