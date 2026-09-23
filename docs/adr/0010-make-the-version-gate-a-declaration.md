# Make the version gate a declaration, not a bare cfg

Status: accepted

A version gate says that one grammar element exists only from a target version,
or only before one (CONTEXT.md). Today a gate is a raw
`#[cfg(feature = "since-pgN")]`, which recursa-codegen evaluates and then
forgets. So a build knows what it accepts, but nothing can say which version a
parsed statement needs. pg-analyze needs that: it analyses SQL for a server
that can be older than the grammar of the build, and it must reject a construct
that the older server cannot parse, with the message that server would give.

A version gate therefore becomes a declaration that recursa understands. One
annotation both removes the item from an older build, as the `cfg` does now,
and records the requirement in generated data. From that data, a derived trait
answers, for one parsed statement, the lowest version whose grammar accepts it,
and the span of the first construct that sets it. Where `gram.y` raises a
specific `ereport` for that construct in the older version, the gate also
carries its SQLSTATE, message and hint, and cites the `gram.y` line they come
from.

The alternative was a second source: a build script that reads pg-sql's own
sources with `syn` and builds the table beside the `cfg`s, or a hand-kept list
in pg-analyze. Both drift from the gates, and a drifted table reports a version
that is too low, which is a wrong answer with no test to catch it. A
declaration keeps one source of truth. The cost is a recursa change and an edit
of every gate that pg-sql has.

## Consequences

- A gate is data, so the requirement can be reported, not only applied. The
  API answers the question pg-analyze asks: the first construct whose
  requirement is newer than a given target version.
- Three classes of gate need different work: an item gate (a node, field or
  variant) falls out of the declaration; a lexer or keyword gate needs the
  token, so it needs the `spans` feature; and a shape rule, where a newer
  grammar makes something optional that was required, is not a gate on an item
  at all and needs its own declaration next to the node.
- The reported version is checked against the oracles: for each corpus
  statement it must equal the oldest target version whose oracle accepts it.
  That check compares results from several builds, so it belongs to the
  six-version gate, not to one build's test run.
- The message text cannot be checked that way. It is verified by citation: a
  gate that carries a message names the `gram.y` line it comes from, as
  principle 11 already requires for the gate itself.
