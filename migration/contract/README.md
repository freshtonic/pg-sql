# Grammar migration inventory contract

Run `cargo run -p pg-sql-migrate -- inventory .`. The command writes the
structured inventory to stdout and performs no filesystem writes. Redirecting
stdout is an explicit caller action, not inventory behaviour.

The canonical report is checked in as `inventory.json`. Verify it without
writing files with `cargo run -p pg-sql-migrate -- inventory . --check
migration/contract/inventory.json`. To accept an intentional reviewed contract
change, redirect the ordinary stdout command to that path and review the diff.

The report is ordered by source path and byte offset. Every source fact has a
stable qualified ID and a one-based line/column plus a half-open UTF-8 byte
span. `Mapping::migration_contract` is the single executable source of the
reviewed legacy counts; the canonical report renders those results for review.
New, removed, or reclassified constructs fail inventory instead of silently
changing the contract.

Semantic rows cover each attributed grammar type, enum variant, and field.
`semantic.same-shape` preserves a semantic value under its qualified name.
`syntax.fixed-token` explicitly removes only bare keyword and punctuation
fields because they encode accepted syntax rather than semantic information;
`syntax.fixed-token-container` recursively applies the same exclusion to
tuples and wrappers with no semantic payload. The enclosing enum variant keeps
any semantic choice made by such syntax-only fields.
Optional fixed tokens use a fail-closed table keyed by full qualified field ID.
Reviewed PostgreSQL grammar filler such as optional `WITH`, `AS`, `=`, and
`COLUMN` is a syntax-only exclusion. Behavior-affecting presence becomes a
named boolean or domain enum; a new unreviewed ID makes inventory fail instead
of inheriting a field-name policy. The same rule covers optional fixed-token
groups and fixed tokens nested beside semantic payload. `Seq0` maps to `Vec`,
`Seq1` to `vec1::Vec1`,
separators and optional trailing separators move to `#[sep(..., trailing)]`,
and `Surrounded` delimiters move to `#[surrounded(open, this, close)]`.

The test section separately records literal and macro-expanded tests, ignored
tests, differential membership and exclusions, file-recovery fixture sites,
formatter pairs, stress workloads, and benchmark sources. `obsolete_artifacts`
is the reviewed exclusion list. `recursa_gaps` links each mandatory design
session to minimized examples under `migration/gaps`.

`tests.literal_tests` is the pinned immutable legacy pg-sql contract.
`tests.workspace_members` is discovered from the root workspace membership and
separately records post-bootstrap tests such as pg-oracle and migration-tool,
without changing the legacy count. The migration contract pins the exact
workspace member set plus each member's inclusive test count and ignored-test
subset, so adding or removing a member or test requires explicit review.

The source-backed token inventory contains 99 punctuation declarations. A
textual search can appear to find 100 because a comment contains the `=>`
spelling; the token-tree parser deliberately counts declarations, not text.
