//! Orchestrator for the flamegraph diagnostic pipeline. Glues together
//! `flame::run_loop`, the host probe, the git state capture, the profile
//! pipeline, and the markdown renderer; for each fixture it runs an
//! in-process timing pass then dispatches to the platform-specific
//! profiler (Linux: `cargo flamegraph`; macOS: `cargo instruments` +
//! `xctrace` + the FlameGraph perl scripts).
//!
//! See `docs/plans/2026-04-24-flamegraph-diagnostic-design.md` for the
//! design rationale and Task 8 of the implementation plan for the macOS
//! pivot history.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::flame::run_loop;
use crate::flame_report::git::GitState;
use crate::flame_report::host::Host;
use crate::flame_report::profiler::{ProfileCtx, profile};
use crate::flame_report::render::{Meta, Row, RowFailed, RowOk, RowOkNoSvg, render};

/// Parsed CLI arguments for the `flame_report` binary.
#[derive(Debug, Clone)]
pub struct Args {
    pub fixtures: Vec<PathBuf>,
    pub duration_secs: u64,
    pub out: PathBuf,
    /// Path to a checkout of <https://github.com/brendangregg/FlameGraph>.
    /// Required on macOS (used by the profiler to invoke
    /// `stackcollapse-instruments.pl` and `flamegraph.pl`); silently ignored
    /// on Linux. Resolution priority: `--flamegraph-dir` flag, then the
    /// `FLAMEGRAPH_DIR` env var, then `None`.
    pub flamegraph_dir: Option<PathBuf>,
}

impl Args {
    /// Parse argv (without `argv[0]`) into structured `Args`. Hand-rolled to
    /// avoid pulling clap into the orchestrator just for a handful of flags.
    ///
    /// Recognised flags:
    /// - `--duration <seconds>` (default `5`)
    /// - `--out <dir>` (default `docs/perf/flamegraphs`)
    /// - `--flamegraph-dir <path>` (falls back to `FLAMEGRAPH_DIR`)
    ///
    /// All other positional args are treated as fixture paths. At least one
    /// fixture is required.
    pub fn parse(argv: &[String]) -> Result<Self, String> {
        Self::parse_with_env_lookup(argv, |k| std::env::var_os(k))
    }

    /// Inner constructor that accepts a custom env lookup so tests can
    /// exercise both the CLI-flag and env-var fallback paths without
    /// mutating real process environment (which Rust 2024 marks `unsafe`
    /// and which races across parallel tests).
    pub(crate) fn parse_with_env_lookup<F>(argv: &[String], env_lookup: F) -> Result<Self, String>
    where
        F: Fn(&str) -> Option<std::ffi::OsString>,
    {
        let mut fixtures: Vec<PathBuf> = Vec::new();
        let mut duration_secs: u64 = 5;
        let mut out: PathBuf = PathBuf::from("docs/perf/flamegraphs");
        let mut flamegraph_dir: Option<PathBuf> = None;
        let mut i = 0;
        while i < argv.len() {
            match argv[i].as_str() {
                "--duration" => {
                    i += 1;
                    let v = argv.get(i).ok_or("--duration needs a value")?;
                    duration_secs = v.parse().map_err(|_| format!("bad --duration: {v}"))?;
                }
                "--out" => {
                    i += 1;
                    let v = argv.get(i).ok_or("--out needs a value")?;
                    out = PathBuf::from(v);
                }
                "--flamegraph-dir" => {
                    i += 1;
                    let v = argv.get(i).ok_or("--flamegraph-dir needs a value")?;
                    flamegraph_dir = Some(PathBuf::from(v));
                }
                s if s.starts_with("--") => return Err(format!("unknown flag: {s}")),
                s => fixtures.push(PathBuf::from(s)),
            }
            i += 1;
        }
        if fixtures.is_empty() {
            return Err("at least one fixture path is required".into());
        }
        // Env-var fallback fires only when the CLI flag was absent.
        if flamegraph_dir.is_none() {
            flamegraph_dir = env_lookup("FLAMEGRAPH_DIR").map(PathBuf::from);
        }
        Ok(Self {
            fixtures,
            duration_secs,
            out,
            flamegraph_dir,
        })
    }
}

/// Outcome of a successful orchestrator run. Carries both the markdown
/// path (always written, even with partial failures) and a flag indicating
/// whether any fixture failed so the `flame_report` binary can map the
/// result to a non-zero exit code while still surfacing the report path.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub md_path: PathBuf,
    pub any_failed: bool,
}

/// Drive the full diagnostic pipeline:
///
/// 1. Validate every fixture exists.
/// 2. Verify `cargo flamegraph` is installed.
/// 3. Capture git state.
/// 4. Probe host metadata.
/// 5. Compute timestamp + run dir, create directory.
/// 6. For each fixture: run the timing pass in-process, then shell out to
///    `cargo flamegraph` to produce the SVG. Per-fixture failures are
///    recorded as `Row::Failed` and do not abort the run.
/// 7. Render markdown and write it to the run dir.
///
/// Returns a `RunOutcome` carrying the markdown path and an `any_failed`
/// flag. The markdown is written even when fixtures failed, so callers
/// always get a report; the flag lets the `flame_report` binary exit
/// non-zero on partial failure per the design contract.
pub fn run(args: Args) -> Result<RunOutcome, String> {
    // 1. Validate every fixture exists up-front. Catches typos before we
    //    burn minutes profiling the fixtures that do exist.
    let missing: Vec<_> = args.fixtures.iter().filter(|p| !p.is_file()).collect();
    if !missing.is_empty() {
        let list: Vec<String> = missing.iter().map(|p| p.display().to_string()).collect();
        return Err(format!("fixtures not found:\n  {}", list.join("\n  ")));
    }

    // 2. Platform-specific startup checks.
    //    On macOS we run a 4-stage pipeline (cargo-instruments, xctrace,
    //    stackcollapse-instruments.pl, flamegraph.pl). On Linux we still
    //    use cargo-flamegraph, which is perf-based and works.
    check_profiler_toolchain(&args)?;

    // 3. Capture git state. Required — the report dir name depends on it.
    let git = GitState::capture().map_err(|e| format!("git: {e}"))?;

    // 4. Probe host. Best-effort; returns `unknown` rather than failing.
    let host = Host::probe();

    // 5. Compute timestamp + run dir. The compact timestamp is used in the
    //    directory name; a pretty form is stamped into the markdown header.
    let timestamp = iso8601_utc_compact();
    let run_stem = git.run_stem(&timestamp);
    let run_dir = args.out.join(&run_stem);
    fs::create_dir_all(&run_dir).map_err(|e| format!("mkdir {}: {e}", run_dir.display()))?;

    // CARGO_MANIFEST_DIR is `pg-sql/`; its parent is the workspace root.
    // The unwrap is acceptable: in any cargo build the manifest dir always
    // has a parent (the workspace root or the package's containing dir).
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();

    // Bundle the profiler's per-call context once so we don't rebuild it
    // per fixture. `flamegraph_dir` is required on macOS and ignored on
    // Linux; `run_dir` is where intermediate `.trace` / `.xml` / `.folded`
    // artefacts are written and cleaned up.
    let profile_ctx = ProfileCtx {
        flamegraph_dir: args.flamegraph_dir.clone(),
        run_dir: run_dir.clone(),
    };

    // 6. Per-fixture loop. Failures of a single fixture do not abort the
    //    run — they appear as `Row::Failed` (timing pass failed) or
    //    `Row::OkNoSvg` (timing succeeded, profile failed) in the report.
    let mut rows: Vec<Row> = Vec::new();
    let mut any_failed = false;
    for fixture in &args.fixtures {
        let bytes = fs::metadata(fixture).map(|m| m.len()).unwrap_or(0);
        let stem = fixture
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("fixture")
            .to_string();
        let svg_path = run_dir.join(format!("{stem}.svg"));

        // Timing pass — in-process, no profiler. Numbers correspond to what
        // the profile pass samples because both call `flame::run_loop`.
        let timing = run_loop(fixture, Duration::from_secs(args.duration_secs));
        let (iters, elapsed) = match timing {
            Ok(t) => t,
            Err(e) => {
                any_failed = true;
                rows.push(Row::failed(RowFailed {
                    fixture_rel: rel_to_repo(fixture, &repo_root),
                    bytes,
                    reason: format!("timing pass failed: {e}"),
                }));
                continue;
            }
        };

        // Profile pass — delegated to the platform-specific pipeline in
        // `profiler::profile`. On macOS this is a 4-stage Instruments-based
        // pipeline; on Linux it is `cargo flamegraph` as before. Both
        // inherit stdio so any sudo / authorisation prompts reach the user.
        //
        // `run_loop` is permissive and can in principle return `iters == 0`
        // for a `--duration 0` request; guard with `max(1)` so the divisor
        // below is never zero.
        let ns_per_iter = elapsed.as_nanos() as u64 / iters.max(1);
        let profile_result = profile(fixture, args.duration_secs, &svg_path, &profile_ctx);
        match profile_result {
            Ok(()) if svg_path.is_file() => {
                rows.push(Row::ok(RowOk {
                    fixture_rel: rel_to_repo(fixture, &repo_root),
                    bytes,
                    iters,
                    ns_per_iter,
                    svg_rel: format!("{stem}.svg"),
                }));
            }
            Ok(()) => {
                // The profiler claimed success but the SVG isn't on disk.
                // Defensive: treat as profile failure but keep timing data
                // so the user still sees their numbers.
                any_failed = true;
                rows.push(Row::ok_no_svg(RowOkNoSvg {
                    fixture_rel: rel_to_repo(fixture, &repo_root),
                    bytes,
                    iters,
                    ns_per_iter,
                    profile_error: format!("svg missing at {}", svg_path.display()),
                }));
            }
            Err(reason) => {
                // Profile pass failed but we have timing data — record an
                // OkNoSvg row so the user keeps the numbers and sees the
                // error. The run as a whole is a partial failure for
                // exit-code purposes.
                any_failed = true;
                rows.push(Row::ok_no_svg(RowOkNoSvg {
                    fixture_rel: rel_to_repo(fixture, &repo_root),
                    bytes,
                    iters,
                    ns_per_iter,
                    profile_error: reason,
                }));
            }
        }
    }

    // 7. Render markdown and write it. Always written, even with failures,
    //    so the user sees the partial report instead of nothing.
    let meta = Meta {
        short_sha: git.short_sha.clone(),
        full_sha: git.full_sha.clone(),
        branch: git.branch.clone(),
        dirty: git.dirty,
        timestamp_iso: iso8601_utc_pretty_from_compact(&timestamp),
        duration_secs: args.duration_secs,
    };
    let md = render(&meta, &host, &rows);
    let md_path = run_dir.join(format!("{run_stem}.md"));
    fs::write(&md_path, md).map_err(|e| format!("write markdown: {e}"))?;

    if any_failed {
        eprintln!(
            "flame_report: completed with failures — see {}",
            md_path.display()
        );
    }
    Ok(RunOutcome {
        md_path,
        any_failed,
    })
}

/// Render `path` relative to `repo_root` for display in the markdown report.
/// Falls back to the absolute display path when the fixture is outside the
/// repo (e.g. an absolute `/tmp/...` path) — failing here would be silly when
/// all we want is a label.
fn rel_to_repo(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

/// Verify that the platform-appropriate profiling toolchain is installed
/// and points at usable artefacts before we start measuring. Failing fast
/// here means the user gets a clear error in two seconds rather than ten
/// minutes into a multi-fixture run.
///
/// On macOS we need:
/// - `cargo instruments --version` (cargo-instruments)
/// - `xcrun --find xctrace` (Xcode Command Line Tools)
/// - `flamegraph_dir` set, with both `stackcollapse-instruments.pl` and
///   `flamegraph.pl` present.
///
/// On Linux we need `cargo flamegraph --version`. `flamegraph_dir` is
/// silently ignored (cross-platform invocations stay identical).
fn check_profiler_toolchain(args: &Args) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        check_macos_toolchain(args)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = args;
        check_linux_toolchain()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = args;
        Err("flame_report only supports macOS and Linux".into())
    }
}

#[cfg(target_os = "macos")]
fn check_macos_toolchain(args: &Args) -> Result<(), String> {
    if !command_succeeds("cargo", &["instruments", "--version"]) {
        return Err(
            "cargo-instruments not installed. Install with: cargo install cargo-instruments".into(),
        );
    }
    if !command_succeeds("xcrun", &["--find", "xctrace"]) {
        return Err(
            "xctrace not found. Install Xcode Command Line Tools: xcode-select --install".into(),
        );
    }
    let dir = args.flamegraph_dir.as_ref().ok_or(
        "--flamegraph-dir required on macOS (or set FLAMEGRAPH_DIR). \
         Get FlameGraph from https://github.com/brendangregg/FlameGraph",
    )?;
    let stackcollapse = dir.join("stackcollapse-instruments.pl");
    let flamegraph_pl = dir.join("flamegraph.pl");
    if !stackcollapse.is_file() || !flamegraph_pl.is_file() {
        return Err(format!(
            "{} is missing stackcollapse-instruments.pl or flamegraph.pl. \
             Get FlameGraph from https://github.com/brendangregg/FlameGraph",
            dir.display(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn check_linux_toolchain() -> Result<(), String> {
    // NOTE: `args.flamegraph_dir` is intentionally not consulted here.
    // Linux uses `cargo flamegraph` directly and has no use for the
    // FlameGraph perl scripts. We swallow the value silently rather than
    // warn so that a single cross-platform invocation
    // (`flame_report ... --flamegraph-dir ~/tools/FlameGraph`) works on
    // both OSes without conditional shell scripting on the user's side.
    if !command_succeeds("cargo", &["flamegraph", "--version"]) {
        return Err("cargo flamegraph not installed. Try: cargo install flamegraph".into());
    }
    Ok(())
}

/// True when `cmd args...` exits with status zero. False on spawn error or
/// non-zero exit. Used by the toolchain checks to probe for installed CLIs
/// without caring about their stdout.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn command_succeeds(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Compact UTC ISO-8601 timestamp suitable for filesystem use:
/// `YYYYMMDDTHHMMSSZ`. Hand-rolled to avoid pulling in `chrono` or `time`
/// for what is, ultimately, one timestamp string per run.
fn iso8601_utc_compact() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (y, mo, d, h, mi, s) = epoch_to_ymdhms(secs);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

/// Convert the compact form (`YYYYMMDDTHHMMSSZ`, length 16) to the pretty
/// ISO-8601 form (`YYYY-MM-DDTHH:MM:SSZ`) used in the markdown header. If
/// the input doesn't have the expected length, returns it unchanged — this
/// is a display helper, not a validator.
fn iso8601_utc_pretty_from_compact(compact: &str) -> String {
    if compact.len() != 16 {
        return compact.to_string();
    }
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &compact[0..4],
        &compact[4..6],
        &compact[6..8],
        &compact[9..11],
        &compact[11..13],
        &compact[13..15],
    )
}

/// Convert seconds since the Unix epoch into `(year, month, day, hour,
/// minute, second)` in UTC. Hand-rolled Gregorian arithmetic — avoids a
/// date-crate dependency for the single use site here. Years 1970+ only.
fn epoch_to_ymdhms(mut secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let s = (secs % 60) as u32;
    secs /= 60;
    let mi = (secs % 60) as u32;
    secs /= 60;
    let h = (secs % 24) as u32;
    secs /= 24;
    let mut days = secs as i64;
    let mut y: i32 = 1970;
    loop {
        let dy = if is_leap(y) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        y += 1;
    }
    let months = [
        31,
        if is_leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 0usize;
    while mo < 12 && days >= months[mo] {
        days -= months[mo];
        mo += 1;
    }
    (y as u32, (mo + 1) as u32, (days + 1) as u32, h, mi, s)
}

/// Gregorian leap-year rule: divisible by 4, except centuries unless also
/// divisible by 400.
fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_requires_fixtures() {
        let args: Vec<String> = vec![];
        assert!(Args::parse(&args).is_err());
    }

    #[test]
    fn parse_args_accepts_positional_fixtures() {
        let args = vec!["a.sql".into(), "b.sql".into()];
        let a = Args::parse(&args).unwrap();
        assert_eq!(a.fixtures.len(), 2);
        assert_eq!(a.duration_secs, 5); // default
    }

    #[test]
    fn parse_args_reads_duration_and_out() {
        let args = vec![
            "x.sql".into(),
            "--duration".into(),
            "10".into(),
            "--out".into(),
            "custom/out".into(),
        ];
        let a = Args::parse(&args).unwrap();
        assert_eq!(a.duration_secs, 10);
        assert_eq!(a.out.to_string_lossy(), "custom/out");
    }

    #[test]
    fn run_outcome_carries_md_path_and_failure_flag() {
        // Documents the public shape `run` returns on success: a markdown
        // path plus a boolean indicating whether any fixture failed. The
        // `flame_report` binary pivots its exit code on `any_failed`, so
        // both fields are part of the public contract.
        let outcome = RunOutcome {
            md_path: PathBuf::from("/tmp/report.md"),
            any_failed: true,
        };
        assert_eq!(outcome.md_path, PathBuf::from("/tmp/report.md"));
        assert!(outcome.any_failed);
        // Clone is part of the contract so callers can pass the outcome by
        // value while still keeping a copy. Debug must round-trip the field
        // names so logs are intelligible on partial failure.
        let cloned = outcome.clone();
        assert_eq!(cloned.md_path, outcome.md_path);
        assert!(format!("{cloned:?}").contains("any_failed"));
    }

    #[test]
    fn parse_args_reads_flamegraph_dir_flag() {
        // CLI flag wins: when --flamegraph-dir is supplied, that value is
        // used regardless of the FLAMEGRAPH_DIR environment variable. The
        // env lookup closure here returns Some to prove the CLI flag takes
        // precedence — if env-var fallback fired, the assertion would see
        // "/from/env" instead of the CLI value.
        let argv = vec![
            "x.sql".into(),
            "--flamegraph-dir".into(),
            "/cli/path".into(),
        ];
        let env = |_: &str| Some(std::ffi::OsString::from("/from/env"));
        let a = Args::parse_with_env_lookup(&argv, env).unwrap();
        assert_eq!(
            a.flamegraph_dir.as_deref(),
            Some(std::path::Path::new("/cli/path"))
        );
    }

    #[test]
    fn parse_args_falls_back_to_flamegraph_dir_env_var() {
        // No --flamegraph-dir on the CLI: env-var fallback fires.
        let argv = vec!["x.sql".into()];
        let env = |k: &str| {
            if k == "FLAMEGRAPH_DIR" {
                Some(std::ffi::OsString::from("/from/env"))
            } else {
                None
            }
        };
        let a = Args::parse_with_env_lookup(&argv, env).unwrap();
        assert_eq!(
            a.flamegraph_dir.as_deref(),
            Some(std::path::Path::new("/from/env"))
        );
    }

    #[test]
    fn parse_args_flamegraph_dir_none_when_unset() {
        // Neither CLI flag nor env var: the field is None. On macOS the
        // orchestrator turns this into a startup error; on Linux it is
        // silently ignored — but Args::parse never errors over it.
        let argv = vec!["x.sql".into()];
        let env = |_: &str| None;
        let a = Args::parse_with_env_lookup(&argv, env).unwrap();
        assert_eq!(a.flamegraph_dir, None);
    }

    /// Smoke test that the public `Args::parse` entry point round-trips
    /// through `parse_with_env_lookup` without panicking. The other tests
    /// all hit `parse_with_env_lookup` directly to control the env;
    /// without this test, a refactor that drops the closure-passing in
    /// `Args::parse` would silently break the binary.
    #[test]
    fn public_parse_compiles_and_runs() {
        let argv = vec!["x.sql".into()];
        let a = Args::parse(&argv).expect("public Args::parse should succeed");
        assert_eq!(a.fixtures.len(), 1);
        assert_eq!(a.duration_secs, 5);
        // We make no assertion on `flamegraph_dir` because the test
        // process may have FLAMEGRAPH_DIR set in its real environment;
        // the goal here is just to prove the public path doesn't panic.
    }
}
