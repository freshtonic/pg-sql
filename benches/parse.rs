//! pg-sql benchmark harness.
//!
//! A self-contained harness (no criterion): it times `pg-sql` against
//! `sqlparser` on the PostgreSQL regression corpus and the generated stress
//! fixtures, then writes a per-run report directory under `docs/benchmarks/`.
//!
//! Each run writes its own subdirectory `docs/benchmarks/<timestamp>-<commit>/`
//! containing `report.md` (the human report), `time.svg` and `throughput.svg`
//! (the charts, referenced from `report.md` by bare filename), and `data.json`
//! (the run's raw benchmark data, consumed by `cargo xtask bench-report`).
//!
//! Run with `cargo bench -p pg-sql`. The report path is printed on completion.
//! See docs/plans/2026-04-14-benchmark-suite-design.md for the original design.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pg_sql::ast::parse_sql_file;
use pg_sql::bench_data::{BenchRecord, serialize_data_json};
use recursa::Input;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser as SqlParser;

// --- Paths ---

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

/// Repository root — the report is written under `<root>/docs/benchmarks/`.
fn repo_root() -> PathBuf {
    manifest_dir()
        .parent()
        .expect("pg-sql crate has a parent directory")
        .to_path_buf()
}

// --- Parsers under test ---

fn parse_with_pg_sql(sql: &str) -> bool {
    // Match production: run the `tokens!`-generated logos lex pass up front,
    // then parse the borrowed token array.
    let lexed = pg_sql::tokens::pg_lex(sql);
    let mut input = Input::new(sql, &lexed);
    parse_sql_file(&mut input).is_ok()
}

fn parse_with_sqlparser(sql: &str) -> bool {
    SqlParser::parse_sql(&PostgreSqlDialect {}, sql).is_ok()
}

/// PostgreSQL 17.9's raw parser via the pg-oracle FFI bridge. The bridge
/// allocates a `CString` per call and serialises through a global mutex —
/// both overheads count toward this parser's measured time, mirroring the
/// "parse one SQL string from scratch" model used for the other two.
///
/// `pg_oracle::parse_ok` panics on a NUL byte (`CString::new`). We now
/// benchmark the full regression corpus rather than the intersection so a
/// stray NUL would crash the run; treat any NUL-containing input as a
/// rejection — it's structurally invalid C-string input and the parser
/// can't see it anyway.
fn parse_with_postgres(sql: &str) -> bool {
    if sql.as_bytes().contains(&0) {
        return false;
    }
    pg_oracle::parse_ok(sql)
}

// --- Corpus loading ---

/// Load every `.sql` file under `dir` (non-recursive) into `(name, contents)`
/// pairs, sorted by filename for determinism. Files that are not valid UTF-8
/// (e.g. `collate.windows.win1252.sql`) are skipped with a warning.
fn load_sql_dir(dir: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("sql"))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            match fs::read_to_string(e.path()) {
                Ok(contents) => Some((name, contents)),
                Err(err) => {
                    eprintln!("warning: skipping fixture {name}: {err}");
                    None
                }
            }
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// --- Measurement ---

/// One benchmark: a name, the SQL inputs to parse per iteration, and the total
/// byte volume those inputs represent (for throughput).
///
/// The three `*_ok` flags record whether each parser *accepted* the workload
/// on a single up-front probe. They drive the table partition and the chart
/// inclusion filter: rows in which all three parsers succeeded form the "fair
/// head-to-head" set; the others surface as time-to-error and are reported
/// but excluded from the charts.
struct Bench {
    name: String,
    inputs: Vec<String>,
    bytes: u64,
    pg_sql_ok: bool,
    sqlparser_ok: bool,
    postgres_ok: bool,
}

/// The timing result for the three parsers on one benchmark.
struct Row {
    name: String,
    bytes: u64,
    pg_sql: Duration,
    sqlparser: Duration,
    postgres: Duration,
    pg_sql_ok: bool,
    sqlparser_ok: bool,
    postgres_ok: bool,
}

impl Row {
    /// `true` when every parser accepted the workload — the row qualifies for
    /// the head-to-head charts and the intersection table.
    fn all_accept(&self) -> bool {
        self.pg_sql_ok && self.sqlparser_ok && self.postgres_ok
    }
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
    let tick = |ok| if ok { '✓' } else { '✗' };
    println!(
        "  {:<24} pg-sql {:>9.3} ms {}   sqlparser {:>9.3} ms {}   postgres {:>9.3} ms {}",
        b.name,
        ms(pg_sql),
        tick(b.pg_sql_ok),
        ms(sqlparser),
        tick(b.sqlparser_ok),
        ms(postgres),
        tick(b.postgres_ok),
    );
    Row {
        name: b.name.clone(),
        bytes: b.bytes,
        pg_sql,
        sqlparser,
        postgres,
        pg_sql_ok: b.pg_sql_ok,
        sqlparser_ok: b.sqlparser_ok,
        postgres_ok: b.postgres_ok,
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

/// Build the benchmark set: one `corpus/<file>` benchmark for *every*
/// regression SQL file (including those some parser rejects — the rejected
/// rows are still timed as "time to error" and reported, but excluded from
/// the head-to-head charts), plus one benchmark per stress fixture.
fn build_benches() -> Vec<Bench> {
    let mut benches = Vec::new();

    // Corpus: one benchmark per regression SQL file. We do a single up-front
    // probe per parser to record acceptance; the timing pass that follows
    // still calls each parser, so rejected rows are timed as "time to error"
    // and surface in the report's non-intersection section. The
    // partition (intersection vs non-intersection) is derived from the
    // recorded `*_ok` flags at report-time.
    let corpus = load_sql_dir(&corpus_sql_dir());
    let total = corpus.len();
    let mut pg_ok = 0usize;
    let mut sp_ok = 0usize;
    let mut pgo_ok = 0usize;
    let mut intersection = 0usize;
    for (name, sql) in corpus {
        let a = parse_with_pg_sql(&sql);
        let b = parse_with_sqlparser(&sql);
        let c = parse_with_postgres(&sql);
        pg_ok += a as usize;
        sp_ok += b as usize;
        pgo_ok += c as usize;
        if a && b && c {
            intersection += 1;
        }
        let bytes = sql.len() as u64;
        let stem = name.strip_suffix(".sql").unwrap_or(&name);
        benches.push(Bench {
            name: format!("corpus/{stem}"),
            inputs: vec![sql],
            bytes,
            pg_sql_ok: a,
            sqlparser_ok: b,
            postgres_ok: c,
        });
    }
    eprintln!(
        "corpus: {total} files — pg-sql accepts {pg_ok}, sqlparser accepts {sp_ok}, \
         postgres accepts {pgo_ok}; {intersection} accepted by all three (the \
         intersection forms the head-to-head charts; the remaining \
         {} are timed but excluded from the charts).",
        total - intersection
    );

    // Stress fixtures: one `stress/<file>` benchmark per generated file. The
    // filename stem (e.g. `insert_values_100`) already names the shape and
    // size; the `stress/` prefix groups them apart from the `corpus/` entries.
    let stress = stress_dir();
    for (_shape, sizes) in stress_shapes() {
        for (_, file) in sizes {
            let sql = fs::read_to_string(stress.join(file))
                .unwrap_or_else(|e| panic!("read stress fixture {file}: {e}"));
            let pg_sql_ok = parse_with_pg_sql(&sql);
            let sqlparser_ok = parse_with_sqlparser(&sql);
            let postgres_ok = parse_with_postgres(&sql);
            let bytes = sql.len() as u64;
            let stem = file.strip_suffix(".sql").unwrap_or(file);
            benches.push(Bench {
                name: format!("stress/{stem}"),
                inputs: vec![sql],
                bytes,
                pg_sql_ok,
                sqlparser_ok,
                postgres_ok,
            });
        }
    }

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

/// Markdown character for a parser's acceptance result.
fn mark(ok: bool) -> char {
    if ok { '✓' } else { '✗' }
}

/// Render one results table — header plus one row per `Row`. Each per-parser
/// time cell carries a trailing ✓/✗ telling the reader whether that parser
/// accepted the workload; for rejected rows the timing is "time to error".
fn write_results_table(md: &mut String, rows: &[&Row]) {
    let _ = writeln!(
        md,
        "| Benchmark | Bytes | pg-sql time | sqlparser time | postgres time \
         | pg-sql throughput | sqlparser throughput | postgres throughput \
         | pg-sql vs sqlparser | pg-sql vs postgres |\n\
         |---|--:|--:|--:|--:|--:|--:|--:|--:|--:|"
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
            "| {} | {} | {:.3} ms {} | {:.3} ms {} | {:.3} ms {} \
             | {:.1} MiB/s | {:.1} MiB/s | {:.1} MiB/s | {:.2}× | {:.2}× |",
            r.name,
            r.bytes,
            ms(r.pg_sql),
            mark(r.pg_sql_ok),
            ms(r.sqlparser),
            mark(r.sqlparser_ok),
            ms(r.postgres),
            mark(r.postgres_ok),
            mib_per_s(r.bytes, r.pg_sql),
            mib_per_s(r.bytes, r.sqlparser),
            mib_per_s(r.bytes, r.postgres),
            sp_speedup,
            po_speedup,
        );
    }
}

fn build_report(rows: &[Row], timestamp: &str, commit: &str) -> Report {
    // Partition: rows where all three parsers accepted (the head-to-head set)
    // first, then everything else. Within each partition the input order
    // (alphabetical from load_sql_dir + stress in declaration order) is
    // preserved so the table is deterministic.
    let intersection: Vec<&Row> = rows.iter().filter(|r| r.all_accept()).collect();
    let rejected: Vec<&Row> = rows.iter().filter(|r| !r.all_accept()).collect();

    // Charts use only the intersection — mixing in time-to-error rows would
    // mislead a quick read.
    let chart_labels: Vec<String> = intersection.iter().map(|r| r.name.clone()).collect();
    let pg_time: Vec<f64> = intersection.iter().map(|r| ms(r.pg_sql)).collect();
    let sp_time: Vec<f64> = intersection.iter().map(|r| ms(r.sqlparser)).collect();
    let po_time: Vec<f64> = intersection.iter().map(|r| ms(r.postgres)).collect();
    let pg_tput: Vec<f64> = intersection
        .iter()
        .map(|r| mib_per_s(r.bytes, r.pg_sql))
        .collect();
    let sp_tput: Vec<f64> = intersection
        .iter()
        .map(|r| mib_per_s(r.bytes, r.sqlparser))
        .collect();
    let po_tput: Vec<f64> = intersection
        .iter()
        .map(|r| mib_per_s(r.bytes, r.postgres))
        .collect();

    let mut md = String::new();
    let _ = writeln!(md, "# pg-sql parser benchmark");
    md.push('\n');
    let _ = writeln!(md, "- **Generated:** {timestamp}");
    let _ = writeln!(md, "- **Commit:** `{commit}`");
    let _ = writeln!(
        md,
        "- **Parsers:** pg-sql (this crate) vs \
         [`sqlparser`](https://crates.io/crates/sqlparser) vs PostgreSQL's \
         raw parser (via [`pg-oracle`](../../pg-oracle/), linking the \
         vendored PostgreSQL 17.9 source)"
    );
    md.push('\n');
    let _ = writeln!(
        md,
        "Every regression SQL file is benchmarked with all three parsers. Time \
         is the median wall-clock per iteration; throughput is the workload's \
         byte volume divided by that time. The **first table** lists rows in \
         which all three parsers accepted the input (the fair head-to-head set \
         — these are the rows the charts cover); the **second table** lists \
         rows where at least one parser rejected — their timings still appear \
         (as time-to-error), but the head-to-head speedup is no longer \
         apples-to-apples. The per-parser ✓/✗ columns disambiguate. The \
         postgres column measures `pg_oracle::parse_ok`, which includes a \
         per-call `CString` allocation and a global-mutex acquisition on top \
         of the underlying `raw_parser()` invocation — both overheads count \
         toward the measured time, mirroring the end-to-end \"parse one SQL \
         string from scratch\" model used for the other two parsers."
    );
    md.push('\n');

    let total = rows.len();
    let n_intersection = intersection.len();
    let n_rejected = rejected.len();
    let _ = writeln!(
        md,
        "- **Benchmarks:** {total} ({n_intersection} accepted by all three \
         parsers, {n_rejected} rejected by ≥ 1)"
    );
    md.push('\n');

    // Two-table layout: intersection first, then rejected.
    let _ = writeln!(md, "## Results — intersection (all three parsers accept)\n");
    write_results_table(&mut md, &intersection);
    md.push('\n');
    if !rejected.is_empty() {
        let _ = writeln!(md, "## Results — rejected by ≥ 1 parser (time-to-error)\n");
        write_results_table(&mut md, &rejected);
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
    let _ = writeln!(md, "## Parse time (lower is better) — intersection only\n");
    let _ = writeln!(md, "![Parse time per benchmark](time.svg)\n");
    let _ = writeln!(md, "## Throughput (higher is better) — intersection only\n");
    let _ = writeln!(md, "![Throughput per benchmark](throughput.svg)\n");
    let _ = writeln!(
        md,
        "_Each benchmark shows three side-by-side bars — blue = **pg-sql**, \
         amber = **sqlparser**, green = **postgres** (PostgreSQL 17.9 raw \
         parser via pg-oracle). Charts cover the intersection set only._"
    );

    Report {
        markdown: md,
        time_svg,
        throughput_svg,
    }
}

fn main() {
    println!("pg-sql benchmark harness");
    let benches = build_benches();

    println!("running {} benchmarks...", benches.len());
    let rows: Vec<Row> = benches.iter().map(run_bench).collect();

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

    let report = build_report(&rows, &timestamp, &commit);

    // Each run gets its own subdirectory; the report references the charts by
    // bare filename, so report.md, the SVGs, and data.json all sit together.
    let run_dir = repo_root()
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

    // Raw data for trend analysis (`cargo xtask bench-report`).
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
