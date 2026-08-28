//! Profile-pass shell-out: produce a single flamegraph SVG for one fixture.
//!
//! Two implementations sit behind `cfg`:
//!
//! - **macOS:** `cargo-flamegraph` is broken on Apple Silicon because the
//!   inferno collapser cannot parse Instruments' xctrace XML. We instead run
//!   a 4-step pipeline: `cargo instruments` to record a `.trace` bundle,
//!   `xcrun xctrace export` to extract the time-profile XML, the FlameGraph
//!   project's `stackcollapse-instruments.pl` to fold the stacks, and finally
//!   `flamegraph.pl` to render the SVG. The intermediate `.xml` and `.folded`
//!   files plus the `.trace` bundle are deleted on success — only the SVG is
//!   retained. (A future enhancement could expose a `--no-cleanup` flag for
//!   power users who need to inspect the intermediate artefacts.)
//!
//! - **Linux:** the existing `cargo flamegraph` invocation works fine
//!   (perf-based) so we keep it untouched.
//!
//! Both paths inherit stdio so any sudo / Instruments authorisation prompts
//! reach the user's terminal directly.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

/// Render an `ExitStatus` as a short string for error messages. Uses the
/// numeric exit code when present, or `"signal"` when the child was
/// killed by a signal. Shared across the four macOS pipeline steps so
/// their error messages stay consistent.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn exit_code(s: &ExitStatus) -> String {
    s.code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".into())
}

/// Per-call context carrying the platform-specific knobs the profiler needs.
/// `flamegraph_dir` is required on macOS (location of the `FlameGraph`
/// checkout) and ignored on Linux. `run_dir` is where intermediate artefacts
/// (the `.trace` bundle, `.xml`, `.folded`) are written before being cleaned
/// up.
#[derive(Debug, Clone)]
pub struct ProfileCtx {
    pub flamegraph_dir: Option<PathBuf>,
    pub run_dir: PathBuf,
}

/// Profile a single fixture. Spawns one or more child processes (depending
/// on the OS) with stdio inherited so authorisation prompts reach the user.
/// On success the SVG lands at `svg_out`; on failure returns a short error
/// string identifying which step failed.
pub fn profile(
    fixture: &Path,
    duration_secs: u64,
    svg_out: &Path,
    ctx: &ProfileCtx,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        profile_macos(fixture, duration_secs, svg_out, ctx)
    }
    #[cfg(target_os = "linux")]
    {
        let _ = ctx; // flamegraph_dir is unused on Linux.
        profile_linux(fixture, duration_secs, svg_out)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (fixture, duration_secs, svg_out, ctx);
        Err("profile pass not implemented for this OS (only macOS and Linux are supported)".into())
    }
}

/// macOS pipeline: `cargo instruments` → `xctrace export` → `stackcollapse` →
/// `flamegraph.pl`. See module docs for why this is needed.
#[cfg(target_os = "macos")]
fn profile_macos(
    fixture: &Path,
    duration_secs: u64,
    svg_out: &Path,
    ctx: &ProfileCtx,
) -> Result<(), String> {
    // SVG basename without extension is the stem we use for the .trace,
    // .xml, and .folded artefacts. Keeping them all together in run_dir
    // makes cleanup one rmdir away.
    let stem = svg_out
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("invalid svg output path: {}", svg_out.display()))?;
    let trace_path = ctx.run_dir.join(format!("{stem}.trace"));
    let xml_path = ctx.run_dir.join(format!("{stem}.xml"));
    let folded_path = ctx.run_dir.join(format!("{stem}.folded"));

    let flamegraph_dir = ctx
        .flamegraph_dir
        .as_ref()
        .ok_or("flamegraph_dir is required on macOS (use --flamegraph-dir or FLAMEGRAPH_DIR)")?;
    let stackcollapse = flamegraph_dir.join("stackcollapse-instruments.pl");
    let flamegraph_pl = flamegraph_dir.join("flamegraph.pl");

    // Step 1: record. cargo-instruments wraps `xcrun instruments` and
    // produces a .trace bundle (which is actually a directory).
    //
    // -t time          => time-profile template (sampling profiler)
    // -p pg-sql        => qualify the workspace package
    // --bin flame_target --release => same binary the Linux path profiles
    // --no-open        => don't pop up Instruments.app on completion
    // -o <trace>       => write the trace bundle here instead of the cwd
    // -- <fixture> --duration <n> => args forwarded to flame_target
    let status = Command::new("cargo")
        .args([
            "instruments",
            "-t",
            "time",
            "-p",
            "pg-sql",
            "--bin",
            "flame_target",
            "--release",
            "--no-open",
            "-o",
        ])
        .arg(&trace_path)
        .arg("--")
        .arg(fixture)
        .arg("--duration")
        .arg(duration_secs.to_string())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("cargo-instruments spawn failed: {e}"))?;
    if !status.success() {
        return Err(format!(
            "cargo-instruments exited {} (record step)",
            exit_code(&status)
        ));
    }

    // Step 2: export the time-profile table to XML. xctrace ships with the
    // Xcode Command Line Tools.
    let xml_file = std::fs::File::create(&xml_path)
        .map_err(|e| format!("create {}: {e}", xml_path.display()))?;
    let status = Command::new("xcrun")
        .args(["xctrace", "export", "--input"])
        .arg(&trace_path)
        .args([
            "--xpath",
            r#"/trace-toc/run/data/table[@schema="time-profile"]"#,
        ])
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(xml_file))
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("xctrace export spawn failed: {e}"))?;
    if !status.success() {
        return Err(format!("xctrace export exited {}", exit_code(&status)));
    }

    // Step 3: fold stacks via stackcollapse-instruments.pl. The script reads
    // XML on stdin (or from a positional file arg, which is what we use).
    let folded_file = std::fs::File::create(&folded_path)
        .map_err(|e| format!("create {}: {e}", folded_path.display()))?;
    let status = Command::new(&stackcollapse)
        .arg(&xml_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(folded_file))
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("stackcollapse-instruments.pl spawn failed: {e}"))?;
    if !status.success() {
        return Err(format!(
            "stackcollapse-instruments.pl exited {}",
            exit_code(&status)
        ));
    }

    // Step 4: render SVG.
    let svg_file =
        std::fs::File::create(svg_out).map_err(|e| format!("create {}: {e}", svg_out.display()))?;
    let status = Command::new(&flamegraph_pl)
        .arg(&folded_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(svg_file))
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("flamegraph.pl spawn failed: {e}"))?;
    if !status.success() {
        return Err(format!("flamegraph.pl exited {}", exit_code(&status)));
    }

    // Cleanup intermediate artefacts. Failures here are non-fatal: the SVG
    // (the actual deliverable) is already on disk. .trace is a bundle
    // (directory), the others are plain files.
    let _ = std::fs::remove_file(&xml_path);
    let _ = std::fs::remove_file(&folded_path);
    let _ = std::fs::remove_dir_all(&trace_path);

    Ok(())
}

/// Linux pipeline — unchanged from before this module existed: `cargo
/// flamegraph` does the right thing on perf-based systems.
#[cfg(target_os = "linux")]
fn profile_linux(fixture: &Path, duration_secs: u64, svg_out: &Path) -> Result<(), String> {
    // CARGO_PROFILE_RELEASE_DEBUG=true forces debug symbols into the
    // release build so the flamegraph frames are symbolised. Without it,
    // cargo flamegraph emits a warning and produces an unsymbolised graph.
    let status = Command::new("cargo")
        .args([
            "flamegraph",
            "--release",
            "-p",
            "pg-sql",
            "--bin",
            "flame_target",
            "-o",
        ])
        .arg(svg_out)
        .arg("--")
        .arg(fixture)
        .arg("--duration")
        .arg(duration_secs.to_string())
        .env("CARGO_PROFILE_RELEASE_DEBUG", "true")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("cargo-flamegraph spawn failed: {e}"))?;
    if !status.success() {
        return Err(format!("cargo-flamegraph exited {}", exit_code(&status)));
    }
    if !svg_out.is_file() {
        return Err(format!(
            "cargo-flamegraph claimed success but no SVG at {}",
            svg_out.display()
        ));
    }
    Ok(())
}
