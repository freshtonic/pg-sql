# Handoff: Recursa's LALR parser

pg-sql uses Recursa's one implicit LALR parser. It has no `parser_style`,
`parsers(...)`, or `max_lookahead` declaration; parser selection is no longer
part of the grammar language. The active architecture and migration decision
are recorded in Recursa's [ADR 0005](../recursa/docs/adr/0005-sole-lr-parser.md)
and [parser design](../recursa/docs/table-driven-parsing.md).

## Current gates

- The PostgreSQL differential remains 234/234 with zero LALR conflicts.
- Parsed values, provenance, diagnostics, strict framing, Pretty, and Explorer
  behavior continue through the sole parser.
- Recursa generation must complete in at most 10 seconds from a forced miss,
  reuse a generation in at most 1 second, and finish a release rebuild in under
  60 seconds.

Measured on 2026-09-16 after the declared-precedence migration: a forced
generation takes 7.47 seconds and reuse takes 39 milliseconds. The PostgreSQL
differential passes 234/234 over 43,474 statements and successful generation
reports zero conflicts and zero warnings. The automaton is 17,881 states,
12,070 rules and 3,437 nonterminals, down from 20,191 / 14,333 / 3,659 for the
same grammar before the migration; the snapshot is
`docs/metrics/lr-statistics-v2.txt`.

Measured on 2026-09-07 after the LR-only cleanup, and not re-measured since:
an edit to `recursa-codegen` followed by the pg-sql release rebuild takes 56.66
seconds, and a pg-sql-only rustc pass takes 30.6 seconds.

Conflict resolutions stay source-scoped and must preserve the zero-conflict
report. Do not add a parser fallback or relax conflict checking to accommodate
a grammar fixture; restructure the grammar or write a justified scoped LR
resolution instead.
