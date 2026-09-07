# Handoff: the table-driven parser

Written 2026-09-06. Everything below is **local and unpushed**.

The goal, from `docs/research/postgres-parser-performance.md` section 8: pg-sql's
recursive-descent parser paid about 7.5x PostgreSQL per nesting level, which is a
property of the technique, not a defect. Recursa gained a table-driven LALR(1)
parser style in bison's shape; pg-sql now selects that style exclusively.

The design is settled and written down. Read these three before anything else:

- `../recursa/docs/table-driven-parsing.md` — the full design, decision by decision.
- `../recursa/docs/adr/0004-lalr-tables-for-the-table-driven-parser-style.md`.
- `tests/table_parser.rs` — the focused table-driven smoke gate. The PostgreSQL
  differential suite is the exhaustive acceptance gate.

## State

| | |
|---|---|
| recursa dependency | `.recursa-revision` pins `81dca1e` |
| recursa validation | complete codegen integration suite passed |
| pg-sql differential | **234 passed / 0 failed** |
| LR conflicts | **0**, from 9,643 |
| parser declaration | `parser_style = table_driven` — pg-sql emits no recursive-descent parser |

The two repos are coupled by a path dependency and by `.recursa-revision`, which
now pins `81dca1e`. **They must be pushed together**; pushing one alone breaks CI.

## What is done

All of recursa is done and merged: issues #111–#128 and #130. That is the whole
table-driven style — LALR(1) construction, comb-vector tables, the runtime loop and
value stack, containers, Pratt lowering to precedence, the lookahead filter,
LAC error paths, provenance, named parsers, a `precedence { }` block mirroring
gram.y's, and the twin-fixture gate that parses every example under both parsers.

pg-sql resolved #68, taking conflicts 9,643 → 522 → 33 → 31 → 23 → 17 → 0,
plus the psql separation. The final differential is 234/234.

## Resolution

**12 — recursa #132. Resolved with scoped declarations.**
gram.y writes every position admitting both a parenthesised query and a
parenthesised expression as two alternatives that each own their `(`
(gram.y:16715, 15391, 13492, 15152), and keeps `%expect 0` through token precedence
(`%left '(' ')'` at gram.y:898 outranking `%right UMINUS` at 896; rationale at
12658). Written that way in pg-sql, **recursive descent** rejects it with `RCA0200`,
an unresolved shared prefix that balanced dispatch cannot separate because both
alternatives close on the same `)` with identical residuals. Taking the remedy
`RCA0200` names produces the shape that costs the 12 LR conflicts.

This is the same tension as recursa #129: **recursive descent and LALR want
different grammar shapes, and one grammar has to serve both.** #129 tried to solve
it with `#[parse(same_as = T)]`; that was implemented, proved structurally
incapable, and deliberately **not merged** (branch `table-driven/129-shared-nonterminal`
in `../recursa`, kept for the record). Do not resurrect it without a use case —
the proof is on issue #129.

**Decision:** add scoped LR conflict resolutions, recorded in
`docs/adr/0008-use-scoped-lr-conflict-resolutions.md`. It must select conflicts
by stable grammar-source identities and lookaheads, never generated state
numbers; declare the intended action or reducing source rule; match exactly the
conflicts it claims; and appear in the automaton dump. The 12 cells are ten
shift/reduce and two reduce/reduce conflicts. Stale, missing, ambiguous or over-broad
declarations fail the build. Every unresolved conflict remains a hard build
error.

This was chosen over `#[parse(backtrack)]`, whose bounds and rollback semantics
are not yet defined for this arbitrarily nested prefix, and over extending
balanced dispatch to discriminate inside the enclosure, for which no algorithm
or complexity bound exists. An AST rearrangement does not remove the recognition
problem: it either recreates `RCA0200`, admits two derivations, or requires the
much larger parser-specific-CST architecture.

**3 — `func_arg_list`. Resolved.** Merging `FunctionCallTail`'s variants would share every
nonterminal to `')'` as gram.y does, but pg-sql's `body` is `FunctionCallBody`, so
typed literals would then admit `*`, `DISTINCT`, `ALL`, `VARIADIC`, which gram.y
rejects grammatically (`AexprConst`, gram.y:17231). Over-acceptance bought for 3
conflicts that cannot reach zero under the prior shape.

**1 — `SET SESSION . CHARACTERISTICS`. Resolved.**

**1 — `UESCAPE`. Resolved.** The table-driven lexer/parser now preserves the
PostgreSQL scanner's continuation boundary without admitting an invalid sequence.

## Next steps, in order

1. Keep the table-driven smoke gate and the 234/234 PostgreSQL differential green.
2. **pg-sql #70** — benchmark the table-driven parser. The gate is in
   `../recursa/docs/table-driven-parsing.md` section 1: geomean pg-sql/PostgreSQL
   at or below 1.0, and nesting cost within 1.5x of PostgreSQL.
3. Push both repos together.

Recursa still supports recursive-descent grammars and checks their predictive
ambiguities when that parser style is selected. pg-sql does not select or test a
second parser style. Its table-only generation omits recursive-descent predictive
FOLLOW statics, including `__RECURSA_EMPTY_FOLLOW` and
`__RECURSA_EXPECTED_SETS`.

## Measurements so far

The grammar restructuring done for LALR-cleanliness made the **recursive-descent**
parser faster. Run `2026-09-06T07-01-19Z-effe2aa` on an idle machine, over the 236
benchmarks common to the last three runs (`docs/notes/perf.md`, Track T):

| Run | pg-sql | PostgreSQL | geomean vs PostgreSQL | geomean vs sqlparser |
|---|--:|--:|--:|--:|
| `7522830` | 144.7 ms | 39.3 ms | 3.421x | 0.681x |
| `effe2aa` | 140.8 ms | 39.7 ms | **3.302x** | **0.671x** |

Nesting: 1.754 → 1.683 us per level, still about 7x PostgreSQL. These measurements
are recursive-descent-only historical baselines. The table-driven parser is now
generated but has not yet been benchmarked.

Read the ratio, not the totals: the PostgreSQL control moved 1.0% between runs,
marginally above the 0.88% same-commit spread, so the machine was slightly slower
while pg-sql got faster.

## Working notes for whoever resumes

- **Fast conflict loop** (about 45 s, no pg-sql build):
  ```sh
  cd ../recursa && unset RUSTUP_TOOLCHAIN
  cargo test -p recursa-codegen --release --test lr_pg_sql --locked \
    pg_sql_grammar_constructs_and_reports_its_conflicts -- --ignored --exact --nocapture
  ```
- **Always `unset RUSTUP_TOOLCHAIN`** first. The session env forces stable; both
  repos pin a nightly and need it.
- **Never run `cargo clippy`** on these repos — it takes about an hour.
- **The benchmark command in the README does not work** (pg-sql #65). Use:
  `cargo bench -p pg-sql --features postgres-oracle --bench parse`.
- `cargo fmt --all` inside pg-sql **reformats the sibling recursa checkout** (47
  files). Pre-existing trap, unfixed.
- Commit signing: if a commit fails to sign, the ssh-agent is empty. Run
  `export SSH_AUTH_SOCK=$(find /private/tmp -maxdepth 2 -type s -name Listeners | head -1)`
  and retry. Never `--no-gpg-sign`.
- `docs/benchmarks/` is gitignored; reports stay local.
- Adding a recursa diagnostic code needs the numeric ranges in
  `evidence_schema` and `remedy_schema` (`build_diagnostic.rs`) extended, not just
  a registry row. A code outside them silently renders a user's grammar mistake as
  an internal Recursa failure. This bit twice (RCA3212, RCA0406); there is now a
  guard test, `only_internal_codes_carry_the_internal_invariant_schema`.

## Open issues filed along the way

recursa **#129** (rejected, branch parked), **#131** (closed matchers cannot model a
region running to end of input — psql tolerates EOF inside a string, recursa does
not; fails closed), **#132** (the blocker above).

pg-sql **#68** (resolved), **#69** (superseded by the table-only surface),
**#70** (benchmark pending).

## Stale worktrees

Left from earlier work, unrelated: `../recursa-perf-r2`, `../recursa-wt-first`,
`../pg-sql-wt-68`, `../pg-sql-wt-first`, `../pg-sql-wt-parity`, and four under
`.claude/worktrees/`. `../recursa-wt-129` is the parked #129 branch — keep until
#132 is decided.
