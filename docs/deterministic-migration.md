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
`migration-tool/tests` are the permanent executable proof. This machinery does
not authorize the full grammar migration. The next port-plan step is the
mandatory PostgreSQL lexical and grammar expressiveness grilling session.
