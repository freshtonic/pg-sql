# The psql oracle: pg-psql against psql's own lexer

`pg-oracle` links PostgreSQL's raw parser so that pg-sql can be compared with
the server (ADR 0002). `pg-psql-oracle` does the same one layer up: it links
`src/fe_utils/psqlscan.l` and `src/bin/psql/psqlscanslash.l` of the same
pinned PostgreSQL release, so that pg-psql can be compared with psql.

The two oracles share one pin table, `pg-oracle/pins.tsv`, and one build
script, `pg-oracle/scripts/build-pg.sh` (ADR 0009). A build of
`pg-psql-oracle` selects its release with the version feature, as every other
crate does.

## What the oracle links

| Source | What it gives |
|---|---|
| `src/fe_utils/psqlscan.c` | The psql SQL lexer: text runs, interpolation, `;` and the backslash boundary. |
| `src/bin/psql/psqlscanslash.c` | `psql_scan_slash_command`, the one authority on where a command name ends. |
| `src/fe_utils/conditional.c` | The `\if` stack that `psqlscanslash.c` refers to. |
| `src/interfaces/libpq/libpq.a` | `PQExpBuffer` and the multibyte helpers. |
| `src/common/libpgcommon.a`, `src/port/libpgport.a` | The frontend runtime. |

`csrc/psql_oracle.c` is the shim. `src/lib.rs` turns its result into
[`Scan`], which reports the text psql forwards to the server, one event for
each interpolation, one for each `;` that submits the query buffer, one for
each backslash command with the name `psqlscanslash.l` read, and whether the
document ended with a complete buffer.

The interpolation events come from psql's own `get_variable` callback. That
matters: the oracle reports what the scanner recognised, not what a second
reading of the text suggests, so a colon that psql swallowed inside a string
or a dollar-quoted body can never appear as an interpolation.

### The PostgreSQL tree

`pg-psql-oracle` builds its tree at `target/pg-trees/<ref>`, which is
`build-pg.sh`'s own default. The tree is keyed by the pinned release, so one
debug build, one release build and a manual
`pg-oracle/scripts/build-pg.sh <feature>` all share it. `pg-oracle` keeps its
own tree under its `OUT_DIR`, so the two build scripts never write to the same
directory and can run at the same time. The cost is one more PostgreSQL build
for each target version; the benefit is that neither crate can disturb the
other's oracle.

## Adaptations, and what each one costs

psql reads one line at a time and resets the query buffer after every
submission. pg-psql renders one SQL string for a whole document (ADR 0007).
Three adaptations make the two comparable. Each is deliberate, and each is
recorded here because it bounds what the oracle can prove.

1. **The whole document is one input.** The flex rules of `psqlscan.l` do not
   read line boundaries, so the lexing is the same. psql's line-at-a-time
   reading does bound how far a backslash command's arguments can run, so the
   shim resumes SQL lexing at the first line break after a command name,
   which is where psql's own input buffer would have ended. A command whose
   arguments end earlier than the line does — `\timing on SELECT 1;` — has its
   tail read as arguments here and as SQL by psql. Every such command is an
   unmodelled meta-command in pg-psql, so no document is decided by this
   alone.
2. **The query buffer is never reset.** The `{whitespace}` rule of
   `psqlscan.l` does not echo while the buffer is empty, so psql drops the
   leading trivia of every statement. The shim seeds the buffer with one byte
   and removes it at the end, so the trivia survives and the forwarded text
   can be compared byte for byte with pg-psql's rendering.
3. **Backslash-command arguments are not read.** The shim calls
   `psql_scan_slash_command` and nothing else, so `psql_scan_slash_option`
   never runs and a backquoted argument never starts a shell command. The
   cost is that an interpolation inside an argument is not reported.

Two boundaries are outside the classification table in [`classify`], because
they are not `PSQL_CMD_SEND` in `command.c`: `\watch` runs the query buffer
and resets it, and `\r` discards it. Both answer `Command::Other`. Neither
appears in the corpus.

The oracle quotes a bound value itself, mirroring `PQescapeLiteral` and
`PQescapeIdentifier` (`fe-exec.c`, `PQescapeInternal`), because those need the
client encoding and the `standard_conforming_strings` of a live connection and
the oracle has none. The oracle therefore proves where psql substitutes, not
how libpq escapes; pg-psql's own tests cover the escaping.

## The differential

`pg-psql/tests/psql_oracle.rs` reads every document with both sides. It needs
the `psql-oracle` feature, exactly as pg-sql's differential needs
`postgres-oracle`:

```sh
cargo test -p pg-psql --no-default-features --features pg17,psql-oracle
```

### The corpus

`baselines/psql-oracle/corpus.tsv` is the PostgreSQL 17.9 regression SQL
files, frozen by Git blob ID exactly as the SQL differential corpus is
(`docs/differential-baseline.md`). The tests read each blob from the
`vendor/postgres` object database, not from the checked-out tree, so a move of
an oracle pin does not change the corpus.

One of the 226 files is left out. `collate.windows.win1252.sql` is WIN1252,
not UTF-8. psql lexes it with a WIN1252 client encoding, where
`psqlscan_prepare_buffer` substitutes `0xFF` for the trailing bytes of every
multibyte character (`psqlscan_int.h`); pg-psql reads a Rust `&str`, so the two
cannot be compared over it.

Beside the corpus are the shaped documents in `shaped_documents()`: the
constructs pg-psql's own tests cover — every interpolation form, every lexical
shell that swallows a colon, the send and discard commands, the number forms
whose extent moved in 15, and the unterminated regions — read here by psql
rather than by a written expectation.

### What is compared

Four facts, for each document, unbound and then with every variable psql
recognised bound to an inert value:

- the text forwarded to the server;
- the extent and name of each interpolation;
- the extent of each query-buffer boundary; and
- whether the document ends with a complete buffer.

The oracle's text is composed the way pg-psql composes its own: a send command
becomes the `;` that separates the two statements it stands between, and a
discard command (18) removes the buffer it ends, keeping the whitespace before
it.

### The pinned outcomes

`baselines/psql-oracle/<feature>.json` records, for each target version, the
oracle pin, the corpus identity, the document count, and every document that
does not agree with its class. A document that agrees and is listed, or
disagrees and is not, fails the test, so the list cannot go stale in either
direction.

Regenerate after a change of pin, of oracle or of the psql grammar, and review
the diff:

```sh
cargo test -p pg-psql --no-default-features --features pg17,psql-oracle \
  --test psql_oracle -- --ignored regenerate_psql_oracle_baseline --nocapture
```

### The gap classes

An **expected psql gap** is a document where pg-psql does not agree with psql.
Every class is a deliberate limit of what pg-psql models, and a disagreement
that fits none of them is `unclassified`, which fails the test.

| Class | Why |
|---|---|
| `unmodelled-meta-command` | The document holds a backslash command that is not a send or discard command. psql stops SQL lexing there and reads the rest of the line as arguments; pg-psql keeps the whole command as one `SqlAtom::MetaCommand` token and forwards it (ADR 0007; freshtonic/pg-sql#11, #12, #13). |
| `send-command-arguments` | The document holds a send or discard command with arguments. psql reads them and the server never sees them; pg-psql ends the statement at the command name and forwards the rest as SQL text. It is the same gap as the one above, narrowed to a command pg-psql does model. |
| `boundary-suppressed-by-nesting` | psql suppresses the `;` boundary inside parentheses and inside a tracked `BEGIN ... END` block of a `CREATE FUNCTION` (`psqlscan.l` 650, `psqlscan_is_create_routine`). pg-psql tracks neither, because it renders one string for the whole document rather than splitting it into submissions, so it only ever has more boundaries than psql. The forwarded text is the same. |
| `open-lexical-region` | The document reaches end of input inside a string, a quoted identifier, a dollar-quoted body or a comment. psql asks for another line. pg-psql has no open region: it either refuses the document, which fails closed, or reads the region's bytes as shorter tokens, which does not. See "What the oracle found" (ADR 0007, freshtonic/recursa#131). |
| `refused` | pg-psql refuses a document for any other reason. The list is empty at every target version. |

### Outcomes by target version

| Feature | Documents | Agree | `unmodelled-meta-command` | `send-command-arguments` | `boundary-suppressed-by-nesting` | `open-lexical-region` |
|---|---|---|---|---|---|---|
| `pg14` | 296 | 180 | 105 | 1 | 4 | 6 |
| `pg15` | 296 | 182 | 105 | 1 | 4 | 4 |
| `pg16` | 296 | 182 | 105 | 1 | 4 | 4 |
| `pg17` | 296 | 182 | 104 | 2 | 4 | 4 |
| `pg18` | 305 | 191 | 104 | 2 | 4 | 4 |
| `pg19-beta` | 305 | 191 | 104 | 2 | 4 | 4 |

Three differences across the six, and each has a cause:

- `pg18` and `pg19-beta` have nine more documents, the pipeline and
  prepared-statement commands of 18. All nine agree.
- Before 17 a vertical tab does not end a command name in `psqlscanslash.l`,
  so `\g<VT>x` is one unknown command rather than `\g` with an argument
  (`docs/research/psql-14-19-syntax-changes.md`, item A4). The shaped document
  `name/vertical-tab` changes class at 17, not outcome.
- `pg14` has two more `open-lexical-region` documents: `number/exponent-string`
  and `number/param-junk`. See below.

## What the oracle found

**`\gdesc` was missing, and is fixed here.** It returns `PSQL_CMD_SEND` in
every target version (`REL_17_11:command.c` 1570), so it ends the query buffer
and its text goes to the server. pg-psql kept it as an unparsed
meta-command. It is now a `SendCommand`, and `pg-psql/tests/psql.rs` records
the corrected behaviour.

**An unterminated `E'...'` is re-read, not refused.** ADR 0007 says that
pg-psql refuses a document that reaches end of input inside an open lexical
region, so that "the region is refused, never reinterpreted as ordinary
text". That holds for a plain string, a quoted identifier, a dollar-quoted
body and a block comment, and the four `shaped/open/*` documents show it. It
does not hold in general, because an unmatched quoted-string *pattern* simply
does not match, and the lexer then reads the same bytes as shorter tokens.
`SELECT 1e'\' :v` at `pg14` is the case the oracle found:

- psql gives the `e` back (`realfail1`), so `e'` opens an escape string, `\'`
  is an escaped quote, and the document ends inside the string. `:v` is
  string content and psql interpolates nothing.
- pg-psql finds no `EscapeString` token, falls back to `e` as a word and
  `'\'` as a plain string, and then reads `:v` as an interpolation and
  substitutes it.

The direction is the wrong one: a colon inside an unterminated string is
substituted. It needs a closed matcher for the quoted-string forms
(freshtonic/recursa#131), which is not a small change, so it is listed rather
than fixed. From 15 the same document agrees, because `1e` is one
trailing-junk token in both and no escape string is opened.

The rest of the gaps are the documented limits: 104 or 105 corpus and shaped
documents hold a meta-command pg-psql keeps as one unparsed token, 1 or 2 hold
a send command with arguments, 4 hold a `;` psql suppresses inside parentheses
or a `BEGIN ... END` block, and 4 end inside an open region.
