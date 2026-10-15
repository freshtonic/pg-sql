# Build times

Measurements of what the build of this workspace costs, and what was tried
against it. Parse performance has its own journal, `docs/notes/perf.md`.
Entries read oldest first. All times are on the M1 Max (8 performance cores)
with the machine otherwise idle, unless a line says otherwise.

## 2026-10-15: the gates, and the generated crate

### The gates

One version of the old six-version gate cost 288 s: three compiles of the
generated grammar (`spans`, `spans,postgres-oracle`, and once more through
pg-psql, whose `recursa` dependency carried recursa's empty `default`
feature and so was a different build unit), 15 test binaries (7 s to link,
13 s to run), pg-psql, and the oracles. Six versions were about 35 min. The
hours that agent sessions saw came from multipliers on top: two or three
gates sharing ten cores at a load of 50 to 85, fresh worktrees building up
to 12 PostgreSQL trees at 47 s each, and a disk at 97 percent holding 270 GB
of stale `target` directories.

ADR 0011 and ADR 0012 record what changed: one merge gate configuration,
shared with `cargo loop`; a release gate for the rest; the dev profile
without debug info (5 GB of unpacked debug objects per configuration, 4 s of
compile); one PostgreSQL tree per pinned commit in `~/.cache/pg-sql`; a
machine-wide gate lock; and a prune of stale configurations after each gate.
After the change: merge gate 25 s with everything cached, 79 s after a
library edit; release gate 268 s for one version; a fresh worktree's `target`
is 4 GB.

### The generated crate

`cargo check` of pg-sql costs as much as `cargo build`: the frontend is the
cost, not LLVM. The generated crate is about 70 MB of token-stream source in
1,540 node files and a 10 MB root, with 71k functions and 47k impl blocks.
`-Z time-passes` of a full non-incremental compile, about 60 s: macro
expansion 16 s (parsing the 70 MB, the builtin derives and `macro_rules`
transcription; proc macros are 0.07 s of it), type check 6 s, borrow check
4 s, crate metadata 5.5 s for a 344 MB rmeta, codegen 4 s, LLVM 5 s, and
resolution, lowering and mono collection for the rest. An incremental
rebuild after a one-line edit is 47 s: the 16 s of expansion, 4 s to persist
the cache, and the hashing of everything else.

Tried, with `cargo check` on an idle machine (noise about plus or minus 2 s):

| Change | Effect |
|---|---|
| Strip 5.7 MB of repeated doc comments and 8.3 MB of repeated `#[allow]` attributes from the generated files | none |
| The `arbitrary` feature off | 2 s |
| The `spans` feature off | at most 2 s |
| The `explorer` feature off | not possible: document framing requires the explorer plan (recursa RCA9105) |
| Flat types with `Clone, Copy` only, not `Debug, Eq, Hash, PartialEq` | 1 to 2 s |
| One generic flat parse entry per crate in place of four closure bodies per node | full compile 83 s to 72 s with the line above, borrow check, codegen and LLVM each a third smaller |
| An empty `scan_requirements` for the 933 nodes whose requirement ceiling is the baseline, and `NodeRef` without `Eq, Hash, PartialEq` | within noise, 1 MB less generated |

A rustc self-profile (`-Z self-profile`, aggregated by generated function
family) attributes the type checker's time: the Pretty bodies, of which every
node has a nested and a Flat copy, about half of it; the flat parse wrappers
a third, now removed; then the visit impls, the explorer schema statics and
the reduce actions. The next lever is structural: a table-driven Pretty, one
static plan per node and one generic walker in recursa-core, in place of two
rendered bodies per node. Expansion time falls only with generated volume.
