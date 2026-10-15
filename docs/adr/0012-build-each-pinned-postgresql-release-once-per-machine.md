# Build each pinned PostgreSQL release once per machine

Status: accepted

The two oracles (ADR 0002, `docs/psql-oracle.md`) link the pinned PostgreSQL
release of the target version, which their build scripts extract from the
`vendor/postgres` object store and build with `configure && make`, about
fifty seconds a release. pg-oracle kept the tree under its `OUT_DIR`, whose
hash moves with every build input, and pg-psql-oracle under
`target/pg-trees`, so one checkout held fifteen trees for six pins, and every
new worktree, which this project creates for nearly every issue, built all
twelve again from nothing.

Both build scripts now read one tree per pinned commit from a cache outside
every checkout, `${XDG_CACHE_HOME:-~/.cache}/pg-sql/postgres/<ref>-<commit>`,
which `PG_SQL_POSTGRES_CACHE` moves. The tree is named by the commit, so a
moved pin builds a new tree beside the old one and never disturbs a checkout
still on the old pin; a stamp in the tree records the commit, as before. A
lock beside the tree serialises builds from several checkouts. The
alternatives were six vendored source trees, which still build, and prebuilt
oracle libraries, which break across toolchains.

## Consequences

- The oracle's correctness no longer depends on anything in `target`. The
  `pins.tsv` commit, the stamp and the list of generated files the build
  script checks are the whole contract.
- A build of pg-oracle or pg-psql-oracle in CI builds into the runner's home
  directory; the fetch of the pinned releases is unchanged.
- `pg-oracle/scripts/build-pg.sh <feature>` by hand builds into the cache
  too, so a tree can be prepared before a gate runs.
