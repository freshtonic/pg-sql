# Performance parity plan — 2026-09-01

Decisions behind this plan are recorded in ADR 0006 and the CONTEXT.md
vocabulary (parity gate, canonical workloads, perf journal). The reference
material is the retired repository at `../recursa-old`: its
`docs/notes/perf.md` journal (1,159 lines, diagnostic sections newest-last),
`docs/next-perf-fixes.md` (parked items), and roughly thirty `perf(...)`
commits.

## Where we are

- Interim statement-level bench (2026-09-01): pg-sql 50,881.7 ms summed
  per-bench medians vs sqlparser 216.1 ms vs PostgreSQL raw parser 40.8 ms;
  33,168 statements timed across all three engines.
- recursa-old final state: 0.525x sqlparser geomean over 240 benchmarks.
- Build stage: the grammar analysis in the build script costs 18-23 s per
  grammar edit before rustc.
- Bench wall time: ~45 minutes, dominated by the three-engine acceptance
  probe over 43,489 frozen statements plus pg-sql's own slowness.

## Tracks

### Track T — tooling foundation

Port the old crate's `flame.rs`/`flame_target` harness onto the current
runtime, create `docs/notes/perf.md` seeded with the current baseline
numbers, and wire the three canonical workloads as named, runnable profiling
targets. Nothing in the other tracks lands without a journal entry naming a
canonical workload and its before/after numbers.

### Track P — profile and attribute

Produce the first complete attribution of the ~235x on the canonical
workloads: flame profiles plus allocation counts, decomposed at least into
lex/classify, dispatch/decision, value construction (provenance: spans,
occurrences, element metadata), error/expected-set paths, and trivia
handling. Deliverable is a ranked cost table in the journal, each row mapped
against the candidate menu below with an adopt/adapt/reject-with-reason
verdict. If value construction's intrinsic floor is shown to exceed the
parity gate, stop and bring the finding back as a decision (ADR 0006).

### Track R — runtime to parity

Strictly profile-first waves. Each wave: pick the top-ranked cost, implement
against the current model, verify the full library suite and the differential
targets stay green, journal the delta on the canonical workloads. The parity
gate closes the track; the old 0.525x is the stretch reference.

Candidate menu mined from recursa-old (validate against fresh profiles; old
commit references in parentheses):

- Classifier cache: avoid re-classifying identifier/keyword tokens between
  peek and parse at the same cursor.
- Kind-match dispatch everywhere a sequential peek cascade remains (arc:
  b2170aa, 886de80, 1fd73fb, 0589199, ceac2ff, 4531a24, 4e5658a).
- Single-pass ASCII trivia scan (8f74f51) and bounds-check elision in the
  input window (b22265c).
- Error-path allocation removal: shared source via Arc (72d7e3c), label
  types as Cow<'static, str> (parked item 4).
- Peek+parse fusion for keyword-led optionals (parked item 3).
- Fork/transaction cost reduction (parked item 5).
- Identifier fast path past the classifier DFA - only with a sound gate
  (the old repo reverted the unsound version; parked with two safe designs).
- HashSet keyword membership (4bd622b); redundant-postcondition removal
  (0484fa4); byte-scan hot leaf parsers (4a4ebf8, 6abefe3).

### Track C — codegen build stage (algorithmic only)

Measured hot spots in the analysis, in current knowledge order: FxHash (or
equivalent) for the decision-state interning maps whose SipHash dominates the
profile; `Vec<DecisionItem>` clone reduction; a cutoff-reachability pre-gate
so open element languages skip FIRST-5 materialization in the caller-overlap
optional-viability path (~4.6 s today). Target: analysis at or under 10 s on
the pg-sql grammar, measured by direct build-script timing. Re-profile after
each change; no caching machinery in this track.

### Track B — bench wall time

Bring `cargo bench -p pg-sql --features postgres-oracle` to ten minutes or
less without changing what is measured: cache the three-engine acceptance
probe keyed to the frozen statement baselines (invalidate on baseline or
engine change), tune per-bench iteration budgets, keep report format and
per-engine exclusion accounting identical. The oracle test suite is
explicitly out of scope (unchanged, off the inner loop).

## Constraints

- No handwritten parser exceptions; every runtime change preserves the
  derivation-only principles and the strict baselines (library suite,
  differential targets, accepted-gap ledger).
- The provenance-bearing value model is not negotiable within this effort.
- Build generation stays in the build script.
- Runs parallel to the port sequence; never gates #10+.

## Gates

- Parity gate: geomean pg-sql/sqlparser <= 1.0 on the interim bench,
  re-affirmed at #20.
- Codegen gate: analysis <= 10 s on the pg-sql grammar.
- Bench gate: full bench run <= 10 minutes.
- Every landed change carries a perf-journal entry with canonical-workload
  numbers; suites stay green throughout.
