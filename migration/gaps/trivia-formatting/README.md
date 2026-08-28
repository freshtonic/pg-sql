# Trivia and formatting gap examples

| Example | Expected behavior | Legacy/current limitation |
| --- | --- | --- |
| `adjacent-strings.sql` | Concatenate adjacent strings while retaining the intervening block comment. | Trivia ownership must survive string folding. |
| `comment-stop.sql` | The line comment/newline prevents accidental same-line concatenation. | The parser needs trivia-aware adjacency. |
| `comment-ownership.sql` | The comment has one stable owner and remains before the comma after formatting. | Token-only formatting cannot infer ownership reliably. |
| `idempotence.sql` | Parse-format-format is byte-stable and both comments remain. | Current Recursa needs comment-preserving round trips. |
