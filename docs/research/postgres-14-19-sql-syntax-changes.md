# SQL syntax changes in PostgreSQL 14 to 19

Research date: 2026-09-22.
Reference source: `vendor/postgres`, read from git objects only. The tags and commits are:

- `REL_13_0` (`29be9983a64`)
- `REL_14_0` (`86a4dc1e6f2`)
- `REL_15_0` (`2a7ce2e2ce4`)
- `REL_16_0` (`c372fbbd8e9`)
- `REL_17_0` (`d7ec59a63d7`)
- `REL_18_0` (`3d6a828938a`)
- PostgreSQL 19: `REL_19_STABLE` at `b73d13c` ("Stamp 19beta4", 2026-09-21), the pin of the `pg19-beta` target version. The first version of this document used `7a74e5ed92d` (2026-08-28).

**PostgreSQL 19 is pre-release.** The document uses `b73d13c` on `REL_19_STABLE`. It contains `REL_19_BETA3` (`3638289fb57`, 2026-08-10) and `7a74e5ed92d`. The commits after BETA3 include `3e8bcc8644f`, which reverts `ALTER TABLE ... MERGE/SPLIT PARTITION(S)`, and the commits after `7a74e5ed92d` revert SQL/PGQ, `FOR PORTION OF` and the new `CREATE SCHEMA` elements. The section "PostgreSQL 19" lists these changes. The 19 grammar can change again before release.

For version N, the change set is `REL_(N-1)_0 .. REL_N_0`. The sources are:

- The release notes: `REL_N_0:doc/src/sgml/release-N.sgml`. In this document, "RN*N*" means that file, and a section name in quotes refers to one of its sections.
- The grammar: `src/backend/parser/gram.y`.
- The keyword list: `src/include/parser/kwlist.h`.
- The lexer: `src/backend/parser/scan.l`, and `parser.c` for lookahead tokens.

Line numbers refer to the file at the tag of that version. Commit hashes come from the `<!-- Author ... -->` comments in the release notes, or from `git log`. If the release notes and the grammar do not agree, the grammar wins, and the item says so.

Scope. The document includes statements, clauses, options, operator syntax, lexical syntax, keywords and removed syntax. It does not include ordinary `f(x)` functions (one summary line per version), configuration parameters, psql, ecpg or performance. Some items are "option values with no gram.y change". PostgreSQL parses these options through a generic option production, so a grammar that copies `gram.y` accepts them already.

## Summary

| Version | Largest syntax additions |
|---|---|
| 14 | SQL-standard routine bodies (`RETURN expr`, `BEGIN ATOMIC ... END`). CTE `SEARCH`/`CYCLE`. `GROUP BY DISTINCT`. `JOIN ... USING (...) AS alias`. Almost all keywords can be bare column labels. Postfix operators are removed. |
| 15 | `MERGE`. Publication `TABLES IN SCHEMA`, column lists and row filters. `UNIQUE NULLS NOT DISTINCT`. `GRANT ... ON PARAMETER`. The lexer rejects trailing junk after numbers. |
| 16 | SQL/JSON constructors (`JSON_OBJECT`, `JSON_ARRAY`, `JSON_OBJECTAGG`, `JSON_ARRAYAGG`) and `IS JSON`. Hexadecimal, octal and binary integer literals, and `_` digit separators. A subquery in `FROM` does not need an alias. `GRANT role ... WITH INHERIT/SET`. `SYSTEM_USER`. |
| 17 | `JSON_TABLE`, `JSON_QUERY`, `JSON_VALUE`, `JSON_EXISTS`, `JSON()`, `JSON_SCALAR`, `JSON_SERIALIZE`. `MERGE ... WHEN NOT MATCHED BY SOURCE` and `MERGE ... RETURNING` with `merge_action()`. `AT LOCAL`. `ALTER COLUMN ... SET EXPRESSION`. |
| 18 | Virtual generated columns, which become the default. Temporal `WITHOUT OVERLAPS` and `PERIOD` keys. `[NOT] ENFORCED`. Table-level `NOT NULL` constraints. `RETURNING old/new`. `VACUUM`/`ANALYZE ONLY`. |
| 19 (pre-release) | `ON CONFLICT DO SELECT`. `IGNORE/RESPECT NULLS`. `REPACK`. `WAIT FOR LSN`. `CHECKPOINT (options)`. Publication `ALL SEQUENCES` and `ALL TABLES EXCEPT`. `standard_conforming_strings` is always on. At `b73d13c`, SQL/PGQ and `FOR PORTION OF` are reverted. |

Verdict on breaking changes. Each release breaks some syntax, but most breaks are small. The largest grammar breaks are the removal of postfix operators in 14 and the stricter numeric lexing in 15 (`123abc` and `0x1F` are errors). Keywords also cause breaks. `system_user` becomes reserved in 16. `json` changes from unreserved to `COL_NAME` in 17. The SQL/JSON words `json_*` and `merge_action` in 16 and 17 are new `COL_NAME` keywords, so an unqualified user function or type with one of these names stops working. In 19, `ignore` and `respect` are new keywords that need `AS` when they are column labels. Three lexical changes alter meaning: `E'\v'` (17), `FORMAT JSON` lookahead (16) and the removal of `standard_conforming_strings = off` (19). The remaining breaks are rejections at execution time or changes of meaning, with no grammar change. The consolidated table at the end lists all of them.

## PostgreSQL 14

Change set `REL_13_0..REL_14_0`. RN14 = `REL_14_0:doc/src/sgml/release-14.sgml`. gram.y = `REL_14_0:src/backend/parser/gram.y`.

### New statements

| # | Syntax | Example | Source |
|---|---|---|---|
| 1 | SQL-standard function and procedure bodies: `RETURN a_expr` or `BEGIN ATOMIC stmt; ... END`. PostgreSQL parses the body when it creates the routine. The options list is now optional (`opt_createfunc_opt_list`), so `LANGUAGE` and `AS` are not necessary. | `CREATE FUNCTION add(a int, b int) RETURNS int RETURN a + b;` / `CREATE PROCEDURE p() BEGIN ATOMIC INSERT INTO t VALUES (1); END;` | RN14 "Functions"; gram.y `ReturnStmt` (7933), `opt_routine_body` (7941), `routine_body_stmt_list` (7962); commit e717a9a18 |

Transaction control is not permitted in a routine body. The forms `BEGIN` and `END` without `TRANSACTION` moved to `TransactionStmtLegacy` (10016). Only `toplevel_stmt` (898) can reach that production. At top level, both forms still parse (commit e717a9a18).

### Changes to existing statements

| # | Change | Example | Source |
|---|---|---|---|
| 2 | `ALTER TABLE ... DETACH PARTITION ... CONCURRENTLY` and `... FINALIZE` | `ALTER TABLE p DETACH PARTITION p1 CONCURRENTLY;` | RN14 "Partitioning"; gram.y `partition_cmd` 2130, 2143; commit 71f4c8c6f |
| 3 | Column `COMPRESSION` in `CREATE TABLE`, `ALTER COLUMN ... SET COMPRESSION`, `LIKE ... INCLUDING COMPRESSION` | `CREATE TABLE t (b text COMPRESSION lz4);` | RN14 "General Performance"; gram.y `columnDef` 3465, `column_compression` 3525, `alter_table_cmd` 2306, `TableLikeOption` 3758; commit bbe0a81db |
| 4 | `CREATE STATISTICS` accepts expressions (`stats_param`: `ColId`, `func_expr_windowless` or `'(' a_expr ')'`) | `CREATE STATISTICS s ON (a + b), lower(c) FROM t;` | RN14 "Optimizer"; gram.y `CreateStatsStmt` 4105, `stats_param` 4143; commit a4d75c86b |
| 5 | `CREATE OR REPLACE TRIGGER`. The grammar also accepts `CREATE OR REPLACE CONSTRAINT TRIGGER`, but its action rejects that form. | `CREATE OR REPLACE TRIGGER trg BEFORE INSERT ON t FOR EACH ROW EXECUTE FUNCTION f();` | RN14 "Utility Commands"; gram.y `CreateTrigStmt` 5348, 5370; commit 92bf7e2d0 |
| 6 | `GRANTED BY` on privilege `GRANT`/`REVOKE`. Before 14, only role grants accepted it. | `GRANT SELECT ON t TO u GRANTED BY CURRENT_USER;` | RN14 "Utility Commands"; gram.y `GrantStmt` 6858, `RevokeStmt` 6876/6891, `opt_granted_by` 7203; commit 6aaaa76bb |
| 7 | `ALTER SUBSCRIPTION ... ADD PUBLICATION` / `DROP PUBLICATION` | `ALTER SUBSCRIPTION s ADD PUBLICATION p2;` | RN14 "Utility Commands"; gram.y `AlterSubscriptionStmt` 9725, 9735; commit 82ed7748b |
| 8 | `CURRENT_ROLE` is a `RoleSpec` wherever `CURRENT_USER` is one | `ALTER TABLE t OWNER TO CURRENT_ROLE;` | RN14 "Utility Commands"; gram.y `RoleSpec` 15315; commit 45b980570 |
| 9 | `REINDEX` takes a general parenthesized option list. The new `TABLESPACE` option uses this list. | `REINDEX (TABLESPACE ts, VERBOSE) TABLE t;` | RN14 "Utility Commands"; gram.y `ReindexStmt` 8401, 8413; commits b5913f612, c5b286047 |
| 10 | `CLUSTER` takes a parenthesized option list | `CLUSTER (VERBOSE) t USING t_idx;` | gram.y `ClusterStmt` 10612; commit b5913f612 (not in RN14) |
| 11 | `VACUUM`, `ANALYZE`, `EXPLAIN`, `REINDEX` and `CLUSTER` share `utility_option_list` (10708). New option names, for example `PROCESS_TOAST` and `INDEX_CLEANUP auto`, are option values, not grammar. | `VACUUM (PROCESS_TOAST false) t;` | gram.y `utility_option_name` 10731; RN14 "Vacuuming"; commit 7cb3048f3 |
| 12 | `DECLARE ... ASENSITIVE CURSOR` | `DECLARE c ASENSITIVE CURSOR FOR SELECT 1;` | gram.y `cursor_options` 11273; commit dd13ad9d39a (not in RN14) |
| 13 | `COMMENT`, `SECURITY LABEL`, `DROP` and `ALTER EXTENSION ADD/DROP` share general object-type productions. The grammar accepts more object types for `SECURITY LABEL` and `ALTER EXTENSION`. Execution rejects the types that the statement does not support. | `SECURITY LABEL ON INDEX i IS 'x';` (parses; execution rejects it) | gram.y `object_type_name` 6338, `SecLabelStmt` 6581; commit a332b366d4f (not in RN14) |

### Queries and expressions

| # | Change | Example | Source |
|---|---|---|---|
| 14 | Most keywords can be bare column labels without `AS`. Only 39 keywords still need `AS` (see Keywords). | `SELECT 1 year_total, 2 analyze;` | RN14 "SELECT, INSERT"; gram.y `target_el` 15029 (`a_expr BareColLabel`), `BareColLabel` 15449, `bare_label_keyword` 15973; commit 06a7c3154 |
| 15 | CTE `SEARCH { DEPTH \| BREADTH } FIRST BY ... SET col` and `CYCLE cols SET col [TO v DEFAULT v] USING col` | `WITH RECURSIVE t(n) AS (...) SEARCH DEPTH FIRST BY n SET ord SELECT ...` | RN14 "SELECT, INSERT"; gram.y `opt_search_clause` 11562, `opt_cycle_clause` 11587/11599; commits 3696a600e, f4adc41c4 |
| 16 | `GROUP BY DISTINCT`, and the explicit default quantifier `GROUP BY ALL` before a grouping list. This `ALL` keeps duplicate grouping sets. It is not the "group by every non-aggregate column" `GROUP BY ALL` that 19 added and then reverted. | `GROUP BY DISTINCT CUBE (a, b), CUBE (b, c)` | RN14 "SELECT, INSERT"; gram.y `group_clause` 11928, `set_quantifier` 11699; commit be45be9c3 |
| 17 | Alias on `JOIN ... USING` | `SELECT j.x FROM a JOIN b USING (x) AS j;` | RN14 "SELECT, INSERT"; gram.y `join_qual` 12392, `opt_alias_clause_for_join_using` 12328; commit 055fee7eb |
| 18 | `SUBSTRING(text SIMILAR pattern ESCAPE esc)` | `SELECT substring('abc' SIMILAR 'a#"b#"c' ESCAPE '#');` | RN14 "Functions"; gram.y `substr_list` 14883; commit 78c887679 |
| 19 | `OVERLAY(...)` and `SUBSTRING(...)` also accept a plain argument list (`func_arg_list_opt`), including named notation | `SELECT overlay('abcdef', 'X', 2, 3);` | gram.y `func_expr_common_subexpr` 14123, 14158; commit 40c24bfef |
| 20 | Some query features have no gram.y change. They are parse-analysis changes: generic subscripting and `jsonb` subscripting (c7aba7c14, 676887a3b), `DEFAULT` in multi-row `INSERT ... VALUES` (17958972f), and table-qualified columns in `ON CONFLICT ... WHERE` (6c0373ab7). | `UPDATE t SET j['a'] = '1';` | RN14 "Data Types", "SELECT, INSERT" |

Functions: RN14 "Functions" lists about 20 new plain `f(x)` functions.

### Lexical and literal syntax

- `scan.l` has no lexical change. The only change is the new `collabel` argument of the `PG_KEYWORD` macro.
- The postfix-operator rule `a_expr qual_Op %prec POSTFIXOP` is removed (`REL_13_0` gram.y 13228; commit 1ed6b8956). See the breaking changes below.
- New operators in the catalog (no grammar change): `<<|` and `|>>` for points (0cc993278), and `pg_lsn +/- numeric` (9bae7e4cd).
- Internal parse modes `MODE_TYPE_NAME`, `MODE_PLPGSQL_EXPR` and `MODE_PLPGSQL_ASSIGN1..3` are added, with the productions `PLpgSQL_Expr` and `PLAssignStmt` (commit c9d529848). They are not SQL syntax.

### Keywords

Commit 06a7c3154 adds the `BARE_LABEL`/`AS_LABEL` column to `kwlist.h`. No keyword changes category and no keyword is removed.

| Keyword | Change | Category |
|---|---|---|
| `asensitive`, `atomic`, `breadth`, `compression`, `depth`, `finalize`, `return` | new | UNRESERVED, BARE_LABEL |
| all others | new label column | BARE_LABEL, except the 39 AS_LABEL words below |

The AS_LABEL keywords in 14 are: `array as char character create day except fetch filter for from grant group having hour intersect into isnull limit minute month notnull offset on order over overlaps precision returning second to union varying where window with within without year`.

### Removed or changed syntax (breaking)

- **Postfix (right-unary) operators are removed.** `SELECT 5 !;` is a syntax error. `CREATE OPERATOR` rejects an operator with only `RIGHTARG`. RN14 "Migration to Version 14"; commit 1ed6b8956.
- **The operators `!` and `!!` (factorial) are removed** from the catalog. Use `factorial()`. RN14 "Migration"; commit 76f412ab3.
- **The geometric operators `@` and `~` (containment) are removed** from the catalog. Use `<@` and `@>`. RN14 "Migration"; commits 2f70fdb06, 112d411fb.
- **`IS [NOT] OF (type_list)` is removed.** `SELECT x IS OF (int)` is a syntax error. `REL_13_0` gram.y 13439, 13653 are deleted; commit 926fa801ac9. RN14 does not list this change.
- **`CREATE LANGUAGE` and `DROP LANGUAGE` do not accept a string literal as the name.** `CREATE LANGUAGE 'plpgsql'` fails. `CREATE FUNCTION ... LANGUAGE 'sql'` still works. RN14 "Migration"; gram.y `CreatePLangStmt` 4477; commit 5333e014a.
- **Publication names in `CREATE SUBSCRIPTION` and `ALTER SUBSCRIPTION ... SET PUBLICATION` are `name` (`ColId`), not `ColLabel`.** An unquoted reserved word, for example `PUBLICATION select`, is a syntax error. gram.y `CreateSubscriptionStmt` 9678; commit a6964bc1bb0. RN14 does not list this change.
- **Change of meaning: `EXTRACT` returns `numeric`, not `float8`.** `EXTRACT(field FROM date)` gives an error for units that `date` does not have. The parse does not change. RN14 "Migration"; commit a2da77cdb.
- **Change of meaning: deparsed views keep SQL-standard function syntax.** For example, `EXTRACT(...)` is no longer shown as `date_part(...)`. RN14 "Utility Commands"; commit 40c24bfef.

## PostgreSQL 15

Change set `REL_14_0..REL_15_0`. RN15 = `REL_15_0:doc/src/sgml/release-15.sgml`. gram.y = `REL_15_0:src/backend/parser/gram.y`.

SQL/JSON went into the 15 development branch and was removed before 15.0 (commit `96ef3237bf7`, "Revert SQL/JSON features"). `REL_15_0` gram.y has no JSON production, so this section does not list SQL/JSON.

### New statements

| Statement | Example | Source |
|---|---|---|
| `MERGE`, with optional `WITH`. The actions are `WHEN [NOT] MATCHED [AND cond] THEN` followed by `UPDATE SET ...`, `DELETE`, `INSERT [(cols)] [OVERRIDING {SYSTEM\|USER} VALUE] VALUES (...)`, `INSERT DEFAULT VALUES` or `DO NOTHING`. `INSERT` is permitted only under `NOT MATCHED`. `UPDATE` and `DELETE` are permitted only under `MATCHED`. `MERGE` is an `ExplainableStmt` and a `PreparableStmt`. It has no `RETURNING`. | `MERGE INTO t USING s ON t.id = s.id WHEN MATCHED AND s.del THEN DELETE WHEN MATCHED THEN UPDATE SET v = s.v WHEN NOT MATCHED THEN INSERT VALUES (s.id, s.v);` | RN15 "Utility Commands", commit 7103ebb7a; gram.y `MergeStmt` (12182), `merge_when_clause` (12205), `opt_merge_when_condition` (12249), `merge_update` (12254), `merge_delete` (12267), `merge_insert` (12280), `merge_values_clause` (12328) |

The grammar also accepts `MERGE` in a CTE body and in `COPY (...) TO`, because both use `PreparableStmt` (`common_table_expr` 12631, `CopyStmt` 3283). Parse analysis rejects the CTE case with "MERGE not supported in WITH query" (`REL_15_0:src/backend/parser/parse_cte.c:133`).

### Changes to existing statements

| Change | Example | Source |
|---|---|---|
| `CREATE PUBLICATION` and `ALTER PUBLICATION ... ADD\|SET\|DROP` take a list of publication objects. An object is `TABLE rel` or `TABLES IN SCHEMA {name \| CURRENT_SCHEMA}`. After the first object, an item can omit the `TABLE` or `TABLES IN SCHEMA` word. (`FOR ALL TABLES IN SCHEMA` existed only during development. Commit f256236fb removed it.) | `CREATE PUBLICATION p FOR TABLES IN SCHEMA s1, s2;` | RN15 "Logical Replication", commits 5a2832465, f256236fb; gram.y `CreatePublicationStmt` (10310), `PublicationObjSpec` (10353, 10363, 10370), `pub_obj_list` (10431), `AlterPublicationStmt` (10454) |
| A published table takes a column list | `CREATE PUBLICATION p FOR TABLE t (a, b);` | RN15 "Logical Replication", commit 923def9a5; `PublicationObjSpec: TABLE relation_expr opt_column_list OptWhereClause` |
| A published table takes a row filter. The filter needs parentheses. | `CREATE PUBLICATION p FOR TABLE t WHERE (x > 0);` | RN15 "Logical Replication", commit 52e4f0cd4; `OptWhereClause` (4203) |
| `ALTER SUBSCRIPTION ... SKIP (option = value)` | `ALTER SUBSCRIPTION sub SKIP (lsn = '0/14C0378');` | RN15 "Logical Replication", commit 208c5d65b; `AlterSubscriptionStmt` (10606) |
| `ALTER DATABASE ... REFRESH COLLATION VERSION` | `ALTER DATABASE db REFRESH COLLATION VERSION;` | RN15 "Server", commit 37851a8b8; `AlterDatabaseStmt` (11146) |
| `ALTER TABLE ... SET ACCESS METHOD` | `ALTER TABLE t SET ACCESS METHOD heap2;` | RN15 "Utility Commands", commit b0483263d; `alter_table_cmd` (2792) |
| `NULLS [NOT] DISTINCT` on `UNIQUE` column and table constraints and on `CREATE UNIQUE INDEX`. In `CREATE INDEX` the clause comes after `INCLUDE` and before `WITH`. | `CREATE UNIQUE INDEX i ON t (a) NULLS NOT DISTINCT;` | RN15 "Indexes", commit 94aa7cc5f; `opt_unique_null_treatment` (3905), used in `ColConstraintElem` (3787), `ConstraintElem` (4015), `IndexStmt` (7859) |
| A foreign-key `SET NULL` or `SET DEFAULT` action takes a column list. The grammar accepts it after `ON UPDATE` too, but the action code rejects that form. | `... REFERENCES p ON DELETE SET NULL (b)` | RN15 "Utility Commands", commit d6f96ed94; `key_action` (4277) |
| `GRANT`/`REVOKE ... ON PARAMETER name [, ...]`. A name is a dotted `ColId` list. The new privilege is `ALTER SYSTEM`. The privilege `SET` uses the existing `privilege: ColId` path. | `GRANT SET, ALTER SYSTEM ON PARAMETER work_mem TO r;` | RN15 "Privileges", commit a0ffa885e; `privilege` (7449), `privilege_target: PARAMETER parameter_name_list` (7601), `parameter_name_list` (7466) |

These options have no gram.y change. Generic productions accept them, and the backend validates them:

- `COPY ... (HEADER MATCH)`. `HEADER` also works in text format (43f33dc01, 072132f04).
- `CREATE DATABASE ... STRATEGY`, `OID`, `LOCALE_PROVIDER`, `ICU_LOCALE` (9c08aea6a, aa0105141, f2553d430).
- `CREATE UNLOGGED SEQUENCE` and `ALTER SEQUENCE ... SET LOGGED|UNLOGGED` (344d62fb9). The 14 grammar already accepted `UNLOGGED` in `CreateSeqStmt`.
- `CREATE VIEW ... WITH (security_invoker = true)`, which is a reloption (7faa5fc84).

### Queries and expressions

- `numeric(p, s)` accepts a negative scale or a scale larger than the precision, for example `'1234'::numeric(4,-2)`. The type modifier is already a general `expr_list`, so gram.y does not change. RN15 "Data Types", commit 085f931f5.
- There is no other new query or expression syntax. `a_expr: UNIQUE select_with_parens` gets `opt_unique_null_treatment`, but the `UNIQUE (subquery)` predicate still gives "not implemented".
- Functions: RN15 "Functions" lists about 20 new plain `f(x)` functions, for example `regexp_count()`.

### Lexical and literal syntax

| Change | Example | Source |
|---|---|---|
| A numeric literal that is followed directly by an identifier character is an error ("trailing junk after numeric literal"). An incomplete exponent is also an error. | `SELECT 123abc;` was `123 AS abc`, and is now an error. `SELECT 0x1F;` was `0 AS x1F`, and is now an error. `SELECT 1e+;` is an error. | RN15 "Migration to Version 15", commit 2549f0661; `REL_15_0:src/backend/parser/scan.l` rules `integer_junk`, `decimal_junk`, `real_junk`, `realfail` |
| A parameter that is followed directly by an identifier character is an error ("trailing junk after parameter") | `SELECT $1abc;` | commit 2549f0661; scan.l `param_junk` |
| RN15 says that `U&""` is now rejected. The core scanner already rejected a zero-length delimited identifier in 14 (`REL_14_0:scan.l` 779, 789). The commit changes only ecpg. The grammar wins: this is not a server change. | `SELECT 1 AS U&"";` | RN15 "Migration", commit a18b6d2dc |
| jsonpath numeric literals accept `.1` and `1.`, and reject trailing junk. This is inside a jsonpath string, not SQL lexing. | `'$.a ? (@ > .1)'::jsonpath` | RN15 "Migration", commit e26114c81 |

### Keywords

| Keyword | Change | Category |
|---|---|---|
| `matched`, `merge`, `parameter` | new | UNRESERVED, BARE_LABEL |

No keyword changes category and no keyword is removed.

### Removed or changed syntax (breaking)

- **Trailing junk after a numeric literal or a parameter is a lexical error.** In 14, `SELECT 123abc` returned the column `abc` with the value 123. `0x10` was `0 AS x10`. `$1abc` was `$1 AS abc`. `1e` and `1e+` lexed as `1` and an identifier or operator. RN15 "Migration", commit 2549f0661.
- The other "Migration" items are not syntax changes. `CREATE OR REPLACE VIEW` now rejects a change of column collation (commit 2523928b2). This is a semantic check.

## PostgreSQL 16

Change set `REL_15_0..REL_16_0`. RN16 = `REL_16_0:doc/src/sgml/release-16.sgml`. gram.y = `REL_16_0:src/backend/parser/gram.y`.

### New statements

PostgreSQL 16 adds no new top-level statement.

### Changes to existing statements

| Statement | Change | Example | Source |
|---|---|---|---|
| `CREATE TABLE` | A column definition accepts `STORAGE`, including `STORAGE DEFAULT` | `CREATE TABLE t (a text STORAGE EXTERNAL);` | RN16 "Utility Commands"; `columnDef` 3707, `column_storage` 3781; commits 784cedda0, b9424d014 |
| `ALTER TABLE ... SET STORAGE` | Accepts `DEFAULT`. In 15 the argument was `ColId`, which excludes the reserved word `DEFAULT`. | `ALTER TABLE t ALTER a SET STORAGE DEFAULT;` | `alter_table_cmd` 2472; commit b9424d014 |
| `CREATE STATISTICS` | The name is optional, but only without `IF NOT EXISTS` | `CREATE STATISTICS ON a, b FROM t;` | RN16 "Utility Commands"; `CreateStatsStmt` 4476, 4490; commit 624aa2a13 |
| `REINDEX DATABASE`, `REINDEX SYSTEM` | The name is optional | `REINDEX DATABASE;` | RN16 "Utility Commands"; `ReindexStmt` 9065; commits 2cbc3c17a, 0a5f06b84 |
| `GRANT role TO role` | Accepts `WITH` and a list of `ColLabel {OPTION \| TRUE \| FALSE}`. The options are `ADMIN`, `INHERIT` and `SET`. Parse analysis validates the names. | `GRANT r TO u WITH INHERIT FALSE, SET TRUE, ADMIN OPTION;` | RN16 "Privileges"; `GrantRoleStmt` 7771, `grant_role_opt` 7819; commits e3ce2de09, 3d14e171e |
| `REVOKE ... OPTION FOR role` | `ADMIN` is generalized to `ColId`, so `INHERIT OPTION FOR` and `SET OPTION FOR` parse | `REVOKE INHERIT OPTION FOR r FROM u;` | `RevokeRoleStmt` 7797; commit e3ce2de09 |
| `CREATE DATABASE` | A numeric option value is `NumericOnly`, not `SignedIconst`, so `OID` values above 2^31 parse | `CREATE DATABASE d OID = 3000000000;` | `createdb_opt_item` 11130; commit 34fa0ddae5c |
| Utility options | `utility_option_elem` accepts `FORMAT_LA`, so `EXPLAIN (FORMAT JSON)` still works after the `FORMAT JSON` lookahead | `EXPLAIN (FORMAT JSON) SELECT 1;` | `utility_option_elem` 11705 |

New option names with no gram.y change:

- `EXPLAIN (GENERIC_PLAN)` (3c05284d8).
- `COPY FROM ... (DEFAULT '\D')` (9f8377f7a).
- `VACUUM (PROCESS_MAIN ...)`, `SKIP_DATABASE_STATS`, `ONLY_DATABASE_STATS` (4211fbd84, a46a7011b).
- Subscription options `password_required`, `run_as_owner`, `origin`, and `streaming = parallel` (c3afe8cf5, 482675987, 366283961, 216a78482).
- `CREATE COLLATION ... (rules = ...)` and `CREATE DATABASE ... ICU_RULES` (30a53b792).

### Queries and expressions

| Construct | Syntax | Example | Source |
|---|---|---|---|
| Subquery in `FROM` with no alias | `select_with_parens opt_alias_clause` no longer requires the alias. This also applies to `VALUES` and `LATERAL`. | `SELECT * FROM (SELECT 1);` | RN16 "General Queries"; `table_ref` 13290, 13299; commit bcedd8f5f |
| `JSON_OBJECT(...)` | `JSON_OBJECT(json_name_and_value_list [NULL\|ABSENT ON NULL] [WITH\|WITHOUT UNIQUE [KEYS]] [RETURNING Typename [FORMAT JSON [ENCODING name]]])`. A pair is `c_expr VALUE json_value_expr` or `a_expr ':' json_value_expr`. The empty form `JSON_OBJECT([RETURNING ...])` is accepted. The legacy call `json_object(func_arg_list)` still parses. | `SELECT JSON_OBJECT('a' : 1, 'b' VALUE 2 ABSENT ON NULL WITH UNIQUE KEYS RETURNING jsonb);` | RN16 "Functions"; `func_expr_common_subexpr` 15555, 15561, 15575; `json_name_and_value` 16415; commit 7081ac46a |
| `JSON_ARRAY(...)` | `JSON_ARRAY(json_value_expr_list [NULL\|ABSENT ON NULL] [RETURNING ...])`, `JSON_ARRAY(select_no_parens [FORMAT JSON] [RETURNING ...])`, or `JSON_ARRAY([RETURNING ...])` | `SELECT JSON_ARRAY(SELECT a FROM t);` | RN16 "Functions"; 15586, 15600, 15616; commit 7081ac46a |
| `JSON_OBJECTAGG`, `JSON_ARRAYAGG` | `JSON_OBJECTAGG(json_name_and_value [null clause] [unique clause] [RETURNING ...])`; `JSON_ARRAYAGG(json_value_expr [ORDER BY ...] [null clause] [RETURNING ...])`. Both accept `FILTER` and `OVER`. | `SELECT JSON_ARRAYAGG(a ORDER BY a ABSENT ON NULL) FILTER (WHERE a > 0) FROM t;` | RN16 "Functions"; `json_aggregate_func` 16446, `func_expr` 15231; commit 7081ac46a |
| `FORMAT JSON [ENCODING name]` on a value | `json_value_expr: a_expr json_format_clause_opt` | `JSON_ARRAY('[1]' FORMAT JSON)` | `json_value_expr` 16353, `json_format_clause_opt` 16362 |
| `IS [NOT] JSON` | `a_expr IS [NOT] JSON [VALUE\|ARRAY\|OBJECT\|SCALAR] [WITH\|WITHOUT UNIQUE [KEYS]]` | `SELECT '{"a":1}' IS JSON OBJECT WITH UNIQUE KEYS;` | RN16 "Functions"; `a_expr` 14840, 14858; `json_predicate_type_constraint` 16391; commit 6ee30209a |
| `SYSTEM_USER` | A special SQL-value keyword with no parentheses, like `CURRENT_USER`. RN16 calls it a "server variable". The grammar parses it as a special expression, so the grammar wins. | `SELECT SYSTEM_USER;` | RN16 "Functions"; `func_expr_common_subexpr` 15316; commit 0823d061b |
| `XMLSERIALIZE ... [NO] INDENT` | `XMLSERIALIZE(document_or_content a_expr AS SimpleTypename xml_indent_option)` | `SELECT XMLSERIALIZE(DOCUMENT x AS text INDENT);` | RN16 "Functions"; 15544, `xml_indent_option` 15678; commit 483bdb2af |

The grammar wins over the SQL standard in three places. These forms do not parse in 16:

- `KEY k VALUE v` in `JSON_OBJECT`. This form is commented out at 16415.
- `expr FORMAT JSON IS JSON`. This form is commented out at 14850 and 14869.
- `NULL ON NULL` in `JSON_ARRAY(SELECT ...)`. This form is commented out at 15600.

Functions: RN16 "Functions" lists about 10 new plain `f(x)` functions, for example `any_value` and `array_sample`.

### Lexical and literal syntax

| Change | Example | Source |
|---|---|---|
| Hexadecimal, octal and binary integer literals (`0x`, `0o`, `0b`) | `SELECT 0x42F, 0o273, 0b100101;` | RN16 "Data Types"; scan.l `hexinteger`, `octinteger`, `bininteger`; commit 6fcda9aba |
| A prefix with no digit is an error ("invalid hexadecimal integer" and the octal and binary forms) | `SELECT 0x;` | scan.l `hexfail`, `octfail`, `binfail`; commit 6fcda9aba |
| Underscores between digits in integer, numeric, real and non-decimal literals. A `_` must come before a digit. | `SELECT 1_000_000, 1_000.000_5, 0x_FF, 1e1_0;` | RN16 "Data Types"; scan.l `decinteger {decdigit}(_?{decdigit})*`; commit faff8f8e4 |
| A parameter uses `decinteger`, so `$1_0` lexes as a parameter in 16.0. 17 reverses this (see 17). | `$1_0` | scan.l `param` |
| jsonpath strings accept the same new numeric forms | `'$ ? (@ > 0x10)'::jsonpath` | RN16 "General Queries"; commit 102a5c164 |
| Lookahead tokens: `parser.c` changes `FORMAT` to `FORMAT_LA` when `JSON` follows, and `WITHOUT` to `WITHOUT_LA` when `TIME` follows | `timestamp WITHOUT TIME ZONE` | `REL_16_0:src/backend/parser/parser.c` (case `FORMAT`, line 197); gram.y 795, 14295 |

### Keywords

| Keyword | Change | Category |
|---|---|---|
| `absent`, `format`, `indent`, `json`, `keys`, `scalar` | new | UNRESERVED, BARE_LABEL |
| `json_array`, `json_arrayagg`, `json_object`, `json_objectagg` | new | COL_NAME, BARE_LABEL |
| `system_user` | new | RESERVED, BARE_LABEL |

No keyword is removed. No existing keyword changes category. The token `REF` is renamed to `REF_P`, but its spelling in SQL does not change (commit 717ec1aae).

### Removed or changed syntax (breaking)

- **`system_user` is a reserved keyword.** An unquoted `system_user` cannot be a table, column, type or function name. It is still valid as a bare column label. Commit 0823d061b.
- **`json_array`, `json_arrayagg`, `json_object` and `json_objectagg` are COL_NAME keywords.** An unqualified call to a user function with one of these names goes to the SQL/JSON syntax, except `json_object(func_arg_list)`, which a legacy production keeps. These words cannot be unquoted type or function names in DDL. A schema-qualified name still works. Commit 7081ac46a.
- **`FORMAT` followed by `JSON` is a lookahead token.** An identifier `format` followed by the word `json` becomes `FORMAT_LA`, which cannot be a column reference. For example, `SELECT format json FROM t` (the column `format` with the bare alias `json`) fails. This comes from reading `parser.c` and gram.y. It was not run on a server.
- **Views created from `ON SELECT` rules are rejected.** `CREATE RULE "_RETURN" AS ON SELECT TO t DO INSTEAD SELECT ...` no longer changes a table into a view. The grammar does not change. RN16 "Migration"; commit b23cd185f.
- **Change of meaning: `REINDEX DATABASE` skips the indexes on system catalogs.** Use `REINDEX SYSTEM` for them. RN16 "Migration"; commit 2cbc3c17a.
- **Change of meaning: `GRANT role` sets the membership's `INHERIT` status from the member role's `INHERIT` attribute.** `WITH INHERIT` overrides it. RN16 "Migration"; commit e3ce2de09.
- Execution now rejects some DDL, with no grammar change:
  - A primary key that uses a `NULLS NOT DISTINCT` index (d95952325).
  - Parent and child tables whose `GENERATED` status does not match (8bf6ec3ba).
- Datetime string input (not SQL grammar). The input form `YyearMmonthDday` is removed (5b3c59535). `epoch` and `infinity` with other fields are rejected (bcc704b52).

## PostgreSQL 17

Change set `REL_16_0..REL_17_0`. RN17 = `REL_17_0:doc/src/sgml/release-17.sgml`. gram.y = `REL_17_0:src/backend/parser/gram.y`. scan.l = `REL_17_0:src/backend/parser/scan.l`.

Three grammar features were added during development and reverted before `REL_17_0`, so 17.0 does not have them:

- `ALTER TABLE ... MERGE/SPLIT PARTITION(S)` (reverted in 84f594da358).
- Temporal keys, `WITHOUT OVERLAPS` and `PERIOD` (reverted in 8aee330af55).
- The structural not-null constraint changes (reverted in 6f8bb7c1e96).

The keyword `plan` was added for `JSON_TABLE ... PLAN` (de3600452b6), but no production uses it. 17.0 has no `PLAN` clause.

### New statements

17 adds no new top-level statement.

### Changes to existing statements

| Change | Example | Source |
|---|---|---|
| `ALTER TABLE ... ALTER COLUMN ... SET EXPRESSION AS (expr)` | `ALTER TABLE t ALTER COLUMN g SET EXPRESSION AS (a * 2);` | RN17 "Utility Commands"; `alter_table_cmd` 2441; commit 5d06e99a3 |
| `SET STATISTICS DEFAULT` (`set_statistics_value`: `SignedIconst` or `DEFAULT`) for `ALTER TABLE/INDEX ... ALTER ... SET STATISTICS` and `ALTER STATISTICS` | `ALTER TABLE t ALTER c SET STATISTICS DEFAULT;` | RN17 "Utility Commands"; `set_statistics_value` 3093, lines 2470, 2480, 4662, 4671; commits 4f622503d, 012460ee9 |
| `ALTER TABLE ... SET ACCESS METHOD DEFAULT` (`set_access_method_name`) | `ALTER TABLE t SET ACCESS METHOD DEFAULT;` | RN17 "Utility Commands"; `set_access_method_name` 3098, line 2882; commits d61a6cad6, 374c7a229 |
| `COPY` legacy options `FORCE NOT NULL *` and `FORCE NULL *` | `COPY t FROM STDIN CSV FORCE NOT NULL *;` | RN17 "Utility Commands"; `copy_opt_item` 3475, 3483; commit f6d4c9cf1 |
| `COPY` options `ON_ERROR stop\|ignore` and `LOG_VERBOSITY default\|verbose`. `copy_generic_opt_arg` accepts the reserved word `DEFAULT` as an argument. | `COPY t FROM STDIN (ON_ERROR ignore, LOG_VERBOSITY default);` | RN17 "Utility Commands"; `copy_generic_opt_arg` 3539; commits 9e2d87011, b725b7eec, f5a227895 |
| `CLUSTER (options)` with no table name | `CLUSTER (VERBOSE);` | RN17 "Utility Commands"; `ClusterStmt` 11739; commit cdaedfc96 |
| `MERGE ... WHEN NOT MATCHED BY SOURCE`, and the explicit `WHEN NOT MATCHED BY TARGET`. `BY SOURCE` takes `UPDATE`, `DELETE` or `DO NOTHING`. `[BY TARGET]` takes `INSERT` or `DO NOTHING`. | `MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED BY SOURCE THEN DELETE;` | RN17 "MERGE"; `merge_when_tgt_matched` 12503, `merge_when_tgt_not_matched` 12508, `merge_when_clause` 12459; commit 0294df2f1 |
| `MERGE ... RETURNING` | `MERGE INTO t USING s ON ... WHEN MATCHED THEN UPDATE SET v = s.v RETURNING merge_action(), t.*;` | RN17 "MERGE"; `MergeStmt` 12428; commit c649fa24a |
| `ALTER DOMAIN ... ADD [CONSTRAINT name] NOT NULL`. `ADD` now takes `DomainConstraint`, which accepts only `CHECK` and `NOT NULL`. In 16 the grammar accepted any `TableConstraint`, and execution rejected the other kinds. | `ALTER DOMAIN d ADD CONSTRAINT nn NOT NULL;` | `DomainConstraint` 4265, `DomainConstraintElem` 4277, line 11552; commit 9895b35cb (not in RN17) |
| `ALTER OPERATOR ... SET (...)` accepts an option with no value. It can set `HASHES`, `MERGES`, `COMMUTATOR` and `NEGATOR`. | `ALTER OPERATOR === (int, int) SET (HASHES, MERGES);` | RN17 "Additional Modules"; `operator_def_elem` 10248; commit 2b5154bea |
| Sequence option `LOGGED`/`UNLOGGED` in `SeqOptElem`. Only identity-column options accept it. | `... GENERATED ALWAYS AS IDENTITY (UNLOGGED)` | `SeqOptElem` 4911, 4951; commit f7567f9e53d (back-patched to 15 and 16; not in RN17) |
| `ALTER TYPE name DROP VALUE 'x'` parses, and then always gives "dropping an enum value is not implemented" | `ALTER TYPE e DROP VALUE 'a';` | `AlterEnumStmt` 6513; commit af3ee8a086c |

New option values and names with no gram.y change:

- `EXPLAIN (MEMORY)` and `EXPLAIN (SERIALIZE [TEXT|BINARY|NONE])` (5de890e36, 06286709e).
- Subscription option `failover` (776621a5e).
- The privilege `MAINTAIN` (ecb0fd337).
- The event trigger event `login` (e83d1b0c4).
- `MERGE` into updatable views, which is a semantic change (5f2e179bd).

### Queries and expressions

| Construct | Example | Source |
|---|---|---|
| `JSON_TABLE(context, path [AS name] [PASSING ...] COLUMNS (...) [behavior ON ERROR])` as a `FROM` item, with optional `LATERAL` and alias. The column kinds are: `name FOR ORDINALITY`; `name type [FORMAT JSON] [PATH 'p'] [wrapper] [quotes] [ON EMPTY] [ON ERROR]`; `name type EXISTS [PATH 'p'] [ON ERROR]`; `NESTED [PATH] 'p' [AS name] COLUMNS (...)`. The path must be a string constant. | `SELECT * FROM JSON_TABLE(j, '$.a[*]' COLUMNS (i FOR ORDINALITY, x int PATH '$.x' DEFAULT 0 ON EMPTY, NESTED PATH '$.b[*]' COLUMNS (y text PATH '$'))) AS jt;` | RN17 "Functions"; `json_table` 14131, `json_table_column_definition` 14171, `table_ref` 13519, 13526, `json_table_column_path_clause_opt` 14270; commits de3600452, bb766cde6 |
| `JSON_QUERY(ctx, path [PASSING v AS n, ...] [RETURNING type [FORMAT JSON]] [wrapper] [quotes] [ON EMPTY] [ON ERROR])` | `JSON_QUERY(j, '$.a' WITH CONDITIONAL WRAPPER OMIT QUOTES EMPTY ARRAY ON EMPTY)` | RN17 "Functions"; `func_expr_common_subexpr` 16043; commit 6185c9737 |
| `JSON_EXISTS(ctx, path [PASSING ...] [ON ERROR])` | `JSON_EXISTS(j, '$.a ? (@ > $x)' PASSING 1 AS x FALSE ON ERROR)` | RN17 "Functions"; 16065; commit 6185c9737 |
| `JSON_VALUE(ctx, path [PASSING ...] [RETURNING type] [ON EMPTY] [ON ERROR])` | `JSON_VALUE(j, '$.n' RETURNING int DEFAULT -1 ON ERROR)` | RN17 "Functions"; 16081; commit 6185c9737 |
| Shared SQL/JSON clauses. `PASSING expr AS ColLabel` (`json_passing_clause_opt` 16825). Wrapper `WITH\|WITHOUT [CONDITIONAL\|UNCONDITIONAL] [ARRAY] WRAPPER` (`json_wrapper_behavior` 16847). Quotes `KEEP\|OMIT QUOTES [ON SCALAR STRING]` (`json_quotes_clause_opt` 16940). Behaviors `ERROR`, `NULL`, `TRUE`, `FALSE`, `UNKNOWN`, `EMPTY [ARRAY]`, `EMPTY OBJECT`, `DEFAULT expr`, each followed by `ON EMPTY` or `ON ERROR` (`json_behavior` 16859, `json_behavior_clause_opt` 16878). Format `FORMAT JSON [ENCODING UTF8\|UTF16\|UTF32]` (`json_format_clause` 16905). | `... WITHOUT ARRAY WRAPPER KEEP QUOTES ON SCALAR STRING NULL ON EMPTY ERROR ON ERROR` | commit 6185c9737 |
| `JSON(expr [FORMAT JSON [ENCODING ...]] [WITH\|WITHOUT UNIQUE [KEYS]])` | `JSON('{"a":1}' WITH UNIQUE KEYS)` | RN17 "Functions"; 16007; commit 03734a7fe |
| `JSON_SCALAR(expr)` | `JSON_SCALAR(1.5)` | RN17 "Functions"; 16017; commit 03734a7fe |
| `JSON_SERIALIZE(expr [FORMAT JSON] [RETURNING type [FORMAT JSON]])` | `JSON_SERIALIZE('{"a":1}'::json RETURNING bytea)` | RN17 "Functions"; 16026; commit 03734a7fe |
| `merge_action()`, a special construct with no arguments, valid only in `MERGE ... RETURNING` | `RETURNING merge_action()` | RN17 "MERGE"; 16035; commit c649fa24a |
| `expr AT LOCAL`, with the precedence of `AT TIME ZONE` | `SELECT now() AT LOCAL;` | RN17 "Functions"; `a_expr` 14792, `%left AT` 894; commit 97957fdba |
| `json` is a dedicated type production `JsonType` in `SimpleTypename` and `ConstTypename`, not a `GenericType`. It takes no type modifier. | `SELECT '1'::json;` still works | `JsonType` 14737; commit 03734a7fe |
| The `XMLTABLE` column option `PATH expr` is an explicit production, because `path` is now a keyword. Users see no change. | `XMLTABLE('/r' PASSING x COLUMNS a int PATH 'a')` | `xmltable_column_option_el` 14101; commit de3600452 |

Functions: RN17 "Functions" lists about 20 new plain `f(x)` functions (for example `to_bin()`, `random(min, max)`) and new jsonpath methods (in jsonpath strings, not SQL).

### Lexical and literal syntax

| Change | Example | Source |
|---|---|---|
| Vertical tab (`\v`, 0x0B) is whitespace in SQL text. Before 17, it was an invalid character. | `SELECT<VT>1;` | scan.l `space` 222; commit ae6d06f0968 (not in RN17) |
| In `E''` strings, `\v` means vertical tab. Before 17, it meant the letter `v`. | `E'a\vb'` | scan.l `unescape_single_char` 1420; commit ae6d06f0968 (not in RN17) |
| A positional parameter accepts only the digits 0 to 9 again. `$1_0` is a "trailing junk" error. 16.0 lexed it as a parameter and read it as `$1`. | `SELECT $1_0;` gives an error | scan.l `param` 416, `param_junk` 438; commit 98b4f53d156 (back-patched to 16.x as 315661ecafb) |
| One `integer_junk` rule that uses `{identifier}` replaces four rules. The same inputs are rejected. | `SELECT 123abc;` still gives an error | scan.l 435; commit 7dcbf0afa28 |
| `interval` input: `ago` must come last, and an empty unit cannot occur twice. `interval` input also accepts `'infinity'`. These are type input rules, not grammar. | `interval '1 day ago 2 hours'` is rejected | RN17 "Migration to Version 17", commits 165d581f1, 617f9b7d4; RN17 "Data Types", 519fc1bd9 |

### Keywords

All new keywords are BARE_LABEL.

| Keyword | Change | Category |
|---|---|---|
| `json` | **category change: UNRESERVED to COL_NAME** (03734a7fe) | COL_NAME |
| `json_exists`, `json_query`, `json_value` | new (6185c9737) | COL_NAME |
| `json_scalar`, `json_serialize` | new (03734a7fe) | COL_NAME |
| `json_table` | new (de3600452) | COL_NAME |
| `merge_action` | new (c649fa24a) | COL_NAME |
| `conditional`, `empty`, `error`, `keep`, `omit`, `quotes`, `string`, `unconditional` | new (6185c9737) | UNRESERVED |
| `path`, `plan` | new (de3600452). No production uses `plan`. | UNRESERVED |
| `nested` | new (bb766cde6) | UNRESERVED |
| `source`, `target` | new (0294df2f1) | UNRESERVED |

### Removed or changed syntax (breaking)

- **`json` is a COL_NAME keyword.** A COL_NAME keyword cannot be an unqualified function or type name in `type_function_name`. `json(x)` now parses as the SQL/JSON `JSON()` constructor, not as a call to a user function `json`. `CREATE FUNCTION json(...)` fails unless the name has a schema. A column named `json` and the type `json` still work. Commit 03734a7fe.
- **New COL_NAME keywords** `json_exists`, `json_query`, `json_scalar`, `json_serialize`, `json_table`, `json_value` and `merge_action`. An unqualified call to a user function with one of these names parses as the special construct, or fails. These names cannot be unqualified function or type names in DDL. They are still valid as column names, table names and bare labels.
- **`E'\v'` changes meaning.** It was the letter `v`. Now it is a vertical tab. Commit ae6d06f0968.
- **`$n` with underscores is rejected.** 16.0 accepted `$1_0` and read it as `$1`. Commit 98b4f53d156, also in the 16.x minor releases.
- **`ago` in `interval` input.** An `ago` before other fields is rejected. This is type input. RN17 "Migration".
- `ALTER DOMAIN ... ADD` accepts less in the grammar. `UNIQUE`, `PRIMARY KEY`, `EXCLUDE` and `FOREIGN KEY` passed the 16 grammar, failed later, and are now syntax errors. No statement that worked in 16 stops working, so this is not a break.

## PostgreSQL 18

Change set `REL_17_0..REL_18_0`. RN18 = `REL_18_0:doc/src/sgml/release-18.sgml`. gram.y = `REL_18_0:src/backend/parser/gram.y`.

### New statements

18 adds no new top-level statement. The `stmt` production gets no new alternative.

### Changes to existing statements

| # | Change | Example | Source |
|---|---|---|---|
| 1 | Generated columns can be virtual. Virtual is the default. `STORED` is optional. | `c int GENERATED ALWAYS AS (a * 2) VIRTUAL` / `c int GENERATED ALWAYS AS (a * 2)` | RN18 "Utility Commands", commit 83ea6c540; `ColConstraintElem` 4025, `opt_virtual_or_stored` 4081 |
| 2 | Temporal `PRIMARY KEY` and `UNIQUE`: `WITHOUT OVERLAPS` on the last key column | `PRIMARY KEY (id, valid_at WITHOUT OVERLAPS)` | RN18 "Constraints", commit fc0438b4e; `ConstraintElem` 4230, 4265; `opt_without_overlaps` 4411 |
| 3 | Temporal foreign keys: `PERIOD` on the last column of both column lists | `FOREIGN KEY (id, PERIOD valid_at) REFERENCES t (id, PERIOD valid_at)` | RN18 "Constraints", commit 89f908a6d; `ConstraintElem` 4319; `optionalPeriodName` 4426; `opt_column_and_period_list` 4431 |
| 4 | `ENFORCED` and `NOT ENFORCED` constraint attributes, for `CHECK` and foreign keys, in column and table form. `processCASbits` rejects them for other constraint kinds. | `CHECK (a > 0) NOT ENFORCED`; `REFERENCES p NOT ENFORCED` | RN18 "Constraints", commits ca87c415e, eec0040c4; `ConstraintAttr` 4102; `ConstraintAttributeElem` 6243 |
| 5 | `ALTER TABLE ... ALTER CONSTRAINT name` accepts `[NOT] ENFORCED` and `NO INHERIT`. The new form `ALTER CONSTRAINT name INHERIT` is added. | `ALTER TABLE t ALTER CONSTRAINT c NOT ENFORCED;` | RN18 "Constraints", commits f4e53e10b, 4a02af8b1, eec0040c4; `alter_table_cmd` 2656, 2687 |
| 6 | `NOT NULL` as a table constraint. It can have a name, and it accepts `NO INHERIT` and `NOT VALID`. | `CONSTRAINT nn NOT NULL a NOT VALID`; `ALTER TABLE t ADD NOT NULL a` | RN18 "Constraints", commits 14e87ffa5, a379061a2; `ConstraintElem` 4217 |
| 7 | The column constraint `NOT NULL` accepts `NO INHERIT` | `a int NOT NULL NO INHERIT` | commit 14e87ffa5; `ColConstraintElem` 3946 `NOT NULL_P opt_no_inherit` |
| 8 | `ALTER DEFAULT PRIVILEGES ... ON LARGE OBJECTS` | `ALTER DEFAULT PRIVILEGES GRANT SELECT ON LARGE OBJECTS TO r;` | RN18 "Privileges", commit 0d6c47766; `defacl_privilege_target` 8183 |
| 9 | `VACUUM` and `ANALYZE` take a relation expression: `ONLY name`, `name *` | `VACUUM ONLY parent;` | RN18 "Utility Commands", commit 62ddf7ee9; `vacuum_relation` 12021 (`relation_expr opt_name_list`) |
| 10 | `CREATE FOREIGN TABLE ... (LIKE src)` works. gram.y does not change; the fix is in `parse_utilcmd.c`. | `CREATE FOREIGN TABLE ft (LIKE t) SERVER s;` | RN18 "Utility Commands", commit 302cf1575 |
| 11 | `COPY` options `REJECT_LIMIT n` and `LOG_VERBOSITY silent`, with no gram.y change | `COPY t FROM 'f' (ON_ERROR ignore, REJECT_LIMIT 10);` | RN18 "COPY", commits 4ac2a9bec, e7834a1a2 |

RN18 "EXPLAIN" lists only output changes, for example `BUFFERS` is on by default with `ANALYZE` (c2a4078eb). gram.y does not change.

### Queries and expressions

| # | Change | Example | Source |
|---|---|---|---|
| 12 | `RETURNING` accepts the qualifiers `old` and `new`, and an optional `WITH (OLD AS a, NEW AS b)` clause that renames them. This applies to `INSERT`, `UPDATE`, `DELETE` and `MERGE`. | `UPDATE t SET v = v + 1 RETURNING old.v, new.v;` / `DELETE FROM t RETURNING WITH (OLD AS o) o.*;` | RN18 "Utility Commands", commit 80feb727c; `returning_clause` 12377, `returning_with_clause` 12392, `returning_option` 12402, `returning_option_kind` 12414 |

The `in_expr` nonterminal is removed. `a_expr IN_P select_with_parens` and `a_expr IN_P '(' expr_list ')'` (15273, 15286) replace it. The accepted language does not change.

Functions: RN18 "Functions" and "Data Types" list new plain `f(x)` functions, for example `uuidv7()`, `casefold()` and `array_sort()`. `EXTRACT(WEEK FROM interval)` is semantic only.

### Lexical and literal syntax

| # | Change | Source |
|---|---|---|
| 13 | A positional parameter whose number does not fit in `int32` is a lexer error ("parameter number too large"). Before 18, the value overflowed without an error. RN18 does not list this change. | `REL_18_0:src/backend/parser/scan.l` 994 `{param}`; commit d35cd061998 (not in `REL_17_9`) |

The other `scan.l` changes (b1ef48980dd, fadff3fc945) are not visible to users.

### Keywords

| Keyword | Change | Category |
|---|---|---|
| `enforced`, `objects`, `period`, `virtual` | new | UNRESERVED, BARE_LABEL |
| `recheck` | **removed** (7da1bdc2c2f) | was UNRESERVED, BARE_LABEL |

No keyword changes category.

### Removed or changed syntax (breaking)

- **`RECHECK` in `CREATE OPERATOR CLASS` is removed.** 17 accepted `OPERATOR n op RECHECK` with a NOTICE (`opt_recheck`). 18 gives a syntax error. `opclass_item` 6698; commit 7da1bdc2c2f. RN18 does not list this change.
- **Change of meaning: `VACUUM` and `ANALYZE` on an inheritance parent also process its children.** Use `ONLY` for the old behavior. RN18 "Migration to Version 18"; commit 62ddf7ee9.
- **Unlogged partitioned tables are rejected.** `CREATE UNLOGGED TABLE ... PARTITION BY` and `ALTER TABLE ... SET UNLOGGED` on a partitioned table give an error. Before 18, they had no effect. The check is in `tablecmds.c`. RN18 "Migration"; commit e2bab2d79.
- **The `RULE` privilege is removed.** `GRANT RULE ON t TO r` is rejected. The check is in `aclchk.c`. RN18 "Migration"; commit fefa76f70.
- **`COPY ... FREEZE` into a foreign table is rejected.** RN18 "COPY"; commit 401a6956f.
- **A parameter number that does not fit in `int32` is a lexer error** (item 13).
- Data format, not SQL: in CSV mode, `COPY FROM` a file no longer treats `\.` as the end of the data (770233748, da8a4c166).

## PostgreSQL 19 (pre-release)

**PostgreSQL 19 is not released.** Change set `REL_18_0..b73d13c` (`REL_19_STABLE`, "Stamp 19beta4", 2026-09-21). This is the pin of the `pg19-beta` target version (`pg-oracle/pins.tsv`). RN19 = `b73d13c:doc/src/sgml/release-19.sgml`. gram.y, scan.l and kwlist.h = the same commit. Line numbers in this section refer to `b73d13c`, unless an item names another commit.

The first version of this section used the snapshot `7a74e5ed92d` (2026-08-28). The subsection "Changes from `7a74e5ed92d` to `b73d13c`" gives the differences. Five commits changed the parser in that range. Three of them are reverts, and they remove much of the 19 syntax.

The grammar wins over the notes in two cases:

- RN19 at `7a74e5ed92d` listed `ALTER TABLE ... MERGE PARTITIONS` and `SPLIT PARTITION` (f2e4cc427, 4b3d17362). Commit `3e8bcc8644f` (2026-08-27) removed both productions. RN19 at `b73d13c` does not list them. 19 does not have this syntax.
- `GROUP BY ALL` (ef38a4d9756) was added and then reverted by `372b8d1adb7` (2026-07-17). It is not in gram.y, and RN19 does not list it.

### Changes from `7a74e5ed92d` to `b73d13c`

Files compared: `src/backend/parser/gram.y`, `scan.l`, `parser.c`, `src/include/parser/kwlist.h` and `doc/src/sgml/release-19.sgml`. `parser.c` did not change.

| Commit | Change | Effect on this section |
|---|---|---|
| `2b9e1aff4d3` (2026-09-07) | Reverts SQL/PGQ (2f094e7ac and the later fixes). | `CREATE/ALTER PROPERTY GRAPH`, `PROPERTY GRAPH` as an object type, `GRAPH_TABLE`, graph patterns and the `labeled_expr_list` use by PGQ are removed. The keywords `destination`, `edge`, `graph`, `graph_table`, `node`, `properties`, `property`, `relationship` and `vertex` are removed. In scan.l, the `RIGHT_ARROW` token (`->`) is removed, and `\|` is no longer a "self" character. In gram.y, the `RIGHT_ARROW` and `'\|'` arms of `a_expr`, `b_expr` and `MathOp` (8d2beee027a) are removed. `->` and `\|` are generic `Op` again, as in 18. The rename of `xml_attribute_list` to `labeled_expr_list` stays. |
| `a9d2f728240` (2026-09-15) | Reverts `UPDATE/DELETE ... FOR PORTION OF` (8e72d914c). | `for_portion_of_clause` is removed, the keyword `portion` is removed, and the precedence changes for `TO` and `USING` and the `%prec IS` on `opt_interval` are removed. The precedence declarations are the same as in 18. |
| `3c5d28ba64e` (2026-09-11) | Reverts "Support more object types within CREATE SCHEMA" (d51697484). | `schema_stmt` accepts only the 18 elements again. |
| `f23de46e15b` (2026-08-29) | Disallows `ONLY` in `REPACK`. | `RepackStmt` uses `qualified_name opt_name_list`, not `vacuum_relation`. Thus `REPACK ONLY t` and `REPACK t *` are syntax errors. |
| `7635a320679` (2026-09-02) | Query jumbling for `WAIT FOR LSN`. | `WaitStmt` records the location of the LSN literal. No syntax change. |

RN19 at `b73d13c` also removes the items for SQL/PGQ, `FOR PORTION OF`, the new `CREATE SCHEMA` elements, `MERGE/SPLIT PARTITIONS`, online checksums and the `pg_get_*_ddl()` functions. It renames the `WAIT FOR` documentation page to `WAIT`. The grammar still starts the statement with `WAIT FOR LSN`. RN19 adds a `SUPPORT` clause for `CREATE AGGREGATE` (165aa5040). `CREATE AGGREGATE` takes a generic `definition`, so this has no gram.y change.

### New statements

| Statement | Example | Source |
|---|---|---|
| `REPACK [(options)] [table [(cols)] [USING INDEX [name]]]`, and `REPACK [(options)] USING INDEX` with no table. The table is a `qualified_name`, so `ONLY` and `*` are not accepted (`f23de46e15b`). It replaces `VACUUM FULL` and `CLUSTER`. `CONCURRENTLY` is an option in the list, not a keyword. | `REPACK (CONCURRENTLY, VERBOSE) t USING INDEX t_pkey;` | RN19 "Utility Commands", commits ac58465e0, 28d534e2a, f23de46e15b; `RepackStmt` (12080), `opt_usingindex` (1148), `opt_name_list` (12268) |
| `WAIT FOR LSN 'lsn' [WITH (option [, ...])]`. RN19 names the command `WAIT`. | `WAIT FOR LSN '0/3000060' WITH (MODE 'replay', TIMEOUT '1s');` | RN19 "Streaming Replication and Recovery", commits 447aae13b, 49a181b5d; `WaitStmt` (16635), `opt_wait_with_clause` (16646) |

### Changes to existing statements

| Change | Example | Source |
|---|---|---|
| `INSERT ... ON CONFLICT [target] DO SELECT [FOR UPDATE\|NO KEY UPDATE\|SHARE\|KEY SHARE] [WHERE ...] RETURNING ...` | `INSERT INTO t VALUES (1) ON CONFLICT (id) DO SELECT FOR UPDATE RETURNING *;` | RN19 "Query Commands", commit 88327092f; `opt_on_conflict` (12579), `opt_for_locking_strength` (13826) |
| `CHECKPOINT [(option [, ...])]`, for example `MODE`, `FLUSH_UNLOGGED`. The option list is shared with `REINDEX`, `ANALYZE` and `CLUSTER` (1dfe3ef3f96). That refactor does not change what those statements accept. | `CHECKPOINT (MODE SPREAD, FLUSH_UNLOGGED);` | RN19 "Utility Commands", commits a4f126516, 2f698d7f4, 8d33fbacb; `CheckPointStmt` (2100), `opt_utility_option_list` (1159) |
| Publications: `FOR ALL SEQUENCES`, and a list of `ALL` objects. `ALTER PUBLICATION ... SET ALL SEQUENCES`. The raw parser rejects a list that names `ALL TABLES` or `ALL SEQUENCES` two times (`preprocess_pub_all_objtype_list`). | `CREATE PUBLICATION p FOR ALL TABLES, ALL SEQUENCES;` | RN19 "Logical Replication", commit 96b378497; `PublicationAllObjSpec` (10907), `pub_all_obj_type_list` (10923), `CreatePublicationStmt` (10772), `AlterPublicationStmt` (10971) |
| Publications: `FOR ALL TABLES EXCEPT (TABLE t1 [, [TABLE] t2 ...])`, and `ALTER PUBLICATION ... SET ALL TABLES EXCEPT (...)`. Each table is a `relation_expr`. | `CREATE PUBLICATION p FOR ALL TABLES EXCEPT (TABLE t1, t2);` | RN19 "Logical Replication", commits fd366065e, 493f8c643, 5984ea868; `opt_pub_except_clause` (10902), `pub_except_obj_list` (10941) |
| Subscriptions: `CREATE SUBSCRIPTION s SERVER fsrv PUBLICATION p`, `ALTER SUBSCRIPTION s SERVER fsrv`, `ALTER SUBSCRIPTION s REFRESH SEQUENCES` | `ALTER SUBSCRIPTION s REFRESH SEQUENCES;` | RN19 "Logical Replication", commits 8185bb534, f0b3573c3; `CreateSubscriptionStmt` (11034), `AlterSubscriptionStmt` (11063) |
| `CREATE/ALTER FOREIGN DATA WRAPPER ... CONNECTION func \| NO CONNECTION` | `CREATE FOREIGN DATA WRAPPER w CONNECTION f;` | RN19 "Utility Commands", commit 8185bb534; `fdw_option` (5541) |
| `SET var TO NULL` / `SET var = NULL` empties a list-valued setting | `SET search_path TO NULL;` | RN19 "Server Configuration", commit ff4597acd; `generic_set` (1706) |
| `COPY ... TO ... (FORMAT json)`. The production needs a new `FORMAT_LA copy_generic_opt_arg` arm, because `json` is a COL_NAME keyword and `parser.c` changes `FORMAT` before `JSON` to `FORMAT_LA`. 18 rejects `(FORMAT json)`. The legacy unparenthesized `COPY t TO STDOUT JSON` is also accepted. | `COPY t TO STDOUT (FORMAT json);` | RN19 "COPY", commit 7dadd38cd; `copy_opt_item` (3546), `copy_generic_opt_elem` (3648) |
| `CREATE CONSTRAINT TRIGGER ... ENFORCED` is accepted and has no effect. In 18, `processCASbits` rejects it, because the trigger passes no `is_enforced` pointer. `NOT VALID`, `NO INHERIT` and `NOT ENFORCED` are still rejected, with a new error message. RN19 does not list this change. | `CREATE CONSTRAINT TRIGGER tr AFTER INSERT ON t ENFORCED FOR EACH ROW EXECUTE FUNCTION f();` | commit 87251e11496; `CreateTrigStmt` (6096), `processCASbits` (19753) |

These RN19 items have no gram.y change:

- The `COPY` options `FORCE_ARRAY` (4c0390ac5), `ON_ERROR set_null` (2a525cc97) and `HEADER n` (bc2f348e8).
- `COPY partitioned_table TO` (4bea91f21).
- `GRANTED BY` now selects the effective grantor (dd1398f13).
- `ALTER CONSTRAINT ... [NOT] ENFORCED` works on `CHECK` constraints (342051d73).
- `EXPLAIN (IO)` (681daed93).
- `CLUSTER` builds a `RepackStmt` node, but it accepts the same syntax (ac58465e0).
- `ANALYZE` uses `opt_utility_option_list`, and `FETCH` records its direction keyword. Neither changes the syntax.
- The `SUPPORT` clause of `CREATE AGGREGATE` (165aa5040).

### Queries and expressions

- **`IGNORE NULLS` / `RESPECT NULLS`** after `FILTER` and before `OVER`. RN19 says that `lead`, `lag`, `first_value`, `last_value` and `nth_value` support it. The grammar accepts it after any `func_application`, also with no `OVER`. Example: `lag(x) IGNORE NULLS OVER w`. RN19 "Query Commands", commit 25a30bbd4; `func_expr` (16017), `null_treatment` (16668).
- A rename with no language change: `xml_attribute_list` is now `labeled_expr_list` (16568).
- Semantic changes only: `IS JSON` works on domains (3b4c2b9db). `GROUP BY` accepts target-list subqueries that refer to grouped columns (415100aa6).
- Removed before the pin (see "Changes from `7a74e5ed92d` to `b73d13c`"): `GRAPH_TABLE` and graph patterns, `FOR PORTION OF`, and the `RIGHT_ARROW` and `'|'` arms of `a_expr` and `b_expr`.

### Lexical and literal syntax

- **`standard_conforming_strings` is always on.** `SET standard_conforming_strings = off` gives an error. A plain `'...'` literal always uses the standard state, so a backslash in it is literal. The `escape_string_warning` setting is removed. `E'...'` does not change. RN19 "Migration to Version 19", commit 457620845; scan.l `{xqstart}` (541).
- At `b73d13c`, scan.l has no other change from 18. The `RIGHT_ARROW` token and the single-character `|` of `7a74e5ed92d` are reverted (`2b9e1aff4d3`).

### Keywords

| Keyword | Change | Category |
|---|---|---|
| `lsn`, `repack`, `wait` | new | UNRESERVED, BARE_LABEL |
| `ignore`, `respect` | new | UNRESERVED, **AS_LABEL** |

No keyword is removed. No existing keyword changes category. Source: `kwlist.h` diff `REL_18_0..b73d13c`. At `7a74e5ed92d`, the list also had `destination`, `edge`, `graph`, `node`, `portion`, `properties`, `property`, `relationship`, `vertex` (UNRESERVED, BARE_LABEL) and `graph_table` (COL_NAME, BARE_LABEL). The reverts removed them.

### Removed or changed syntax (breaking)

- **`ignore` and `respect` need `AS` as column labels.** `SELECT 1 ignore;` is a syntax error. Write `SELECT 1 AS ignore;`. Both words are still valid as `ColId`. Commit 25a30bbd4.
- **A backslash in a plain string literal is always literal.** `SET standard_conforming_strings = off` gives an error. With that setting off, `'a\nb'` contained a newline. Dumps made with the setting off do not load correctly. RN19 "Migration", commit 457620845.
- **`COPY FROM ... WHERE` rejects system columns.** The check is in analysis. RN19 "Migration", commit 21c69dc73.
- **CR or LF in database, role and tablespace names is rejected.** RN19 "Migration", commit b380a56a3.
- **Change of meaning: `JSON_ARRAY(query)` with no rows returns `[]`, not NULL.** RN19 "Migration", commit 8d829f5a0.
- **Change of meaning: `USING gist (inetcol)` selects the core `inet_ops` opclass,** not the btree_gist opclass. RN19 "Migration", commit b352d3d80.
- **The `MULE_INTERNAL` encoding is rejected.** RN19 "Migration", commit 77645d44e.
- At `7a74e5ed92d`, `graph_table` was a new COL_NAME keyword, and `CREATE SCHEMA` ran its elements in the order written (a9c350d9e). Both are reverted at `b73d13c` (`2b9e1aff4d3`, `e0fdc3f54b4`).

## Breaking changes across 14–19

| Version | Change | What used to work | What happens now | Source |
|---|---|---|---|---|
| 14 | Postfix operators removed | `SELECT 5 !;`, user-defined right-unary operators | Syntax error | RN14 "Migration to Version 14"; `REL_13_0` gram.y 13228 deleted; 1ed6b8956 |
| 14 | `!` and `!!` removed (catalog) | `SELECT 5 !`, `SELECT !! 5` | Error; use `factorial()` | RN14 "Migration"; 76f412ab3 |
| 14 | Geometric `@` and `~` removed (catalog) | `box @ box`, `box ~ box` | Operator does not exist; use `<@` / `@>` | RN14 "Migration"; 2f70fdb06, 112d411fb |
| 14 | `IS [NOT] OF` removed | `SELECT x IS OF (integer)` | Syntax error | `REL_13_0` gram.y 13439, 13653 deleted; 926fa801ac9 (not in RN14) |
| 14 | String literal as a language name | `CREATE LANGUAGE 'plpgsql'` | Syntax error | RN14 "Migration"; `REL_14_0` gram.y `CreatePLangStmt` 4477; 5333e014a |
| 14 | Subscription publication names are `ColId` | `... PUBLICATION select` (reserved word unquoted) | Syntax error | `REL_14_0` gram.y `CreateSubscriptionStmt` 9678; a6964bc1bb0 (not in RN14) |
| 14 | `EXTRACT` returns `numeric` (meaning) | `EXTRACT(epoch FROM ts)` returned `float8` | Returns `numeric`; `EXTRACT(hour FROM date)` errors | RN14 "Migration"; a2da77cdb |
| 15 | Trailing junk after a numeric literal | `SELECT 123abc` = `123 AS abc`; `0x1F` = `0 AS x1F` | Lexical error | RN15 "Migration to Version 15"; `REL_15_0` scan.l `integer_junk` and related rules; 2549f0661 |
| 15 | Incomplete exponent | `1e`, `1e+` | Lexical error | 2549f0661; scan.l `realfail` |
| 15 | Trailing junk after a parameter | `$1abc` = `$1 AS abc` | Lexical error | 2549f0661; scan.l `param_junk` |
| 16 | `system_user` is RESERVED | Unquoted `system_user` as a table, column or function name | Syntax error (a bare label still works) | kwlist.h; `REL_16_0` gram.y `reserved_keyword`; 0823d061b |
| 16 | `json_array`, `json_arrayagg`, `json_object`, `json_objectagg` are COL_NAME | Unqualified user function or type with one of these names | Parses as SQL/JSON syntax, or fails in DDL | kwlist.h; `REL_16_0` gram.y 15555–15616, 16446; 7081ac46a |
| 16 | `FORMAT` + `JSON` lookahead | `SELECT format json FROM t` | Syntax error (unconfirmed on a server) | `REL_16_0:src/backend/parser/parser.c` case `FORMAT` |
| 16 | `ON SELECT` rule views removed | `CREATE RULE "_RETURN" AS ON SELECT TO tbl ...` changed a table into a view | Error at execution | RN16 "Migration to Version 16"; b23cd185f |
| 16 | `REINDEX DATABASE` scope (meaning) | Reindexed catalog indexes too | Skips catalog indexes | RN16 "Migration"; 2cbc3c17a |
| 16 | Role membership `INHERIT` (meaning) | `ALTER ROLE ... [NO]INHERIT` affected all memberships | Each membership keeps its own status from `GRANT` time | RN16 "Migration"; e3ce2de09 |
| 16 | `NULLS NOT DISTINCT` index as a primary key | Accepted | Error at execution | RN16 "Migration"; d95952325 |
| 16 | `GENERATED` status across inheritance | Parent and child could differ | Error | RN16 "Migration"; 8bf6ec3ba |
| 17 | `json` UNRESERVED to COL_NAME | `json(x)` called a user function; `CREATE FUNCTION json(...)` | `json(...)` is the `JSON()` constructor; an unqualified function or type named `json` fails | kwlist.h; `REL_17_0` gram.y 16007, `JsonType` 14737; 03734a7fe |
| 17 | New COL_NAME keywords `json_exists`, `json_query`, `json_scalar`, `json_serialize`, `json_table`, `json_value`, `merge_action` | Unqualified user functions or types with these names | Parses as the special construct, or fails | kwlist.h; `REL_17_0` gram.y `col_name_keyword`; 6185c9737, 03734a7fe, de3600452, c649fa24a |
| 17 | `E'\v'` | The letter `v` | Vertical tab (0x0B) | `REL_17_0` scan.l 1420; ae6d06f0968 |
| 17 | Underscores in `$n` | `$1_0` read as `$1` (16.0–16.3) | Syntax error | `REL_17_0` scan.l 416; 98b4f53d156 |
| 17 | `ago` in `interval` input | `interval '1 day ago 2 hours'` | Rejected | RN17 "Migration to Version 17"; 165d581f1, 617f9b7d4 |
| 18 | `RECHECK` removed from `CREATE OPERATOR CLASS` | `OPERATOR 1 = RECHECK` with a NOTICE | Syntax error; `recheck` is no longer a keyword | `REL_18_0` gram.y `opclass_item` 6698; 7da1bdc2c2f (not in RN18) |
| 18 | `VACUUM`/`ANALYZE` on an inheritance parent (meaning) | Processed only the parent | Also processes the children; `ONLY` gives the old behavior | RN18 "Migration to Version 18"; 62ddf7ee9 |
| 18 | Unlogged partitioned tables | Accepted, with no effect | Error | RN18 "Migration"; e2bab2d79 |
| 18 | `RULE` privilege | `GRANT RULE ON t TO r` accepted, did nothing | Error | RN18 "Migration"; fefa76f70 |
| 18 | `COPY FREEZE` into a foreign table | Accepted, `FREEZE` ignored | Error | RN18 "COPY"; 401a6956f |
| 18 | Parameter number overflow | `$99999999999` lexed, and the value wrapped | Lexer error "parameter number too large" | `REL_18_0` scan.l 994; d35cd061998 (not in RN18) |
| 19 | `ignore`, `respect` need `AS` | `SELECT 1 ignore` | Syntax error | kwlist.h at b73d13c; 25a30bbd4 |
| 19 | `standard_conforming_strings` always on | `SET standard_conforming_strings = off`; `'a\nb'` as an escape | `SET` errors; the backslash is literal | RN19 "Migration to Version 19"; 457620845 |
| 19 | System columns in `COPY FROM ... WHERE` | `COPY t FROM stdin WHERE xmin <> 0` | Rejected | RN19 "Migration"; 21c69dc73 |
| 19 | CR/LF in database, role, tablespace names | `CREATE ROLE "a<LF>b"` | Rejected | RN19 "Migration"; b380a56a3 |
| 19 | `JSON_ARRAY(query)` with no rows (meaning) | NULL | `[]` | RN19 "Migration"; 8d829f5a0 |
| 19 | Default GiST opclass for `inet`/`cidr` (meaning) | btree_gist opclass | Core `inet_ops` | RN19 "Migration"; b352d3d80 |
| 19 | `MULE_INTERNAL` encoding | `ENCODING 'MULE_INTERNAL'` | Rejected | RN19 "Migration"; 77645d44e |

## Notes for pg-sql

pg-sql's grammar and differential oracle are pinned to PostgreSQL 17.9. A read-only grep of `src/` shows that pg-sql does not model the syntax items below. A grep is evidence, not proof.

PostgreSQL 18:

- Virtual generated columns: `VIRTUAL`, and `GENERATED ALWAYS AS (expr)` without `STORED`. `src/ast/ddl/table.rs` has only the stored form.
- `WITHOUT OVERLAPS` in `PRIMARY KEY` and `UNIQUE`.
- `PERIOD` in foreign-key column lists.
- `ENFORCED` / `NOT ENFORCED` constraint attributes.
- `ALTER TABLE ... ALTER CONSTRAINT name [NOT] ENFORCED | [NO] INHERIT`.
- `NOT NULL col` as a table constraint, and `NOT NULL NO INHERIT` on a column. (`NO INHERIT` exists only for `CHECK`.)
- `ALTER DEFAULT PRIVILEGES ... ON LARGE OBJECTS`. There is no `OBJECTS` token.
- `VACUUM ONLY` / `ANALYZE ONLY`. `src/ast/utility/vacuum.rs` uses `QualifiedName`, not a relation expression.
- `RETURNING WITH (OLD AS a, NEW AS b)`. The qualifiers `old.x` and `new.x` already parse as ordinary column references.
- The removal of `RECHECK`. pg-sql still accepts it, as 17 does (`src/ast/ddl/operator.rs:103`, `src/tokens.rs:649`).
- The lexer error for a parameter number that overflows `int32`. Not checked.

PostgreSQL 19 (pre-release), at `b73d13c`:

- `REPACK`.
- `WAIT FOR LSN`.
- `IGNORE NULLS` / `RESPECT NULLS`, and the AS_LABEL status of `ignore` and `respect`.
- `INSERT ... ON CONFLICT ... DO SELECT`.
- `CHECKPOINT (options)`. `src/ast/utility/checkpoint.rs` has only the bare form.
- Publication `FOR ALL SEQUENCES`, lists of `ALL` objects, and `ALL TABLES EXCEPT (TABLE ...)`.
- `CREATE/ALTER SUBSCRIPTION ... SERVER`, and `ALTER SUBSCRIPTION ... REFRESH SEQUENCES`.
- `FOREIGN DATA WRAPPER ... CONNECTION func | NO CONNECTION`.
- `SET var TO NULL`.
- The legacy unparenthesized `COPY ... JSON` option, and `(FORMAT json)`, which needs the `FORMAT_LA` arm.
- `CREATE CONSTRAINT TRIGGER ... ENFORCED`. It needs the 18 `ENFORCED` constraint attribute first.
- The keywords `lsn`, `repack`, `wait`, `ignore` and `respect`.
- The `standard_conforming_strings = off` removal (lexer state). pg-sql always lexes as if the setting is on (ADR 0009), so there is nothing to do.

The snapshot `7a74e5ed92d` also had property graphs, `GRAPH_TABLE`, `FOR PORTION OF`, the new `CREATE SCHEMA` elements, the `RIGHT_ARROW` token and the single-character `|`. `b73d13c` reverts all of them, so the `pg19-beta` target version does not add them.
