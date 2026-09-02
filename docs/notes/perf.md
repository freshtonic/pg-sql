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

## Diagnostic result (2026-09-02, Track P: full attribution of the parse-time gap)

Track P of the parity plan (issue #32). Commit `2ec2847` (harness
extensions committed first, so all artifacts carry a clean SHA), pinned
recursa `c4e7f3f`, Apple M1 Max (MacBookPro18,2, 10 cores), macOS 15.5,
bench profile (opt 3 + debuginfo). All artifacts referenced below live in
`docs/perf/flamegraphs/2026-09-01-2ec2847/` (the run directory is stamped
with the UTC date; the analysis is 2026-09-02 local). Instruments:

- `scripts/flame-profile`-equivalent sampling (10 s `sample` at 1 ms per
  workload; `<workload>.folded`, `.svg`, `.sample.txt`).
- `scripts/flame-attribute` (new): buckets folded stacks into the Track P
  cost categories by self time, carrying allocator samples to the nearest
  classifiable caller.
- `benches/flame.rs --count-allocs` (new): counting global allocator, one
  pass per workload, split lex vs parse and accepted vs rejected
  (`<workload>.allocs.txt`).
- `benches/flame.rs --engine sqlparser` (new): sqlparser 0.52 timed over
  the same statements in the same loop (`<workload>.sqlparser.txt`).

### Canonical timings (clean loops, no profiler attached)

`<workload>.clean.txt` / `<workload>.sqlparser.txt`:

| Workload | pg-sql / stmt | sqlparser / stmt | Ratio | recursa-old final |
|---|--:|--:|--:|--:|
| `bool_chain` | 25.0 ms | 1.048 ms | 24x | 0.393 ms (64x ago) |
| `select_list_10000` | 255.9 ms | 7.60 ms | 34x | 3.511 ms (73x ago) |
| `corpus` (mean/stmt) | 1.396 ms | 0.0052 ms | 267x | — |

recursa-old reference numbers are the final trend report
(`../recursa-old/docs/benchmarks/REPORT.md`, run 2026-06-09, commit
`1dd6b09`, geomean 0.525x sqlparser over 240 benchmarks). Old fixtures
share the shape and name but not the byte-for-byte text, so the sqlparser
column (same harness, same statements) is the like-for-like denominator.
Corpus caveats: the 10 s pg-sql loop covers only the first ~7,200 frozen
statements; sqlparser accepts 77.9% of the frozen statements and its
rejects are cheap errors. The interim benchmark's 235x (2026-09-01
baseline above) remains the corpus figure of record; 267x here is the
same order.

### Bucket attribution (self time; sampled 10 s per workload)

`scripts/flame-attribute <run>/<workload>.folded`. The `alloc` share is
allocator/memcpy self time attributed to the bucket whose frame allocated
(it is included in the bucket's self%, not additional).

| Bucket | bool_chain | select_list_10000 | corpus |
|---|--:|--:|--:|
| dispatch/decision | 69.8% (22.8% alloc) | 83.2% (26.0% alloc) | 5.9% (1.5% alloc) |
| lex/classify | 29.1% (3.9% alloc) | 16.3% (1.8% alloc) | 93.8% (28.6% alloc) |
| value construction (spans/occurrences/element metadata) | 0.2% | 0.4% | 0.2% |
| error/expected paths | ~0% | ~0% | ~0% |
| trivia handling | 0% (non-opting grammar; skip rules are inside lex arbitration) | 0% | 0% |
| other | 0.9% | 0.0% | 0.2% |

Dominant frames inside the two hot buckets:

- dispatch/decision: `FollowSet::union` is 38.6% of bool_chain (27.6%
  compute + 11.0% allocator) and 49.2% of select_list_10000 (37.6% +
  11.6%). The generated Pratt body `Expr::__recursa_parse_bpu` carries
  another 15.1% / 16.4%, two thirds of it allocator traffic from the
  same FOLLOW composition; packed-table decoding
  (`__recursa_decode_pratt_*`) adds ~6% / ~7%.
- lex/classify splits into lexer *construction* vs *execution*
  (inclusive samples under `CompiledLexMatcher::new`+`Regex::new` vs
  `CompiledLexMatcher::matched`):

| Workload | lexer construction | matcher execution |
|---|--:|--:|
| `corpus` | 86.7% | 3.6% |
| `bool_chain` | 4.9% | 23.2% |
| `select_list_10000` | 0.5% | 15.5% |

### Allocation counts (`--count-allocs`, one pass)

| Workload | Phase | Statements | Allocs/stmt | Bytes/stmt |
|---|---|--:|--:|--:|
| `bool_chain` (6.0 KB) | lex | 1 | 11,690 | 3.47 MB |
| | parse | 1 | 312,370 | 38.9 MB |
| `select_list_10000` (672 KB) | lex | 1 | 11,693 | 5.37 MB |
| | parse | 1 | 3,170,143 | 526.9 MB |
| `corpus` (3.76 MB total) | lex | 43,474 | 11,672 | 3.32 MB |
| | parse (accepted) | 42,460 | 765 | 111 KB |
| | parse (rejected) | 989 | 624 | 99 KB |

One corpus pass allocates 540.5 million times (149.3 GB) to parse
3.76 MB of SQL. Two structures explain nearly all of it:

- **The lex phase costs ~11,700 allocations and ~3.3 MB per statement
  regardless of statement size.** `recursa_core::lex::lex` compiles all
  113 generated rules (106 `Regex::new`) on every call and rebuilds the
  regex_automata NFAs/DFAs from scratch, then throws them away. That is
  the fixed per-statement cost that dominates the corpus (86.7%
  inclusive) and the reason the corpus profile is qualitatively
  different from the stress profiles, which amortize one `lex` call over
  one huge statement.
- **The parse phase's allocations are FOLLOW-set composition, not value
  construction.** `FollowSet::union` allocates three `Vec`s per call
  (two 80-byte bitset word arrays at kind_count 595, plus the merged
  expected list); `with_caller` allocates a fourth (it clones
  `self.kinds` for the Pratt boundary). The generated grammar has 749
  static `.union(` sites and 724 `.with_caller(` sites — one
  `with_caller` per child-node descent, so every operand of every
  expression re-derives its NUD FOLLOW by composition. `Expr` (node
  1165, 68 NUD / 102 LED branches) additionally builds a Pratt guard
  chain (one union per distinct binding-power level, ~8, gated by
  `bp >= min_bp`) eagerly on every `Expr` entry, before any extender is
  seen — the guard is only ever consulted on the rare `pratt_ambiguity`
  error path. bool_chain parses one 6 KB statement with 312k
  allocations; select_list_10000 with 3.17 M.

Error/expected paths: rejected corpus statements cost slightly *less*
than accepted ones (624 vs 765 allocs/stmt) — error construction copies
one expected-atom list per failure and is invisible in the profiles.
recursa-old's error-path pathology (a source `String` per speculative
failure) does not exist in this runtime.

### The empirical lever: provenance-free parse mode

Authorized by ADR 0006 as a measurement instrument. Mechanism: the parse
seam already runs with `CaptureRequest::Default` and every pg-sql node at
`CapturePolicy::Optional`, so node spans are *already not retained*; what
the provenance layer still does on this seam is (a) retain repetition-
separator spans unconditionally (`parse_repetition_separator`), which
forces the occurrence spine above every separated list, and (b) run the
`ProvenanceCollector::finish*` walk per node. Element metadata is static
(`&'static ElementMetadata`) and costs nothing at parse time.

Measurement patch (scratch vendored copy of the pinned recursa under
`scratch/recursa-provfree/`, wired in via a temporary uncommitted
Cargo.toml path switch; runtime-only, env-gated, acceptance verified
unchanged):

- `RECURSA_PERF_PROVFREE=1` — `ProvenanceCollector::finish_repeated`
  returns `Occurrence::absent()` immediately: no spans, no occurrences,
  no separator retention, `Origin::Uncaptured` roots.
- `RECURSA_PERF_LEXCACHE=1` — `lex()` reuses this thread's compiled
  matcher table (pointer-keyed on the static rule slice), pricing the
  per-statement lexer construction identified above.

Like-for-like results (same binary, 10 s clean loops; per statement):

Like-for-like, per statement (ms), gap = multiple of sqlparser on the same
loop:

| Workload | off | provfree (Δ) | lexcache (×) | provlex (×) | sqlparser | gap off→provlex |
| --- | --: | --: | --: | --: | --: | --: |
| bool_chain | 23.66 | 23.32 (−1.4%) | 22.09 (1.1×) | 22.44 (1.1×) | 1.05 | 23× → 21.4× |
| select_list_10000 | 223.3 | 221.4 (−0.9%) | 223.8 (1.0×) | 220.7 (1.0×) | 7.60 | 29× → 29.0× |
| corpus | 1.204 | 1.194 (−0.8%) | 0.0805 (15.0×) | 0.0807 (14.9×) | 0.0052 | 230× → 15.4× |

Two facts settle ADR 0006's escalation question:

1. **The provenance layer is not the floor.** Disabling span/occurrence/
   separator retention (provfree) moves every workload by under 1.5%. The
   provenance-bearing value model is *not* what separates pg-sql from
   sqlparser; keeping it (ADR 0006) costs ~1%, nowhere near the parity gate.
   No escalation is warranted.
2. **Per-statement lexer construction is the dominant realistic-workload
   cost.** Sharing the compiled matcher table (lexcache) makes the corpus
   15× faster on its own — 230× → 15.4× vs sqlparser — because realistic SQL
   is many short statements and every `lex()` today rebuilds 106 regexes.
   On the two single-huge-statement workloads it is noise (they lex once),
   where the residual gap is parse-bound FOLLOW allocation instead.

### Ranked cost table

By share of the parse-time gap, largest first. "Where" names the crate a
fix must live in; every row cites a profile artifact.

| Rank | Cost | bool_chain | select_list | corpus | Where | Artifact |
|--:|---|--:|--:|--:|---|---|
| 1 | FOLLOW-set composition (`FollowSet::union` + `with_caller`) | 38.6% | 49.2% | 1.9% | recursa-core | `*.folded` dispatch bucket |
| 2 | Per-statement lexer construction (`Regex::new` x106, DFA rebuild) | 4.9% | 0.5% | 86.7% | recursa-core | lex-split table |
| 3 | Generated Pratt body allocation (`__recursa_parse_bpu`, guard chain) | 15.1% | 16.4% | 0.7% | recursa-codegen | dispatch bucket frames |
| 4 | Packed-table decode (`__recursa_decode_*`) | ~6% | ~7% | 0.4% | recursa-codegen | dispatch bucket frames |
| 5 | Matcher execution (`CompiledLexMatcher::matched`, 113 rules/offset) | 23.2% | 15.5% | 3.6% | recursa-core | lex-split table |
| 6 | Value construction (spans/occurrences/element metadata) | 0.2% | 0.4% | 0.2% | recursa-core | value bucket |
| 7 | Error/expected-set paths | ~0% | ~0% | ~0% | — | error bucket, allocs.txt |
| 8 | Trivia handling | 0% | 0% | 0% | — | trivia bucket |

### Candidate-menu verdicts

Each row is a candidate from the plan (Track R menu), with the fresh-profile
verdict ADR 0006 requires.

- **Classifier cache (peek/parse re-classify):** REJECT — not applicable.
  This runtime has no classifier DFA in the parse path; identifier/keyword
  resolution happens once in `lex()` and the parser dispatches on the u16
  kind. `__recursa_classify_words` is 0.42% self on corpus. Nothing to cache.
- **Kind-match dispatch everywhere:** ADOPT-as-done. Dispatch is already
  table-driven (`__recursa_decode_dispatch_*`, packed base64 states); no
  sequential peek cascade surfaces in any profile. The residual cost is the
  FOLLOW allocation around the dispatch, not the branch selection (see #1).
- **Single-pass ASCII trivia scan + bounds-check elision:** REJECT — spent.
  Trivia skip rules are already inside the one `lex()` arbitration pass;
  the trivia bucket is 0% everywhere. The old `consume_ignored`-per-Pratt
  and byte-cursor `Input::remaining` do not exist here (Input is
  token-record indexed).
- **Error-path allocation removal (Arc source, Cow labels):** REJECT — not a
  hot spot. Rejected corpus statements allocate fewer times than accepted
  ones (624 vs 765 allocs/stmt); the error/expected bucket is ~0% self. The
  recursa-old per-speculation source `String` pathology is absent.
- **Peek+parse fusion for keyword-led optionals:** REJECT-for-now. Optional
  groups dispatch through the same table machinery; no redundant
  peek-then-parse frame is measurable.
- **Fork/transaction cost reduction:** REJECT — no fork in the seam. The
  strict statement seam checkpoints the cursor once at the root; there is no
  per-branch `Input::fork` clone. No fork frame in any profile.
- **Identifier fast path past the classifier DFA:** REJECT — moot (no parse
  classifier DFA). The real lexer-DFA cost is construction, addressed by
  lexer-matcher sharing, not an identifier gate.
- **HashSet keyword membership / redundant-postcondition removal / byte-scan
  hot leaf parsers:** REJECT — spent or moot. Keyword membership is resolved
  in `lex`; no postcondition regex or `Vec<char>` leaf-scan frame appears
  (those were recursa-old handwritten parsers; this runtime is fully
  derived).

Two levers the menu did not name, both surfaced by the profiles, are the
ones worth adopting:

- **ADOPT — FOLLOW-set allocation removal (rank 1/3/4).** Options, all
  recursa-side: intern the 749 static union *results* at build time
  (operands repeatedly compose the same constant pair); make the Pratt
  guard chain lazy (build only when `next_is_pratt_boundary`, since the
  guard is consulted only on the rare ambiguity error); return
  borrow-or-static FOLLOW so `with_caller` of a constant caller allocates
  nothing.
- **ADOPT — per-statement lexer construction sharing (rank 2/5).**
  recursa-core `lex()` should compile the `&'static` rule table (and its
  regex_automata caches) once and reuse it, instead of rebuilding 106
  regexes per call. Priced directly below.

Priced against the matrix above and the profiles, largest expected win first:

1. **Share the compiled lexer-matcher table across `lex()` calls**
   (recursa-core). Measured 15.0× on corpus in isolation, collapsing the
   headline realistic-SQL gap from 230× to 15.4×. This is the single largest
   lever and touches only lexer construction, not parse semantics — compile
   the `&'static` rule table and its regex_automata caches once and reuse.
   Predicted: closes most of the parity gap on multi-statement input; the
   parity gate is decided here.

2. **Remove FOLLOW-set allocation on the hot dispatch path** (recursa-core +
   recursa-codegen). Dominates the two single-statement, parse-bound
   workloads (`FollowSet::union` 27–40% self in Track T's profiles; ranks
   1/3/4 in the cost table). After lexcache lands, this is what moves
   bool_chain/select_list from ~21–29× toward parity: intern the 749 static
   union results at build time, make the Pratt guard chain lazy, and return
   borrow-or-static FOLLOW so a constant caller allocates nothing.
   Predicted: the primary remaining lever once lex construction is amortized.

3. **Provenance layer — reject as a target.** ~1% across the matrix; it is a
   measurement instrument, not an optimization target, and confirms ADR
   0006's model decision holds. Do not spend Track R effort here.

Handoff to Track R (#33): land lever 1 first (recursa-side lexer-matcher
sharing, re-pin pg-sql, re-run the corpus workload — expect the parity gate
to fall on realistic SQL), then lever 2. Every wave journals the
canonical-workload delta per ADR 0006.
