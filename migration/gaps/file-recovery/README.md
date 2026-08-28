# SQL file parsing and recovery gap examples

| Example | Expected behavior | Legacy/current limitation |
| --- | --- | --- |
| `broken-continuation.sql` | Emit a structured failure for statement one, then a statement item for `select 1`. | Strict parsing alone cannot continue after failure. |
| `semicolon-nesting.sql` | Split only at the two top-level semicolons, with exact item spans. | Segmentation must understand strings and nesting. |
| `copy-raw-lines.sql` | Emit statement, COPY raw payload region through `\\.`, then statement. | COPY payload is not PostgreSQL grammar tokens. |
| `psql-directive.sql` | Preserve directive regions and the enclosed statement as ordered file items. | psql directives require source-region capture and recovery projection. |
