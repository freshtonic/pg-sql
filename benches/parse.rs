//! pg-sql interim benchmark harness (statement-level).
//!
//! A self-contained harness (no criterion): it times `pg-sql` against
//! `sqlparser` and PostgreSQL 17.9's raw parser (via `pg-oracle`) on the
//! PostgreSQL regression corpus and the generated stress fixtures, then
//! writes a per-run report directory under `docs/benchmarks/`.
//!
//! This harness measures **statement-level** parsing. A file-level seam
//! now exists (`pg_sql::document::parse_sql`, #10 closed), but it is a
//! different code path and is deliberately not what this harness times:
//! for each corpus
//! file it takes the frozen per-file statement list that the differential
//! suite pins (`tests/support/baseline.rs`, `FrozenStatements::pinned()`)
//! and times each engine over the statements that *all three* engines
//! accept. A statement rejected by any engine is excluded for every engine,
//! and the exclusions are counted in the report. The full Criterion port
//! and file-level corpus parsing are #20.
//!
//! Each run writes its own subdirectory `docs/benchmarks/<timestamp>-<commit>/`
//! containing `report.md` (the human report), `time.svg` and `throughput.svg`
//! (the charts, referenced from `report.md` by bare filename), and `data.json`
//! (the run's raw benchmark data, serialized by `pg_sql::bench_data`).
//!
//! Run with `cargo bench -p pg-sql --features postgres-oracle`. The feature
//! flag is required: the bench target declares
//! `required-features = ["postgres-oracle"]`, and without the flag cargo
//! silently skips the target. The report path is printed on completion.

// The differential test support module. Mounting it here keeps the bench on
// the exact statement membership the differential suite pins, so the two
// cannot drift. Only part of the module is used by the harness, and rustc
// drops the module's `#[test]` functions in this `harness = false` build,
// which strands its test-module imports.
#[allow(dead_code, unused_imports)]
#[path = "../tests/support/mod.rs"]
mod support;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pg_sql::ast::Statement;
use pg_sql::bench_data::{BenchRecord, serialize_data_json};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser as SqlParser;
use support::baseline::FrozenStatements;
use support::diff_check::lex_statement_source;

// --- Paths ---

/// The crate manifest directory — also the repository root, so the report is
/// written under `<manifest>/docs/benchmarks/`.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The PostgreSQL regression SQL corpus, vendored as a submodule.
fn corpus_sql_dir() -> PathBuf {
    manifest_dir().join("vendor/postgres/src/test/regress/sql")
}

/// Generated stress fixtures (see `src/bin/gen_stress.rs`).
fn stress_dir() -> PathBuf {
    manifest_dir().join("fixtures/stress")
}

// --- Parsers under test ---

/// Strict statement-level parse with pg-sql: the generated lex pass, the
/// differential suite's document-terminator exclusion, then the generated
/// `Statement` parser. The lex and rebuild are counted in the measured time,
/// mirroring the "parse one SQL string from scratch" model used for the
/// other two engines.
fn parse_with_pg_sql(sql: &str) -> bool {
    let lexed = lex_statement_source(sql);
    if lexed.errors().next().is_some() {
        return false;
    }
    let mut input = lexed.input();
    match Statement::parse(&mut input) {
        Ok(parsed) => {
            std::hint::black_box(&parsed);
            input.is_eof()
        }
        Err(_) => false,
    }
}

fn parse_with_sqlparser(sql: &str) -> bool {
    SqlParser::parse_sql(&PostgreSqlDialect {}, sql).is_ok()
}

/// PostgreSQL 17.9's raw parser via the pg-oracle FFI bridge. The bridge
/// allocates a `CString` per call and serialises through a global mutex —
/// both overheads count toward this parser's measured time, mirroring the
/// "parse one SQL string from scratch" model used for the other two.
///
/// `pg_oracle::parse_ok` panics on a NUL byte (`CString::new`); treat any
/// NUL-containing input as a rejection — it's structurally invalid C-string
/// input and the parser can't see it anyway.
fn parse_with_postgres(sql: &str) -> bool {
    if sql.as_bytes().contains(&0) {
        return false;
    }
    pg_oracle::parse_ok(sql)
}

// --- Measurement ---

/// Per-engine statement rejection counts for one benchmark. A statement can
/// be rejected by more than one engine, so the counts can sum past the
/// number of excluded statements.
#[derive(Clone, Copy, Default)]
struct Rejections {
    pg_sql: usize,
    sqlparser: usize,
    postgres: usize,
}

impl Rejections {
    fn add(&mut self, other: Rejections) {
        self.pg_sql += other.pg_sql;
        self.sqlparser += other.sqlparser;
        self.postgres += other.postgres;
    }
}

/// One benchmark: a name, the SQL statements every engine accepts (timed per
/// iteration), and the total byte volume those statements represent (for
/// throughput).
///
/// `stmts_total` counts the frozen statements the workload started from;
/// `stmts_total - inputs.len()` of them were excluded because at least one
/// engine rejected them, with the per-engine causes in `rejections`.
struct Bench {
    name: String,
    inputs: Vec<String>,
    bytes: u64,
    stmts_total: usize,
    rejections: Rejections,
}

impl Bench {
    fn excluded(&self) -> usize {
        self.stmts_total - self.inputs.len()
    }
}

/// The timing result for the three parsers on one benchmark.
struct Row {
    name: String,
    bytes: u64,
    stmts_timed: usize,
    stmts_excluded: usize,
    pg_sql: Duration,
    sqlparser: Duration,
    postgres: Duration,
}

/// Time a closure: warm up briefly, then collect samples until both a minimum
/// count and a time budget are met, capped so a pathological case cannot
/// stall the suite. Returns the median sample.
fn measure(mut f: impl FnMut()) -> Duration {
    let warm_end = Instant::now() + Duration::from_millis(100);
    while Instant::now() < warm_end {
        f();
    }

    let mut samples = Vec::new();
    let start = Instant::now();
    loop {
        let t = Instant::now();
        f();
        samples.push(t.elapsed());
        let elapsed = start.elapsed();
        let enough = samples.len() >= 5 && elapsed >= Duration::from_millis(500);
        if enough || elapsed >= Duration::from_secs(3) || samples.len() >= 500 {
            break;
        }
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn run_bench(b: &Bench) -> Row {
    // A workload whose statements were all excluded has nothing to time.
    let (pg_sql, sqlparser, postgres) = if b.inputs.is_empty() {
        (Duration::ZERO, Duration::ZERO, Duration::ZERO)
    } else {
        let pg_sql = measure(|| {
            for sql in &b.inputs {
                std::hint::black_box(parse_with_pg_sql(sql));
            }
        });
        let sqlparser = measure(|| {
            for sql in &b.inputs {
                std::hint::black_box(parse_with_sqlparser(sql));
            }
        });
        let postgres = measure(|| {
            for sql in &b.inputs {
                std::hint::black_box(parse_with_postgres(sql));
            }
        });
        (pg_sql, sqlparser, postgres)
    };
    println!(
        "  {:<28} {:>4} stmts ({:>3} excl)   pg-sql {:>9.3} ms   sqlparser {:>9.3} ms   postgres {:>9.3} ms",
        b.name,
        b.inputs.len(),
        b.excluded(),
        ms(pg_sql),
        ms(sqlparser),
        ms(postgres),
    );
    Row {
        name: b.name.clone(),
        bytes: b.bytes,
        stmts_timed: b.inputs.len(),
        stmts_excluded: b.excluded(),
        pg_sql,
        sqlparser,
        postgres,
    }
}

// --- Benchmark set ---

/// (shape, [(size, filename)]) — must match `src/bin/gen_stress.rs`.
fn stress_shapes() -> Vec<(&'static str, Vec<(usize, &'static str)>)> {
    vec![
        (
            "insert_values",
            vec![
                (100, "insert_values_100.sql"),
                (1_000, "insert_values_1000.sql"),
                (10_000, "insert_values_10000.sql"),
            ],
        ),
        (
            "bool_chain",
            vec![
                (10, "bool_chain_10.sql"),
                (100, "bool_chain_100.sql"),
                (1_000, "bool_chain_1000.sql"),
            ],
        ),
        (
            "select_list",
            vec![
                (100, "select_list_100.sql"),
                (1_000, "select_list_1000.sql"),
                (10_000, "select_list_10000.sql"),
            ],
        ),
        // Sizes kept small: pg-sql's recursive-descent parser is pathologically
        // slow on deep subquery nesting.
        (
            "nested_subquery",
            vec![
                (5, "nested_subquery_5.sql"),
                (10, "nested_subquery_10.sql"),
                (15, "nested_subquery_15.sql"),
            ],
        ),
        (
            "in_list",
            vec![
                (100, "in_list_100.sql"),
                (1_000, "in_list_1000.sql"),
                (10_000, "in_list_10000.sql"),
            ],
        ),
    ]
}

/// Partition `statements` into the subset every engine accepts (returned as
/// owned inputs plus their byte volume) and per-engine rejection counts.
fn probe_statements(statements: &[&str]) -> (Vec<String>, u64, Rejections) {
    let mut inputs = Vec::new();
    let mut bytes = 0u64;
    let mut rejections = Rejections::default();
    for source in statements {
        let a = parse_with_pg_sql(source);
        let b = parse_with_sqlparser(source);
        let c = parse_with_postgres(source);
        rejections.pg_sql += usize::from(!a);
        rejections.sqlparser += usize::from(!b);
        rejections.postgres += usize::from(!c);
        if a && b && c {
            bytes += source.len() as u64;
            inputs.push((*source).to_owned());
        }
    }
    (inputs, bytes, rejections)
}

/// Build the benchmark set: one `corpus/<file>` benchmark per frozen corpus
/// file (the exact membership the differential baseline pins), plus one
/// `stress/<file>` benchmark per generated stress fixture (each is a single
/// statement, so the statement-level model applies unchanged).
fn build_benches() -> Vec<Bench> {
    let mut benches = Vec::new();

    let frozen = FrozenStatements::pinned();
    let corpus_dir = corpus_sql_dir();
    for name in frozen.file_names() {
        let path = corpus_dir.join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read corpus file {}: {e}", path.display()));
        let statements = frozen
            .file(name)
            .statements(&text)
            .unwrap_or_else(|error| panic!("{name}: cannot load frozen statements: {error}"));
        let (inputs, bytes, rejections) = probe_statements(&statements);
        let stem = name.strip_suffix(".sql").unwrap_or(name);
        benches.push(Bench {
            name: format!("corpus/{stem}"),
            stmts_total: statements.len(),
            inputs,
            bytes,
            rejections,
        });
    }

    // Stress fixtures: one single-statement benchmark per generated file. The
    // filename stem (e.g. `insert_values_100`) already names the shape and
    // size; the `stress/` prefix groups them apart from the `corpus/` entries.
    let stress = stress_dir();
    for (_shape, sizes) in stress_shapes() {
        for (_, file) in sizes {
            let sql = fs::read_to_string(stress.join(file))
                .unwrap_or_else(|e| panic!("read stress fixture {file}: {e}"));
            let (inputs, bytes, rejections) = probe_statements(&[sql.as_str()]);
            let stem = file.strip_suffix(".sql").unwrap_or(file);
            benches.push(Bench {
                name: format!("stress/{stem}"),
                stmts_total: 1,
                inputs,
                bytes,
                rejections,
            });
        }
    }

    let totals = benches.iter().fold(
        (0usize, 0usize, Rejections::default()),
        |(total, timed, mut rejections), bench| {
            rejections.add(bench.rejections);
            (
                total + bench.stmts_total,
                timed + bench.inputs.len(),
                rejections,
            )
        },
    );
    let (total, timed, rejections) = totals;
    eprintln!(
        "statement probe: {total} frozen statements — {timed} accepted by all three \
         engines and timed, {} excluded (rejections: pg-sql {}, sqlparser {}, \
         postgres {}).",
        total - timed,
        rejections.pg_sql,
        rejections.sqlparser,
        rejections.postgres,
    );

    benches
}

// --- Formatting helpers ---

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Throughput in MiB/s for `bytes` parsed in `d`.
fn mib_per_s(bytes: u64, d: Duration) -> f64 {
    let secs = d.as_secs_f64();
    if secs <= 0.0 {
        0.0
    } else {
        bytes as f64 / secs / (1024.0 * 1024.0)
    }
}

/// Run `prog` with `args`, returning trimmed stdout, or `None` on any failure.
fn command_stdout(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// --- Report generation ---

/// Colours for the three parsers — used by the SVG charts and named in prose.
const PG_COLOUR: &str = "#3b82f6"; // blue   — pg-sql
const SP_COLOUR: &str = "#f59e0b"; // amber  — sqlparser
const PO_COLOUR: &str = "#10b981"; // green  — postgres (pg-oracle)

/// Escape the five XML metacharacters so labels are safe inside SVG text.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Render a standalone SVG grouped-bar chart comparing pg-sql, sqlparser,
/// and the PostgreSQL raw parser. Each benchmark contributes three
/// side-by-side bars in distinct colours (`PG_COLOUR`, `SP_COLOUR`,
/// `PO_COLOUR`); x-axis labels are rotated 90° counter-clockwise so they
/// read vertically (bottom-to-top, matplotlib `rotation=90` style).
/// Returned as a complete `<svg>` document for writing to a sidecar `.svg`
/// file — GitHub strips inline SVG from markdown, but renders an SVG
/// referenced as an image.
fn svg_chart(
    title: &str,
    y_label: &str,
    labels: &[String],
    pg_sql: &[f64],
    sqlparser: &[f64],
    postgres: &[f64],
) -> String {
    let n = labels.len();
    let max = pg_sql
        .iter()
        .chain(sqlparser)
        .chain(postgres)
        .copied()
        .fold(0.0_f64, f64::max);
    // Round the axis ceiling up so the tallest bar is not flush with the top.
    let ceiling = if max <= 0.0 { 1.0 } else { max * 1.1 };

    // Layout, in px.
    let bar_w = 10.0_f64;
    let bar_gap = 2.0; // between a group's bars
    let group_gap = 14.0; // between groups
    let group_w = bar_w * 3.0 + bar_gap * 2.0;
    let pitch = group_w + group_gap;
    let plot_w = pitch * n as f64;
    let plot_h = 280.0;
    let m_left = 64.0;
    let m_right = 16.0;
    let m_top = 52.0; // title + legend
    let m_bottom = 156.0; // vertical labels
    let width = m_left + plot_w + m_right;
    let height = m_top + plot_h + m_bottom;
    let x0 = m_left;
    let base_y = m_top + plot_h;
    let mid_x = width / 2.0;

    // Palette as locals so no literal `#` appears in the raw-string SVG
    // fragments (a `"#` sequence would close an `r#"…"#` raw string early).
    let bg = "#ffffff";
    let ink = "#111827"; // title
    let txt = "#374151"; // labels / legend text
    let grid = "#e5e7eb";
    let tick = "#6b7280"; // y-tick numbers
    let axis = "#9ca3af";
    let pg = PG_COLOUR;
    let sp = SP_COLOUR;
    let po = PO_COLOUR;

    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" font-family="-apple-system,Segoe UI,Helvetica,Arial,sans-serif">"#
    );
    let _ = writeln!(
        s,
        r#"<rect width="{width:.0}" height="{height:.0}" fill="{bg}"/>"#
    );
    let _ = writeln!(
        s,
        r#"<text x="{mid_x:.1}" y="22" font-size="15" font-weight="600" fill="{ink}" text-anchor="middle">{}</text>"#,
        xml_escape(title)
    );

    // Legend — three swatches centred under the title.
    let _ = writeln!(
        s,
        r#"<rect x="{lx:.1}" y="34" width="11" height="11" fill="{pg}"/><text x="{tx:.1}" y="43" font-size="11" fill="{txt}">pg-sql</text>"#,
        lx = mid_x - 140.0,
        tx = mid_x - 125.0,
    );
    let _ = writeln!(
        s,
        r#"<rect x="{lx:.1}" y="34" width="11" height="11" fill="{sp}"/><text x="{tx:.1}" y="43" font-size="11" fill="{txt}">sqlparser</text>"#,
        lx = mid_x - 40.0,
        tx = mid_x - 25.0,
    );
    let _ = writeln!(
        s,
        r#"<rect x="{lx:.1}" y="34" width="11" height="11" fill="{po}"/><text x="{tx:.1}" y="43" font-size="11" fill="{txt}">postgres</text>"#,
        lx = mid_x + 60.0,
        tx = mid_x + 75.0,
    );

    // Y gridlines + tick labels (5 ticks).
    for i in 0..=4 {
        let frac = i as f64 / 4.0;
        let y = base_y - frac * plot_h;
        let _ = writeln!(
            s,
            r#"<line x1="{x0:.1}" y1="{y:.1}" x2="{x1:.1}" y2="{y:.1}" stroke="{grid}"/>"#,
            x1 = x0 + plot_w,
        );
        let _ = writeln!(
            s,
            r#"<text x="{tx:.1}" y="{ty:.1}" font-size="10" fill="{tick}" text-anchor="end">{val:.1}</text>"#,
            tx = x0 - 6.0,
            ty = y + 3.5,
            val = ceiling * frac,
        );
    }
    // Y-axis title, rotated.
    let cy = m_top + plot_h / 2.0;
    let _ = writeln!(
        s,
        r#"<text x="16" y="{cy:.1}" font-size="11" fill="{txt}" text-anchor="middle" transform="rotate(-90 16 {cy:.1})">{}</text>"#,
        xml_escape(y_label)
    );
    // Baseline.
    let _ = writeln!(
        s,
        r#"<line x1="{x0:.1}" y1="{base_y:.1}" x2="{x1:.1}" y2="{base_y:.1}" stroke="{axis}"/>"#,
        x1 = x0 + plot_w,
    );

    // Bars and rotated x-axis labels.
    for i in 0..n {
        let gx = x0 + i as f64 * pitch;
        let h_pg = (pg_sql[i] / ceiling) * plot_h;
        let h_sp = (sqlparser[i] / ceiling) * plot_h;
        let h_po = (postgres[i] / ceiling) * plot_h;
        let _ = writeln!(
            s,
            r#"<rect x="{x:.1}" y="{y:.1}" width="{bar_w:.0}" height="{h:.2}" fill="{pg}"/>"#,
            x = gx,
            y = base_y - h_pg,
            h = h_pg,
        );
        let _ = writeln!(
            s,
            r#"<rect x="{x:.1}" y="{y:.1}" width="{bar_w:.0}" height="{h:.2}" fill="{sp}"/>"#,
            x = gx + bar_w + bar_gap,
            y = base_y - h_sp,
            h = h_sp,
        );
        let _ = writeln!(
            s,
            r#"<rect x="{x:.1}" y="{y:.1}" width="{bar_w:.0}" height="{h:.2}" fill="{po}"/>"#,
            x = gx + (bar_w + bar_gap) * 2.0,
            y = base_y - h_po,
            h = h_po,
        );
        // Label, rotated 90° counter-clockwise: reads bottom-to-top with its
        // last character against the axis (`text-anchor="end"`).
        let lx = gx + group_w / 2.0;
        let ly = base_y + 7.0;
        let _ = writeln!(
            s,
            r#"<text x="{lx:.1}" y="{ly:.1}" font-size="10" fill="{txt}" text-anchor="end" transform="rotate(-90 {lx:.1} {ly:.1})">{}</text>"#,
            xml_escape(&labels[i])
        );
    }

    s.push_str("</svg>\n");
    s
}

/// A finished benchmark report: the markdown plus the two sidecar SVG
/// charts it references.
struct Report {
    markdown: String,
    time_svg: String,
    throughput_svg: String,
}

/// Render one results table — header plus one row per `Row`.
fn write_results_table(md: &mut String, rows: &[&Row]) {
    let _ = writeln!(
        md,
        "| Benchmark | Stmts | Excluded | Bytes | pg-sql time | sqlparser time \
         | postgres time | pg-sql throughput | sqlparser throughput \
         | postgres throughput | pg-sql vs sqlparser | pg-sql vs postgres |\n\
         |---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|"
    );
    for r in rows {
        let sp_speedup = if r.pg_sql.as_secs_f64() > 0.0 {
            r.sqlparser.as_secs_f64() / r.pg_sql.as_secs_f64()
        } else {
            0.0
        };
        let po_speedup = if r.pg_sql.as_secs_f64() > 0.0 {
            r.postgres.as_secs_f64() / r.pg_sql.as_secs_f64()
        } else {
            0.0
        };
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {:.3} ms | {:.3} ms | {:.3} ms \
             | {:.1} MiB/s | {:.1} MiB/s | {:.1} MiB/s | {:.2}× | {:.2}× |",
            r.name,
            r.stmts_timed,
            r.stmts_excluded,
            r.bytes,
            ms(r.pg_sql),
            ms(r.sqlparser),
            ms(r.postgres),
            mib_per_s(r.bytes, r.pg_sql),
            mib_per_s(r.bytes, r.sqlparser),
            mib_per_s(r.bytes, r.postgres),
            sp_speedup,
            po_speedup,
        );
    }
}

fn build_report(rows: &[Row], totals: &BenchTotals, timestamp: &str, commit: &str) -> Report {
    // Partition: rows with at least one timed statement carry measurements;
    // rows whose statements were all excluded have nothing to time. Within
    // each partition the input order (alphabetical corpus files, then stress
    // fixtures in declaration order) is preserved so the table is
    // deterministic.
    let timed: Vec<&Row> = rows.iter().filter(|r| r.stmts_timed > 0).collect();
    let empty: Vec<&Row> = rows.iter().filter(|r| r.stmts_timed == 0).collect();

    let chart_labels: Vec<String> = timed.iter().map(|r| r.name.clone()).collect();
    let pg_time: Vec<f64> = timed.iter().map(|r| ms(r.pg_sql)).collect();
    let sp_time: Vec<f64> = timed.iter().map(|r| ms(r.sqlparser)).collect();
    let po_time: Vec<f64> = timed.iter().map(|r| ms(r.postgres)).collect();
    let pg_tput: Vec<f64> = timed.iter().map(|r| mib_per_s(r.bytes, r.pg_sql)).collect();
    let sp_tput: Vec<f64> = timed
        .iter()
        .map(|r| mib_per_s(r.bytes, r.sqlparser))
        .collect();
    let po_tput: Vec<f64> = timed
        .iter()
        .map(|r| mib_per_s(r.bytes, r.postgres))
        .collect();

    let mut md = String::new();
    let _ = writeln!(
        md,
        "# pg-sql parser benchmark — statement-level interim results"
    );
    md.push('\n');
    let _ = writeln!(md, "- **Generated:** {timestamp}");
    let _ = writeln!(md, "- **Commit:** `{commit}`");
    let _ = writeln!(
        md,
        "- **Parsers:** pg-sql (this crate) vs \
         [`sqlparser`](https://crates.io/crates/sqlparser) vs PostgreSQL's \
         raw parser (via [`pg-oracle`](../../../pg-oracle/), linking the \
         vendored PostgreSQL 17.9 source)"
    );
    md.push('\n');
    let _ = writeln!(
        md,
        "**These are statement-level measurements.** A file-level seam exists \
         (`pg_sql::document::parse_sql`) but is a different code path and is \
         not timed here, so each `corpus/<file>` \
         benchmark parses the frozen per-file statement list pinned by the \
         differential suite (`tests/support/baseline.rs`), one statement at a \
         time, rather than the whole file in one call. The Criterion port and \
         file-level corpus parsing are tracked in issue #20."
    );
    md.push('\n');
    let _ = writeln!(
        md,
        "All three engines time the **same statement set**: a statement \
         rejected by any engine (or carrying a NUL byte, which the \
         PostgreSQL C bridge cannot accept) is excluded for every engine, \
         and counted in the Excluded column. Time is the median wall-clock \
         per iteration over a benchmark's accepted statements; throughput is \
         the accepted statements' byte volume divided by that time. The \
         pg-sql column includes the generated lex pass and the differential \
         suite's document-terminator exclusion; the postgres column measures \
         `pg_oracle::parse_ok`, which includes a per-call `CString` \
         allocation and a global-mutex acquisition on top of the underlying \
         `raw_parser()` invocation — these overheads count toward the \
         measured times, mirroring the end-to-end \"parse one SQL string \
         from scratch\" model used for all engines."
    );
    md.push('\n');

    let _ = writeln!(md, "- **Benchmarks:** {}", rows.len());
    let _ = writeln!(
        md,
        "- **Frozen statements:** {} ({} timed by all three engines, {} \
         excluded)",
        totals.stmts_total,
        totals.stmts_timed,
        totals.stmts_total - totals.stmts_timed,
    );
    let _ = writeln!(
        md,
        "- **Per-engine rejections (causes of exclusion):** pg-sql {}, \
         sqlparser {}, postgres {}",
        totals.rejections.pg_sql, totals.rejections.sqlparser, totals.rejections.postgres,
    );
    md.push('\n');

    let _ = writeln!(md, "## Results\n");
    write_results_table(&mut md, &timed);
    md.push('\n');
    if !empty.is_empty() {
        let _ = writeln!(
            md,
            "## Workloads with no statement accepted by all three engines\n"
        );
        write_results_table(&mut md, &empty);
        md.push('\n');
    }

    // Charts — rendered as sidecar SVG files, referenced as images so they
    // render in both VS Code's preview and on GitHub.
    let time_svg = svg_chart(
        "Parse time (ms) — lower is better",
        "Time (ms)",
        &chart_labels,
        &pg_time,
        &sp_time,
        &po_time,
    );
    let throughput_svg = svg_chart(
        "Throughput (MiB/s) — higher is better",
        "MiB/s",
        &chart_labels,
        &pg_tput,
        &sp_tput,
        &po_tput,
    );

    // The charts sit in the same per-run directory as this report, so they
    // are referenced by bare filename.
    let _ = writeln!(md, "## Parse time (lower is better)\n");
    let _ = writeln!(md, "![Parse time per benchmark](time.svg)\n");
    let _ = writeln!(md, "## Throughput (higher is better)\n");
    let _ = writeln!(md, "![Throughput per benchmark](throughput.svg)\n");
    let _ = writeln!(
        md,
        "_Each benchmark shows three side-by-side bars — blue = **pg-sql**, \
         amber = **sqlparser**, green = **postgres** (PostgreSQL 17.9 raw \
         parser via pg-oracle). All bars cover the same accepted-by-all \
         statement set._"
    );

    Report {
        markdown: md,
        time_svg,
        throughput_svg,
    }
}

/// Whole-run statement accounting for the report header.
struct BenchTotals {
    stmts_total: usize,
    stmts_timed: usize,
    rejections: Rejections,
}

fn bench_totals(benches: &[Bench]) -> BenchTotals {
    let mut totals = BenchTotals {
        stmts_total: 0,
        stmts_timed: 0,
        rejections: Rejections::default(),
    };
    for bench in benches {
        totals.stmts_total += bench.stmts_total;
        totals.stmts_timed += bench.inputs.len();
        totals.rejections.add(bench.rejections);
    }
    totals
}

fn main() {
    println!("pg-sql benchmark harness (statement-level interim)");
    let benches = build_benches();
    let totals = bench_totals(&benches);

    println!("running {} benchmarks...", benches.len());
    let rows: Vec<Row> = benches.iter().map(run_bench).collect();

    // Aggregate engine totals over every timed benchmark, for a headline.
    let sums: BTreeMap<&str, Duration> = [
        ("pg-sql", rows.iter().map(|r| r.pg_sql).sum()),
        ("sqlparser", rows.iter().map(|r| r.sqlparser).sum()),
        ("postgres", rows.iter().map(|r| r.postgres).sum()),
    ]
    .into_iter()
    .collect();
    for (engine, total) in &sums {
        println!("total median time, {engine}: {:.3} ms", ms(*total));
    }

    // Identify the run: an ISO-8601 UTC timestamp (`:` swapped for `-` so it is
    // a safe filename) and the short commit SHA.
    let timestamp = command_stdout("date", &["-u", "+%Y-%m-%dT%H-%M-%SZ"])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("epoch-{secs}")
        });
    let commit = command_stdout("git", &["rev-parse", "--short", "HEAD"])
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let report = build_report(&rows, &totals, &timestamp, &commit);

    // Each run gets its own subdirectory; the report references the charts by
    // bare filename, so report.md, the SVGs, and data.json all sit together.
    let run_dir = manifest_dir()
        .join("docs/benchmarks")
        .join(format!("{timestamp}-{commit}"));
    fs::create_dir_all(&run_dir).unwrap_or_else(|e| panic!("create {}: {e}", run_dir.display()));
    let write = |name: &str, content: &str| {
        let path = run_dir.join(name);
        fs::write(&path, content).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        path
    };
    write("time.svg", &report.time_svg);
    write("throughput.svg", &report.throughput_svg);
    let md_path = write("report.md", &report.markdown);

    // Raw data for trend analysis, serialized by `pg_sql::bench_data`.
    let records: Vec<BenchRecord> = rows
        .iter()
        .map(|r| BenchRecord {
            name: r.name.clone(),
            pg_sql_ns: r.pg_sql.as_nanos(),
            sqlparser_ns: r.sqlparser.as_nanos(),
            postgres_ns: r.postgres.as_nanos(),
            bytes: r.bytes,
        })
        .collect();
    write(
        "data.json",
        &serialize_data_json(&timestamp, &commit, &records),
    );

    println!("\nbenchmark report written to {}", md_path.display());
}
