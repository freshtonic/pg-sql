# Deterministic migration proof

The migration tool publishes a new tree and never edits the imported legacy
tree. It plans and validates every file before staging the complete result next
to the destination; a single rename publishes the result. Any unsupported
construct, invalid span, unsafe path, symlink, or I/O failure removes the staged
tree and leaves the destination absent.

The grammar and repetitive test-call passes are deliberately separate:

```sh
cargo run -p pg-sql-migrate -- rewrite grammar \
  LEGACY_SOURCE NEW_REPOSITORY/grammar-proof \
  --new-repository-root NEW_REPOSITORY \
  --manifest migration-tool/fixtures/rewrite/grammar/manifest.json

cargo run -p pg-sql-migrate -- rewrite test-calls \
  LEGACY_SOURCE NEW_REPOSITORY/test-call-proof \
  --new-repository-root NEW_REPOSITORY
```

Each destination must not exist. Running a command again from identical source
bytes into another absent destination produces identical file bytes, ordering,
membership, and Unix modes. The grammar manifest names every reviewed rewrite
shape and every explicitly unsupported fixture; its inventory counts are
validated against the checked migration contract before rewriting.

The fixtures under `migration-tool/fixtures/rewrite` and integration tests under
`migration-tool/tests` are the permanent executable proof.

## Reviewed execution

The one-shot grammar migration was executed from pg-sql commit
`b61ff1b85e566950a0675a4d26758430cebb6a92` against Recursa commit
`8ae631142147919eeb3197cb87fe2f4aa0e9a8e3`. The grammar-only and test-call-only
passes were published independently. Applying the test-call pass to the grammar
output produced the canonical tree, and repeating both passes produced the same
SHA-256 digest.

[`migration/execution.json`](../migration/execution.json) records the exact
commands, immutable Git identities, each pass digest, the complete semantic-row
review, all omitted legacy generated/CLI/profiling paths, and the
compile-checkpoint result.
Verify that record against the current published files with:

```sh
cargo run -p pg-sql-migrate -- execution verify \
  --repository-root . \
  --record migration/execution.json
```

Verification resolves the recorded Git objects, requires the immutable
non-publication inputs and detached Recursa checkout to be clean, regenerates
the full canonical inventory from the frozen commit, and directly reruns the
grammar-only, test-call-only, combined, and repeated-combined passes. It then
reproduces the pinned-Recursa compile checkpoint and checks the exact diagnostic
code and count.

The published migrated-payload digest is
`bb71971012f8f17b1068882885fb473556fd6ca6d98da5d0336ea191cb01e039`.
The digest commits to every retained imported pg-sql file's reviewed mode,
relative path, byte length, and bytes; the PostgreSQL gitlink is verified by its
separate recorded Git identity.

The complete publication digest is
`fd1391ba25965635564f6de54b205ac3f18e1f68ab2ade3f7c699ca2a79e8cc6`.
It additionally binds the exact `Cargo.toml`/`src/**` membership, actual Unix
modes, and the reviewed repository-integration addition `build.rs`; unexpected
files, missing files, symlinks, and mode changes fail verification.

The grammar publication rejects immutable inputs that already contain trailing
horizontal whitespace or an extra blank line at EOF. It expands safe
field/item deletions to complete lines and removes only whitespace exposed by
those deletions. `git diff --check -- Cargo.toml src` is therefore clean without
formatting or hand-editing the transformed declarations. The same public
grammar command also removes the exact reviewed Cargo binary blocks whose
source files are omitted, and fails closed if those manifest blocks drift.

The issue-8 compile checkpoint invokes current Recursa generation and reaches
stable discovery diagnostic `RCA1013` at the imported embedded `#[cfg(test)]`
modules (64 occurrences). Issue 9 owns restructuring and reconciling that
complete embedded test inventory while compiling the migrated PostgreSQL
statement grammar. This
checkpoint therefore does not claim strict-statement parity or a compiling root
crate.
