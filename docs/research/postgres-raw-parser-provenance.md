# Provenance retained by PostgreSQL's raw parser

Research date: 2026-09-08.
Reference source: `vendor/postgres`, PostgreSQL 17.9 (`6d396980fc5`).

## Conclusion

PostgreSQL's `raw_parser` retains **sparse source positions**, not an
exact-source provenance graph. Across every `RawParseMode`, 65 raw-parser-reachable
node types retain 67 position fields. Almost every field is one `int` byte offset,
usually the first token of a construct. The two exceptions in shape are the outer
`RawStmt`, which has a statement start and byte length, and `JsonTablePathSpec`, which
has separate anchors for its string and optional name.

The raw tree does not retain the source string, comments, whitespace, exact token
spellings, token extents, or end positions for ordinary nodes. PostgreSQL says that an
original source string is required to use its locations
(`vendor/postgres/src/include/nodes/parsenodes.h:5-12`).

This is materially less provenance than pg-sql's complete occurrence partition. A
PostgreSQL-like public mode in pg-sql would need direct, selected-node anchors plus an
outer statement range, while bypassing the occurrence stack and exact-source metadata.
Filtering a complete occurrence graph after parsing would produce the desired result
shape but would not provide a like-for-like performance comparison.

## Representation and semantics

`ParseLoc` is an alias for `int`, with `-1` meaning unknown
(`vendor/postgres/src/include/nodes/nodes.h:234-240`). Locations are byte offsets from
the beginning of the input, not character, line, or column positions. PostgreSQL uses a
single location, "usually the first token location", rather than Bison's normal
beginning/end pair (`vendor/postgres/src/include/parser/scanner.h:35-44`).

The scanner calculates a token location by subtracting the scan-buffer base from the
current token pointer (`vendor/postgres/src/backend/parser/scan.l:99-110`). Bison then
propagates the first right-hand-side location to a reduced nonterminal; empty
reductions get `-1` (`vendor/postgres/src/backend/parser/gram.y:69-100`). Thus location
tracking is present transiently for tokens and grammar symbols even when the resulting
AST node has no `ParseLoc` field.

`RawStmt` is the sole range-like raw node:

- `stmt_location` is the start offset.
- `stmt_len` is the byte length; zero means the rest of the input.

The fields and their semantics are declared at
`vendor/postgres/src/include/nodes/parsenodes.h:2002-2025`. In the default
multi-statement grammar, the boundaries are derived from semicolon positions. As a
result, a later statement's range can begin immediately after the previous semicolon,
before leading whitespace or comments, rather than at its first token
(`vendor/postgres/src/backend/parser/gram.y:960-988`). `makeRawStmt` initializes the
length to zero, and a following semicolon supplies the previous statement's end
(`vendor/postgres/src/backend/parser/gram.y:18534-18558`).

The scanner does copy the complete source into a Flex scan buffer, but that buffer is
scanner state, not a field of the returned tree
(`vendor/postgres/src/backend/parser/scan.l:1253-1289`). `scanner_finish` leaves small
buffers to the surrounding memory-context reset and frees larger ones; this is still
transient parser memory (`vendor/postgres/src/backend/parser/scan.l:1295-1314`).

## Inventory method

The inventory was built in three passes:

1. Enumerate every `ParseLoc` field in `parsenodes.h` and `primnodes.h`.
2. Establish raw-parser reachability from grammar actions in
   `vendor/postgres/src/backend/parser/gram.y`, including constructors called by those
   actions.
3. Exclude fields whose nodes are created only by parse analysis, rewriting, planning,
   or execution.

The second pass is necessary because searching only for `makeNode(Type)` in `gram.y`
misses indirect construction. For example, grammar actions call `makeA_Expr`,
`makeBoolExpr`, `makeTypeName*`, `makeDefElem*`, `makeFuncCall`, `makeGroupingSet`, and
the SQL/JSON constructors. Their bodies allocate and fill the corresponding nodes in
`vendor/postgres/src/backend/nodes/makefuncs.c:25-58`, `:413-427`, `:486-528`,
`:603-665`, `:859-871`, and `:890-990`. `RangeVar` is likewise made either directly by
the grammar or through `makeRangeVar` (`makefuncs.c:466-483`).

Conversely, `PartitionRangeDatum.location` looks like a raw field from its header, but
it is not raw-parser-reachable. Parse analysis explicitly converts raw grammar
expressions into `PartitionRangeDatum` nodes in
`vendor/postgres/src/backend/parser/parse_utilcmd.c:4116-4139` and assigns their
locations at `:4183-4193` and `:4231-4236`. It is therefore excluded from the count.

The count covers all modes accepted by `raw_parser`, including the type-name and
PL/pgSQL expression/assignment modes documented in
`vendor/postgres/src/include/parser/parser.h:22-62`. `PLAssignStmt` is consequently in
the inventory even though it does not occur in `RAW_PARSE_DEFAULT` output.

## Raw-parser-reachable nodes

### Common names, expressions, and clauses

The following fields are in `vendor/postgres/src/include/nodes/parsenodes.h`:

| Node | Field line | Anchor meaning where specialized |
| --- | ---: | --- |
| `TypeName` | 275 | token |
| `ColumnRef` | 295 | token |
| `ParamRef` | 305 | token |
| `A_Expr` | 338 | token/operator |
| `A_Const` | 364 | literal token |
| `TypeCast` | 375 | token |
| `CollateClause` | 386 | token |
| `RoleSpec` | 406 | token |
| `FuncCall` | 436 | token |
| `A_ArrayExpr` | 493 | token |
| `ResTarget` | 520 | token |
| `SortBy` | 550 | operator, or unknown when none |
| `WindowDef` | 571 | construct location |

These raw or hybrid expression nodes are in
`vendor/postgres/src/include/nodes/primnodes.h`:

| Node | Field line | Note |
| --- | ---: | --- |
| `GroupingFunc` | 554 | Exists in raw output before later fields are filled (`:531-554`). |
| `MergeSupportFunc` | 636 | `MERGE_ACTION()` token. |
| `NamedArgExpr` | 797 | Argument-name token. |
| `BoolExpr` | 941 | `AND`, `OR`, or `NOT` token. |
| `SubLink` | 1018 | Raw sublink is explicitly documented at `:975-980`. |
| `CaseExpr` | 1316 | `CASE` token. |
| `CaseWhen` | 1327 | `WHEN` token. |
| `RowExpr` | 1435 | `ROW`/row construct token. |
| `CoalesceExpr` | 1494 | `COALESCE` token. |
| `MinMaxExpr` | 1520 | `GREATEST` or `LEAST` token. |
| `SQLValueFunction` | 1564 | Special-function token. |
| `XmlExpr` | 1617 | SQL/XML construct token. |
| `NullTest` | 1962 | Test token. |
| `BooleanTest` | 1984 | Test token. |
| `SetToDefault` | 2078 | `DEFAULT` token. |

`RangeVar.location`, at `primnodes.h:93-95`, is also raw-parser-reachable and is used
widely for relation names. The grammar's locking-clause representation explicitly uses
`RangeVar` because it carries a location (`parsenodes.h:822-829`).

### FROM, definition, and partition components

These fields are all in `vendor/postgres/src/include/nodes/parsenodes.h`:

| Node | Field line | Anchor meaning where specialized |
| --- | ---: | --- |
| `RangeTableFunc` | 664 | table-function token |
| `RangeTableFuncCol` | 682 | column token |
| `RangeTableSample` | 702 | sampling method name |
| `ColumnDef` | 745 | definition location |
| `DefElem` | 819 | option token |
| `XmlSerialize` | 849 | token |
| `PartitionElem` | 867 | token |
| `PartitionSpec` | 887 | token |
| `PartitionBoundSpec` | 914 | token |
| `Constraint` | 2772 | constraint token |
| `PublicationObjSpec` | 4157 | publication-object token |

`CreateTableSpaceStmt.location` at `parsenodes.h:2785` is deliberately absent: it is a
`char *` containing the SQL `LOCATION` clause's value, not a source position.

### Query structure

These fields are all in `vendor/postgres/src/include/nodes/parsenodes.h`:

| Node | Field line |
| --- | ---: |
| `GroupingSet` | 1511 |
| `WithClause` | 1597 |
| `InferClause` | 1612 |
| `OnConflictClause` | 1628 |
| `CTESearchClause` | 1649 |
| `CTECycleClause` | 1660 |
| `CommonTableExpr` | 1684 |

Some of these are deliberately hybrid structures. For example,
`CommonTableExpr.ctequery` holds a raw statement before analysis and a `Query`
afterward (`parsenodes.h:1668-1685`), but its location is already present in raw output.

### SQL/JSON

These fields are in `vendor/postgres/src/include/nodes/parsenodes.h`:

| Node | Field line | Anchor meaning where specialized |
| --- | ---: | --- |
| `JsonFuncExpr` | 1799 | function token |
| `JsonTablePathSpec` | 1813, 1814 | optional name and path string, respectively |
| `JsonTable` | 1831 | token |
| `JsonTableColumn` | 1864 | token |
| `JsonParseExpr` | 1889 | token |
| `JsonScalarExpr` | 1901 | token |
| `JsonSerializeExpr` | 1913 | token |
| `JsonObjectConstructor` | 1927 | token |
| `JsonArrayConstructor` | 1940 | token |
| `JsonArrayQueryConstructor` | 1954 | token |
| `JsonAggConstructor` | 1969 | token |

Three raw SQL/JSON support nodes are in
`vendor/postgres/src/include/nodes/primnodes.h`: `JsonFormat.location` at line 1653,
`JsonIsPredicate.location` at line 1739, and `JsonBehavior.location` at line 1793.
The grammar calls their constructors at `gram.y:15231-15261` and `:16868-16943`; the
constructors assign the anchors at `makefuncs.c:890-935` and `:953-969`.

### Statement envelope and specialized statements

These fields are all in `vendor/postgres/src/include/nodes/parsenodes.h`:

| Node | Field line | Meaning |
| --- | ---: | --- |
| `RawStmt` | 2023-2024 | statement start and byte length |
| `PLAssignStmt` | 2232 | assignment name; special raw-parser mode |
| `TransactionStmt` | 3678 | statement token |
| `DeallocateStmt` | 4069 | statement token |

Most statement-root structs have no position field at all. The `RawStmt` range is their
only retained position unless a nested component appears in one of the tables above.

## What is not raw-parser provenance

The same headers contain many locations produced or propagated later:

- Analyzed `Query.stmt_location` and `stmt_len` are generally populated only on
  top-level queries (`vendor/postgres/src/include/nodes/parsenodes.h:231-240`) and are
  copied from `RawStmt` by parse analysis
  (`vendor/postgres/src/backend/parser/analyze.c:255-256`).
- Planned `PlannedStmt.stmt_location` and `stmt_len` are copied from `Query`
  (`vendor/postgres/src/include/nodes/plannodes.h:95-100` and
  `vendor/postgres/src/backend/optimizer/plan/planner.c:561-562`).
- Analysis/execution expression nodes with locations include `TableFunc`, `Var`,
  `Const`, `Param`, `Aggref`, `WindowFunc`, `FuncExpr`, `OpExpr`,
  `ScalarArrayOpExpr`, coercion nodes, `ArrayExpr`, `JsonConstructorExpr`, `JsonExpr`,
  and `CoerceToDomain*`. They have no construction path from raw grammar output and
  are excluded.
- `YYLTYPE`, lookahead locations, scanner callbacks, and `ParseState.p_sourcetext` are
  transient parser/analyzer state rather than returned raw-tree fields
  (`vendor/postgres/src/include/parser/scanner.h:35-44,108,123-130` and
  `vendor/postgres/src/include/parser/parse_node.h:327-336`).

## Copy, equality, serialization, and jumbling

`ParseLoc` receives special generated-node treatment
(`vendor/postgres/src/backend/nodes/gen_node_support.pl:780-784,1013-1016`):

- `copyObject` preserves the integer value. `COPY_LOCATION_FIELD` is intentionally the
  scalar-copy operation (`vendor/postgres/src/backend/nodes/copyfuncs.c:60-62`).
- `equal` deliberately ignores all parse locations, so otherwise identical syntax at
  different source positions compares equal
  (`vendor/postgres/src/backend/nodes/equalfuncs.c:3-8,79-81`).
- Normal `nodeToString` writes `-1` for locations; only
  `nodeToStringWithLocations` writes actual values
  (`vendor/postgres/src/backend/nodes/outfuncs.c:29,91-94,760-799`).
- Normal `stringToNode` restores locations as `-1`. Restoring serialized locations is
  available only through the debug-gated location-aware path
  (`vendor/postgres/src/backend/nodes/readfuncs.c:120-131` and
  `vendor/postgres/src/backend/nodes/read.c:46-80,90-100`).

The custom operation annotations do not change those rules. `A_Expr` is
`custom_read_write`, `A_Const` is `custom_copy_equal`, `custom_read_write`, and
`custom_query_jumble`, and `BoolExpr` is `custom_read_write`
(`parsenodes.h:329-365` and `primnodes.h:934-942`). Their manual implementations still
use `COPY_LOCATION_FIELD`, `COMPARE_LOCATION_FIELD`, `WRITE_LOCATION_FIELD`, and
`READ_LOCATION_FIELD`; see `copyfuncs.c:107-143`, `equalfuncs.c:133-143`,
`outfuncs.c:575-645,695-708`, and `readfuncs.c:303-343,438-522`.

Locations are also ignored by query jumbling unless a field explicitly carries
`query_jumble_location` (`gen_node_support.pl:1284-1312`). `RawStmt` opts out of query
jumbling altogether (`parsenodes.h:2017-2025`). Of the raw-reachable fields, only
`TransactionStmt.location` and `DeallocateStmt.location` carry the explicit location
annotation (`parsenodes.h:3667-3679,4056-4070`). This later jumbling behavior is not
part of `raw_parser` construction cost.

## Implications for pg-sql

A genuinely PostgreSQL-equivalent retained-provenance mode would have this contract:

1. Retain one optional byte-offset anchor on the pg-sql nodes corresponding to the 65
   PostgreSQL node roles above.
2. Retain a start/byte-length pair for each outer statement.
3. Do not retain the source buffer, trivia, exact spelling, end offsets, or provenance
   for every grammar occurrence.
4. Treat anchors as diagnostic metadata: preserve them on copy, ignore them for AST
   equality, and omit them from ordinary serialization unless explicitly requested.

The mapping must be semantic rather than based only on type names. There are 1,466
distinct public struct/enum names under `src/ast`, but only 21 names exactly intersect
the 65 raw-parser-reachable PostgreSQL types in this inventory. PostgreSQL's raw tree
sometimes represents a construct with a generic or hybrid node where pg-sql has several
grammar-specific nodes, and many PostgreSQL nodes have no location at all. A policy
table at the AST-generation boundary would make the many-to-one mapping explicit and
reviewable. The counts are reproducible by extracting `^pub (struct|enum)` names from
`src/ast/**/*.rs` and intersecting them with the two inventory tables above.

Implementation at the semantic-action boundary matters for performance. PostgreSQL
computes and carries one integer location for each token/nonterminal, but only writes an
integer into selected result nodes. It does not allocate or fold a retained occurrence
tree. Pg-sql should therefore use a distinct compact anchor channel: take the selected
anchor directly from the reduced token or child span and leave `OccurrenceStack`
disabled in this mode.

That distinction cannot be achieved merely by changing capture annotations in the
current Recursa model. If any capture is requested or mandatory, the LR driver creates
one grammar-wide occurrence stack (`../recursa/recursa-core/src/lr.rs:160-171`); it then
records every shifted token (`lr.rs:259-265`) and folds every reduction
(`lr.rs:297-322`). Repetition folding also retains item and separator structure
(`../recursa/recursa-core/src/lr/provenance.rs:336-383,586-609`), while child
accumulators retain the structural path needed to expose captured descendants
(`../recursa/recursa-core/src/parsed.rs:1240-1285`). Building that complete sidecar and
pruning it afterward—or simply marking fewer final nodes—would continue paying costs
that PostgreSQL does not incur.

This sparse mode could support PostgreSQL-style error cursors and statement slicing,
provided the caller still has the original source. It cannot support exact-source
rendering, comment attachment, exact keyword/operator spelling, or arbitrary-node
spans. Those capabilities should therefore be absent from the public result type, or
expressly unavailable, rather than silently returning incomplete data.

For benchmarking, compare PostgreSQL `raw_parser` with this sparse mode, not with a
fully provenance-free mode and not with pg-sql's complete occurrence partition. The
comparison will then include the cost both parsers necessarily pay to know token byte
positions, while comparing like retained result metadata: selected anchors and outer
statement boundaries.
