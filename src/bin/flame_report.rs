//! Orchestrator binary for flamegraph diagnostic runs. All real work lives in
//! `pg_sql::flame_report::orchestrator`; this wrapper only parses argv and
//! maps results to exit codes.
//!
//! See `docs/plans/2026-04-24-flamegraph-diagnostic-design.md`.
//!
//! Usage: `flame_report <fixture>... [--duration N] [--out DIR]`

use std::process::ExitCode;

use pg_sql::flame_report::orchestrator::{Args, run};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match Args::parse(&argv) {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("flame_report: {msg}");
            eprintln!("usage: flame_report <fixture>... [--duration N] [--out DIR]");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(outcome) => {
            // Always print the markdown path so callers (CI, scripts, humans)
            // can locate the report regardless of whether fixtures failed.
            println!("{}", outcome.md_path.display());
            // Per the design contract, exit non-zero when any fixture failed
            // so partial failures are detectable in CI.
            if outcome.any_failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(msg) => {
            eprintln!("flame_report: {msg}");
            ExitCode::from(1)
        }
    }
}
