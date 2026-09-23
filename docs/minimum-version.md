# The minimum PostgreSQL version of a parsed statement

Issue [#92](https://github.com/freshtonic/pg-sql/issues/92). Decision:
[ADR 0010](adr/0010-make-the-version-gate-a-declaration.md). Terms:
`CONTEXT.md` (target version, version gate, version parity). API:
`pg_sql::minimum_version`.

A pg-sql build reproduces the raw parser of one target version (ADR 0009).
A consumer can analyse SQL for a server that is older than the build. It must
reject each construct that the older grammar cannot parse, and give the
message that server gives. pg-sql answers, for one parsed statement:

- the lowest target version whose grammar accepts it;
- the first construct, in source order, that a given older version rejects;
- where `gram.y` raises a specific `ereport`, the SQLSTATE, message and hint.

The answer comes from the gates, not from a list that a person keeps. Each
gate is a `#[config(since = pgN)]` declaration. Recursa removes the element
from an older build, as the earlier `#[cfg]` did, and records the requirement
beside the node. A scan over the parsed value then reports it.

## The classes of gate

### Class 1: an item gate

A node, field or variant that exists only from version N. The scan reports it.
Most gates are of this class.

```rust,ignore
#[config(since = pg17)]
JsonTable(boxed!(JsonTableRef)),
```

### Class 2: a lexical gate

A gate on a `tokens!` entry removes the entry but records nothing, because two
same-named entries under opposite predicates cannot be told apart in a parsed
value. The requirement belongs to the token, not to the shape of the value.

`pg_sql::minimum_version::LEXICAL_GATES` names each `tokens!` gate that widens
what a build accepts, and `lexical_minimum_version` scans the tokens against
it. The answer always carries the span of the token.

The list is short, because most `tokens!` gates do one of two other things.
A gate that only narrows a pattern needs no entry: an older build accepts
more, so nothing needs the newer version. A gate that adds a keyword also
needs no entry, because a new keyword arrives together with a gated node,
field or variant, which the parsed value records.

### Class 3: a shape rule

A newer grammar sometimes makes optional what an older one required. Then the
requirement belongs to the absence of the value, which `on = absent` declares.
Four such rules exist, all of PostgreSQL 16:

| Construct | Site | Message in 15 |
|---|---|---|
| A subquery in `FROM` with no alias | `ParenQueryRef.alias` | 42601, "subquery in FROM must have an alias" |
| The same after `LATERAL` | `LateralSubquery.alias` | the same |
| `CREATE STATISTICS` with no name | `CreateStatisticsStmt.name` | a plain syntax error |
| `REINDEX DATABASE` with no name | `ReindexAllTarget.name` | a plain syntax error |

The first two carry the SQLSTATE, the message and the hint of the `ereport`
that REL_15_19 `gram.y` raises. The other two have no `ereport`: the older
rule has no alternative without the name, so the parser gives a plain syntax
error, and the gate carries only its citation.

### Class 4: a widening

A newer grammar sometimes changes the type of a field to a wider one, which
accepts everything the older one does. Then the requirement does **not**
belong to the field, and a gate on the field reports a version that is too
high. `ALTER TABLE t ALTER a SET STORAGE plain` parses in 14, although
PostgreSQL 16 widened the argument from `ColId` to a type that also admits
`DEFAULT`.

A widening therefore keeps a plain `#[cfg]`, which removes but never records,
and the gate moves to the shape that the newer type adds:

```rust,ignore
pub enum ColumnStorageMode {
    #[config(since = pg16)]   // the shape that 16 adds
    #[tok(DEFAULT)]
    Default,
    Name(crate::tokens::ColId),   // the 14 shape, which records nothing
}
```

Some widenings have no such shape to gate, so their requirement is not
reported. Each one reports a version that is too low, and a consumer that must
not miss one gives these clauses its own check. The complete list:

| Clause | From | Why no gate |
|---|---|---|
| `REVOKE ColId OPTION FOR` | 16 | The requirement depends on the word. REL_15_19 `gram.y` has only `REVOKE ADMIN OPTION FOR`, so `REVOKE INHERIT OPTION FOR r FROM u` needs 16 and `REVOKE ADMIN OPTION FOR r FROM u` needs 14 (commit e3ce2de09). |
| `GRANT role TO role WITH <name> OPTION`, and a list of more than one option | 16 | The same. The `TRUE` and `FALSE` values that 16 adds do carry a gate, which covers every frozen corpus case. |
| `ALTER OPERATOR ... SET (name)` with no value | 17 | The value sits in the shared `DefList`, where a `DefElem` with no value is legal in 14 elsewhere. REL_17_11 `gram.y` 10252 adds `operator_def_elem: ColLabel`; REL_16_15 `gram.y` 10103 has no such form. |
| `COPY ... (FORCE NOT NULL *)`, `FORCE NULL *` | 17 | The `*` sits in `CopyForceTarget`, which `FORCE_QUOTE *` also uses, and that form is legal in 14. |
| `ANALYZE ONLY t`, `ANALYZE t *`, and the same for `VACUUM` | 18 | The extra shapes sit in the shared `RelationExpr`, where `ONLY t` is legal in 14 elsewhere. |
| `FOREIGN KEY (a, PERIOD b) REFERENCES t (c, PERIOD d)` | 18 | The `PERIOD` element sits in a list shape that the wider type adds; it has no gate yet. |

One widening is not split yet, and it reports a version that is too **high**.
The 15 publication object list (`pub_obj_list`) also covers the 14 `FOR TABLE
relation_expr_list`, so `CREATE PUBLICATION p FOR TABLE t` reports 15,
although 14 parses it. The same holds for `ALTER PUBLICATION ...
{ADD|SET|DROP}`. To split it, gate `TABLES IN SCHEMA`, the column list and the
`WHERE` clause, which are the three shapes that 15 adds.

Each of these needs its own node, or a shape rule on the wider node, before a
gate can carry it. The frozen corpus exercises only the first three, and
`tests/minimum_version_corpus.rs` pins those seven statements.

## The verification

`tests/minimum_version_corpus.rs` checks the reported minimum of every frozen
corpus statement against the oracle outcome of all six target versions, which
the version baselines record (`baselines/target-versions/*.json`). The check
needs no PostgreSQL build of its own, and `scripts/gate-versions` runs it once
for each version.

ADR 0010 asks for equality: the reported minimum must be the oldest target
version whose oracle accepts the statement. The equality holds with two named
exceptions, because an oracle records only "accepts" or "rejects" and cannot
say what it parsed.

**An older grammar reads the same text as something else.** Before 17,
`JSON_VALUE(j, '$.a')` is an ordinary function call, and the 16 oracle accepts
it. Before 16, `SELECT 0x42F` is the integer `0` with the column label `x42F`,
and the 14 oracle accepts it. The report of 17 or 16 looks too high, and it is
right: the statement that this build parsed needs the newer version. The test
frozen list `REINTERPRETED_BY_AN_OLDER_GRAMMAR` names each construct that may
do this, so a new one cannot arrive unnoticed.

**A widening that depends on a word.** The two residues above report a version
that is too low. The test frozen list `UNRECORDED_WIDENINGS` names the seven
corpus statements that do this.

Every other statement must satisfy the equality. In particular a report that
is too low fails the test wherever no frozen entry excuses it, because that is
the dangerous direction: it would let a consumer send a statement to a server
that refuses it.

## The span

`VersionRequirement::span` carries the extent of the construct. A lexical
requirement always has one. An item or shape requirement has none today: this
grammar builds an arena-backed AST, and Recursa runs the requirement scan from
a root occurrence cursor only for `Parsed<T>`, not for `ArenaParsed<F>`. The
scan over a detached value reports no span, which is the contract of
`Requires` itself.

The fix belongs to Recursa: an `ArenaParsed` pair of methods beside the
`Parsed` pair that `recursa-core/src/parsed.rs` already has. It needs no
change in pg-sql, because `span()` already returns `Option<Span>`. Until then
a consumer uses the statement's own `source_bounds()`.
