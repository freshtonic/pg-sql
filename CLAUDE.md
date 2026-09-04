# pg-sql Development Guidelines

## Principles

1. **NEVER manually implement Parse/Scan/Visit/FormatTokens/Debug** They MUST be derived. When encountering a piece of SQL syntax that seems to make derivation with `recursa` impossible STOP what you are doing, explain the problem and ask for feedback.

2. **All AST nodes MUST derive these traits: Debug, Clone, FormatTokens, Parse, Visit, PartialEq, PartialOrd, Hash**

3. **All AST nodes MUST derive a Arbitrary but behind a feature gate `#[cfg_attr(feature = "arbitrary", Arbitrary)]`**

4. **Use method syntax, not UFCS.** Write `T::parse(input, rules)` not `<T as Parse>::parse(input, rules)`.

5. **Test against real Postgres.** Use testcontainers for regression tests. Each test gets a private Postgres 17 instance.

6. **Grow the grammar incrementally.** Each new test file drives new token/AST additions. Don't build grammar that isn't tested.

7. **Newtype-style AST nodes MUST implement `Deref` for the type that they wrap**

8. **A new embedded test needs three edits, not one.** Adding a test under `embedded-tests/` also requires a row in `embedded-tests/inventory.tsv`, an `introduced` row in `embedded-tests/reconciliation.tsv` (rationale `issue-9-generated-expression` under `shared/expr.tests.rs`, otherwise `issue-9-generated-statement`), and the pinned count in `tests/embedded_inventory.rs` bumped. Every other suite stays green while those ledgers are wrong, so run `cargo test -p pg-sql --test embedded_inventory` before calling the change done. The contract behind the ledgers is `docs/deterministic-migration.md`.
