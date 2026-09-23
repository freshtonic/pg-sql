# Select one target version per build with mutually exclusive features

Status: accepted

pg-sql supports PostgreSQL 14 to 19. Each build selects exactly one target
version with a version feature: `pg14`, `pg15`, `pg16`, `pg17`, `pg18` or
`pg19-beta`. The default is `pg17`. The features are mutually exclusive: a build
with zero, or with two or more, stops with `compile_error!`. This is not the
usual Cargo practice, where features are additive. We use exclusive features
because the versions are not additive. PostgreSQL removes syntax (`RECHECK` in
18), changes keyword categories (`json` in 17), and changes lexing (trailing
junk after numbers in 15). So one grammar cannot accept every version.

The alternative was to compile all six grammars into one crate and select one
at run time (`pg_sql::v16::parse`). We rejected it. Each generated grammar is
about 25 MB, and the LR generation in `build.rs` already controls the edit
loop, so six grammars would cost six times as much in both. The cost of our
choice is feature unification: if two crates in one dependency graph select
different versions, the build fails. The `compile_error!` message names this
cause. A crate between a consumer and pg-sql (`pg-psql` now) declares
`default-features = false` on its `pg-sql` dependency and forwards the version
features.

## Consequences

- A version gate is a `cfg` on an AST node, field, variant, lexer rule, keyword
  entry or scoped LR resolution. It uses cumulative helper features
  `since-pg15` … `since-pg19`: "added in N" is `feature = "since-pgN"`, and
  "removed in N" is `not(feature = "since-pgN")`. recursa-codegen reads Cargo's
  `cfg` values, so the generated grammar matches the selected version.
  (ADR 0010 replaces the `cfg` by a `#[config]` declaration over the same
  features, for every gate that recursa sees. The features do not change.)
- The AST type differs between builds. Changes are additive where possible, so
  consumer code for an older version usually compiles with a newer target
  version.
- Each version is pinned to its latest minor release (a named commit for 19
  until 19.0). The oracle, the differential baseline and the test ledger exist
  once for each target version. The merge gate builds and tests all six.
- pg-psql follows the same target version, so the psql document and its SQL
  always come from the same release.
- Version parity means the raw parser only. Changes that execution or parse
  analysis enforce are out of scope. So is `standard_conforming_strings = off`:
  every version lexes as if the setting is on, which is also the only mode in 19.
