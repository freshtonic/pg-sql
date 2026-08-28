# Make the pg-sql grammar port reproducible

Status: accepted

Bootstrap the new `pg-sql` repository from the `pg-sql` and `pg-oracle` Git
objects at legacy Recursa commit `1e71421`, rather than copying its working
tree, and preserve that untouched import as the first source commit. Recreate
the PostgreSQL submodule at the legacy PostgreSQL 17.9 gitlink. The legacy
repository is an immutable input whose commit and tree identities are verified
before and after migration.

Keep an auditable migration tool in the new repository. Its inventory mode
reports exact source constructs and unsupported cases without changing files;
its rewrite mode applies span-based edits that preserve comments and source
ordering, writes only to the new repository, and either completes the full
deterministic grammar transformation or publishes no partial result. Known
Recursa capability gaps are designed and implemented before the final rewrite
rather than represented by generated placeholders or handwritten parser
exceptions.
