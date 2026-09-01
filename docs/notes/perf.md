# pg-sql perf journal

This is the running performance journal for the parity effort (ADR 0006,
`docs/plans/2026-09-01-performance-parity.md`). It follows the discipline of
recursa-old's journal:

- Every profile and every landed performance change gets a section here.
- Every section names at least one **canonical workload** and gives its
  numbers.
- Diagnostic-result sections are append-only and read **newest last**.

## Canonical workloads

Three named workloads (CONTEXT.md). Every journal entry and every profile
names one of them.

| Name | Input | What it stresses |
|---|---|---|
| `corpus` | All 43,474 frozen corpus statements (the differential-baseline membership, `tests/support/baseline.rs`) | Real statement mix, small statements, error/expected-set paths for the 262 statements pg-sql rejects |
| `select_list_10000` | `fixtures/stress/select_list_10000.sql` | One SELECT with a 10,000-item select list; repetition and value construction |
| `bool_chain` | `fixtures/stress/bool_chain_1000.sql` | One WHERE clause with 1,000 `AND` terms; the Pratt loop |

The stress fixtures are checked in and regenerable with
`cargo run --bin gen-stress -p pg-sql`. The `corpus` workload is the
statement set that `benches/parse.rs` calls the "corpus head-to-head"; the
head-to-head engine comparison itself stays in that bench.

## Running the flame harness

The harness is `benches/flame.rs` (Track T port of recursa-old's
`flame.rs`/`flame_target`). It parses one canonical workload with pg-sql in
a tight loop for a fixed duration, through the exact statement seam that
`benches/parse.rs` times (`lex_statement_source` + `Statement::parse`). It
prints its PID for attach-style profilers and a machine-readable
`iters=... elapsed_ns=...` line on completion.

```bash
# Time one workload without a profiler (duration is in seconds, default 5):
cargo bench -p pg-sql --features postgres-oracle --bench flame -- \
    select_list_10000 --duration 5

# List the workload names:
cargo bench -p pg-sql --features postgres-oracle --bench flame -- --list
```

### One-command profile (macOS)

`scripts/flame-profile` runs the harness, samples it with `/usr/bin/sample`,
folds the stacks with the FlameGraph scripts, and writes an SVG:

```bash
# One-time: clone the FlameGraph scripts (or set FLAMEGRAPH_DIR).
git clone https://github.com/brendangregg/FlameGraph ~/tools/FlameGraph

scripts/flame-profile bool_chain 10        # workload, sample seconds
```

Output lands in `docs/perf/flamegraphs/<date>-<sha>[-dirty]/` (gitignored;
runs are not committed — their summaries belong in this journal):

- `<workload>.svg` — the flamegraph
- `<workload>.folded` — folded stacks, for `awk` self-time queries
- `<workload>.sample.txt` — the raw `sample` call tree
- `<workload>.stats.txt` — the harness timing line

`sample` needs no root for your own processes, and the bench profile keeps
debug symbols (`[profile.bench] debug = true`), so parser frames resolve.
recursa-old's `cargo flamegraph`/`cargo instruments` pipelines stay
available, but on Apple Silicon both are broken or need manual steps (see
the old journal); the `sample` pipeline replaces them.

To get a top self-time table from a folded file:

```bash
awk -F';' '{n=split($0,a,";"); split(a[n],b," ");
            self[b[1]]+=b[2]; tot+=b[2]}
     END  {for (s in self) printf "%8.2f%% %s\n", 100*self[s]/tot, s}' \
    docs/perf/flamegraphs/<run>/<workload>.folded | sort -rn | head -15
```

### Linux

Run the same binary under `perf`:

```bash
cargo bench -p pg-sql --features postgres-oracle --bench flame --no-run
perf record -g <printed executable> bool_chain --duration 10
```

## Baseline (2026-09-01)

Interim statement-level benchmark (`cargo bench -p pg-sql --features
postgres-oracle`, `benches/parse.rs`), summed per-bench medians over the
33,168 statements that all three engines accept:

| Engine | Total median time |
|---|--:|
| pg-sql | 50,881.7 ms |
| sqlparser 0.52 | 216.1 ms |
| PostgreSQL 17.9 raw parser | 40.8 ms |

That is roughly **235x** sqlparser. The reference ceiling is recursa-old's
final 0.525x geomean over 240 benchmarks; the parity gate is geomean
pg-sql/sqlparser <= 1.0 (ADR 0006).

## Diagnostic result (2026-09-01, flame harness port — first profiles)

First profiles on the ported harness, base commit `c8ca209` (harness itself
uncommitted, hence `-dirty`), Apple M-series, macOS 15.5, bench profile
(opt 3 + debuginfo), 10 s `sample` at 1 ms per workload via
`scripts/flame-profile <workload> 10`. Run directory:
`docs/perf/flamegraphs/2026-09-01-c8ca209-dirty/`.

Harness timings (15 s loops around the 10 s sampling window):

| Workload | Statements parsed | Accepted | Per statement | Throughput |
|---|--:|--:|--:|--:|
| `corpus` | 11,752 (0 full passes of 43,474) | 11,490 | 1.28 ms | 0.057 MiB/s |
| `select_list_10000` | 62 | 62 | 242.6 ms | 0.271 MiB/s |
| `bool_chain` | 513 | 513 | 29.3 ms | 0.196 MiB/s |

Top self-time frames (share of all samples; demangled and grouped):

`bool_chain` — `bool_chain.svg`:

| Self % | Frame |
|--:|---|
| 27.5% | `recursa_core::grammar::FollowSet::union` |
| 17.2% | allocator (`nanov2_free` / `nanov2_malloc_type` / `_malloc_zone_malloc` / `_free`) |
| 8.6% | `regex_automata` meta `Regex::search` + memmem prefilter |
| 6.6% | generated `__recursa_decode_pratt_*` (Expr) |
| 3.7% | generated `Expr::__recursa_parse_bpu` |
| 3.5% | `recursa_core::lex::CompiledLexMatcher::matched` |
| 3.3% | `_tlv_get_addr` (thread-local access) |

`select_list_10000` — `select_list_10000.svg`:

| Self % | Frame |
|--:|---|
| 39.8% | `recursa_core::grammar::FollowSet::union` |
| 16.3% | allocator |
| 6.0% | generated `__recursa_decode_pratt_*` (Expr) |
| 5.8% | `regex_automata` meta `Regex::search` + memmem prefilter |
| 3.0% | generated `Expr::__recursa_parse_bpu` |
| 2.8% | `recursa_core::lex::CompiledLexMatcher::matched` |

`corpus` — `corpus.svg`:

| Self % | Frame |
|--:|---|
| ~33% | `regex_automata` lazy-DFA determinization (`determinize::next`, `epsilon_closure`, `add_nfa_states`, state interning, SipHash) |
| ~8% | allocator |
| 5.5% | `_platform_memmove` + `_platform_memcmp` |

Observations (attribution proper is Track P; these are the harness
shake-out headlines):

- `FollowSet::union` alone is 27–40% of self time on both stress
  workloads. It sits under the Pratt/dispatch machinery, which makes
  dispatch/decision the leading Track P bucket to attribute first.
- Allocator traffic is the second bucket everywhere (~8–17% self), before
  any explicit value-construction frame shows up.
- The `corpus` profile is qualitatively different: many small statements
  make per-statement `regex_automata` lazy-DFA determinization dominate
  (~33%), which points at lex-machinery construction cost that the
  single-statement stress loops amortize away.

Suites: library suite 1092 passed / 0 failed / 2 ignored, unchanged with
the harness in the tree.
