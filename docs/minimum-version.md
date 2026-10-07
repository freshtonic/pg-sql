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

Six widenings had no such shape, so each reported a version that was too low,
which is the dangerous direction: it lets a consumer send a statement to a
server that refuses it. Issue [#93](https://github.com/freshtonic/pg-sql/issues/93)
gave every one of them a shape, and three devices did the whole job:

| Clause | From | The element that now carries the gate |
|---|---|---|
| `REVOKE <name> OPTION FOR` | 16 | `RevokeRoleOptionName::Other`. REL_15_19 `gram.y` 7738 accepts only `ADMIN`, so `Admin` is the 14 shape and `Other` admits the rest of `ColId` (REL_16_15 `gram.y` 7805, commit e3ce2de09). |
| `GRANT role TO role WITH <name> OPTION`, and a list of more than one option | 16 | `GrantRoleOptionName::Other` and `WithRoleOpts.more`, against REL_15_19 `gram.y` 7752 `opt_grant_admin_option: WITH ADMIN OPTION` (REL_16_15 `gram.y` 7778, 7821). The `TRUE` and `FALSE` values already carried a gate. |
| `ALTER OPERATOR ... SET (name)` with no value | 17 | `OperatorDefElem.value`, with `on = absent`. The node is gram.y's own `operator_def_list`, not the shared `DefList`, in every version (REL_17_11 `gram.y` 10252; REL_16_15 `gram.y` 10104). |
| `COPY ... FORCE NOT NULL *`, `FORCE NULL *` | 17 | `CopyForceNullTarget::Star`. `gram.y` writes one `copy_opt_item` arm per spelling and shares no nonterminal, so these two options hold a target of their own and `FORCE QUOTE *` keeps `CopyForceTarget` (REL_17_11 `gram.y` 3463-3484). |
| `ANALYZE ONLY t`, `ANALYZE t *`, and the same for `VACUUM` | 18 | `VacuumRelationName::Only` and `VacuumInheritedRelation.star`. `VACUUM` and `ANALYZE` share one relation node of their own, so the shared `RelationExpr`, where `ONLY t` is legal in 14, needs no gate (REL_18_6 `gram.y` 12021; REL_17_11 `gram.y` 11913). |
| `FOREIGN KEY (a, PERIOD b) REFERENCES t (c, PERIOD d)` | 18 | `ForeignKeyReferencedColumnList`'s `PERIOD` element, beside the one the referencing list already had. `gram.y` makes the two independent, so `REFERENCES t (c, PERIOD d)` alone needs 18 (REL_18_6 `gram.y` `opt_column_and_period_list`). |

The three devices are: a gate on the variant or field that holds the added
shape; `on = absent` where the newer grammar made a value optional; and, where
the difference is a **word** and not a shape, one variant for the single
keyword the older grammar accepts beside a variant that admits the rest of the
admission set. The last one does not widen anything (principle 9): the two
variants together are exactly `gram.y`'s `ColId` or `ColLabel`, and
`tokens!` states the remainder as `ColId - { ADMIN }` and
`ColLabel - { ADMIN }`. A quoted `"admin"` is no keyword, so it takes the
`Other` variant and reports 16, which is right.

One widening is not split yet, and it goes the other way: it reports a version
that is too **high**. The 15 publication object list (`pub_obj_list`) also
covers the 14 `FOR TABLE relation_expr_list`, so `CREATE PUBLICATION p FOR
TABLE t` reports 15 although 14 parses it. The same holds for `ALTER
PUBLICATION ... {ADD|SET|DROP}`. To split it, gate `TABLES IN SCHEMA`, the
column list and the `WHERE` clause, which are the three shapes that 15 adds.

`ALTER TYPE t SET (name)` shares `operator_def_list` with `ALTER OPERATOR`, and
pg-sql routes it through the shared `DefList` instead. A build before 17
therefore accepts it, which is an over-acceptance gap rather than a version
report, and the frozen corpus has no such statement. Giving it the same
`OperatorDefList` is the fix, and it waits on splitting `SetDefinitionClause`,
which `ALTER SUBSCRIPTION` and `ALTER PUBLICATION` share with it.

## The verification

`tests/minimum_version_corpus.rs` checks the reported minimum of every frozen
corpus statement against the oracle outcome of all six target versions, which
the version baselines record (`baselines/target-versions/*.json`). The check
needs no PostgreSQL build of its own, and `scripts/gate-versions` runs it once
for each version.

ADR 0010 asks for equality: the reported minimum must be the oldest target
version whose oracle accepts the statement. The equality holds with one named
exception, because an oracle records only "accepts" or "rejects" and cannot
say what it parsed.

**An older grammar reads the same text as something else.** Before 17,
`JSON_VALUE(j, '$.a')` is an ordinary function call, and the 16 oracle accepts
it. Before 16, `SELECT 0x42F` is the integer `0` with the column label `x42F`,
and the 14 oracle accepts it. The report of 17 or 16 looks too high, and it is
right: the statement that this build parsed needs the newer version. The test
frozen list `REINTERPRETED_BY_AN_OLDER_GRAMMAR` names each construct that may
do this, so a new one cannot arrive unnoticed. The unsplit publication
widening is in the same list, because its effect on the check is the same.

A report that is too low has no exception. The test keeps the frozen list
`UNRECORDED_WIDENINGS`, which issue #93 emptied, so an under-report now fails
the test wherever it appears. A new widening belongs in the grammar as a gated
shape, never in that list.

Every other statement must satisfy the equality.

## The span

`VersionRequirement::span` carries the extent of the construct. The scan runs
from the parse's own root occurrence cursor, so every answer keeps its span:
an item gate points at the construct, a shape rule points at the element that
holds no value (the parenthesised subquery that needs the alias), and a
lexical gate points at the token. `recursa#136` added the `ArenaParsed` pair
of methods that this needs, beside the `Parsed` pair.

The answer therefore takes the parse, not the detached AST:

```rust,ignore
use pg_sql::{MinimumVersion, TargetVersion};

let found = parsed.minimum_version_above(TargetVersion::Pg15).unwrap();
assert_eq!(&source[found.span().unwrap().range()], "(SELECT 1)");
```

A build without the `spans` Cargo feature retains no provenance, so the span
is `None` and only the version and the message data remain. pg-analyze enables
`spans` in every build.
