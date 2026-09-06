# Give psql its own grammar and crate

Status: accepted

psql is a client program, not a SQL dialect. It scans its input, substitutes
variable interpolations textually, and sends the resulting string to the
server. PostgreSQL's `gram.y` therefore contains no psql construct at all,
and `pg-sql`'s goal is to mirror `gram.y` (CLAUDE.md principle 9).

psql moves into its own crate, `pg-psql`, with its own grammar and AST. A
psql document is parsed by that grammar first, substituted, rendered back to
SQL text, and only then parsed by the SQL grammar. Recursa permits exactly
one `grammar!` per crate and enforces it, so a separate grammar is
necessarily a separate crate rather than a module.

## Why

`pg-sql` admitted psql interpolation inside its expression grammar. That was
a departure from `gram.y` and the direct cause of 8 of the 31 remaining LALR
conflicts: `TypeCastValue::PsqlVar` admitted `:'x'` after a type-name
keyword, so `SELECT int :'x'` read as a typed literal, while `int :` also
begins a SQL/JSON `JSON_OBJECT` key/value entry. No lookahead settles that,
because both readings stay live past the colon. The server never faces the
choice, because psql has already substituted. Removing the construct takes
the grammar to 23 conflicts in 6 states, from 31 in 14.

The same admission distorted the rest of the grammar. `[:2]` was represented
as a colon-prefixed value rather than as `gram.y`'s empty `opt_slice_bound`,
purely to keep a slice colon and a psql colon from overlapping; `['a':'b']`
was recorded as a limitation for the same reason. With psql gone both are
simply the optional bounds PostgreSQL writes.

## What the psql grammar recognises

Exactly what `vendor/postgres/src/fe_utils/psqlscan.l` recognises: runs of
SQL text, the four interpolation forms (`:name`, `:'name'`, `:"name"`,
`:{?name}`), and the terminators that submit a query buffer. Each
interpolation is one token, as psql scans it.

Respecting SQL lexical structure is most of the work, and it is what
`psqlscan.l` spends most of its rules on: a colon inside a single-quoted
string, an `E`/`U&`/`B`/`X` string, a dollar-quoted body, a quoted
identifier, or a line or block comment is not an interpolation. Here each of
those is one token or ignored trivia, so the property holds by construction
rather than by a rule that must be remembered at every site.

psql meta-commands (`\set`, `\d`, `\if`, `\copy`) stay out of scope; they are
issues #11, #12 and #13. `psqlscan.l` does not scan them either — it returns
`LEXRES_BACKSLASH` and hands off to `psqlscanslash.l` — so an unmodelled
backslash command stays inside a text run and renders verbatim, which makes
the gap visible at the SQL parse rather than silent.

One divergence is known and deliberate. `psqlscan.l:983` has a single
`<<EOF>>` rule covering every scanner state, so psql tolerates reaching end
of input inside an open string, dollar-quoted body or comment. recursa's
closed matchers require their closer to exist, so `pg-psql` refuses such a
document instead (freshtonic/recursa#131). The direction is what matters:
the region is refused, never reinterpreted as ordinary text, so a colon
inside an unterminated string can never be substituted. A caller is told the
document is not psql rather than handed a wrong rendering.

## Substitution and the source map

`:'name'` is quoted as a string literal, `:"name"` as a quoted identifier,
`:name` is substituted raw, and `:{?name}` becomes `TRUE` or `FALSE`, each
following psql and libpq exactly. An unbound variable is left verbatim:
substitution never invents a value, so the SQL parse fails on the surviving
colon and an unbound variable stays a diagnosable condition rather than a
silent rewrite.

Rendering copies the source and rewrites only the regions psql rewrites.
That is what keeps the map exact: outside a recorded region the rendered
text is the user's own bytes at a known shift, so an offset in the rendered
SQL translates back to an offset in the psql source. Without that, rendering
would destroy the tie between a diagnostic and the user's text, which is
what pg-sql's occurrence model exists to preserve. Substitution is a single
pass; psql rescans the value of a `:name`, and composing the maps for that
is deferred.

## What pg-sql loses

`pg_sql::document::parse_sql` no longer distinguishes psql input. The
`SqlParseError::Psql` variant and `PsqlSyntaxError` are removed rather than
retained, because once the grammar mirrors `gram.y` there is nothing left in
it that could recognise psql: the old variant was produced by walking the
parsed AST for a psql node, and no such node exists. Re-creating the
diagnostic would mean a textual sniff — a heuristic, not a parse fact — and
`pg-sql` cannot depend on `pg-psql` to borrow the real answer, since the
dependency runs the other way. A psql document is therefore rejected as
ordinary invalid input, with the failing statement named as any rejection
names it, and callers with psql scripts use `pg-psql`.
