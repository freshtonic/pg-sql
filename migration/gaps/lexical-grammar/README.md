# Lexical and grammar migration examples

These fixtures preserve the PostgreSQL behavior that the deterministic grammar
migration must map. They are review inputs, not permission to retain callbacks
or handwritten lexer repair passes.

| Example | Expected PostgreSQL parser behavior | Accepted migration mapping |
| --- | --- | --- |
| `admissions.sql` | `window` is admitted as an identifier in this position. | Named category/flag admission expressions. |
| `custom-operator-boundary.sql` | Accept `!=`; `--comment` starts trivia, then parse `2`. | `operator_run` with `/*` and `--` fences plus PostgreSQL trailing/qualifying characters. |
| `psql-variable-split.sql` | Preserve both quoted psql-variable forms as captured source regions. | Structural `Colon` plus a source-backed bare/single-quoted/double-quoted `PsqlVariableName = AllWordKinds - { NULL, TRUE, FALSE }`. |
| `dollar-and-nested-comment.sql` | Capture the matching dollar body and skip the nested comment. | `same_delimiter` plus a non-emitting `nested` block-comment rule. |
| `numeric-callback.sql` | Reject at the numeric token with trailing-junk location. | `next_exclusion` with `[A-Za-z0-9_]` excluded after the match. |
| `window-ref-postcondition.sql` | Accept unquoted, quoted, and Unicode-quoted names as window references. | A source-backed identifier bound to `WindowRefName = ColId - { ROWS, RANGE, GROUPS }`. |
| `function-table-alias-overlap.sql` | Preserve alias `t` and typed column `a int`. | Frozen FIRST-k dispatch with consumer `max_lookahead = 5`; this remains separate from lexical matcher migration. |
