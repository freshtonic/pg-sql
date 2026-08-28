# pg-sql Development Guidelines

## Principles

1. **NEVER manually implement Parse/Scan/Visit/FormatTokens/Debug** They MUST be derived. When encountering a piece of SQL syntax that seems to make derivation with `recursa` impossible STOP what you are doing, explain the problem and ask for feedback.

2. **All AST nodes MUST derive these traits: Debug, Clone, FormatTokens, Parse, Visit, PartialEq, PartialOrd, Hash**

3. **All AST nodes MUST derive a Arbitrary but behind a feature gate `#[cfg_attr(feature = "arbitrary", Arbitrary)]`**

4. **Use method syntax, not UFCS.** Write `T::parse(input, rules)` not `<T as Parse>::parse(input, rules)`.

5. **Test against real Postgres.** Use testcontainers for regression tests. Each test gets a private Postgres 17 instance.

6. **Grow the grammar incrementally.** Each new test file drives new token/AST additions. Don't build grammar that isn't tested.

7. **Newtype-style AST nodes MUST implement `Deref` for the type that they wrap**