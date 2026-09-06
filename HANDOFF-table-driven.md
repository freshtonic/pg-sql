# Handoff: the table-driven parser

Written 2026-09-06. Everything below is **local and unpushed**.

The goal, from `docs/research/postgres-parser-performance.md` section 8: pg-sql's
recursive-descent parser pays about 7.5x PostgreSQL per nesting level, which is a
property of the technique, not a defect. So recursa gained a second parser style
that generates an LALR(1) parser in bison's shape, and pg-sql is being made
LALR-clean so it can use it.

The design is settled and written down. Read these three before anything else:

- `../recursa/docs/table-driven-parsing.md` — the full design, decision by decision.
- `../recursa/docs/adr/0004-lalr-tables-for-the-table-driven-parser-style.md`.
- The module header of `tests/two_parsers.rs` in this repo — **the authoritative,
  in-tree record of every remaining LR conflict and what each group needs.** It is
  kept current and it is the first thing to read when resuming.

## State

| | |
|---|---|
| recursa `main` | `08daf79`, **73 commits ahead of origin** |
| pg-sql `main` | `0fab429`, **36 commits ahead of origin** |
| recursa suite | 885 passed, 0 failed |
| pg-sql differential | **231 passed / 3 failed** — the acceptance bar, unchanged all the way through |
| LR conflicts | **17**, from 9,643 |
| `parsers(rd, lr)` declared | **No.** recursa refuses to generate with any conflict |

The two repos are coupled by a path dependency and by `.recursa-revision`, which
now pins `08daf79`. **They must be pushed together**; pushing one alone breaks CI.

## What is done

All of recursa is done and merged: issues #111–#128 and #130. That is the whole
table-driven style — LALR(1) construction, comb-vector tables, the runtime loop and
value stack, containers, Pratt lowering to precedence, the lookahead filter,
LAC error paths, provenance, named parsers, a `precedence { }` block mirroring
gram.y's, and the twin-fixture gate that parses every example under both parsers.

pg-sql has had four passes on #68, taking conflicts 9,643 → 522 → 33 → 31 → 23 → 17,
plus the psql separation. Every pass held the differential at 231/3.

## The remaining 17 conflicts

**12 — recursa #132. The blocker, and it needs a decision, not just work.**
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

Options worth weighing, none yet chosen:
1. Give recursive descent a bounded committed-try at these sites — the
   `#[parse(backtrack)]` valve `../recursa/docs/api-design.md` has reserved since
   the start and nobody has built. Costs RD speed at those sites only.
2. Teach balanced dispatch to separate alternatives whose residuals differ only
   after the closing token.
3. Let a grammar declare a per-site conflict resolution, bison's `%expect` narrowed
   to one state. Retreats from ADR 0004's "every conflict is a hard build error".

**3 — `func_arg_list`.** Merging `FunctionCallTail`'s variants would share every
nonterminal to `')'` as gram.y does, but pg-sql's `body` is `FunctionCallBody`, so
typed literals would then admit `*`, `DISTINCT`, `ALL`, `VARIADIC`, which gram.y
rejects grammatically (`AexprConst`, gram.y:17231). Over-acceptance bought for 3
conflicts that cannot reach zero anyway. Left deliberately.

**1 — `SET SESSION . CHARACTERISTICS`.** Described in the in-tree note.

**1 — `UESCAPE`.** Not expressible in an LALR grammar that sees the two tokens:
`UESCAPE` is a bare-label keyword (gram.y:18474) so it may legitimately follow the
literal, and `scan.l` merges them. Probably permanent.

## Next steps, in order

1. **Decide recursa #132** (the three options above). It gates everything.
2. Implement it; conflicts should fall to about 5.
3. Decide whether the last 5 are acceptable, or press on to zero.
4. At zero: declare `parsers(rd = recursive_descent, lr = table_driven)` in
   `src/lib.rs` (rd first — a table-driven default beside framing or the explorer
   is refused by design), and enable `tests/two_parsers.rs`, which is behind the
   `table-driven` cargo feature and has never been able to compile.
5. **pg-sql #69** — the style equivalence gate: run the differential through both
   parsers and fail on any disagreement in value, provenance or failing token.
6. **pg-sql #70** — benchmark the table-driven parser. The gate is in
   `../recursa/docs/table-driven-parsing.md` section 1: geomean pg-sql/PostgreSQL
   at or below 1.0, and nesting cost within 1.5x of PostgreSQL.
7. Push both repos together.

## Measurements so far

The grammar restructuring done for LALR-cleanliness made the **recursive-descent**
parser faster. Run `2026-09-06T07-01-19Z-effe2aa` on an idle machine, over the 236
benchmarks common to the last three runs (`docs/notes/perf.md`, Track T):

| Run | pg-sql | PostgreSQL | geomean vs PostgreSQL | geomean vs sqlparser |
|---|--:|--:|--:|--:|
| `7522830` | 144.7 ms | 39.3 ms | 3.421x | 0.681x |
| `effe2aa` | 140.8 ms | 39.7 ms | **3.302x** | **0.671x** |

Nesting: 1.754 → 1.683 us per level, still about 7x PostgreSQL. Only the
table-driven parser will move that number, and it has **not been measured yet** —
it cannot be generated until the conflicts reach zero.

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

pg-sql **#68** (open, this work), **#69**, **#70** (both blocked on #68).

## Stale worktrees

Left from earlier work, unrelated: `../recursa-perf-r2`, `../recursa-wt-first`,
`../pg-sql-wt-68`, `../pg-sql-wt-first`, `../pg-sql-wt-parity`, and four under
`.claude/worktrees/`. `../recursa-wt-129` is the parked #129 branch — keep until
#132 is decided.
