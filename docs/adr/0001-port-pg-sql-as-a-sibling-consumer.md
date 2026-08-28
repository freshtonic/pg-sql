# Port pg-sql as a sibling consumer

Status: accepted

Port `pg-sql` into a fresh repository beside Recursa rather than adding it to
the Recursa workspace. The new repository depends on the unpublished
`recursa` and `recursa-codegen` packages through paths into that sibling
checkout, while the legacy `recursa-old/pg-sql` tree remains an immutable
migration input. This keeps a large language implementation out of the parser
generator's release and CI boundary, at the cost of requiring a fixed sibling
checkout layout in development and CI.

The port preserves PostgreSQL language coverage, semantic AST information,
formatting behaviour, and oracle-backed conformance rather than Rust source or
AST API compatibility. Missing general parser-generator capabilities may be
added to Recursa only after their own design sessions; the port must not bypass
Recursa with `pg-sql`-specific handwritten parsing exceptions.
