# Split the merge gate from the release gate

Status: accepted. Supersedes decision 13 of
`docs/plans/2026-09-22-target-versions.md`.

Until now the merge gate built and tested all six target versions with
`spans`, and the no-`spans` and `arbitrary` combinations on `pg17`. Each
version is its own build of the generated grammar, and the gate built it
three times per version (once for the core tests, once more with the oracle
feature, once more through pg-psql), so one run was fifteen compiles of a
crate that takes a minute to compile on a quiet machine, before any test ran.
Agent sessions ran two or three such gates at once on ten cores, and a run
took hours.

The merge gate now proves one gate configuration: the default target version,
`pg17`, with `spans`, `arbitrary` and both oracles, in one cargo invocation
over pg-sql, pg-psql, pg-oracle and pg-psql-oracle. That is one compile of
the grammar, and it is the same configuration the edit loop uses
(`cargo loop`), so the loop and the gate share one build and one incremental
cache. The other five versions, the `pg17` build without `spans` and without
`arbitrary`, the parity gate of ADR 0006 and the corpus coverage of each
version baseline move to the release gate, which runs before a release and
only then. The alternative, keeping every version on every merge and making
each build cheaper, was measured first: the floor is six compiles of the
grammar per run, about six minutes of CPU before tests, for a check that the
other versions rarely fail.

## Consequences

- A branch can merge to main with a version gate that is wrong for an older
  version. CLAUDE.md principle 11 therefore asks for `scripts/release-gate
  pgN` on the oldest affected version before merging a change to a
  `#[config(since/before)]` gate, and the release gate catches the rest.
- Both gates take one machine-wide lock, so gates from several sessions queue
  instead of sharing the cores. Two gates on ten cores finish later than the
  same two one after the other.
- Both gates prune the build artefacts of every configuration but the merge
  gate's at their end, because a stale configuration is a gigabyte nothing
  reads again, and a full disk slows every build.
- The dev profile carries no debug info. The gates never run a debugger, and
  the unpacked debug objects of one configuration were five gigabytes that
  every test link read. `--profile dev-debug` restores line tables.
