# Why the PostgreSQL parser is fast

Research date: 2026-09-04. Filed here 2026-09-05.
Reference source: `vendor/postgres`, PostgreSQL 17.9 (`6d396980fc5`).
Tools used for measurement: flex 2.6.4, GNU Bison 3.8.2.

This document is cross-repo. Sections 1 to 6 describe PostgreSQL's parser and stand on
their own. Section 7 is a list of work items for **recursa**, not for pg-sql, because
that is where the parsing machinery lives; it is filed here because pg-sql is where the
measurements were taken and where the performance ledger
(`docs/notes/perf.md`) records them.

**Status.** The defect found in section 7.7 is fixed: recursa `f134fad` and `f52e722`,
pinned here since pg-sql `b36a44b`. The ledger carries the numbers. Every other item in
section 7 is open. Each item states its own status, so read those rather than assuming
the list is current as a whole.

## Short answer

Both things are true, and they are not independent.

Postgres uses flex and bison for their table-driven scanners and parsers, which cost a
small constant per input byte and per token. But the fast modes of those tools are not
the default modes. Postgres writes the lexical rules and the grammar in a specific shape
so that the generated code stays in the fast mode, and it makes the build fail when the
shape is broken. Four properties do most of the work:

1. The scanner is a flat DFA with one pointer add per input byte.
2. The scanner never reconsiders a byte it has consumed.
3. Word identity is resolved exactly once, by a minimal perfect hash, at the moment the
   word is scanned. After that, every decision is an integer table index.
4. The parser reads a lookahead token only when the state needs one. Almost half of the
   states do not.

Everything downstream — parse tree construction, memory, error handling — is arranged so
that it does not undo those four properties.

## Method

I read the vendored source, then generated the scanner and the parser with the project's
own flags and measured the output.

```
flex -b -CF -p -p -o scan_CF.c scan.l      # the flags from src/backend/parser/Makefile
flex     -p -p -o scan_default.c scan.l    # for contrast
bison -Wno-deprecated -d -v -o gram.c gram.y
```

I did not profile Postgres, pg-sql, or recursa. The numbers below are static properties
of the generated code. The recommendations at the end are structural arguments, not
measured contributions to the 3.5x difference.

## 1. The lexer

### 1.1 A flat transition table, one pointer add per byte

`src/backend/parser/Makefile` sets:

```make
scan.c: FLEXFLAGS = -CF -p -p
scan.c: FLEX_NO_BACKUP=yes
```

`-CF` selects the fast, fully uncompressed table representation. The measured effect on
the inner loop is large.

Default flex tables (`scan_default.c`):

```c
YY_CHAR yy_c = yy_ec[YY_SC_TO_UI(*yy_cp)];        /* equivalence class lookup */
if (yy_accept[yy_current_state]) { ... }           /* accept test + 2 stores  */
while (yy_chk[yy_base[yy_current_state] + yy_c] != yy_current_state) {
    yy_current_state = (int) yy_def[yy_current_state];   /* default-state chase */
    if (yy_current_state >= 237) yy_c = yy_meta[yy_c];
}
yy_current_state = yy_nxt[yy_base[yy_current_state] + yy_c];
```

That is five or more dependent memory reads per byte, and the `while` can run more than
once per byte.

Postgres tables (`scan_CF.c`):

```c
for (yy_c = YY_SC_TO_UI(*yy_cp);
     (yy_trans_info = &yy_current_state[yy_c])->yy_verify == yy_c;
     yy_c = YY_SC_TO_UI(*++yy_cp))
    yy_current_state += yy_trans_info->yy_nxt;
```

One load, one compare, one add per byte. The state is a *pointer* into the table, so the
transition is pointer arithmetic. There is no equivalence-class indirection and no
default-state chase.

The price is table size:

| Table | Entries | Bytes |
| --- | --- | --- |
| `yy_transition` (`{int32 verify, int32 nxt}`) | 37,182 | 297,456 (290 KB) |
| `yy_start_state_list` | 25 | 200 |

States are 258 entries apart. The 25 start-state entries are the 12 exclusive start
conditions plus INITIAL, each in a beginning-of-line and a not-beginning-of-line form.

### 1.2 No backing up, enforced by the build

This is the technique that is easiest to miss and cheapest to copy. The header of
`scan.l` states the rule:

> The rules are designed so that the scanner never has to backtrack, in the sense that
> there is always a rule that can match the input consumed so far. [...] this makes for a
> useful speed increase --- several percent faster when measuring raw parsing.

The build enforces it. `src/Makefile.global.in`:

```make
%.c: %.l
	$(FLEX) $(if $(FLEX_NO_BACKUP),-b) $(FLEXFLAGS) -o'$@' $<
	@$(if $(FLEX_NO_BACKUP),if [ `wc -l <lex.backup` -eq 1 ]; then rm lex.backup; \
	  else echo "Scanner requires backup; see lex.backup." 1>&2; exit 1; fi)
```

My run confirms the property holds today: `lex.backup` contains one line, `No backing
up.`

The measured benefit is a smaller inner loop. I generated a small scanner with one
backing-up state, also with `-CF`:

```c
for (...) {
    yy_current_state += yy_trans_info->yy_nxt;
    if (yy_current_state[-1].yy_nxt) {            /* accept test           */
        (yy_last_accepting_state) = yy_current_state;   /* store            */
        (yy_last_accepting_cpos) = yy_cp;               /* store            */
    }
}
```

So a backing-up scanner pays one extra load, one branch and two stores *per input byte*,
and pays a rewind when a match fails. Postgres's scanner pays none of that.
`grep -c yy_last_accepting`: 2 references in the Postgres scanner (declarations only),
against 10 in the small backing-up scanner.

The technique that achieves the property is to **add explicit rules that match the
failing prefixes**. `scan.l` is full of them, and each one carries a comment saying so:

| Rule | Exists to absorb |
| --- | --- |
| `quotecontinuefail  {whitespace}*"-"?` | a failed string continuation |
| `dolqfailed  \${dolq_start}{dolq_cont}*` | a `$tag` with no closing `$` |
| `xufailed  [uU]&` | `U&` not followed by a quote |
| `numericfail  {decinteger}\.\.` | `1..10`, thrown back with `yyless` |
| `realfail  ({decinteger}|{numeric})[Ee][-+]` | `1e+` with no digits |
| `hexfail`, `octfail`, `binfail` | `0x`, `0o`, `0b` with no digits |
| `integer_junk`, `numeric_junk`, `real_junk`, `param_junk` | a digit run glued to an identifier |

Most of these rules only raise an error. Their purpose is not diagnostics; it is to keep
the DFA in a state where some rule always matches, so flex does not have to remember a
last-accepting position.

Where an error rule is not enough, the action throws input back with `yyless()`, which is
an index assignment, not a scanner rewind. The `{operator}` rule is the clearest case: it
matches the longest run of operator characters, then trims embedded `/*` or `--`, then
trims trailing `+`/`-`, then re-classifies a 1-character or 2-character remainder into
the correct token by hand. All that logic exists so that no *scanner* backtracking is
needed.

### 1.3 Scan the whole statement from one buffer

`scanner_init` copies the statement once into a private buffer with flex's two
end-of-buffer sentinels, then hands it to `yy_scan_buffer`:

```c
yyext->scanbuf = (char *) palloc(slen + 2);
memcpy(yyext->scanbuf, str, slen);
yyext->scanbuf[slen] = yyext->scanbuf[slen + 1] = YY_END_OF_BUFFER_CHAR;
yy_scan_buffer(yyext->scanbuf, slen + 2, scanner);
```

There is no refill logic, no read callback, and no copy per token. Token locations are a
subtraction: `#define SET_YYLLOC() (*(yylloc) = yytext - yyextra->scanbuf)`.
`%option never-interactive` removes the `isatty` check and the one-byte-at-a-time read
path.

`scanner_finish` does not even call `yylex_destroy`, and frees the scan buffer only if it
is at least 8 KB. Small parses leak into the memory context and the context reset
reclaims them. See section 5.

### 1.4 The scanner is a stream, not an array

Postgres never materialises a token vector. `base_yylex` returns one token at a time, and
holds at most one token of lookahead. The working set of the lexer/parser interface is
two tokens.

## 2. Word identity resolved once

The identifier rule is the whole of the keyword logic:

```c
{identifier}	{
	SET_YYLLOC();
	kwnum = ScanKeywordLookup(yytext, yyextra->keywordlist);
	if (kwnum >= 0) {
		yylval->keyword = GetScanKeyword(kwnum, yyextra->keywordlist);
		return yyextra->keyword_tokens[kwnum];      /* a distinct bison token */
	}
	ident = downcase_truncate_identifier(yytext, yyleng, true);
	yylval->str = ident;
	return IDENT;
}
```

`kwlist.h` holds 491 keywords. Each one becomes its own bison terminal. **After this
point no code in the parser compares a string.** Every keyword decision is an index into
an integer table.

`ScanKeywordLookup` (`src/common/kwlookup.c`) is three steps:

1. Reject on length. `if (len > keywords->max_kw_len) return -1;` — most identifiers in
   real SQL are shorter or longer than any keyword, so this is the common exit.
2. One minimal perfect hash probe.
3. One byte-by-byte compare against the single keyword the hash names, with ASCII-only
   downcasing folded into the compare loop.

The hash is generated at build time by `src/tools/PerfectHash.pm`, using the Czech–Havas–
Majewski (1992) acyclic-graph construction. The runtime shape is:

```c
uint32 a = seed1, b = seed2;
while (keylen--) {
    unsigned char c = *k++ | 0x20;      /* ASCII case fold, one OR */
    a = a * 257 + c;
    b = b * 31  + c;
}
return h[a % nhash] + h[b % nhash];
```

Two multiply-adds per byte, two table reads, one add. The multipliers are chosen from
`(17, 31, 127, 8191)` because they are cheap shift-and-add primes. The case fold is a
single `| 0x20`, which is correct for ASCII letters and harmless for digits and `$`.

The result can be out of range, which means "definitely not a keyword" — the caller
checks the range before the compare. Because the hash is minimal and perfect, only one
compare is ever needed.

## 3. The grammar is shaped to stay LALR(1)

### 3.1 A token-stream filter instead of extra lookahead

SQL needs more than one token of lookahead in a handful of places. Postgres does not make
the parser handle that. It puts a filter between the scanner and the parser
(`parser.c:base_yylex`) that rewrites the token stream:

| Token | Followed by | Becomes |
| --- | --- | --- |
| `NOT` | `BETWEEN` `IN` `LIKE` `ILIKE` `SIMILAR` | `NOT_LA` |
| `NULLS` | `FIRST` `LAST` | `NULLS_LA` |
| `WITH` | `TIME` `ORDINALITY` | `WITH_LA` |
| `WITHOUT` | `TIME` | `WITHOUT_LA` |
| `FORMAT` | `JSON` | `FORMAT_LA` |
| `UIDENT`/`USCONST` | `UESCAPE` `SCONST` | `IDENT`/`SCONST`, unescaped |

The comment on the function is explicit about why this and not the alternatives:

> Using a filter is simpler than trying to recognize multiword tokens directly in scan.l,
> because we'd have to allow for comments between the words. Furthermore it's not clear
> how to do that without re-introducing scanner backtrack, which would cost more
> performance than this filter layer does.

Note the shape of the filter. The `switch` on `cur_token` has a `default: return
cur_token;` that fires for every token except six. For those six, the token length is
hard-coded rather than measured, "since we have such a small set of possibilities,
hardwiring seems feasible and more efficient". The lookahead is held in `yyextra` and
returned on the next call, so the scanner is still called once per token.

This is a general pattern worth naming: **when the grammar needs more lookahead than the
parsing method allows, buy it once in a fixed-cost filter rather than in the parser's
per-decision cost.**

### 3.2 Keyword categories instead of context-sensitive matching

491 keywords are partitioned into four categories:

| Category | Count |
| --- | --- |
| `UNRESERVED_KEYWORD` | 327 |
| `COL_NAME_KEYWORD` | 63 |
| `TYPE_FUNC_NAME_KEYWORD` | 23 |
| `RESERVED_KEYWORD` | 78 |

Each category is one nonterminal in `gram.y` (`unreserved_keyword:`, and so on) listing
its members as alternatives. `ColId` and friends then admit the appropriate categories.
The instruction to contributors is:

> Put a new keyword into the first list that it can go into without causing shift or
> reduce conflicts. The earlier lists define "less reserved" categories of keywords.

So "is this word usable as a name here" is decided by the LALR tables, at table-lookup
cost, and never by a runtime predicate. `check_keywords.pl` runs at build time and fails
the build if a keyword's category in `kwlist.h` disagrees with the list it appears in
inside `gram.y`.

### 3.3 Precedence declarations instead of grammar levels

The precedence block near line 830 of `gram.y` declares about 20 levels, from
`UNION/EXCEPT` up through `TYPECAST` and `'.'`. Without them, an SQL expression grammar
needs a chain of nonterminals — one per level — and every simple operand pays a reduction
at every level. The precedence table collapses that chain into shift/reduce decisions
already resolved in the tables.

The comments are careful about the cost of this power: precedence assignments have global
effect, so the file repeatedly gives new keywords "the same precedence as IDENT" so that
they behave exactly like a non-keyword and do not silently mask an ambiguity elsewhere.

### 3.4 Zero conflicts, declared

`%expect 0`. The grammar is required to be conflict-free. A deterministic LALR(1) parser
with no conflicts needs one stack, no GLR splitting, and no backtracking, and its cost per
token is bounded.

## 4. The parser

Measured from `gram.c` generated by bison 3.8.2:

| Property | Value |
| --- | --- |
| Terminals | 539 |
| Nonterminals | 728 |
| Rules | 3,408 |
| States | 6,458 |
| `YYLAST` | 123,277 |

Table sizes:

| Table | Entries | KB |
| --- | --- | --- |
| `yytable` | 123,278 | 240.8 |
| `yycheck` | 123,278 | 240.8 |
| `yypact` | 6,458 | 25.2 |
| `yydefact` | 6,458 | 12.6 |
| `yystos` | 6,458 | 12.6 |
| `yyr1` | 3,409 | 6.7 |
| `yyrline` | 3,409 | 6.7 |
| `yyr2` | 3,409 | 3.3 |
| `yytranslate` | 777 | 1.5 |
| `yypgoto` / `yydefgoto` | 728 each | 2.8 |
| **Parser total** | | **≈ 553** |
| Scanner `yy_transition` | 37,182 | 290 |
| **Combined static tables** | | **≈ 843 KB** |

The cost model is the point. Grammar size is paid in table space, not in time. Adding a
rule costs bytes; it does not slow the parse.

### The lookahead is read lazily

The shift path in `yybackup` is:

```c
yyn = yypact[yystate];
if (yypact_value_is_default (yyn)) goto yydefault;   /* no token read at all */
if (yychar == YYEMPTY) yychar = yylex (&yylval, &yylloc, yyscanner);
yytoken = YYTRANSLATE (yychar);
yyn += yytoken;
if (yyn < 0 || YYLAST < yyn || yycheck[yyn] != yytoken) goto yydefault;
yyn = yytable[yyn];
```

The default-action test comes **before** the call to the lexer. I counted how often that
matters:

- 2,889 of 6,458 states (**44.7%**) have `yypact[state] == YYPACT_NINF`, so they reduce
  without reading a lookahead token at all.
- 4,104 of 6,458 states (63.5%) have a non-zero default reduction available.

Nearly half the parser's decisions therefore never touch the scanner, never touch
`yytable`, and never touch `yycheck`. This is the largest single reason the LALR loop is
cheap in practice, and it is invisible if you only look at the worst-case cost model.

The shift itself is: one table read, a bounds test, one compare, one table read, three
pointer increments (state stack, value stack, location stack). The stacks are `int16` and
`YYSTYPE`; they stay in cache.

### Location tracking is halved

`YYLLOC_DEFAULT` is overridden to track only the start offset of each nonterminal, not the
start and the end:

```c
#define YYLLOC_DEFAULT(Current, Rhs, N) \
	do { if ((N) > 0) (Current) = (Rhs)[1]; else (Current) = (-1); } while (0)
```

The comment explains that the cleaner alternative — scanning the right-hand side for the
first real location — "would add overhead to every rule reduction, and so far there's not
been a compelling reason to pay that overhead". A location is one `int` byte offset, not a
line/column pair; line and column are computed only when an error is reported.

## 5. Memory

Every parse tree node comes from `makeNode`, which is `palloc0` plus a tag store. `palloc`
in the common case is a pointer bump inside the current memory context
(`AllocSetAllocChunkFromBlock`):

```c
chunk = (MemoryChunk *) (block->freeptr);
block->freeptr += (chunk_size + ALLOC_CHUNKHDRSZ);
```

Three consequences:

1. Allocation is a bump, not a general allocator call.
2. Nothing is freed individually. The whole tree dies when the context is reset, in one
   operation, with no traversal and no per-node destructor.
3. Errors are safe. `#define YYMALLOC palloc` puts even bison's own stack growth in the
   context, so an error thrown mid-parse leaks nothing.

`List`, the structure that holds every repetition in the tree, was rewritten from cons
cells to an expansible array (`src/include/nodes/pg_list.h`), with the first cells
allocated inline with the header. So a list of N children is one allocation and one
contiguous array, not N cons cells.

## 6. What the parser does not do

`raw_parser` produces a *raw* parse tree. From `parser.c`:

> the grammar is not allowed to perform any table access [...] Therefore, the data
> structures returned by the grammar are "raw" parsetrees that still need to be analyzed
> by analyze.c and related files.

No name resolution, no type resolution, no catalog lookup, no validation beyond syntax.
All of that is `analyze.c` and the twenty `parse_*.c` files, which are a different phase.
When people say "the Postgres parser", the fast thing they are measuring is only
`scan.l` + `gram.y` + `makeNode`.

This matters for any comparison. If a competing parser resolves types, validates
identifiers, or normalises anything during the parse, it is not doing the same work.

## 7. Applying this to recursa

Recursa already has the equivalents of several of these techniques.

| Postgres technique | Recursa status |
| --- | --- |
| Table-driven DFA scanner | logos lexer (branch `experiment/logos-lexer`, median 13.5x over the scannerless lexer) |
| Minimal perfect hash for keywords | `phf::Map<&[u8], u16>` in `__recursa_classify_words`, with a max-length gate and an ASCII fold, generated by `render_word_classifiers` |
| A distinct terminal per keyword | yes — the classifier rewrites `record.kind` to the keyword's own kind |
| Precedence instead of grammar levels | Pratt expressions |
| No parser backtracking | yes — the only `input.restore` is the transactional root-parse failure path in `parsed.rs` |
| Dense state → token dispatch rows | yes — `DENSE_MIN_EDGES` / `DENSE_BUDGET_CELLS` in `dispatch.rs` |
| One pass over the input, no rescanning | yes, since `f134fad` — document framing lexes once and slices, where it used to re-lex the remaining document per island |

The following are the gaps, ordered by how much structural work each removes. Only 7.7
was profiled at the time of writing; the ordering of the rest is a hypothesis to test,
not a measurement.

### 7.1 Stop comparing strings during the parse

`DispatchTable::position_matches` in `recursa-core/src/dispatch.rs` runs, per candidate
edge:

```rust
4 | 5 => source.get(record.span.range()).is_some_and(|text| {
        options.iter().any(|option| text.eq_ignore_ascii_case(option))
    }),
```

That is a source read, a UTF-8 slice, and a linear scan of case-insensitive string
compares, inside the dispatch loop. Postgres does zero string work in the parser, because
the scanner already turned the word into a distinct terminal.

Recursa is one step from the same position: `__recursa_classify` has already assigned each
word-shaped fixed token its own `TokenKind`. So a predicate whose option list is a fixed
set of canonical spellings is *equivalent to* a token-kind set test, and generation can
prove it. Predicate modes 1–6 over closed spelling lists should lower to a kind-set
membership test at generation time, leaving `position_matches` with the
`expected.contains(record.kind)` test it already does first, and nothing else.

The "reclaimable keyword" case — a keyword admitted as content at some positions — is
exactly Postgres's `unreserved_keyword:` production, and it is a kind-set union, not a
text test. Recursa's "Token admission set" concept already describes the right structure;
the work is to make sure no admission set survives into the runtime as a spelling list.

### 7.2 Do not copy the token vector — partly done, and smaller than it looked

`LexResult::input_with_builder` still builds the parser input with:

```rust
records: Cow::Owned(self.records.clone()),
```

`TokenRecord` measures 24 bytes (verified), so a 2,000-token statement copies 48 KB to
start a parse, where Postgres's parser working set is two tokens.

Two corrections to what this item originally claimed. First, at the statement level the
copy is **0.4%** of the measured path (7.7), not the significant cost it was ranked as.
Second, the copy that did matter was the one per island in `input_bounded`, which is gone:
`f134fad` replaced it with `input_window`, copying only the records of one island, so
partitioning a document now copies each record once in total rather than once per island.

What remains is the whole-result copy in `input_with_builder`, which is low priority on
this evidence. `Cow::Borrowed` is still the obvious fix, but it needs `Input` to carry a
record lifetime distinct from the source lifetime, which is invasive — that is why
`f134fad` slices into owned per-island copies instead of borrowing.

### 7.3 Classify inside the lexer, not in a second pass

`__recursa_classify` runs over the finished record array and re-reads the source text for
each candidate. Postgres classifies inside the `{identifier}` rule action, while the token
text is still hot from the scan. Folding recursa's classifier into the lexer callback
removes one pass over the record array and one source re-read per candidate word.

While there, `if ![#(#candidate_kinds),*].contains(&record.kind) { return; }` builds an
array and scans it per token. A `matches!` arm or a bitset makes the intent explicit and
does not depend on the optimiser to collapse it.

### 7.4 Give dispatch states a "no lookahead needed" path

44.7% of Postgres's parser states act without reading a token. `DispatchTable::follow_edge`
begins with `let record = record?;` — it always needs a lookahead.

The recursa analogue is a dispatch state with exactly one outgoing edge whose position is
unconditional: such a state has one possible action regardless of the lookahead, so
generated code should take it without a table consult and without a peek. Worth checking
whether `emission_plan` already collapses these; if it does not, this is a cheap win that
scales with grammar size, because chain states are the states a large grammar accumulates
most of.

### 7.5 Adopt the no-backup discipline as a generation-time gate

Postgres's real lesson here is not the flag; it is that a *performance* property is
protected by a *build failure*. The property — "there is always a rule that can match the
input consumed so far" — is achieved by adding explicit error rules for the failing
prefixes (`hexfail`, `realfail`, `dolqfailed`, `xufailed`, `quotecontinuefail`,
`integer_junk`), and it is verified on every build by parsing `lex.backup`.

Two things to consider for recursa:

- Check whether the logos DFA for the current SQL grammar takes a backtracking path, and
  on which rules. Numeric literals, operator runs and unterminated-region matchers are the
  likely candidates, by analogy with `scan.l`.
- If it does, the fix is the Postgres fix: add lexical rules that match the failing
  prefixes and produce a classified lexical error, rather than letting the scanner rewind.
  Recursa's closed lexical matchers and `LexDiagnosticCode` already give these rules a
  natural home.

Then make it a gate, so it cannot regress silently.

### 7.6 Consider an arena for the parse tree

Postgres's per-node cost is a pointer bump, and the whole tree is discarded by one context
reset with no traversal. Recursa's AST is typed Rust structs; token text already borrows
from the source (better than Postgres, which `pstrdup`s), but every repetition is a `Vec`
with its own allocation and every recursion point a `Box`, and the whole tree is torn down
by drop glue at the end.

This is the largest remaining structural difference and also the largest design change. It
would mean either a bump arena (`bumpalo`) with `&'arena [T]` in place of `Vec<T>`, or an
index-based node pool. Both change the shape of the generated AST and the public API, so
this is a "measure first" item, not a quick win. Postgres's `List` rewrite from cons cells
to arrays is the precedent for how much it can be worth.

### 7.7 The benchmark does not compare the same work — measured

`raw_parser` builds an untyped tree and does nothing else, so the comparison needs a
check. I measured it. The results move in the opposite direction to what I expected, so
this section replaces the speculation that stood here before.

Setup: the vendored regression corpus, `--release` (opt 3 + debuginfo), best of 5 passes,
on the same machine as the committed benchmark runs. Other agents were compiling in
sibling worktrees, so absolute milliseconds are an upper bound. The cross-check is good:
my reproduction of the bench path measures 4.58 us per statement against the committed
run's 4.5 us, so the harness below is timing what `benches/parse.rs` times.

Reference point, from `docs/benchmarks/2026-09-04T12-30-49Z-954ffdd/data.json`:

| Engine | Total | Throughput |
| --- | --: | --: |
| PostgreSQL 17.9 `raw_parser` | 39.3 ms | 82.0 MiB/s |
| pg-sql | 138.1 ms | 23.3 MiB/s |
| sqlparser 0.52 | 208.2 ms | 15.5 MiB/s |

pg-sql / postgres = **3.52x**. That is the number under examination.

**Three confounds are not real.** The perf journal already settled two of them
(`docs/notes/perf.md`, Track P, 2026-09-02): the grammar does not opt into trivia, so
trivia handling is 0% of profile self time; and provenance capture, disabled behind
`RECURSA_PERF_PROVFREE`, moves every workload by under 1.5%. The third — that the record
clone in `LexResult::input_with_builder` is expensive — I measured at **0.4%** of the
bench path. My earlier claim that it was worth attention at the statement level was wrong.

**One confound is real but small.** `parse_with_pg_sql` calls the differential suite's
`lex_statement_source`, which lexes the statement, and then, if the last token is a
`SEMI`, rebuilds the whole token stream through `LexBuilder` to drop the terminator.
75.7% of the 43,474 frozen statements end in a semicolon, so the rebuild fires for three
statements in four. Postgres needs no equivalent: `raw_parser` takes the semicolon in
`stmtmulti` at no cost.

Phase split over 43,084 corpus statements (3.78 MB):

| Phase | Time | Share |
| --- | --: | --: |
| whole bench path | 198.8 ms | 100% |
| `pg_sql::lex` | 39.1 ms | 19.7% |
| `LexBuilder` rebuild | 21.2 ms | 10.7% |
| `LexResult::input()` record clone | 0.8 ms | 0.4% |
| `Statement::parse` | 137.6 ms | 69.2% |

So **11.1% of pg-sql's measured time is harness scaffolding**. Removing it moves the
ratio from about 3.5x to about 3.1x. It does not explain the gap. Two further facts fall
out: parse now costs three and a half times what lexing costs, so after the logos work
the parse side is the one to attack; and the confound direction is the opposite of the
usual worry, because Postgres is *penalised* in the same harness by a `CString`
allocation, a global mutex acquisition, and an `AllocSetContextCreate`/`MemoryContextDelete`
pair on every call, none of which a real backend pays per statement.

**The real finding is the entry point.** The bench header says "The ported crate has no
file-level parse interface yet (#10)". Issue #10 is closed. `pg_sql::document::parse_sql`
exists, `src/lib.rs` declares `framing(island = ast::file::SqlDocumentItem, boundary = SEMI)`,
and that is the true analogue of `raw_parser`, which takes a multi-statement string. The
benchmark measures a statement-level seam that no caller of the library uses.

Measured over the 106 corpus files that the document path fully accepts (1.26 MB) and
their 11,180 statements (1.24 MB):

| Path | Throughput | Per call |
| --- | --: | --: |
| bench seam, per statement | 23.1 MiB/s | 4.58 us |
| `parse_sql()`, per statement | 10.0 MiB/s | 10.62 us |
| `parse_sql()`, per whole file | **0.7 MiB/s** | 16.3 ms |
| `lex()` only, per statement | 112.5 MiB/s | 0.94 us |
| `lex()` only, per whole file | 131.1 MiB/s | 86.3 us |

Lexing gets *faster* per byte on whole files, as amortised fixed cost should. Parsing
gets 14x slower per byte. That is not a constant; it is a scaling defect.

**Document framing is quadratic in statement count.** Timing `parse_sql` over N copies of
one 42-byte `SELECT`:

| Statements | Bytes | Time | Per statement | Growth exponent |
| --: | --: | --: | --: | --: |
| 10 | 420 | 0.140 ms | 14.0 us | |
| 100 | 4,200 | 4.94 ms | 49.4 us | 1.76 |
| 400 | 16,800 | 69.6 ms | 174.1 us | 1.88 |
| 800 | 33,600 | 259.1 ms | 323.9 us | 1.90 |
| 1600 | 67,200 | 1034.6 ms | 646.6 us | **2.00** |

PostgreSQL parses those same 67 KB in about 0.8 ms. pg-sql's document path takes 1.03
seconds — roughly **1300x**, and the multiple grows with the input.

The cause is in recursa, in `recursa-core/src/framing.rs`, and it is structural rather
than incidental. The island loop is:

```rust
while byte < source.len() {
    let lexed = lex_rebased(source, byte, lex)?;      // re-lexes byte..source.len()
    let boundaries = records.iter().copied()
        .filter(|r| r.kind == plan.boundary_kind)
        .collect::<Vec<_>>();                          // scans the whole remainder
    for boundary in &boundaries {
        let mut input = island_input(source, island_start..boundary.end, lex, plan)?;
        // island_input -> lex_window  -> lex(tail) again
        //              -> shifted record Vec
        //              -> input_bounded -> scans every record, clones the Vec
        ...
    }
}
```

Every island re-lexes the entire remaining document, rescans it for boundary tokens, then
re-lexes its own window and copies the record vector twice. With N islands that is
Theta(N^2), and the measured exponent of 2.00 confirms it.

This is exactly the cost model Postgres avoids by construction. `raw_parser` scans each
byte once, holds one token of lookahead, and lets `stmtmulti` accumulate statements in the
LALR stack. It never rescans, never re-lexes, and never materialises a token array, so
statement count is free.

**What this means for the ranking.** The 3.52x figure is not inflated by unfair work on
pg-sql's side; if anything it is generous, because the harness charges Postgres for
overheads it would not pay and measures pg-sql through a seam that avoids its own
document path. Two items therefore change:

- Item 7.2 (the record clone) is **not** a statement-level cost (0.4%). It becomes
  important only inside the framing loop, where it runs once per island over the whole
  document.
- A new item outranks everything else in this list, because it is a scaling defect rather
  than a constant: **make document framing single-pass.** Lex the document once, then give
  each island a borrowed window over that one record array — `Cow::Borrowed` plus a range,
  not a re-lex and two copies. `Input::input_bounded`'s whole-vector validation scan has to
  go with it. That restores the linear cost model the rest of the design assumes.

Two smaller follow-ups fall out of the measurement: correct the stale `#10` claim in the
`benches/parse.rs` header, and consider timing `parse_sql` as a fourth engine column so
the benchmark tracks the interface the library actually exposes.

**Fixed, 2026-09-05.** Framing now lexes once per segment and slices that single record
array; candidate boundaries are visited lazily; `input_bounded` and its whole-vector
bounds scan are gone, replaced by `input_window`. Controlled A/B, same pg-sql commit and
lockfile, only recursa's two files differing:

| Measure | Before | After |
| --- | --: | --: |
| 1,600 statements | 849.1 ms | 16.4 ms |
| Growth exponent at 1,600 | 1.99 | 1.06 |
| 113 corpus files | 2,517.4 ms, 0.5 MiB/s | 129.2 ms, 10.2 MiB/s |
| Per file | 22.28 ms | 1.14 ms |

One island of the design had nothing pinning it, and it is the part a later reader is
most likely to remove as redundant. Framing still restarts its lex after a payload
region, because the grammar does not tokenize payload interiors: an unbalanced quote,
dollar tag or comment opener inside one changes how every later island lexes. `f52e722`
pins that with four leaking interiors, and it is a guard rather than an assertion because
it was watched to fail — with the post-payload re-lex removed, `it's raw` swallows the
terminator line and the island after it fails with a lexical error at the wrong offset.

The stale `#10` claim in `benches/parse.rs` was corrected separately in `92bdade`, which
kept the statement-level seam but said why it is the one timed. A document-level
canonical workload is tracked as issue #61, deliberately assigned to a session with no
stake in this defect, so the workload is not shaped by the bug it would have caught.

## Sources

All paths relative to `~/projects/pg-sql/vendor/postgres`.

| Topic | File |
| --- | --- |
| Scanner rules, no-backup rationale, `scanner_init` | `src/backend/parser/scan.l` |
| Flex and bison flags, no-backup gate | `src/backend/parser/Makefile`, `src/Makefile.global.in`, `src/backend/parser/meson.build` |
| Lookahead filter, `raw_parser` | `src/backend/parser/parser.c` |
| Grammar, precedence, keyword categories | `src/backend/parser/gram.y` |
| Keyword/category consistency gate | `src/backend/parser/check_keywords.pl` |
| Keyword lookup | `src/common/kwlookup.c`, `src/include/parser/kwlist.h` |
| Perfect hash construction | `src/tools/PerfectHash.pm`, `src/tools/gen_keywordlist.pl` |
| Node allocation | `src/include/nodes/nodes.h`, `src/backend/utils/mmgr/aset.c` |
| Array-backed lists | `src/include/nodes/pg_list.h` |
