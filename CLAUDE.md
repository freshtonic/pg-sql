# pg-sql Development Guidelines

## Principles

1. **NEVER manually implement Parse/Pretty/Visit/VisitMut/Debug** They MUST be derived. When encountering a piece of SQL syntax that seems to make derivation with `recursa` impossible STOP what you are doing, explain the problem and ask for feedback.

2. **All AST nodes MUST derive `recursa::Node, Debug, Clone`.** `Parse`, `Pretty`, `Visit` and `VisitMut` come from the `Node` derive and the root `grammar!` `derives(...)` list, never from a per-node derive. Add `PartialEq`, `Eq`, `Hash` only where a consumer needs them.

3. **`Arbitrary` is a recursa configured trait.** Enable it through the root `grammar!` `derives(...)` list under the `arbitrary` cargo feature; never add per-node `cfg_attr` derives for it.

4. **Use method syntax, not UFCS.** Write `T::parse(input)` not `<T as Parse>::parse(input)`.

5. **Test against the target version's oracles.** `pg-oracle` links the raw parser of the pinned release of the build's target version (`pg-oracle/pins.tsv`), and the differential suite compares pg-sql with it (`--features postgres-oracle`). `pg-psql-oracle` links `psqlscan.l` and `psqlscanslash.l` of the same release, and `pg-psql/tests/psql_oracle.rs` compares pg-psql with it (`--features psql-oracle`, `docs/psql-oracle.md`). Both read the one pin table. There are no testcontainers and no running server. A statement that the raw-parser oracle of the target version accepts must parse; one that it rejects must not. A psql document that disagrees with the psql oracle is an expected psql gap with a recorded class, never an unexplained one.

6. **Grow the grammar incrementally.** Each new test file drives new token/AST additions. Don't build grammar that isn't tested.

7. **Newtype-style AST nodes MUST implement `Deref` for the type that they wrap**

8. **A new embedded test needs three edits, not one.** Adding a test under `embedded-tests/` also requires a row in `embedded-tests/inventory.tsv`, an `introduced` row in `embedded-tests/reconciliation.tsv` (rationale `issue-9-generated-expression` under `shared/expr.tests.rs`, otherwise `issue-9-generated-statement`), and the pinned count in `tests/embedded_inventory.rs` bumped. The inventory row ends with the version range of the test: `14-` without a version gate, `17-` for `feature = "since-pg17"`, `14-16` for `not(feature = "since-pg17")`. The range must agree with the test's `cfg`, and `ROWS_PER_TARGET_VERSION` pins one count per target version: bump each version the test runs for. Every other suite stays green while those ledgers are wrong, so run `cargo test -p pg-sql --test embedded_inventory` before calling the change done. The contract behind the ledgers is `docs/deterministic-migration.md`.

9. **An admission set must mirror the `gram.y` nonterminal it stands for.** Widening one to make a case parse is how both over-acceptance and larger LR tables arrive: a qualifier declared as a label set rather than `ColId` propagated through `Expr`, which heads every target list, and made the select head's FIRST set 485 word kinds wide — `union.a` parsed as a column reference and the affected LR states gained needless actions. When PostgreSQL says `ColId`, write `ColId`. These stay invisible until someone hits an `RCA0400` unresolved shift/reduce conflict or profiles the generated parser.

10. **A crate between a consumer and pg-sql uses `default-features = false` on pg-sql and forwards the version features.** The version features `pg14` … `pg19-beta` are mutually exclusive (ADR 0009), and the default is `pg17`. An intermediate crate that keeps pg-sql's default features adds `pg17` to every build through Cargo feature unification, so a consumer that selects another version gets the "more than one target version" build error. Give the crate its own `pg14` … `pg19-beta` and `since-pg15` … `since-pg19` features (default `pg17`) that forward to pg-sql, and call `build-support/target_version.rs` from its build script if it has one. `pg-psql` is the model.

11. **Write a version gate with the `since-pgN` helpers, and cite its source.** "Added in N" is `#[cfg(feature = "since-pgN")]`; "removed in N" is `#[cfg(not(feature = "since-pgN"))]`. Never gate on a version feature (`pg16`) itself: the helpers are cumulative, the version features are not. Put a comment on each gate that cites its entry in `docs/research/postgres-14-19-sql-syntax-changes.md` (or `docs/research/psql-14-19-syntax-changes.md`), or the `gram.y`/`scan.l` change. A "removed in N" change also gets a negative test. The local edit loop builds `pg17`. The merge gate runs `scripts/gate-versions`, which builds and tests all six versions.
