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
//!   counting global allocator and reports allocation counts and bytes,
//!   split by phase (lex vs parse), instead of the timing loop. The counter is
//!   two relaxed atomic adds per
//!   allocation and is disabled outside this mode, so ordinary profiling
//!   runs are unperturbed; the allocator itself still forwards to the
//!   system allocator either way.
//! - `--engine sqlparser` times sqlparser 0.52 (the parity-gate reference)
//!   over the quick suite's three-way membership. `--engine postgres` runs
//!   PostgreSQL's raw parser over either suite for generated-parser profiles.
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

/// Number of allocations observed while counting is enabled.
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
/// Bytes requested by those allocations.
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
/// Whether the wrapper counts. Off by default so profiling runs pay only a
/// relaxed load and branch per allocation.
static COUNTING: AtomicBool = AtomicBool::new(false);

/// System-forwarding allocator that can count allocations and bytes.
struct CountingAllocator;

// SAFETY: forwards every operation to `System` unchanged; the counters are
// relaxed atomics with no allocation of their own.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) && new_size > layout.size() {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add((new_size - layout.size()) as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Reads the counters once.
fn alloc_snapshot() -> (u64, u64) {
    (
        ALLOC_COUNT.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
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
const DISCOVERY_WORKLOADS: [(&str, &str); 14] = [
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
];

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
fn parse_with_postgres(sql: &str) -> bool {
    if sql.as_bytes().contains(&0) {
        return false;
    }
    pg_oracle::parse_ok(sql)
}

/// Accumulated allocation counts for one phase of the accepted seam.
#[derive(Clone, Copy, Default)]
struct PhaseAllocs {
    /// Statements that went through this phase.
    statements: u64,
    /// Allocations observed.
    allocs: u64,
    /// Bytes requested.
    bytes: u64,
}

impl PhaseAllocs {
    fn add(&mut self, before: (u64, u64), after: (u64, u64)) {
        self.statements += 1;
        self.allocs += after.0 - before.0;
        self.bytes += after.1 - before.1;
    }

    fn report(&self, label: &str) {
        let per = |value: u64| value as f64 / self.statements.max(1) as f64;
        println!(
            "{label:<28} statements={:<8} allocs={:<12} bytes={:<14} \
             allocs/stmt={:.1} bytes/stmt={:.0}",
            self.statements,
            self.allocs,
            self.bytes,
            per(self.allocs),
            per(self.bytes),
        );
    }
}

/// One complete counted pass over the accepted workload, with lex and parse
/// phases counted separately. Deliberate parse-error entries and statements
/// rejected by any parity engine were removed while loading the workload, so
/// this pass intentionally does not measure error/expected-set paths.
fn run_alloc_count(inputs: &[String], expected: ExpectedOutcome) {
    let mut lex = PhaseAllocs::default();
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
            let mut input = lexed.input();
            let before_parse = alloc_snapshot();
            let outcome = Statement::parse_without_spans(&mut input);
            let after_parse = alloc_snapshot();
            let accepted = match outcome {
                Ok(parsed) => {
                    std::hint::black_box(&parsed);
                    input.is_eof()
                }
                Err(_) => false,
            };
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

    println!("allocation counts (counting global allocator, one pass):");
    lex.report("lex (accepted workload)");
    parse.report("parse (accepted workload)");
    let total_allocs = lex.allocs + parse.allocs;
    let total_bytes = lex.bytes + parse.bytes;
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
    let parse: fn(&str) -> bool = match engine {
        Engine::PgSql => parse_with_pg_sql,
        Engine::Sqlparser => parse_with_sqlparser,
        Engine::Postgres => parse_with_postgres,
    };
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
    Sqlparser,
    Postgres,
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
                    "sqlparser" => Engine::Sqlparser,
                    "postgres" => Engine::Postgres,
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
    if count_allocs && engine != Engine::PgSql {
        return Err("--count-allocs counts the pg-sql seam only".into());
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
        run_alloc_count(&loaded.inputs, loaded.expected);
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
