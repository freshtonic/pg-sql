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
cost, not LLVM. The one large win is at the end of this section; the
measurements before it are what ruled the other candidates out. The generated crate is about 70 MB of token-stream source in
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
| Flat types with `Clone, Copy` only, not `Debug, Eq, Hash, PartialEq` | 1 to 2 s; reverted, recursa's fixture crates format and compare Flat values |
| One generic flat parse entry per crate in place of four closure bodies per node, and an empty `scan_requirements` for the 933 nodes whose requirement ceiling is the baseline | about 1 s of a 64 s non-incremental compile, 5 MB less generated (recursa branch `perf/generated-item-count`) |

Then a 12 s `sample` of rustc during expansion showed where the 16 s went:
forty-five percent of the main thread was in `escape_string_symbol`, called
from `expand_include_str`. Every node's `explorer()` carried
`include_str!` of the generation's 3 MB diagram plan, so the expander read
and escaped 1,540 copies, 4.5 GB of text. Lexing and parsing the 70 MB were
under three percent. One root static holding the plan, named by every
adapter, took expansion from 16.4 s to 3.5 s and the full compile from
64 s to 35 s; an incremental rebuild after an edit follows, since expansion
runs in full on every rebuild.

| Change | Effect |
|---|---|
| The diagram plan `include_str!` once in the root instead of once per node | expansion 16.4 s to 3.5 s; full non-incremental compile 64 s to 35 s |
| All 9.6 MB of generated doc comments stripped | 2 s, half a second of it expansion; not taken |

A rustc self-profile (`-Z self-profile`, aggregated by generated function
family) attributes the type checker's thread time: the Pretty bodies, of
which every node has a nested and a Flat copy, about half; the flat parse
wrappers a third; then the visit impls, the explorer schema statics and the
reduce actions. But type and borrow checking run on the eight threads of
`-Z threads=8` and take 7 s and 4 s of wall clock, so removing a third of
the type checker's work moved the wall clock by under a second. The floor is
the serial passes: expansion 16 s, metadata 6 s, resolution and lowering
about 5 s. A change pays only when it shrinks those, which means less
generated volume, fewer `include!` rounds, or fewer derive and gate
expansions. The structural candidate is a table-driven Pretty: one static
plan per node and one generic walker in recursa-core, in place of two
rendered bodies per node, which is about a quarter of the generated bytes.
