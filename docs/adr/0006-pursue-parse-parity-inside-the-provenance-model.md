# 6. Pursue parse-time parity inside the provenance-bearing model

Date: 2026-09-01

## Status

Accepted

## Context

The interim statement-level benchmark measures pg-sql at roughly 235x slower
than sqlparser 0.52 (summed per-bench medians), while the retired
`recursa-old/pg-sql` finished at a 0.525x geomean against the same sqlparser
on 240 benchmarks. The old crate reached that point through a documented
performance arc (its `docs/notes/perf.md`): a logos lexer with a classifier
cache that avoids re-classifying identifier and keyword tokens, kind-match
dispatch throughout enum and Pratt selection, a fused single-pass trivia
scan, error-path allocation removal, and bounds-check elision - all
profile-driven. The current runtime already lexes through logos and
dispatches through packed kind tables, but its parsed values carry
provenance: spans, occurrence records, and element metadata that the old
plain-struct AST never paid for.

## Decision

- The performance target is old parity: a geomean at or below 1.0x sqlparser
  on the interim statement-level benchmark, re-affirmed on the Criterion
  suite when it lands (#20). The reference ceiling is the old crate's 0.525x.
- The provenance-bearing value model stays. Optimization happens inside the
  current model; no second construction backend and no flat AST are part of
  this effort.
- Work is strictly profile-first. The old repository's technique arc is a
  ranked candidate menu, not a plan; every candidate is validated against a
  fresh profile of the current runtime before implementation, honoring the
  old journal's own warning that stale hot-path assumptions burn time.
- Grammar generation stays in the build script. The build stage is improved
  by algorithmic work on measured hot spots only; no persistent caching or
  incremental-analysis machinery in this effort.
- The effort runs parallel to the port sequence; it does not gate or pause
  #10 and its successors.

## Consequences

- pg-sql adopts the old repository's performance discipline: a ported flame
  harness, a running perf journal, and named canonical workloads (corpus
  head-to-head, select_list_10000, bool_chain).
- The benchmark run must fit iteration: wall time at or under ten minutes via
  probe caching keyed to the frozen baselines and tuned iteration budgets,
  without changing what is measured.
- If profiling shows the provenance model's intrinsic floor sits above
  parity, that finding returns as an explicit decision (revisit this ADR)
  rather than a silent target miss.
