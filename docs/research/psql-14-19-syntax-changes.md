# psql syntax changes in PostgreSQL 14 to 19

Research date: 2026-09-22.
Reference source: `vendor/postgres`, read from git objects only. The tags and commits are:

- `REL_13_0` (`29be9983a64`), the baseline for 14
- `REL_14_24` (`6b3806732b7`)
- `REL_15_19` (`2ff1375b5dd`)
- `REL_16_15` (`7d3e000c596`)
- `REL_17_11` (`083ac033419`)
- `REL_18_6` (`724edf9bde9`)
- PostgreSQL 19: `b73d13c` (`REL_19_STABLE`, "Stamp 19beta4", 2026-09-21), the planned pin. The first version of this document used `7a74e5ed92d` (2026-08-28).

**The 19 pin.** The first version of this document used `7a74e5ed92d`, because `b73d13c` was not in the local submodule then. Increment #78 fetched `b73d13c` and compared the two for `psqlscan.l`, `psqlscan_int.h`, `psqlscanslash.l`, `command.c` and RN19. The section "PostgreSQL 19" gives the result. `b73d13c` reverts the two lexer changes and `\dG`, because PostgreSQL reverted SQL/PGQ (`2b9e1aff4d3`) and the new `CREATE SCHEMA` elements (`3c5d28ba64e`). Facts in this document that come from `7a74e5ed92d` say so.

**PostgreSQL 19 is pre-release.** Its psql can change before 19.0.

The pins are the **latest minor releases**, not the `.0` releases. For version N, the change set is `REL_(N-1)_latest .. REL_N_latest`. For 14, it is `REL_13_0 .. REL_14_24`. Thus a change that was back-patched into minor releases shows in the change set of the version where it first appears, and it is also in every later pin. The section "Back-patched changes" lists these changes and the first minor release of each branch that has them.

The sources are:

- The SQL lexer of psql: `src/fe_utils/psqlscan.l`, with `src/include/fe_utils/psqlscan_int.h`.
- The backslash-command lexer: `src/bin/psql/psqlscanslash.l`.
- The meta-command dispatch: `exec_command()` in `src/bin/psql/command.c`. The send path is `MainLoop()` in `src/bin/psql/mainloop.c` and `SendQuery()` / `ExecQueryAndProcessResults()` in `src/bin/psql/common.c`.
- The release notes: `doc/src/sgml/release-N.sgml` at the pin of N. In this document, "RN*N*" means that file. The file at a minor tag also contains the notes for every minor release of that branch.
- The reference page: `doc/src/sgml/ref/psql-ref.sgml`.

Line numbers refer to the file at the pin of that version, unless the item names a different tag. Commit hashes come from `git log` on the range, or from the `<!-- Author ... -->` comments in the release notes. **If the release notes and the source do not agree, the source wins**, and the item says so.

Scope. The document includes the lexing of SQL text in psql, variable interpolation, terminators, the send commands, the lexing of backslash-command arguments, and the list of meta-commands and their argument syntax. It does not include display and formatting, `\d` output columns, prompts, variables that only change output, tab completion or performance.

Terms. This document uses the terms of `CONTEXT.md`: **target version**, **version parity** and **version gate**. For pg-psql, version parity covers only the psql language that pg-psql models: `psqlscan.l` lexing and the send commands. Each change has one of two classes:

- **(A)** The change affects what pg-psql models now: `psqlscan.l` lexing (text runs, interpolation, terminators) or the send commands. An (A) change can need a version gate.
- **(B)** The change affects only meta-commands that pg-psql keeps as unparsed text in the catch-all `MetaCommand` token (`pg-psql/src/ast.rs:284`). A (B) change has no version gate now. It is relevant when #11, #12 and #13 model the meta-commands.

## Summary

| Version | `psqlscan.l` (SQL text, interpolation, terminators) | Send commands and `psqlscanslash.l` | Meta-commands |
|---|---|---|---|
| 14 | `;` inside `BEGIN ... END` of `CREATE FUNCTION/PROCEDURE` does not end the query buffer. | No change. | New: `\dX`. `\df` and `\do` take argument-type patterns. Back-patched: `\restrict`, `\unrestrict` (14.19). |
| 15 | Copies the "trailing junk" rules of `scan.l`. `--` comments inside a query go to the server; 14 removed them. | No change. | New: `\dconfig`, `\getenv`. `\dl+`, `\lo_list+`. |
| 16 | Copies the non-decimal integers and `_` digit separators of `scan.l`. The token extents that matter to interpolation do not change. | No new send command. `\bind` sets parameters for the next send. | New: `\bind`, `\drg`. `\dpS`, `\zS`. `\watch` takes named options `i=` and `c=`. |
| 17 | Vertical tab (`\v`) is whitespace. | Vertical tab ends a command name and an argument. Whole-line arguments of `\sf`, `\ef`, `\sv`, `\ev` lose trailing `;`. | `\watch` takes `m=` (`min_rows`). |
| 18 | No lexing change. | **Nine new commands end the query buffer**: `\parse`, `\sendpipeline`, `\startpipeline`, `\syncpipeline`, `\endpipeline`, `\flushrequest`, `\flush`, `\getresults`, `\close_prepared`. | New: `\bind_named`, `\close_prepared`, `\parse` and the pipeline commands. An `x` suffix on list commands. |
| 19 (pre-release) | No lexing change at `b73d13c`. (`7a74e5ed92d` had a `->` rule, `\|` as a single character and `BEGIN ... END` tracking inside `CREATE SCHEMA`. They are reverted.) | No change. | `\dX+`. (`\dG` at `7a74e5ed92d` is reverted.) |

Verdict. The psql language changes little between 14 and 19. Only three changes alter what pg-psql models in a way that a user can see:

1. The trailing-junk rules in 15 change where an `E'...'` string starts after a number. This moves string ends, and so moves interpolation and `;`.
2. The removal of `--` comments in 14 changes the rendered SQL text of a 14 target.
3. The nine buffer-ending commands in 18 are new terminators.

Most other psql changes are new meta-commands (class B). The security fixes of 2025 and 2026 (`\restrict`, `\unrestrict`, `COPY ... FROM STDIN` data) are in every pin, so they need no version gate among the target versions.

## `psqlscan.l` mirrors `scan.l` at every pin

The header of `psqlscan.l` says that its definitions "should exactly match" `src/backend/parser/scan.l`. This research checked that claim at each pin. It compared the flex definitions section of both files, from `%x xb` to the first `%%`, with comments and blank lines removed. At all six pins the only differences are:

- `scan.l` has the start condition `%x xeu`. `psqlscan.l` does not need it, because psql does not decode Unicode escapes.
- `psqlscan.l` has `variable_char [A-Za-z\200-\377_0-9]`, which is psql-specific.

Thus every lexical change of `scan.l` in 14 to 19 is also in `psqlscan.l` at the same pin. The rules of `psqlscan.l` echo each token (`ECHO`). They do not report errors, so a "junk" or "fail" token that is an error in `scan.l` is text that psql sends to the server. What matters for psql is only the **extent** of each token: a token can change where a string, a comment or a dollar-quoted body starts. That changes where psql recognises an interpolation or a terminator.

## Back-patched changes

The pins are minor releases from August 2026. These changes went into several branches after their `.0` release. The table gives the first minor release of each branch that contains the commit (from `git tag --contains`).

| Change | Class | Commits (per branch) | First minor release | Source |
|---|---|---|---|---|
| `\restrict key` enters restricted mode. In restricted mode, psql rejects every backslash command except `\unrestrict key`. CVE-2025-8714. | B | master `71ea0d67954`; 18 `67a2fbb8f9e`; 17 `575f54d4cee`; 16 `7ad8e790988`; 15 `42404050685`; 14 `e4998d089d9` (also 13) | 14.19, 15.14, 16.10, 17.6, 18.0, 19 | RN14 "Release 14.19" (line 6072), RN15 15.14, RN16 16.10, RN17 17.6. RN18 and RN19 do not list it, but `REL_18_0` and `b73d13c` contain it. `REL_14_24:command.c` 236, 399, 417, 2279, 2616 |
| `\unrestrict` reads the whole rest of the line as its key (`OT_WHOLE_LINE`). It does no backquote or variable expansion, and it removes trailing spaces and `;`. CVE-2026-18408. | B | 19 `0119aa30e0f`; 18 `71ca694c73c`; 17 `0bfac9e1f94`; 16 `33d0c63fb34`; 15 `df245c37458`; 14 `2006fca401e` | 14.24, 15.19, 16.15, 17.11, 18.6, 19 | RN16 "Release 16.15" (line 784) and the same entry in the other branches |
| `psqlscan.l` recognises `COPY ... FROM STDIN` (and `STDOUT`, as the grammar does). psql then skips the in-line data up to `\.`, also when the `COPY` fails. The `\;` rule now resets the statement tracking only at the outer level. CVE-2026-6464. | A (all pins) | 19 `d6ab88d374a`; 18 `29921259e83`; 17 `46fa1f6f373`; 16 `2bdfd5cdcc0`; 15 `3fdcfa8f793`; 14 `5c51ae4556f` | 14.24, 15.19, 16.15, 17.11, 18.6, 19 | RN16 "Release 16.15" (line 179); `REL_14_24:psqlscan.l` 650–663 (`;`), 670–683 (`\;`), 973 (`psqlscan_is_copy_from_stdin`) |
| `\if` saves and restores all lexer state (not only the parenthesis depth) over skipped text. This is a prerequisite of the `COPY` fix. | none | 19 `8cf01e213cc`; 18 `900894d35ca`; 17 `dca6627de0f`; 16 `db96e87f930`; 15 `8cdbabea74a`; 14 `7215c7e9643` | same as the `COPY` fix | `REL_14_24:psqlscanslash.l` `psql_scan_get_lex_state` |
| The junk patterns use `{identifier}`, not `{ident_start}`. | A (see 15) | 15 `f37ac613a83`; 16 `4fd4d7653e2`; 17.0 has it (`7dcbf0afa28`) | 15.9, 16.5 | RN15 "Release 15.9" (line 11247) |
| A positional parameter again accepts only decimal digits (`$1_0` is junk). | A (no effect in psql) | 16 `315661ecafb`; 17.0 has it (`98b4f53d156`) | 16.4 | RN16 "Release 16.4" (line 13702) |

Consequences for a target version:

- A target version means the pinned minor release. An earlier minor release of the same major can differ. For example, 15.13 has no `\restrict`. In 15.0 to 15.8, `1xe'\''` lexes as `1x` and then an `E'...'` string. In 15.9 and later it lexes as `1xe` and then a plain string.
- The first three rows are in all six pins. They need no version gate among the target versions.

## PostgreSQL 14

Change set `REL_13_0..REL_14_24`. RN14 = `REL_14_24:doc/src/sgml/release-14.sgml`.

### `psqlscan.l`

| # | Change | Class | Example | Source |
|---|---|---|---|---|
| 1 | A `;` inside a `BEGIN ... END` block does not end the query buffer when the statement starts with `CREATE [OR REPLACE] {FUNCTION\|PROCEDURE}`. The lexer counts `BEGIN` and `END`, and counts `CASE` only inside a `BEGIN`. It ignores words inside parentheses. It is a heuristic: `CREATE FUNCTION begin() ...` can fool it. The prompt shows continuation while the depth is above zero. | A | `CREATE FUNCTION f() RETURNS int BEGIN ATOMIC SELECT 1; SELECT 2; END;` is one query | `psqlscan.l` 650 (`;` checks `begin_depth == 0`), 961 `psqlscan_is_create_routine`, 1015 `psqlscan_track_identifier`, 1216; commits e717a9a18, 029c5ac03db, d9a9f4b4b92. RN14 lists only the SQL feature (e717a9a18, line 28634), not the psql lexing. |
| 2 | Back-patched `COPY ... FROM STDIN` tracking (see "Back-patched changes"). | A (all pins) | `COPY t FROM STDIN;` followed by data and `\.` | `psqlscan.l` 973; 5c51ae4556f |

The rules for interpolation (`:name`, `:'name'`, `:"name"`, `:{?name}`) do not change between `REL_13_0` and `b73d13c`. Neither do the rules for strings, quoted identifiers, dollar quotes and comments, apart from the items in this document.

Note on 13 behavior that 15 changes: in 13 and 14 the `{whitespace}` rule does not echo a `--` comment (`REL_14_24:psqlscan.l` 388, `if (!(output_buf->len == 0 || yytext[0] == '-'))`). psql 14 therefore removes every `--` comment from the query that it sends. See 15 item 2.

### Send commands and `psqlscanslash.l`

No change. The commands that return `PSQL_CMD_SEND` are `\g`, `\gx`, `\gset`, `\gdesc`, `\gexec` and `\crosstabview`, the same as in 13 (`REL_14_24:command.c` 709, 1418, 1514, 1531, 1558). `\watch` also runs the query buffer and then resets it (`exec_command_watch`, 2784). The changes to `psqlscanslash.l` are encoding safety (42f94f56bf9) and the `popen` mode of backquotes (66f8687a8ff). They do not change the syntax.

### Meta-commands

| Change | Class | Source |
|---|---|---|
| New `\dX [pattern]` lists extended statistics. | B | RN14 line 29324; `command.c` 957 (`case 'X'`); ad600bba0 |
| `\df` and `\do` accept argument-type patterns after the name pattern: `\df[anptwS+] [pattern [arg_pattern ...]]`, `\do[S+] [pattern [arg_pattern [arg_pattern]]]`. | B | RN14 line 29279; `psql-ref.sgml`; a3027e1e7 |
| `\restrict`, `\unrestrict`: back-patched into 14.19 and 14.24 (see "Back-patched changes"). The command diff `REL_13_0..REL_14_24` shows them, but 14.0 does not have them. | B | `command.c` 399, 417 |

RN14 also lists changes to `\e`, `\ef` and `\ev` when the editor exits without a save. These change behavior, not syntax.

## PostgreSQL 15

Change set `REL_14_24..REL_15_19`. RN15 = `REL_15_19:doc/src/sgml/release-15.sgml`.

### `psqlscan.l`

| # | Change | Class | Example | Source |
|---|---|---|---|---|
| 1 | psql copies the "trailing junk" rules of `scan.l`: `integer_junk`, `decimal_junk`, `real_junk`, `param_junk`. A number or parameter that an identifier follows directly is one token. `realfail1` (`1e`) is removed; `realfail` needs a sign (`1e+`). psql echoes these tokens and does not report an error. | A | `1e'\''`, `1.e'\''`, `$1e'\''` | `psqlscan.l` 341, 343–348, rules 862, 880, 883; commit 2549f0661 (15.0), then f37ac613a83 (15.9, `{identifier}` in place of `{ident_start}`) |
| 2 | psql sends `--` comments that are inside a query. It still removes whitespace and comments before the first token of a query. | A | `SELECT 1 -- note` is sent with the comment | RN15 line 24762; `psqlscan.l` 393 (`if (output_buf->len > 0) ECHO;`); commit 83884682f |

Effect of item 1. Most junk tokens have the same extent as the tokens that 14 produced, so interpolation and terminators do not change. **One case changes the extent: a letter `e` or `E` directly after a number, followed by a quote.** In 14, `realfail1` gives back the `e` (`yyless`), and `e'` then starts an extended string (`xestart`, line 215), where a backslash escapes a quote. From 15, `1e` is one junk token, and `'` starts a standard string, where a backslash is an ordinary character (with `standard_conforming_strings` on). The string then ends at a different place. This analysis comes from the flex rules. It was not run.

Example. The input is `SELECT 1e'\' , :'v' ;`.

- psql 14: `1`, then `e'\' , :'` is one extended string. `v` follows, and then `' ;` opens a string with no end. psql sees no interpolation and no terminator.
- psql 15 and later: `1e`, then `'\'` is a complete string. `:'v'` is an interpolation, and `;` ends the query.

The `x`, `b`, `n` and `u&` prefixes do not change an extent, because those strings end at the same quote as a standard string.

### Send commands and `psqlscanslash.l`

No change. `psqlscanslash.l` changes only its copyright line.

### Meta-commands

| Change | Class | Source |
|---|---|---|
| New `\dconfig[+] [pattern]` shows server parameters. | B | RN15 line 24700; `command.c` 803; 3e707fbb4, 5e70d8b5d, 139d46ee2 |
| New `\getenv psql_var env_var` sets a psql variable from the environment. | B | RN15 line 24721; `command.c` 369, 1529; 33d3eeadb |
| `\dl` and `\lo_list` accept `+`. | B | RN15 line 24735; 328dfbdab |
| `\watch` can use a pager (`PSQL_WATCH_PAGER`). The syntax does not change. | none | RN15 line 24749; 7c09d2797 |

## PostgreSQL 16

Change set `REL_15_19..REL_16_15`. RN16 = `REL_16_15:doc/src/sgml/release-16.sgml`.

### `psqlscan.l`

| # | Change | Class | Source |
|---|---|---|---|
| 1 | psql copies the non-decimal integer rules (`hexinteger`, `octinteger`, `bininteger`, `hexfail`, `octfail`, `binfail`) and the `_` digit separators (`decinteger`, `numeric`, `real`). One `integer_junk {decinteger}{identifier}` rule covers all integer forms. | A (no effect) | `psqlscan.l` 340–378, rules 889–931; commits 6fcda9aba, faff8f8e4 |
| 2 | 16.0 defines `param \${decinteger}`, so `$1_0` is one parameter token. 16.4 changes it back to `\${decdigit}+` and `param_junk \${decdigit}+{identifier}`. | A (no effect) | `REL_16_0:psqlscan.l` 358; `REL_16_15:psqlscan.l` 356, 378; 315661ecafb (16.4) |

These changes do not move a string, comment or dollar-quote boundary compared with 15. For example, `0x'..'`, `0b'..'` and `0e'\''` have the same extents in 15 and 16: in 15 the prefix is `integer_junk`, and in 16 it is `hexfail`, `binfail` or `integer_junk`. `1_000` is one token in both (in 15 it is `integer_junk`). This analysis comes from the flex rules. It was not run.

### Send commands and `psqlscanslash.l`

No new command returns `PSQL_CMD_SEND`. `\bind [parameter] ...` is new. It stores parameters and sets the send mode to the extended protocol. The next send (`\g` or `;`) uses them. `\bind` reads its arguments up to the end of the line or the next backslash, so `SELECT $1 \bind 1 \g` is the normal form. `psqlscanslash.l` now sets `SHELL_ERROR` and `SHELL_EXIT_CODE` after a backquote (b0d8f2d983c, 31ae2aa9d2c). The syntax does not change.

### Meta-commands

| Change | Class | Source |
|---|---|---|
| New `\bind [parameter] ...`. | B (see note in "Notes for pg-psql") | RN16 line 21546; `command.c` 329, 484; 5b66de343 |
| New `\drg[S] [pattern]` shows role membership. | B | RN16 line 21468; `command.c` 930; d65ddaca9 |
| `\dp` and `\z` accept `S` (`\dpS`, `\zS`). | B | RN16 line 21491; `command.c` 438; d913928c9 |
| `\watch [i[nterval]=seconds] [c[ount]=times] [seconds]`: named options and an execution count. An interval of 0 is valid. | B | RN16 lines 21566, 21585; `command.c` 2907, 2927; 00beecfe8, 6f9ee74d4 |

## PostgreSQL 17

Change set `REL_16_15..REL_17_11`. RN17 = `REL_17_11:doc/src/sgml/release-17.sgml`.

### `psqlscan.l`

| # | Change | Class | Source |
|---|---|---|---|
| 1 | Vertical tab (`\v`, 0x0B) is whitespace (`space [ \t\n\r\f\v]`). Before 17 it was `other`, an ordinary character. Two effects are visible to psql. psql 17 removes a vertical tab before the first token of a query, and 16 sends it. A vertical tab between two strings now counts as horizontal space for string continuation (`non_newline_space`). The token extents that matter to interpolation and terminators do not change. | A (small) | `psqlscan.l` 162–163, 180; commit ae6d06f0968 (not in RN17) |

The junk and parameter commits in the range (7dcbf0afa28, 98b4f53d156, cd2624fd97b) are the 17.0 copies of the 15.9 and 16.4 back-patches.

### Send commands and `psqlscanslash.l`

| # | Change | Class | Source |
|---|---|---|---|
| 1 | Vertical tab is in `space`, so it ends a backslash command name and an unquoted argument. In 14 to 16, `\g<VT>` is not `\g`: the command name continues to the next space or backslash, and psql reports an invalid command. | A (small) | `REL_17_11:psqlscanslash.l` 113, 151–158; `REL_16_15:psqlscanslash.l` 111; ae6d06f0968 |
| 2 | In whole-line mode, the option `semicolon = true` removes trailing `;` and whitespace. `\sf`, `\ef`, `\sv` and `\ev` use it, so `\sf f;` works. | B | `psqlscanslash.l` 644; `command.c` 1177, 2528; 390298f0806 (not in RN17) |

### Meta-commands

| Change | Class | Source |
|---|---|---|
| `\watch` accepts `m[in_rows]=rows`. | B | RN17 line 16500; `command.c` 2933; f347ec76e |

No meta-command is added or removed in 17.

## PostgreSQL 18

Change set `REL_17_11..REL_18_6`. RN18 = `REL_18_6:doc/src/sgml/release-18.sgml`.

### `psqlscan.l`

No lexing change. The range adds `psql_scan_get_location()` for pgbench (c8c74ad7e1c), changes flex options (b1ef48980dd, 6fdd5d95634) and fixes `:{?name}` output to append directly to the buffer (2fd3e2fa5c9, "accidentally-harmless"). None of these changes a token or its output.

### Send commands and `psqlscanslash.l`

**Nine new commands return `PSQL_CMD_SEND`.** For every command that returns `PSQL_CMD_SEND`, `exec_command()` first copies the previous query into an empty buffer (`command.c` 484–489). `MainLoop()` then calls `SendQuery()` with the buffer, moves the buffer to `previous_buf` and resets the lexer (`REL_18_6:mainloop.c` 513–529). Thus each of these commands ends the query buffer, as `\g` does. What happens to the buffer text depends on the send mode (`REL_18_6:common.c` 1602–1725):

| Command | Arguments | Buffer text | Source |
|---|---|---|---|
| `\parse statement_name` | 1, required | Sent: `PQsendPrepare` | `command.c` 424, 2508 |
| `\sendpipeline` | none | Sent with `\bind` or `\bind_named` parameters. Pipeline mode only. | `command.c` 440, 2844 |
| `\close_prepared statement_name` | 1, required | Not sent | `command.c` 350, 755 |
| `\startpipeline` | none | Not sent | `command.c` 450, 3065 |
| `\syncpipeline` | none | Not sent | `command.c` 452, 3084 |
| `\endpipeline` | none | Not sent | `command.c` 378, 3103 |
| `\flushrequest` | none | Not sent | `command.c` 388, 1714 |
| `\flush` | none | Not sent | `command.c` 386, 1695 |
| `\getresults [number_results]` | 0 or 1 | Not sent | `command.c` 396, 1929 |

RN18 (lines 11558, 11585) lists these commands. The source adds a fact that RN18 does not state: all nine end the query buffer. During development the command `\close` existed (d55322b0da6). Commit fc39b286ad7 renamed it to `\close_prepared` before `REL_18_0`, so no release has `\close`.

In pipeline mode, a query that ends with `;` is sent with `PQsendQueryParams` (2cce0fe440f). This changes behavior, not syntax. `psqlscanslash.l` has no syntax change.

### Meta-commands

| Change | Class | Source |
|---|---|---|
| New `\bind_named statement_name [parameter] ...`: parameters for a named prepared statement. It does not end the buffer. | B | RN18 line 11558; `command.c` 342, 556; d55322b0da6 |
| New buffer-ending commands (table above). | A | RN18 lines 11558, 11585; 41625ab8e, 17caf6644, fc39b286ad7 |
| An `x` suffix selects expanded mode on list commands: `\d[Sx+]`, `\l[x+]`, `\lo_list[x+]`, `\z[Sx]` and the other `\d` commands. The dispatch adds the names `lx`, `listx`, `l+x`, `lx+`, `list+x`, `listx+`, `zx`, `zSx`, `zxS`. | B | RN18 line 11645; `command.c` 412, 473; 00f4c2959 |

RN18 "Migration" also says that the server no longer treats `\.` as end of data in CSV `COPY FROM` (770233748). psql still stops in-line `COPY ... FROM STDIN` data at a line that is only `\.` (`REL_18_6:copy.c` 634–650). For in-line data in a psql document, nothing changes.

## PostgreSQL 19 (pre-release)

Change set `REL_18_6..b73d13c`. RN19 = `b73d13c:doc/src/sgml/release-19.sgml`.

### Changes from `7a74e5ed92d` to `b73d13c`

| File | Change | Source |
|---|---|---|
| `psqlscan.l` | The `right_arrow "->"` rule is removed, and `\|` leaves `self`. These copied the SQL/PGQ changes of `scan.l`, which are reverted. | `2b9e1aff4d3` |
| `psqlscan.l`, `psqlscan_int.h`, `psqlscanslash.l` | The `BEGIN ... END` tracking inside `CREATE SCHEMA ... CREATE FUNCTION` (`sub_idents`) is removed, because `CREATE SCHEMA` does not accept a function element again. | `3c5d28ba64e` (reverts d516974840f) |
| `command.c` | `\dG` is removed, and `\d` with no argument does not list property graphs. | `2b9e1aff4d3` |
| RN19 | No psql item changes. | |

### `psqlscan.l`

No lexing change. The diff `REL_18_6..b73d13c` has only memory-allocation macros (`pg_malloc_object`, `pg_malloc_array`). At `7a74e5ed92d` there were two changes: a `->` rule with `\|` as a single character (2f094e7ac69), and `BEGIN ... END` tracking inside `CREATE SCHEMA ... CREATE FUNCTION` (d516974840f, 049b742daad). Both are reverted (see above).

### Send commands and `psqlscanslash.l`

No syntax change. The range has out-of-memory fixes (9d4505b7f82, e793e51abab), and `\getresults` with an invalid value no longer affects the next query (2b0d50e39c5). It removes the obsolete `psql_scan_get_paren_depth` API. `HandleSlashCmds` rejects a missing command name (`cmd == NULL`), which does not change the syntax.

### Meta-commands

| Change | Class | Source |
|---|---|---|
| `\dX` accepts `+` (`\dX[x+]`). | B | RN19 "psql" (`b73d13c:release-19.sgml` 2376); aecc558666a; `command.c` `listExtendedStats(pattern, show_verbose)` |

The other RN19 psql items (prompt escapes `%S` and `%i`, `\pset display_true`/`display_false`, `SERVICEFILE`) are display or variables, and out of scope.

## Breaking changes across 14–19

A "break" here is psql input that a newer psql lexes or runs differently. Changes that only add a command are not breaks.

| Version | Change | Class | What used to happen | What happens now | Source |
|---|---|---|---|---|---|
| 14 | `;` inside `BEGIN ... END` of `CREATE FUNCTION/PROCEDURE` | A | 13 ended the query at the first `;` | The query continues to the `;` after `END` | `REL_14_24:psqlscan.l` 650, 1015; e717a9a18, 029c5ac03db |
| 15 | Number followed by `e'` or `E'` | A | 14: the `e'` starts an extended string | 15: `1e` is one token, and `'` starts a standard string, so the string can end elsewhere | `REL_15_19:psqlscan.l` 343; 2549f0661 |
| 15 | `--` comments inside a query | A | 14 removed them from the query text | 15 sends them | `REL_15_19:psqlscan.l` 393; 83884682f |
| 17 | Vertical tab | A | Ordinary character in SQL text and in command names | Whitespace; it ends a command name | `REL_17_11:psqlscan.l` 162; `psqlscanslash.l` 113; ae6d06f0968 |
| 18 | Nine new buffer-ending commands | A | `\parse` and the others were invalid commands | They end the query buffer | `REL_18_6:command.c` 350–452; `mainloop.c` 513 |
| 14.24, 15.19, 16.15, 17.11, 18.6, 19 | In-line `COPY ... FROM STDIN` data after a failed `COPY` | A (all pins) | psql ran the data lines as SQL | psql skips the data up to `\.`; a failed `COPY` in a script needs a `\.` line | RN16 "Release 16.15"; 5c51ae4556f and back-patches |
| 14.19, 15.14, 16.10, 17.6, 18.0, 19 | Restricted mode | B | No `\restrict` | After `\restrict key`, every backslash command except `\unrestrict key` is an error, also `\g` | RN14 "Release 14.19"; e4998d089d9 and back-patches |
| 14.24, 15.19, 16.15, 17.11, 18.6, 19 | `\unrestrict` argument | B | One normal argument, with backquote and variable expansion | The whole rest of the line, with no expansion | RN16 "Release 16.15"; 2006fca401e and back-patches |

## Notes for pg-psql

pg-psql models `psqlscan.l` text runs, the four interpolation forms, `;`, and the send commands `\g`, `\gx`, `\gset`, `\gexec` and `\crosstabview` (`pg-psql/src/ast.rs` 136–147). Everything else after a backslash is the catch-all `MetaCommand` token (`ast.rs` 284). The lexer is `pg-psql/src/tokens.rs`. pg-psql renders one SQL string for the whole document. It does not split the document into queries (`ast.rs` 104–114). The current pg-psql behavior matches 17 in most places and 14 in one place (item A2). The findings below come from reading the source. The planned psqlscan oracle (#79) can confirm them.

### Class (A): changes that can need a version gate

| # | Target versions | Change | Current pg-psql | Recommendation |
|---|---|---|---|---|
| A1 | only `pg18`, `pg19-beta` | Nine new buffer-ending commands: `\parse`, `\sendpipeline`, `\close_prepared`, `\startpipeline`, `\syncpipeline`, `\endpipeline`, `\flushrequest`, `\flush`, `\getresults`. | Each one is a `MetaCommand` and renders verbatim into the SQL. | Add them as `SendCommand` variants with `since-pg18`. Only `\parse` and `\sendpipeline` send the buffer text. The other seven end the buffer but psql does not send the text, so a rendering that replaces them with `;` sends SQL that psql does not send. Decide this before the variants are added. `\parse`, `\close_prepared` and `\getresults` take arguments (see observation O3). |
| A2 | `pg15` and later vs `pg14` | Trailing-junk tokens: a number or `$n` that an identifier follows directly is one token. | Behaves as 14: `Digits` (`ast.rs` 263) or `DollarNumber` (241), then `EscapeString` (225) for `1e'...'`. | For `since-pg15`, add a junk token, `[0-9]+` / `\$[0-9]+` / decimal followed by an identifier, so that `1e'\''` lexes as `1e` and a standard string. Keep the current behavior with `not(since-pg15)`. Add a gated test with the example in 15. #76 did this: `SqlAtom::Number` (`since-pg15`) also takes `realfail` (`1e+`). |
| A3 | `pg14` only | psql 14 removes `--` comments inside a query from the sent text. | Rendering copies the source, so comments stay (15 behavior). | The server ignores comments, so the SQL parse does not change. Only the rendered text and the source map differ. Decide whether version parity for rendering includes comment removal. If yes, gate with `not(since-pg15)`. #76 decided yes: a `pg14` build lexes a `--` comment as `SqlAtom::LineComment`, and rendering removes it and keeps the line ending. |
| A4 | `pg14` to `pg16` vs `pg17` and later | Vertical tab is whitespace from 17, and it ends a command name from 17. | `ignore` includes `\x0b` (`tokens.rs` 19), and `MetaCommand` ends at any character outside `[A-Za-z0-9_]`: 17 behavior. | Low priority. Rendering copies the source, so the only visible difference is `\g<VT>` in 14 to 16, which psql rejects as an invalid command. A gate is possible with `not(since-pg17)`. |
| A5 | none at `b73d13c` | `BEGIN ... END` tracking inside `CREATE SCHEMA ... CREATE FUNCTION`. It was in `7a74e5ed92d`, and `3c5d28ba64e` reverts it. | No tracking, and no query splitting (`pg-psql/src/ast.rs` `Terminator::Semi`). The tracking only moves query boundaries, and pg-psql renders one SQL string, so it is invisible to pg-psql. #78 confirmed this. | No gate. If 19 gets the extension again, and pg-psql reports query boundaries, gate it with `since-pg19`. |
| A6 | all targets | `BEGIN ... END` tracking in `CREATE FUNCTION/PROCEDURE` (14). | No tracking; invisible in the rendered text. | No gate, because every target has it. Same condition as A5. |
| A7 | all targets | `COPY ... FROM STDIN` in-line data. psql reads it raw up to `\.`, with no interpolation. From the 2026 back-patches, the lexer decides this without the server. | Not modelled. pg-psql treats the data as SQL text and substitutes `:name` in it. | No gate, because every pin has the back-patch. This is a gap for all targets. A document item for in-line data would fix it. |
| A8 | none | 16 non-decimal and `_` literals, 16.4 parameter digits. (`7a74e5ed92d` also had `->` and `\|`; `b73d13c` reverts them.) | Not modelled; no need. | No gate. The token extents that matter do not change. |

### Class (B): meta-commands for #11, #12 and #13

These need no version gate while they stay in the `MetaCommand` token. When #11–#13 model them, each one needs a gate:

- 14: `\dX`; argument-type patterns for `\df` and `\do`.
- 15: `\dconfig[+]`, `\getenv`, `\dl+`, `\lo_list+`.
- 16: `\bind`, `\drg[S]`, `\dpS`, `\zS`, the `\watch` options `i=` and `c=`.
- 17: the `\watch` option `m=`; trailing `;` removal for `\sf`, `\ef`, `\sv`, `\ev`.
- 18: `\bind_named`, and the `x` suffix on list commands.
- 19: `\dX+`. (`\dG` at `7a74e5ed92d` is reverted.)
- All pins (back-patched): `\restrict`, and `\unrestrict` with a whole-line argument.

Two class (B) items interact with the rendered SQL now:

- `\bind` (16) and `\bind_named` (18) do not end the buffer, but their arguments run to the end of the line. In `SELECT $1 \bind 1 \g`, pg-psql renders `SELECT $1 \bind 1 ;`, which the server cannot parse. Consider modelling them with the send commands.
- In restricted mode (`\restrict`), psql rejects `\g` and the other send commands at run time. This is run-time behavior, not syntax. pg-psql needs no change.

### Observations that are not version changes

These are true for 14 to 19 alike. They are not in scope for version gates, but they affect what pg-psql models.

- O1. `\gdesc` returns `PSQL_CMD_SEND` in every version (`REL_14_24:command.c` 1507). It ends the buffer but does not run the query. `\watch` also runs and resets the buffer (`exec_command_watch`). pg-psql models neither as a terminator.
- O2. psql ends a command name only at whitespace or a backslash (`psqlscanslash.l` `<xslashcmd>`, `REL_17_11` 151–160). `\g;` is the invalid command `g;`, and `\dt+` is the command `dt+`. pg-psql lexes `\g;` as `\g` and `;`, and `\dt+` as `\dt` and `+`.
- O3. After a valid command, psql reads the remaining arguments up to the end of the line or the next backslash, and warns about extra ones (`REL_17_11:command.c` 261). Thus `SELECT 1 \g out.txt` sends `SELECT 1` and writes to `out.txt`. pg-psql renders `SELECT 1 ; out.txt`. The same applies to `\gset prefix`, `\crosstabview` columns, and the arguments of the 18 commands in A1.
- O4. Whole-line commands (`\copy`, `\!`, `\sf`, `\sv`, `\ef`, `\ev`, `\h`, `\?`, and `\unrestrict` from the 2026 back-patch) read the rest of the line, including any backslash. A `\g` on the same line is part of the argument, not a send command. pg-psql recognises it as a send command.

### Open doubts

- The 19 pin. Resolved by #78: the document now uses `b73d13c`. 19 is still pre-release, so its psql can change again before 19.0.
- The plan (`docs/plans/2026-09-22-target-versions.md`) gives `REL_17_11` as `6af8851`. In the submodule, `6af885119b5` is "Stamp 17.8". `REL_17_11` is `083ac033419`.
- The lexing examples (A2, A4, 16 item 1) come from the flex rules. They were not run against a psql binary. The psqlscan oracle (#79) can confirm them.
