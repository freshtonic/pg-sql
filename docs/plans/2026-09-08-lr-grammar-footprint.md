# LR grammar and table-footprint plan — 2026-09-08

## Goal

Reduce the work and cache footprint of pg-sql's generated LR parser while
preserving every public Node parser, the typed arena AST, exact-source
provenance, diagnostics, and PostgreSQL parity. Matching Bison's raw counts is
not itself the goal; each structural reduction must improve measured parse
performance on the workload families that exercise it.

The investigation supporting this plan is in
[`docs/research/postgres-parser-performance.md`](../research/postgres-parser-performance.md#9-why-the-current-lr-machine-is-larger-than-bisons).

## Baseline and attribution

| Dimension | PostgreSQL/Bison | pg-sql/Recursa |
| --- | --: | --: |
| Nonterminals | 728 | 4,279 |
| Rules | 3,408 | 14,944 |
| States | 6,458 | 20,715 |
| Packed slots | 123,278 | 262,296 |

Three mechanisms dominate Recursa's excess:

1. Flattening 26 overlapping content-token atom sets creates 5,469 rules and
   6,578 token-only states. Bison shares its keyword categories in 960 rules.
2. Twenty-two Expr restrictions clone 2,205 expression rules; restriction
   kernels exclusively own 2,415 states.
3. The supported public API creates 1,504 parse roots. Start kernels
   exclusively own 3,007 states, and a Statement-only counterfactual has
   16,245 states instead of 20,715.

The 196 shared container helpers, one conflict-driven helper copy, and 120
lookahead-filter rules are too small to explain the gap. They are not initial
optimization targets.

## Measurement foundation

Before changing lowering, add a stable Recursa analysis report replacing the
temporary diagnostic instrumentation used for this investigation. It should
report, in machine-readable form:

- normalized type shapes and materialization roles;
- nonterminal and rule counts by `NonterminalOrigin` and `RuleAction`;
- states by exclusive/mixed kernel origin;
- packed table lengths and decoded byte widths;
- counts for each content-token atom set and Pratt restriction;
- the reachable state set for each public start, without rebuilding the
  automaton.

Commit a pg-sql snapshot of these statistics. Treat unexpected growth as a
review signal rather than a hard universal limit; small grammars may have
different proportions.

Use the quick flame suite for every candidate and the relevant discovery
members for attribution. Run PostgreSQL through the same harness when the
change targets parser-machine behavior. Record shift, reduction, semantic-
action, and token count alongside time so a smaller table is not mistaken for
less runtime work.

## Phase 1 — preserve shared lexical atom structure

### Prototype

Stop flattening every multi-kind content token independently. Lower the atom
DAG into reusable token-category nonterminals, then let content-token wrappers
refer to those categories while retaining the originally shifted token as the
semantic value and provenance leaf.

The first prototype should target only content-token atoms with more than one
kind. Fixed one-kind tokens must remain direct terminals. Intern identical atom
expressions and reusable unions; do not create a category merely because two
sets happen to overlap partially.

An alternative prototype may use terminal-class transitions in the table
runtime rather than category reductions. Compare it explicitly: categories
shrink tables but can add reductions, whereas terminal-class transitions add
runtime decision machinery.

### Predictions and gates

- Content-token rules should fall from 5,469 toward Bison's roughly 960,
  putting total rules near 10,500 before later phases.
- Token-only states should fall materially from 6,578.
- `corpus_*`, `lexical_mix_1000`, and alias-heavy query inputs should improve;
  no quick-suite workload may regress beyond run-to-run noise.
- All public token parsers must still return the precise original token, span,
  and occurrence provenance.

Retain only the design that reduces both table footprint and median runtime.

## Phase 2 — separate hot roots from the full public-root surface

The public API cannot lose its 1,504 parsers. Instead, prototype a compact
table view for `Statement` and other declared hot roots:

1. Compute the states reachable from the root's existing start state.
2. Renumber and repack only those states and the rules they reference.
3. Route that root's generated `parse` implementation to the compact tables.
4. Keep the complete table for every other public Node parser.

First measure whether the duplicate static data and binary size are justified.
The Statement-only automaton is 21.6% smaller in states, but unreachable pages
in the full table may already be cold. Require an instruction-cache/data-cache
or wall-time improvement, not just a lower count. A retained prototype must
improve aggregate corpus throughput by at least 2% without more than a 10%
increase in release binary size.

Also give one-token content parsers a direct checked-token path if Phase 1
leaves them as LR roots; this can remove 26 start pairs without affecting Node
parsers.

## Phase 3 — share Pratt restriction machinery

The 22 Expr languages differ only by excluded variants and minimum binding
power, but currently copy 2,205 rules. Prototype table overlays:

- build the unrestricted Expr rule set once;
- assign stable rule families to Pratt variants;
- represent each restriction as an allowed-rule/action mask plus its minimum
  binding power;
- share identical LR cores where their transitions are equal, retaining
  restriction-specific action rows only where lookahead behavior differs.

Do not move precedence decisions into handwritten pg-sql code. The solution
must be a general Recursa lowering/table feature with conflict diagnostics as
strict as the current cloned grammars.

Start with the 90 one-rule Expr attachment restrictions: they are a bounded
test of identity sharing before changing recursive Expr states. Then address
the 22 full Expr restrictions. Target fewer than 500 restriction rules and a
material reduction from 2,415 restriction-only states. Validate primarily on
`bool_chain`, `in_list_10000`, nested queries, and the query corpus family.

## Phase 4 — compact table and runtime representations

After structural reduction stabilizes the ranges, choose the narrowest safe
storage per table:

- states and rules fit in 16 bits at the current baseline;
- packed offsets and slot indices exceed 16 bits and remain 32-bit;
- actions should use a tagged 16-bit state/rule payload when the final maxima
  permit it;
- preserve a generated-width fallback for larger grammars.

Measure decoded bytes, startup decoding, cache misses, and parse throughput.
Do not optimize encoded source literals while decoded hot tables remain the
larger cost.

## Phase 5 — reconsider typed structural factoring only with runtime evidence

The remaining 1,983 sequence nonterminals are accounted for by 1,026 public
Node applications, 746 attachment units, 87 presence envelopes, and 124 fixed
sequences. These encode the supported typed AST and provenance model, so they
are not presumed waste.

After Phases 1-4, use reducer and state coverage from the discovery suite to
identify any attachment/envelope family that is both frequently executed and
semantically pass-through. Fuse only such proven cases in general Recursa
lowering, with exact-source and diagnostics tests. Avoid AST-specific pg-sql
exceptions.

## Required validation for every phase

- `cargo +nightly test --workspace --all-features`
- `cargo +nightly doc --workspace --all-features --no-deps`
- PostgreSQL differential and accepted-gap baselines unchanged
- generated grammar-statistics snapshot reviewed
- alternating quick-suite measurements against a frozen baseline
- relevant discovery-family profiles and pg-sql/PostgreSQL ratios recorded in
  `docs/notes/perf.md`

The order is deliberate: shared lexical categories attack the largest proven
duplication without changing the public API; root specialization is isolated
behind existing start identities; Pratt sharing is higher risk because it can
change accepted languages; typed structural factoring comes last because the
current evidence says it is mostly intentional.

## Execution record

Execution on 2026-09-08 and 2026-09-09 retained the measurement foundation and rejected
three performance prototypes at their stated gates:

- **Measurement foundation — retained.** `LrAnalysis::statistics` now exposes
  deterministic normalized-shape, materialization-role, origin, action,
  kernel, table, content-token, Pratt-restriction, and per-start reachability
  data. [`docs/metrics/lr-statistics-v1.txt`](../metrics/lr-statistics-v1.txt)
  is the pg-sql snapshot.
- **Phase 1 — rejected.** Reusable multi-kind token categories reduced rules
  from 14,944 to 11,986 and states from 20,715 to 16,651, but enlarged the
  packed action/check vectors enough to grow encoded tables 2.3%. Alternating
  frozen-binary runs showed corpus and boolean-chain regressions of roughly
  1-3%; the extra category reduction on each token outweighed the smaller
  machine.
- **Phase 2 — rejected.** A correctly renumbered Statement-reachable table had
  16,245 states but still occupied 1,742,380 encoded bytes, 94.9% of the
  complete table's 1,835,940 bytes. Retaining both would nearly double static
  table data, far outside the 10% binary-size gate.
- **Phase 3 attachment precondition — rejected as immaterial.** The grammar
  has 96 restricted attachment rules but 93 distinct right-hand-side shapes;
  only four `DEFAULT restricted-Expr` rules are identical, putting the
  absolute upper bound for exact attachment interning at three rules. The
  material opportunity is therefore the 22 full Expr languages (2,205 rules),
  which requires a genuine masked-action/core overlay rather than helper
  interning. No acceptance-risking partial overlay was retained.
- **Phase 4 — rejected.** Lossless 16-bit action/check storage with automatic
  32-bit fallback halved those decoded cells, but the per-lookup width branch
  made corpus about 0.4% slower, `bool_chain` 0.9% slower, and
  `select_list_10000` 2.5% slower in alternating three-second runs. A second
  version selected the width once and monomorphized the loop: binary size grew
  only 0.13% and corpus/wide results were neutral, but two boolean-chain pairs
  still regressed 2.7% and 1.7%, so it too was removed.
- **Phase 5 — no candidate.** Current discovery profiles attribute hot
  semantic reductions to real list extension and expression construction,
  not pass-through attachment or presence-envelope families. Structural
  factoring is deferred until runtime coverage identifies a hot, semantically
  transparent family.

The failed prototypes were removed. A future Pratt-overlay implementation
should begin by assigning each base Expr production a stable family identity,
then prove that restriction-specific LR(0) kernels can share transitions while
keeping separate lookahead actions. It should not encode a restriction check
in every parser action: the Phase 4 result shows that even a predictable hot
lookup branch can erase a substantial footprint gain.

### Final validation

- Recursa's exhaustive workspace tests, strict authored-code Clippy gate,
  documentation-policy tests, and private-item Rustdoc check pass. The
  representative pg-sql analysis still reports 598 terminals, 4,279
  nonterminals, 14,944 rules, 20,715 states, and no conflicts.
- pg-sql formatting, workspace/all-target/all-feature Clippy, denied-warning
  Rustdoc, and the complete `pg-sql` plus `pg-psql` test suites pass. The
  statistics snapshot is byte-identical to a fresh analysis.
- The full pg-sql workspace test run reaches `pg-sql-migrate` and then fails
  seven existing migration-replay tests: the frozen grammar manifest digest
  differs from the live digest, and the frozen AST lacks the live
  `FunctionWithinGroupBody::Star` destination. These migration fixtures are
  outside this performance plan; parser, differential, document, embedded,
  formatting, lexer, table, and doctest coverage all passed before that point.
