//! pg-sql flame harness: profiler-friendly parse loops over the quick and
//! discovery workload suites (ADR 0006, performance-parity plan Track T).
//!
//! This is the port of recursa-old's `flame.rs`/`flame_target` pair onto the
//! current runtime. The old crate exposed `pg_sql::flame::run_loop` from the
//! library and drove it from a `flame_target` bin; the current library
//! surface is strict (generated grammar only), so the whole harness lives in
//! this bench-adjacent target instead. Like `benches/parse.rs` it mounts the
//! differential test support module, so it parses through the exact same
//! provenance-free statement lexing seam
//! (`lex_statement_source` + `Statement::parse_without_spans`) and
//! the exact frozen statement membership that the differential suite pins.
//!
//! The harness does one thing: it parses one named canonical workload with
//! pg-sql in a tight loop for a fixed duration, so an external sampling
//! profiler sees only parser frames. It prints its PID on startup for
//! attach-style profilers and a machine-readable stats line on completion.
//! It measures pg-sql alone; the head-to-head engine comparison stays in
//! `benches/parse.rs`.
//!
//! Quick workloads (CONTEXT.md):
//!
//! - `corpus` — the frozen corpus statements accepted by pg-sql, sqlparser,
//!   and PostgreSQL (the head-to-head benchmark membership). Deliberate legacy
//!   parse-error entries, and statements rejected by any engine, are filtered
//!   while the workload is loaded rather than exercised in the timed loop.
//! - `select_list_10000` — `fixtures/stress/select_list_10000.sql`, one wide
//!   SELECT list (10,000 columns).
//! - `bool_chain` — `fixtures/stress/bool_chain_1000.sql`, one WHERE clause
//!   chaining 1,000 `AND` terms through the Pratt loop.
//!
//! The discovery suite adds a pg-sql/PostgreSQL-only corpus (both aggregate
//! and partitioned by top-level statement family), structurally distinct
//! stress fixtures, a lexer-heavy fixture, and corpus-derived rejection paths.
//! `scripts/flame-profile discovery` profiles each member independently.
//!
//! Usage (`--duration` is in seconds, default 5):
//!
//! ```text
//! cargo bench -p pg-sql --features postgres-oracle --bench flame -- \
//!     select_list_10000 --duration 5
//! ```
//!
//! Track P additions:
//!
//! - `--count-allocs` runs one complete pass over the accepted workload with a
//!   counting global allocator and reports successful allocation calls and their full requested
//!   bytes, split by phase (lex, input setup, and parse), instead of the timing loop. The counter
//!   uses two relaxed atomic adds per successful call and is disabled outside this mode, so
//!   ordinary profiling
//!   runs are unperturbed; the allocator itself still forwards to the
//!   system allocator either way.
//! - `--engine sqlparser` times sqlparser 0.52 (the parity-gate reference)
//!   over the quick suite's three-way membership. `--engine postgres` runs
//!   PostgreSQL's raw parser over either suite for generated-parser profiles.
//!
//! Flat AST additions:
//!
//! - `--engine pg-sql-flat` times `Statement::parse_flat_without_spans` over
//!   the same workloads through the same lex seam, so the pair
//!   `--engine pg-sql` / `--engine pg-sql-flat` isolates the representation
//!   exactly as `benches/parse.rs` does for its `pg-sql` / `pg-sql-flat`
//!   entries. `--count-allocs` accepts either engine.
//! - `short_statements` is a corpus-shaped workload of many short statements
//!   read from a directory of `.sql` files named by `FLAME_SHORT_SQL_DIR`
//!   (default `fixtures/short`). It exists so the short-statement fixed cost
//!   can be profiled where the vendored regression corpus is unavailable (a
//!   worktree without the `vendor/postgres` submodule). Statements are split
//!   on top-level semicolons, so the workload is a profiling shape, not the
//!   frozen differential membership.
//!
//! The `postgres-oracle` feature is optional for this target. Without it the
//! PostgreSQL membership probe is a no-op (workloads keep whatever pg-sql
//! accepts) and `--engine postgres` is rejected.
//!
//! See `docs/notes/perf.md` for the profiling recipes built on top of this
//! target (macOS `sample`/`xctrace`, Linux `perf`).

// The differential test support module, mounted exactly as in
// `benches/parse.rs` so the two harnesses cannot drift apart. Only part of
// the module is used, and rustc drops its `#[test]` functions in this
// `harness = false` build, which strands some of its imports.
#[allow(dead_code, unused_imports)]
#[path = "../tests/support/mod.rs"]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use pg_sql::ast::Statement;
use support::baseline::{FrozenStatements, LegacyItemKind};
use support::diff_check::lex_statement_source;

// --- Counting allocator (Track P allocation attribution) ---

/// Number of successful allocation or reallocation calls observed while counting is enabled.
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
/// Full requested size of those successful allocation or reallocation calls.
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
/// Successful `realloc` calls alone — the growth half of the count above.
/// Reported separately because a growing store's reallocation count per parse
/// is what distinguishes a presized vector from one that doubles.
static REALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
/// Whether the wrapper counts. Off by default so profiling runs pay only a
/// relaxed load and branch per allocation.
static COUNTING: AtomicBool = AtomicBool::new(false);

/// System-forwarding allocator that can count allocations and bytes.
struct CountingAllocator;

// SAFETY: forwards every operation to `System` unchanged; the counters are
// relaxed atomics with no allocation of their own.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && COUNTING.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() && COUNTING.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let pointer = unsafe { System.realloc(ptr, layout, new_size) };
        if !pointer.is_null() && COUNTING.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            REALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Reads the counters once: (calls, bytes, realloc calls).
fn alloc_snapshot() -> (u64, u64, u64) {
    (
        ALLOC_COUNT.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
        REALLOC_COUNT.load(Ordering::Relaxed),
    )
}

// --- Profiling workloads ---

/// The three canonical workloads used for fast before/after checks.
const QUICK_WORKLOADS: [(&str, &str); 3] = [
    (
        "corpus",
        "frozen corpus statements accepted by all three engines",
    ),
    ("select_list_10000", "fixtures/stress/select_list_10000.sql"),
    ("bool_chain", "fixtures/stress/bool_chain_1000.sql"),
];

/// Broader workloads used to discover grammar- and mechanism-specific costs.
const DISCOVERY_WORKLOADS: [(&str, &str); 15] = [
    (
        "corpus_pg_postgres",
        "aggregate corpus accepted by pg-sql and PostgreSQL",
    ),
    ("corpus_query", "pg-sql/PostgreSQL corpus: query family"),
    ("corpus_dml", "pg-sql/PostgreSQL corpus: DML family"),
    ("corpus_ddl", "pg-sql/PostgreSQL corpus: DDL family"),
    (
        "corpus_transaction",
        "pg-sql/PostgreSQL corpus: transaction family",
    ),
    ("corpus_session", "pg-sql/PostgreSQL corpus: session family"),
    (
        "corpus_access",
        "pg-sql/PostgreSQL corpus: access-control family",
    ),
    ("corpus_cursor", "pg-sql/PostgreSQL corpus: cursor family"),
    ("corpus_utility", "pg-sql/PostgreSQL corpus: utility family"),
    (
        "insert_values_10000",
        "fixtures/stress/insert_values_10000.sql",
    ),
    ("in_list_10000", "fixtures/stress/in_list_10000.sql"),
    (
        "nested_subquery_15",
        "fixtures/stress/nested_subquery_15.sql",
    ),
    ("lexical_mix_1000", "fixtures/stress/lexical_mix_1000.sql"),
    (
        "errors_pg_postgres",
        "corpus parse-error items rejected by pg-sql and PostgreSQL",
    ),
    (
        "short_statements",
        "short statements from $FLAME_SHORT_SQL_DIR (default fixtures/short)",
    ),
];

/// Directory of `.sql` files backing the `short_statements` workload.
const SHORT_SQL_DIR_VAR: &str = "FLAME_SHORT_SQL_DIR";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedOutcome {
    Accept,
    Reject,
}

struct LoadedWorkload {
    inputs: Vec<String>,
    expected: ExpectedOutcome,
    supports_sqlparser: bool,
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Loads the frozen corpus items of one legacy kind without probing engines.
fn load_corpus_items(kind: LegacyItemKind) -> Result<Vec<String>, String> {
    let frozen = FrozenStatements::pinned();
    let corpus_dir = manifest_dir().join("vendor/postgres/src/test/regress/sql");
    let mut inputs = Vec::with_capacity(frozen.total_statements());
    for file_name in frozen.file_names() {
        let path = corpus_dir.join(file_name);
        let text = fs::read_to_string(&path)
            .map_err(|e| format!("read corpus file {}: {e}", path.display()))?;
        let statements = frozen
            .file(file_name)
            .statements(&text)
            .map_err(|error| format!("{file_name}: cannot load frozen statements: {error}"))?;
        let kinds = frozen.file(file_name).legacy_item_kinds();
        if kinds.len() != statements.len() {
            return Err(format!(
                "{file_name}: frozen item-kind count {} does not match statement count {}",
                kinds.len(),
                statements.len()
            ));
        }
        inputs.extend(
            statements
                .into_iter()
                .zip(kinds)
                .filter(|(_, item_kind)| **item_kind == kind)
                .map(|(statement, _)| statement.to_owned()),
        );
    }
    Ok(inputs)
}

/// Split a `.sql` file into statements on top-level semicolons.
///
/// A deliberately small splitter for the `short_statements` profiling shape:
/// it tracks single quotes, double quotes, dollar-quoted tags, line comments
/// and block comments, which is enough for the plain DDL/DCL corpus files the
/// workload is pointed at. It is **not** the frozen statement extractor
/// (`tests/support/baseline.rs`); statements it mis-splits are simply
/// dropped by the acceptance probe below.
fn split_statements(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                let mut depth = 1usize;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            }
            quote @ (b'\'' | b'"') => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == quote {
                        if bytes.get(i + 1) == Some(&quote) {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'$' => {
                // A dollar-quote tag is `$`, an optional identifier, `$`.
                let tag_end = text[i + 1..]
                    .find('$')
                    .map(|offset| i + 1 + offset)
                    .filter(|end| {
                        text[i + 1..*end]
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_')
                    });
                match tag_end {
                    Some(end) => {
                        let tag = &text[i..=end];
                        match text[end + 1..].find(tag) {
                            Some(offset) => i = end + 1 + offset + tag.len(),
                            None => i = bytes.len(),
                        }
                    }
                    None => i += 1,
                }
            }
            b';' => {
                i += 1;
                let statement = text[start..i].trim();
                if !statement.is_empty() {
                    out.push(statement.to_owned());
                }
                start = i;
            }
            _ => i += 1,
        }
    }
    let tail = text[start.min(text.len())..].trim();
    if !tail.is_empty() {
        out.push(tail.to_owned());
    }
    out
}

/// Every statement of every `.sql` file in the `short_statements` directory,
/// in sorted file order so the workload is deterministic.
fn load_short_statements() -> Result<Vec<String>, String> {
    let dir = match std::env::var_os(SHORT_SQL_DIR_VAR) {
        Some(value) => PathBuf::from(value),
        None => manifest_dir().join("fixtures/short"),
    };
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("read {SHORT_SQL_DIR_VAR} directory {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no .sql files in {}", dir.display()));
    }
    let mut out = Vec::new();
    for path in files {
        let text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        out.extend(split_statements(&text));
    }
    Ok(out)
}

/// A stable top-level dispatch partition. It intentionally follows the first
/// significant SQL keyword rather than inspecting AST internals, keeping this
/// profiling-only grouping independent of AST representation changes.
fn statement_family(sql: &str) -> &'static str {
    let lexed = lex_statement_source(sql);
    let Some(first) = lexed.tokens().next() else {
        return "utility";
    };
    match first.kind().to_string().as_str() {
        "SELECT" | "VALUES" | "TABLE" | "WITH" | "(" => "query",
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" => "dml",
        "CREATE" | "ALTER" | "DROP" => "ddl",
        "BEGIN" | "START" | "COMMIT" | "END" | "ABORT" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" => {
            "transaction"
        }
        "SET" | "RESET" | "SHOW" | "LOAD" | "DISCARD" => "session",
        "GRANT" | "REVOKE" => "access",
        "DECLARE" | "FETCH" | "MOVE" | "CLOSE" => "cursor",
        _ => "utility",
    }
}

/// Load a profiling workload's statements. Reads everything up front and
/// probes membership once so neither I/O nor parity checks appear in the
/// profiled loop.
fn load_workload(name: &str) -> Result<LoadedWorkload, String> {
    let accepted_inputs = |statements: Vec<String>, require_sqlparser: bool| {
        statements
            .into_iter()
            .filter(|sql| {
                parse_with_pg_sql(sql)
                    && (!require_sqlparser || parse_with_sqlparser(sql))
                    && parse_with_postgres(sql)
            })
            .collect()
    };
    let stress = |file: &str, require_sqlparser: bool| -> Result<LoadedWorkload, String> {
        let path = manifest_dir().join("fixtures/stress").join(file);
        let sql = fs::read_to_string(&path)
            .map_err(|e| format!("read stress fixture {}: {e}", path.display()))?;
        Ok(LoadedWorkload {
            inputs: accepted_inputs(vec![sql], require_sqlparser),
            expected: ExpectedOutcome::Accept,
            supports_sqlparser: require_sqlparser,
        })
    };
    match name {
        "corpus" => {
            let inputs = load_corpus_items(LegacyItemKind::Statement)?;
            Ok(LoadedWorkload {
                inputs: accepted_inputs(inputs, true),
                expected: ExpectedOutcome::Accept,
                supports_sqlparser: true,
            })
        }
        "corpus_pg_postgres" => {
            let inputs = load_corpus_items(LegacyItemKind::Statement)?;
            Ok(LoadedWorkload {
                inputs: accepted_inputs(inputs, false),
                expected: ExpectedOutcome::Accept,
                supports_sqlparser: false,
            })
        }
        name @ ("corpus_query" | "corpus_dml" | "corpus_ddl" | "corpus_transaction"
        | "corpus_session" | "corpus_access" | "corpus_cursor" | "corpus_utility") => {
            let family = name.trim_start_matches("corpus_");
            let inputs = load_corpus_items(LegacyItemKind::Statement)?;
            let inputs = inputs
                .into_iter()
                .filter(|sql| statement_family(sql) == family)
                .collect();
            Ok(LoadedWorkload {
                inputs: accepted_inputs(inputs, false),
                expected: ExpectedOutcome::Accept,
                supports_sqlparser: false,
            })
        }
        "errors_pg_postgres" => {
            let inputs = load_corpus_items(LegacyItemKind::ParseError)?;
            Ok(LoadedWorkload {
                inputs: inputs
                    .into_iter()
                    .filter(|sql| !parse_with_pg_sql(sql) && !parse_with_postgres(sql))
                    .collect(),
                expected: ExpectedOutcome::Reject,
                supports_sqlparser: false,
            })
        }
        "select_list_10000" => stress("select_list_10000.sql", true),
        "bool_chain" => stress("bool_chain_1000.sql", true),
        "insert_values_10000" => stress("insert_values_10000.sql", false),
        "in_list_10000" => stress("in_list_10000.sql", false),
        "nested_subquery_15" => stress("nested_subquery_15.sql", false),
        "lexical_mix_1000" => stress("lexical_mix_1000.sql", false),
        "short_statements" => {
            let inputs = load_short_statements()?;
            Ok(LoadedWorkload {
                inputs: accepted_inputs(inputs, false),
                expected: ExpectedOutcome::Accept,
                supports_sqlparser: false,
            })
        }
        other => Err(format!("unknown workload: {other}")),
    }
}

// --- The profiled loop ---

/// Strict statement-level parse with pg-sql — the generated lex pass, the
/// differential suite's document-terminator exclusion, then the generated
/// provenance-free `Statement` parser. Byte-for-byte the same seam as
/// `parse_with_pg_sql` in `benches/parse.rs`, so profiles correspond to
/// what the interim benchmark times.
fn parse_with_pg_sql(sql: &str) -> bool {
    let lexed = lex_statement_source(sql);
    if lexed.errors().next().is_some() {
        return false;
    }
    let mut input = lexed.input();
    match Statement::parse_without_spans(&mut input) {
        Ok(parsed) => {
            std::hint::black_box(&parsed);
            input.is_eof()
        }
        Err(_) => false,
    }
}

/// Strict statement-level parse with pg-sql's **Flat AST**. The same lex
/// pass, the same `Input`, and the same LR automaton as `parse_with_pg_sql`;
/// only the reduce actions differ, building the tiered flat store instead of
/// the nested arena tree. Byte-for-byte the seam that
/// `parse_with_pg_sql_flat` in `benches/parse.rs` times, including dropping
/// the whole store inside the call.
fn parse_with_pg_sql_flat(sql: &str) -> bool {
    let lexed = lex_statement_source(sql);
    if lexed.errors().next().is_some() {
        return false;
    }
    let mut input = lexed.input();
    match Statement::parse_flat_without_spans(&mut input) {
        Ok(flat) => {
            std::hint::black_box(&flat);
            input.is_eof()
        }
        Err(_) => false,
    }
}

/// Strict statement-level parse with sqlparser 0.52, exactly as
/// `parse_with_sqlparser` in `benches/parse.rs` does it: the parity-gate
/// reference engine over the same statement text.
fn parse_with_sqlparser(sql: &str) -> bool {
    sqlparser::parser::Parser::parse_sql(&sqlparser::dialect::PostgreSqlDialect {}, sql)
        .map(|parsed| {
            std::hint::black_box(&parsed);
        })
        .is_ok()
}

/// Strictly parse one extracted statement with PostgreSQL 17.9's raw parser,
/// matching `parse_with_postgres` in `benches/parse.rs`. This is a workload
/// membership probe only; it runs once during loading and never in the
/// profiled loop.
#[cfg(feature = "postgres-oracle")]
fn parse_with_postgres(sql: &str) -> bool {
    if sql.as_bytes().contains(&0) {
        return false;
    }
    pg_oracle::parse_ok(sql)
}

/// Without `postgres-oracle` there is no PostgreSQL to probe (a worktree
/// without the `vendor/postgres` submodule cannot build the FFI bridge), so
/// the membership probe passes everything and each workload keeps whatever
/// pg-sql itself accepts. `--engine postgres` is rejected in that build.
#[cfg(not(feature = "postgres-oracle"))]
fn parse_with_postgres(_sql: &str) -> bool {
    true
}

/// Accumulated allocation counts for one phase of the accepted seam.
#[derive(Clone, Copy, Default)]
struct PhaseAllocs {
    /// Statements that went through this phase.
    statements: u64,
    /// Successful allocation or reallocation calls observed.
    allocs: u64,
    /// Full sizes requested by those calls.
    bytes: u64,
    /// The `realloc` subset of `allocs` — in-place or copying growth.
    reallocs: u64,
}

impl PhaseAllocs {
    fn add(&mut self, before: (u64, u64, u64), after: (u64, u64, u64)) {
        self.statements += 1;
        self.allocs += after.0 - before.0;
        self.bytes += after.1 - before.1;
        self.reallocs += after.2 - before.2;
    }

    fn report(&self, label: &str) {
        let per = |value: u64| value as f64 / self.statements.max(1) as f64;
        println!(
            "{label:<28} statements={:<8} allocs={:<12} bytes={:<14} \
             allocs/stmt={:.1} reallocs/stmt={:.1} bytes/stmt={:.0}",
            self.statements,
            self.allocs,
            self.bytes,
            per(self.allocs),
            per(self.reallocs),
            per(self.bytes),
        );
    }
}

/// One complete counted pass over the accepted workload, with lex, input setup, and parse phases
/// counted separately. Deliberate parse-error entries and statements
/// rejected by any parity engine were removed while loading the workload, so
/// this pass intentionally does not measure error/expected-set paths.
fn run_alloc_count(inputs: &[String], expected: ExpectedOutcome, engine: Engine) {
    let mut lex = PhaseAllocs::default();
    let mut input_setup = PhaseAllocs::default();
    let mut parse = PhaseAllocs::default();

    COUNTING.store(true, Ordering::Relaxed);
    for sql in inputs {
        let before_lex = alloc_snapshot();
        let lexed = lex_statement_source(sql);
        let after_lex = alloc_snapshot();
        lex.add(before_lex, after_lex);
        let accepted = if lexed.errors().next().is_some() {
            false
        } else {
            let before_input = alloc_snapshot();
            let mut input = lexed.input();
            let after_input = alloc_snapshot();
            input_setup.add(before_input, after_input);
            // Both representations are constructed and dropped inside the
            // counted window, so the two counts differ only by the reduce
            // actions and the store they fill.
            let before_parse = alloc_snapshot();
            let accepted = if engine == Engine::PgSqlFlat {
                match Statement::parse_flat_without_spans(&mut input) {
                    Ok(flat) => {
                        std::hint::black_box(&flat);
                        input.is_eof()
                    }
                    Err(_) => false,
                }
            } else {
                match Statement::parse_without_spans(&mut input) {
                    Ok(parsed) => {
                        std::hint::black_box(&parsed);
                        input.is_eof()
                    }
                    Err(_) => false,
                }
            };
            let after_parse = alloc_snapshot();
            parse.add(before_parse, after_parse);
            accepted
        };
        debug_assert_eq!(
            accepted,
            expected == ExpectedOutcome::Accept,
            "profile workload produced an unexpected parse outcome"
        );
    }
    COUNTING.store(false, Ordering::Relaxed);

    println!("allocation counts (counting global allocator, one pass, engine {engine:?}):");
    println!(
        "metric convention: successful alloc/alloc_zeroed/realloc calls; full requested bytes"
    );
    lex.report("lex (accepted workload)");
    input_setup.report("input setup (accepted)");
    parse.report("parse (accepted workload)");
    let total_allocs = lex.allocs + input_setup.allocs + parse.allocs;
    let total_bytes = lex.bytes + input_setup.bytes + parse.bytes;
    println!(
        "alloc_total statements={} allocs={} bytes={}",
        lex.statements, total_allocs, total_bytes,
    );
}

/// One completed profiling loop: how much work ran inside the deadline.
struct LoopStats {
    /// Complete passes over the workload's statement list.
    passes: u64,
    /// Individual statements parsed (counts partial passes).
    statements: u64,
    /// Statements pg-sql accepted (lexed, parsed, and consumed to EOF).
    accepted: u64,
    /// Source bytes fed to the parser.
    bytes: u64,
    elapsed: Duration,
}

/// Loop the parse seam over `inputs` until `duration` has elapsed. The
/// deadline is checked after every statement, so a long workload (the
/// corpus is ~50 s per pass at the 2026-09-01 baseline) still stops close
/// to the requested duration; at least one statement always runs.
fn run_loop(inputs: &[String], duration: Duration, engine: Engine) -> LoopStats {
    let parse = engine.parse_fn();
    let start = Instant::now();
    let deadline = start + duration;
    let mut stats = LoopStats {
        passes: 0,
        statements: 0,
        accepted: 0,
        bytes: 0,
        elapsed: Duration::ZERO,
    };
    'run: loop {
        for sql in inputs {
            stats.accepted += u64::from(std::hint::black_box(parse(sql)));
            stats.statements += 1;
            stats.bytes += sql.len() as u64;
            if Instant::now() >= deadline {
                break 'run;
            }
        }
        stats.passes += 1;
    }
    stats.elapsed = start.elapsed();
    stats
}

// --- CLI ---

/// Which parser the timing loop runs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Engine {
    PgSql,
    PgSqlFlat,
    Sqlparser,
    Postgres,
}

impl Engine {
    /// The parse seam this engine loops over.
    fn parse_fn(self) -> fn(&str) -> bool {
        match self {
            Engine::PgSql => parse_with_pg_sql,
            Engine::PgSqlFlat => parse_with_pg_sql_flat,
            Engine::Sqlparser => parse_with_sqlparser,
            Engine::Postgres => parse_with_postgres_engine,
        }
    }

    /// Whether the engine is one of pg-sql's two AST representations, the
    /// only engines `--count-allocs` can attribute.
    fn is_pg_sql(self) -> bool {
        matches!(self, Engine::PgSql | Engine::PgSqlFlat)
    }
}

/// `--engine postgres`, kept distinct from the membership probe so the probe
/// can be a no-op in an oracle-free build while the timing loop still
/// refuses to pretend it measured PostgreSQL.
#[cfg(feature = "postgres-oracle")]
fn parse_with_postgres_engine(sql: &str) -> bool {
    parse_with_postgres(sql)
}

#[cfg(not(feature = "postgres-oracle"))]
fn parse_with_postgres_engine(_sql: &str) -> bool {
    unreachable!("--engine postgres is rejected without the postgres-oracle feature")
}

struct Args {
    workload: String,
    duration_secs: u64,
    engine: Engine,
    count_allocs: bool,
}

/// Hand-rolled argv parsing, mirroring the old `flame_target`. `cargo bench`
/// appends a literal `--bench` to the binary's arguments; it is skipped.
fn parse_args(args: &[String]) -> Result<Option<Args>, String> {
    let mut workload: Option<String> = None;
    let mut duration_secs: u64 = 5;
    let mut engine = Engine::PgSql;
    let mut count_allocs = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--bench" => {}
            "--list" => return Ok(None),
            "--duration" => {
                i += 1;
                let v = args.get(i).ok_or("--duration needs a value")?;
                duration_secs = v.parse().map_err(|_| format!("bad --duration: {v}"))?;
            }
            "--engine" => {
                i += 1;
                let v = args.get(i).ok_or("--engine needs a value")?;
                engine = match v.as_str() {
                    "pg-sql" => Engine::PgSql,
                    "pg-sql-flat" => Engine::PgSqlFlat,
                    "sqlparser" => Engine::Sqlparser,
                    "postgres" => {
                        if cfg!(feature = "postgres-oracle") {
                            Engine::Postgres
                        } else {
                            return Err(
                                "--engine postgres needs the postgres-oracle feature".into()
                            );
                        }
                    }
                    other => return Err(format!("bad --engine: {other}")),
                };
            }
            "--count-allocs" => count_allocs = true,
            s if s.starts_with("--") => return Err(format!("unknown flag: {s}")),
            s => {
                if workload.is_some() {
                    return Err("expected exactly one workload name".into());
                }
                workload = Some(s.to_owned());
            }
        }
        i += 1;
    }
    let workload = workload.ok_or("missing workload name")?;
    if count_allocs && !engine.is_pg_sql() {
        return Err("--count-allocs counts the pg-sql and pg-sql-flat seams only".into());
    }
    Ok(Some(Args {
        workload,
        duration_secs,
        engine,
        count_allocs,
    }))
}

fn print_workload_group(label: &str, workloads: &[(&str, &str)]) {
    println!("{label}:");
    for (name, source) in workloads {
        println!("  {name:<20} {source}");
    }
}

fn print_workloads() {
    print_workload_group("quick suite", &QUICK_WORKLOADS);
    println!();
    print_workload_group("discovery suite", &DISCOVERY_WORKLOADS);
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&argv) {
        Ok(Some(args)) => args,
        Ok(None) => {
            print_workloads();
            return ExitCode::SUCCESS;
        }
        Err(msg) => {
            eprintln!("flame: {msg}");
            eprintln!("usage: flame <workload> [--duration <seconds>] | flame --list");
            print_workloads();
            return ExitCode::from(2);
        }
    };

    let loaded = match load_workload(&args.workload) {
        Ok(inputs) => inputs,
        Err(msg) => {
            eprintln!("flame: {msg}");
            print_workloads();
            return ExitCode::from(2);
        }
    };
    if args.engine == Engine::Sqlparser && !loaded.supports_sqlparser {
        eprintln!(
            "flame: {} has pg-sql/PostgreSQL membership and cannot be timed with sqlparser",
            args.workload
        );
        return ExitCode::from(2);
    }
    let input_bytes: u64 = loaded.inputs.iter().map(|s| s.len() as u64).sum();

    println!("pg-sql flame harness");
    println!(
        "workload: {} ({} statements, {} bytes/pass)",
        args.workload,
        loaded.inputs.len(),
        input_bytes,
    );
    println!("expected: {:?}", loaded.expected);
    println!("pid: {}", std::process::id());

    if args.count_allocs {
        run_alloc_count(&loaded.inputs, loaded.expected, args.engine);
        return ExitCode::SUCCESS;
    }

    println!("engine: {:?}", args.engine);
    println!("duration: {} s", args.duration_secs);

    let stats = run_loop(
        &loaded.inputs,
        Duration::from_secs(args.duration_secs),
        args.engine,
    );
    if stats.statements == 0 {
        eprintln!("flame: 0 statements parsed (empty workload?)");
        return ExitCode::from(1);
    }
    let expected_accepts = match loaded.expected {
        ExpectedOutcome::Accept => stats.statements,
        ExpectedOutcome::Reject => 0,
    };
    if stats.accepted != expected_accepts {
        eprintln!(
            "flame: workload outcome drift: expected {:?}, observed {} accepts in {} parses",
            loaded.expected, stats.accepted, stats.statements
        );
        return ExitCode::from(1);
    }

    let secs = stats.elapsed.as_secs_f64();
    println!(
        "passes: {}  statements: {}  accepted: {}  elapsed: {:.3} s",
        stats.passes, stats.statements, stats.accepted, secs,
    );
    println!(
        "throughput: {:.1} statements/s, {:.3} MiB/s",
        stats.statements as f64 / secs,
        stats.bytes as f64 / secs / (1024.0 * 1024.0),
    );
    // Machine-readable summary, same shape as the old flame_target output.
    println!(
        "iters={} statements={} accepted={} bytes={} elapsed_ns={}",
        stats.passes,
        stats.statements,
        stats.accepted,
        stats.bytes,
        stats.elapsed.as_nanos(),
    );
    ExitCode::SUCCESS
}
