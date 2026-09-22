# Differential baseline: PostgreSQL 17.9 corpus, PostgreSQL 17.11 oracle

`baselines/postgresql-17.9.json` is the named, machine-readable floor for
PostgreSQL statement parity. It records the immutable legacy Git identities,
the PostgreSQL 17.9 gitlink, all 226 regression SQL files, the legacy suite's
222 included files and four explained exclusions, and per-file statement and
pass/skip/fail counts.

## Frozen corpus and pinned oracle

The corpus and the oracle are two different inputs:

- The **corpus** is frozen at PostgreSQL 17.9 (`6d396980fc5`). It is the set
  of regression SQL files that the legacy parser split into the frozen
  statement spans. The tests read each file by its Git blob ID from the
  `vendor/postgres` object database, not from the checked-out tree. The
  baseline file names and their `postgres` fields name this corpus.
- The **oracle** is the raw parser of the pinned PostgreSQL release. The
  `vendor/postgres` submodule pins PostgreSQL 17.11 (`REL_17_11`,
  `083ac033419`). `pg-oracle` builds that release and runs `make` again when
  the submodule moves to a different commit.

Thus a move of the pin changes only the oracle. The statement spans, the
legacy item kinds and the per-file counts stay valid, so each outcome change
has one cause: a change in the raw parser. The submodule must contain the
history back to 17.9, because the tests read the frozen blobs from it.

### Pin move from 17.9 to 17.11 (#71)

Between `REL_17_9` and `REL_17_11`, `src/backend/parser/gram.y`, `scan.l`,
`parser.c` and `src/include/parser/kwlist.h` did not change. The only change in
the oracle sources is in node support: `CreateStatsStmt` has a new `owner`
field (commit `75a03c569c7`) and `makeJsonIsPredicate` has a new assertion.
Neither changes which statements the raw parser accepts. The differential
suite has the same result at 17.11 as at 17.9: 222 of 222 corpus files pass,
with no outcome change for any statement.

The 17.11 regression files change 40 corpus files. A simple split of these
files gives 345 statements that are not in the frozen corpus. They are not in
the baseline. A one-time probe of these statements with the differential check
and the 17.11 oracle found no failure.

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

`baselines/postgresql-17.9-accepted-legacy-gaps.json` assigns those aggregate
skips stable statement identities: fixture, zero-based statement index, and
absolute source byte range. It also freezes their current strict-statement
outcome so that a resolved legacy gap cannot silently regress. Seventeen now
pass the generated parser and formatter comparison. The remaining permanent
gap is `join.sql` statement 171 (`16727:16787`), whose strict parse diagnostic
is `RCA4100` with statement-relative region and primary anchor `29:31`.

The fast baseline test validates the ledger's provenance, ordering, unique
identities, ranges, legacy parse-error classification, and per-file counts
against both pinned baseline artifacts. The Linux differential test then
re-derives PostgreSQL acceptance and compares every current outcome, including
the permanent gap's exact diagnostic contract:

```sh
cargo test --locked -p pg-sql --test differential_baseline
cargo test --locked -p pg-sql --features postgres-oracle --test accepted_legacy_gaps frozen_accepted_legacy_gap_contracts_are_exact -- --exact
```

`baselines/postgresql-17.9-statements.json` is the companion statement-span
artifact. It freezes each included source blob, byte length, exact statement
byte ranges, and whether the legacy extractor produced a statement or a parse
error for that range. Validate its provenance, membership, counts, ranges, and
the checked-out PostgreSQL source blobs without rebuilding the legacy parser:

```sh
cargo run --locked -p pg-sql-migrate -- baseline verify-statements
```

Fresh review and update commands are recorded in the artifact itself. Both use
detached disposable clones and the pinned legacy statement-span capture fixture;
updating the artifact is therefore an explicit baseline review rather than a
normal test operation.

CI also performs the full statement-span review from a full-history `pg-sql`
checkout. The review command clones that checkout without hardlinks, detaches at
legacy commit `1e71421d66baac15c8c5264e8f29b5f80122f50e`, recaptures every range,
and byte-compares the canonical result with the committed artifact:

```sh
cargo run --locked -p pg-sql-migrate -- baseline review-statements --legacy-repository . --postgres-repository vendor/postgres --baseline baselines/postgresql-17.9.json --spans baselines/postgresql-17.9-statements.json
```

This gate is deliberately stronger than `verify-statements`: changing the
artifact and its internally consistent metadata cannot pass without reproducing
the same bytes from the immutable legacy parser and PostgreSQL corpus.

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
