//! Markdown rendering for the flamegraph diagnostic report.
//!
//! Pure function from `(Meta, Host, &[Row]) -> String`. Keeping it pure means
//! tests are fast, deterministic, and free of filesystem/clock dependencies.

use crate::flame_report::host::Host;

/// Run-level metadata stamped into the report header.
#[derive(Debug, Clone)]
pub struct Meta {
    pub short_sha: String,
    pub full_sha: String,
    pub branch: String,
    pub dirty: bool,
    pub timestamp_iso: String,
    pub duration_secs: u64,
}

/// A successful per-fixture row: timing pass + profile pass both completed.
#[derive(Debug, Clone)]
pub struct RowOk {
    pub fixture_rel: String,
    pub bytes: u64,
    pub iters: u64,
    pub ns_per_iter: u64,
    pub svg_rel: String,
}

/// A timing-only row: the timing pass succeeded but the profile pass
/// failed. The user keeps the iter/ns/MB/s numbers, and the Flamegraph
/// column surfaces the profile error instead of a (broken) SVG link.
/// Used when e.g. `cargo-instruments` errors out on macOS but the parser
/// itself ran fine.
#[derive(Debug, Clone)]
pub struct RowOkNoSvg {
    pub fixture_rel: String,
    pub bytes: u64,
    pub iters: u64,
    pub ns_per_iter: u64,
    pub profile_error: String,
}

/// A failed per-fixture row: timing pass itself returned an error (no
/// usable timing data). The fixture is still listed in the report (with
/// bytes) so the run is complete-but-degraded rather than aborted.
#[derive(Debug, Clone)]
pub struct RowFailed {
    pub fixture_rel: String,
    pub bytes: u64,
    pub reason: String,
}

/// Per-fixture row in the report. Plain runtime data — not a `Parse`-derived
/// enum — so the single-field-tuple-variant convention does not apply here.
#[derive(Debug, Clone)]
pub enum Row {
    Ok(RowOk),
    OkNoSvg(RowOkNoSvg),
    Failed(RowFailed),
}

impl Row {
    pub fn ok(r: RowOk) -> Self {
        Row::Ok(r)
    }

    pub fn ok_no_svg(r: RowOkNoSvg) -> Self {
        Row::OkNoSvg(r)
    }

    pub fn failed(r: RowFailed) -> Self {
        Row::Failed(r)
    }

    pub fn fixture_rel(&self) -> &str {
        match self {
            Row::Ok(r) => &r.fixture_rel,
            Row::OkNoSvg(r) => &r.fixture_rel,
            Row::Failed(r) => &r.fixture_rel,
        }
    }

    pub fn bytes(&self) -> u64 {
        match self {
            Row::Ok(r) => r.bytes,
            Row::OkNoSvg(r) => r.bytes,
            Row::Failed(r) => r.bytes,
        }
    }
}

/// Maximum number of characters of a profile error to embed in the
/// Flamegraph cell of the Timing table. The full cell wraps the error in
/// `"— (profile failed: <error>)"` (~22 chars of overhead), so 60 keeps
/// the rendered cell at roughly 80 chars — wide enough to be readable,
/// narrow enough that the table doesn't wrap on a typical terminal /
/// markdown viewer.
const PROFILE_ERROR_MAX: usize = 60;

/// Render the full markdown report. Pure function: no I/O, no clock access.
///
/// Layout:
/// 1. `# Flamegraph run <sha[-dirty]> @ <iso-timestamp>` header
/// 2. Metadata bullet list (commit, branch, timestamp, profile, loop duration)
/// 3. `## Host` table (delegated to `Host::render_markdown`)
/// 4. `## Fixtures` table (one row per fixture, full relative path + bytes)
/// 5. `## Timing` table (one row per fixture, file_name label + iters/ns/MB/s/svg)
/// 6. `## Notes` placeholder for human annotation
pub fn render(meta: &Meta, host: &Host, rows: &[Row]) -> String {
    let mut s = String::new();

    // Header — `<short_sha>` or `<short_sha>-dirty`.
    let sha_stem = if meta.dirty {
        format!("{}-dirty", meta.short_sha)
    } else {
        meta.short_sha.clone()
    };
    s.push_str(&format!(
        "# Flamegraph run {sha_stem} @ {}\n\n",
        meta.timestamp_iso
    ));

    // Metadata bullet list.
    let dirty_tag = if meta.dirty { ", dirty" } else { "" };
    s.push_str(&format!(
        "- **Commit:** `{}` (`{}`{})\n",
        meta.full_sha, meta.short_sha, dirty_tag
    ));
    s.push_str(&format!("- **Branch:** `{}`\n", meta.branch));
    s.push_str(&format!("- **Timestamp:** `{}`\n", meta.timestamp_iso));
    s.push_str("- **Build profile:** release\n");
    s.push_str(&format!(
        "- **Loop duration:** {}s per fixture\n\n",
        meta.duration_secs
    ));

    // Host section.
    s.push_str(&host.render_markdown());
    s.push('\n');

    // Fixtures table — full relative path + raw byte count.
    s.push_str("## Fixtures\n\n| Fixture | Bytes |\n|---|--:|\n");
    for row in rows {
        s.push_str(&format!(
            "| `{}` | {} |\n",
            row.fixture_rel(),
            format_num(row.bytes())
        ));
    }
    s.push('\n');

    // Timing table — uses file_name (basename including extension) for the
    // Fixture column to keep the row narrow while staying unambiguous about
    // the file type. Note: "MB/s" is computed against 1 MiB (1_048_576),
    // matching the design example numbers; the label stays "MB/s" for
    // readability.
    s.push_str(
        "## Timing\n\n| Fixture | Iterations | ns / iter | MB/s | Flamegraph |\n|---|--:|--:|--:|---|\n",
    );
    for row in rows {
        match row {
            Row::Ok(r) => {
                let mb_per_s = (r.bytes as f64) / ((r.ns_per_iter as f64) / 1e9) / 1_048_576.0;
                let name = std::path::Path::new(&r.fixture_rel)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("flamegraph");
                s.push_str(&format!(
                    "| `{}` | {} | {} | {:.1} | [svg]({}) |\n",
                    name,
                    format_num(r.iters),
                    format_num(r.ns_per_iter),
                    mb_per_s,
                    r.svg_rel
                ));
            }
            Row::OkNoSvg(r) => {
                // Timing succeeded — render the iter/ns/MB/s columns the
                // same way Ok does. The Flamegraph cell holds the profile
                // error rather than a (would-be-broken) SVG link.
                let mb_per_s = (r.bytes as f64) / ((r.ns_per_iter as f64) / 1e9) / 1_048_576.0;
                let name = std::path::Path::new(&r.fixture_rel)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("flamegraph");
                let truncated = truncate_for_cell(&r.profile_error);
                s.push_str(&format!(
                    "| `{}` | {} | {} | {:.1} | — (profile failed: {}) |\n",
                    name,
                    format_num(r.iters),
                    format_num(r.ns_per_iter),
                    mb_per_s,
                    truncated,
                ));
            }
            Row::Failed(r) => {
                // 5 cells to match the header — markdown silently drops
                // trailing cells if the count is short, so the empty trailing
                // cells must be emitted explicitly.
                let name = std::path::Path::new(&r.fixture_rel)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("fixture");
                s.push_str(&format!(
                    "| `{}` | — failed: {} |  |  |  |\n",
                    name, r.reason
                ));
            }
        }
    }
    s.push('\n');

    // Notes — empty by default; reserved for human annotation post-run.
    s.push_str("## Notes\n\n_Empty by default — reserved for human annotation after the run._\n");

    s
}

/// Truncate a profile error so it fits in a single Flamegraph table cell.
/// Anything longer than `PROFILE_ERROR_MAX` is clipped and given an
/// ellipsis suffix; shorter strings pass through untouched. Operates on
/// chars (not bytes) to stay UTF-8-safe.
fn truncate_for_cell(msg: &str) -> String {
    if msg.chars().count() <= PROFILE_ERROR_MAX {
        msg.to_string()
    } else {
        let head: String = msg.chars().take(PROFILE_ERROR_MAX).collect();
        format!("{head}…")
    }
}

/// Insert thousands separators into a u64. Small enough that a manual loop is
/// fine — no point pulling in a crate for this.
fn format_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flame_report::host::Host;

    fn sample_host() -> Host {
        Host {
            model: "MacBook Pro (Mac14,6)".into(),
            chip: Some("Apple M1 Max".into()),
            cores: "10".into(),
            memory: "64 GB".into(),
            os: "macOS 14.5 (Darwin 24.5.0)".into(),
            arch: "arm64".into(),
            power_source: Some("AC".into()),
            rustc: "1.83.0 (stable)".into(),
            cargo_flamegraph: "0.6.5".into(),
        }
    }

    #[test]
    fn renders_full_report() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234deadbeef".into(),
            branch: "main".into(),
            dirty: false,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let rows = vec![
            Row::ok(RowOk {
                fixture_rel: "pg-sql/fixtures/stress/bool_chain_100.sql".into(),
                bytes: 4821,
                iters: 12400,
                ns_per_iter: 403226,
                svg_rel: "bool_chain_100.svg".into(),
            }),
            Row::failed(RowFailed {
                fixture_rel: "pg-sql/fixtures/sql/broken.sql".into(),
                bytes: 100,
                reason: "cargo-flamegraph exited with status 1".into(),
            }),
        ];
        let md = render(&meta, &sample_host(), &rows);
        assert!(md.contains("# Flamegraph run abc1234 @ 2026-04-24T13:45:02Z"));
        assert!(md.contains("| `pg-sql/fixtures/stress/bool_chain_100.sql` | 4,821 |"));
        // Timing-table label keeps the .sql extension to match the design example.
        assert!(md.contains("| `bool_chain_100.sql` | 12,400 | 403,226 |"));
        // Failed row must produce all 5 cells: stem | failure | empty | empty | empty
        // (markdown silently drops trailing cells, so this is a quiet bug if missed).
        assert!(md.contains(
            "| `broken.sql` | — failed: cargo-flamegraph exited with status 1 |  |  |  |"
        ));
        assert!(md.contains("## Notes"));
    }

    /// Defensive regression test: the failed Timing row must have exactly 6
    /// pipe characters (5 cells = 6 pipes including the leading and trailing
    /// `|`), matching the 5-column Timing table header.
    #[test]
    fn failed_row_has_correct_pipe_count() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234".into(),
            branch: "main".into(),
            dirty: false,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let rows = vec![Row::failed(RowFailed {
            fixture_rel: "pg-sql/fixtures/sql/broken.sql".into(),
            bytes: 100,
            reason: "boom".into(),
        })];
        let md = render(&meta, &sample_host(), &rows);
        let failed_line = md
            .lines()
            .find(|l| l.contains("— failed: boom"))
            .expect("failed row should be in output");
        let pipes = failed_line.chars().filter(|c| *c == '|').count();
        assert_eq!(
            pipes, 6,
            "failed row should have 6 pipes (5 cells), got {pipes}: {failed_line:?}"
        );
    }

    #[test]
    fn dirty_marker_appears_in_header() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234".into(),
            branch: "main".into(),
            dirty: true,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let md = render(&meta, &sample_host(), &[]);
        assert!(md.contains("# Flamegraph run abc1234-dirty @"));
        assert!(md.contains("dirty"));
    }

    /// `Row::OkNoSvg` represents a fixture whose timing pass succeeded but
    /// whose profile pass failed (e.g. cargo-instruments errored, or
    /// flamegraph.pl was missing). The timing columns must render normally
    /// so the user keeps the iter/ns/MB/s data, and the Flamegraph column
    /// must surface the profile error rather than a (broken) link.
    #[test]
    fn ok_no_svg_row_renders_timing_with_profile_error() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234deadbeef".into(),
            branch: "main".into(),
            dirty: false,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let rows = vec![Row::ok_no_svg(RowOkNoSvg {
            fixture_rel: "pg-sql/fixtures/stress/bool_chain_100.sql".into(),
            bytes: 4821,
            iters: 12400,
            ns_per_iter: 403226,
            profile_error: "xctrace export failed".into(),
        })];
        let md = render(&meta, &sample_host(), &rows);
        // Bytes still appear in the Fixtures table.
        assert!(md.contains("| `pg-sql/fixtures/stress/bool_chain_100.sql` | 4,821 |"));
        // Timing columns render normally — user keeps the timing data.
        assert!(md.contains("| `bool_chain_100.sql` | 12,400 | 403,226 |"));
        // Flamegraph column surfaces the profile error.
        assert!(md.contains("— (profile failed: xctrace export failed)"));
    }

    /// Defensive regression test: like the failed-row pipe-count test, the
    /// ok-no-svg row must produce exactly 5 cells (6 pipes) so the markdown
    /// table doesn't lose alignment when the profile pass fails but timing
    /// succeeds.
    #[test]
    fn ok_no_svg_row_has_correct_pipe_count() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234".into(),
            branch: "main".into(),
            dirty: false,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let rows = vec![Row::ok_no_svg(RowOkNoSvg {
            fixture_rel: "pg-sql/fixtures/stress/bool_chain_100.sql".into(),
            bytes: 4821,
            iters: 12400,
            ns_per_iter: 403226,
            profile_error: "boom".into(),
        })];
        let md = render(&meta, &sample_host(), &rows);
        let timing_line = md
            .lines()
            .find(|l| l.contains("`bool_chain_100.sql`") && l.contains("12,400"))
            .expect("ok-no-svg timing row should be in output");
        let pipes = timing_line.chars().filter(|c| *c == '|').count();
        assert_eq!(
            pipes, 6,
            "ok-no-svg row should have 6 pipes (5 cells), got {pipes}: {timing_line:?}"
        );
    }

    /// Long profile errors should be truncated so the Flamegraph cell
    /// stays readable at typical markdown widths. The raw error is bounded
    /// by `PROFILE_ERROR_MAX` (60 chars), and the rendered cell — which
    /// includes the `"— (profile failed: …)"` wrapper — must stay within
    /// roughly 80 chars total.
    #[test]
    fn ok_no_svg_row_truncates_long_profile_error() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234".into(),
            branch: "main".into(),
            dirty: false,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let long_error = "x".repeat(500);
        let rows = vec![Row::ok_no_svg(RowOkNoSvg {
            fixture_rel: "pg-sql/fixtures/stress/bool_chain_100.sql".into(),
            bytes: 4821,
            iters: 12400,
            ns_per_iter: 403226,
            profile_error: long_error,
        })];
        let md = render(&meta, &sample_host(), &rows);
        let cell = md
            .lines()
            .find(|l| l.contains("(profile failed:"))
            .expect("ok-no-svg row should be in output");
        // Bound on the raw error: must not exceed PROFILE_ERROR_MAX chars.
        let xs = cell.matches('x').count();
        assert!(
            xs <= PROFILE_ERROR_MAX,
            "profile_error should be truncated to <= {PROFILE_ERROR_MAX} chars, got {xs} x's: {cell:?}"
        );
        // Bound on the full cell content (between the last two `|`s): must
        // be roughly 80 chars including the wrapper.
        let last_cell = cell
            .rsplit_once('|')
            .and_then(|(rest, _)| rest.rsplit_once('|').map(|(_, c)| c))
            .expect("cell should have at least 2 pipes");
        assert!(
            last_cell.chars().count() <= 90,
            "rendered cell should be ~80 chars, got {} chars: {last_cell:?}",
            last_cell.chars().count()
        );
    }

    /// Update of the original `failed_row_has_correct_pipe_count`'s sibling
    /// test idea applied to all three variants: the `renders_full_report`
    /// expectations should now cover Ok, OkNoSvg, and Failed in one pass to
    /// guarantee row dispatch + cell counts stay correct as variants grow.
    #[test]
    fn renders_full_report_with_all_three_row_variants() {
        let meta = Meta {
            short_sha: "abc1234".into(),
            full_sha: "abc1234deadbeef".into(),
            branch: "main".into(),
            dirty: false,
            timestamp_iso: "2026-04-24T13:45:02Z".into(),
            duration_secs: 5,
        };
        let rows = vec![
            Row::ok(RowOk {
                fixture_rel: "pg-sql/fixtures/stress/bool_chain_100.sql".into(),
                bytes: 4821,
                iters: 12400,
                ns_per_iter: 403226,
                svg_rel: "bool_chain_100.svg".into(),
            }),
            Row::ok_no_svg(RowOkNoSvg {
                fixture_rel: "pg-sql/fixtures/sql/numeric_big.sql".into(),
                bytes: 9999,
                iters: 800,
                ns_per_iter: 6_250_000,
                profile_error: "cargo-instruments missing".into(),
            }),
            Row::failed(RowFailed {
                fixture_rel: "pg-sql/fixtures/sql/broken.sql".into(),
                bytes: 100,
                reason: "timing pass failed: parse error".into(),
            }),
        ];
        let md = render(&meta, &sample_host(), &rows);
        // All three fixtures appear in the Fixtures table.
        assert!(md.contains("| `pg-sql/fixtures/stress/bool_chain_100.sql` | 4,821 |"));
        assert!(md.contains("| `pg-sql/fixtures/sql/numeric_big.sql` | 9,999 |"));
        assert!(md.contains("| `pg-sql/fixtures/sql/broken.sql` | 100 |"));
        // Ok renders with svg link.
        assert!(md.contains("[svg](bool_chain_100.svg)"));
        // OkNoSvg renders with profile error suffix.
        assert!(md.contains("— (profile failed: cargo-instruments missing)"));
        // Failed renders with failure reason.
        assert!(md.contains("— failed: timing pass failed: parse error"));
    }
}
