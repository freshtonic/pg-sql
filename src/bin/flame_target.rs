//! Profilee binary for `cargo flamegraph`. Parses one SQL fixture in a loop
//! for a fixed duration. Deliberately minimal so the flamegraph shows only
//! parser frames.
//!
//! Usage: `flame_target <fixture-path> --duration <seconds>`

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use pg_sql::flame::run_loop;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, duration_secs) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("flame_target: {msg}");
            eprintln!("usage: flame_target <fixture.sql> --duration <seconds>");
            return ExitCode::from(2);
        }
    };
    match run_loop(&path, Duration::from_secs(duration_secs)) {
        Ok((iters, elapsed)) => {
            if iters == 0 {
                eprintln!("flame_target: 0 iterations completed (likely fixture error)");
                return ExitCode::from(1);
            }
            println!("iters={iters} elapsed_ns={}", elapsed.as_nanos());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("flame_target: {e}");
            ExitCode::from(1)
        }
    }
}

fn parse_args(args: &[String]) -> Result<(PathBuf, u64), String> {
    let mut path: Option<PathBuf> = None;
    let mut duration: u64 = 5;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--duration" => {
                i += 1;
                let v = args.get(i).ok_or("--duration needs a value")?;
                duration = v.parse().map_err(|_| format!("bad --duration: {v}"))?;
            }
            s if s.starts_with("--") => return Err(format!("unknown flag: {s}")),
            s => {
                if path.is_some() {
                    return Err("expected exactly one fixture path".into());
                }
                path = Some(PathBuf::from(s));
            }
        }
        i += 1;
    }
    let path = path.ok_or("missing fixture path")?;
    Ok((path, duration))
}
