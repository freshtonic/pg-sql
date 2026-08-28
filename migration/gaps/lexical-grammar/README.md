# Lexical and grammar gap examples

| Example | Expected PostgreSQL parser behavior | Legacy/current limitation |
| --- | --- | --- |
| `admissions.sql` | `window` is admitted as an identifier in this position. | Recursa needs category-based admission sets. |
| `custom-operator-boundary.sql` | Accept `!=`; `--comment` starts trivia, then parse `2`. | Longest-match lexing needs the PostgreSQL operator boundary. |
| `psql-variable-split.sql` | Preserve both quoted psql-variable forms as captured source regions. | Grammar needs contextual psql-variable/source capture. |
| `dollar-and-nested-comment.sql` | Capture the matching dollar body and skip the nested comment. | Both require lexer callbacks beyond regular expressions. |
| `numeric-callback.sql` | Reject at the numeric token with trailing-junk location. | Needs a token postcondition/callback. |
| `window-ref-postcondition.sql` | Accept `window_name` as a window reference. | Identifier admission depends on a grammar postcondition. |
| `function-table-alias-overlap.sql` | Preserve alias `t` and typed column `a int`. | Overlapping alias alternatives need deterministic selection. |
