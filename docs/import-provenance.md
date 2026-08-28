# Phase 1 import provenance

The immutable migration input is the legacy Recursa commit
`1e71421d66baac15c8c5264e8f29b5f80122f50e`.

| Object | Git identity |
| --- | --- |
| Legacy commit tree | `f3191ab707c8a957d1bb5fe142e74fc624fe6661` |
| `pg-sql` tree | `50e1376d16796e5f05db88d99dab42252a9f78a4` |
| `pg-oracle` tree | `0780d057e4d54db150d0f388c45a720a825bcbcf` |
| PostgreSQL 17.9 gitlink | `6d396980fc5aed4f1a525e0bd75cb16b25ed40ca` |
| Initial current-Recursa revision | `384764c5827c73e579324e4a08e57543a2b6e97b` |

Commit `e97d3c3570c2a04ca9a233334b46d3f443800a5a` is the untouched
source-import checkpoint. Its tree contains all 136 `pg-sql` entries and seven
`pg-oracle` entries, checked for exact mode and object-ID equality against the
two legacy trees before the commit was created. `README.md` and `CLAUDE.md`
already had their exact legacy object IDs in the planning commit; the other
134 `pg-sql` entries and all seven `pg-oracle` entries were introduced by the
checkpoint. The only bootstrap addition in that checkpoint is the relocated
root `.gitmodules`; `vendor/postgres` remains the exact legacy gitlink.

Run `scripts/verify-import-provenance` from a clean checkout to verify the
recorded legacy commit and tree, checkpoint manifest, and current immutable
inputs without requiring the legacy repository as a sibling checkout.

The named pre-transformation differential baseline is recorded in
`baselines/postgresql-17.9.json`. It was captured from a detached disposable
clone, never from the original legacy checkout.
