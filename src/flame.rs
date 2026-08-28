//! Shared parse loop used by `flame_target` (under profiler) and
//! `flame_report`'s in-process timing pass. Keep these two callers using the
//! exact same code so timing numbers correspond to what the profiler samples.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use recursa::Input;

use crate::ast::parse_sql_file;

/// Loop `parse_sql_file` on `path` until at least `duration` has elapsed.
/// Returns `(iterations, elapsed)`. Reads the file once, outside the loop so
/// I/O doesn't dominate the flamegraph.
///
/// Returns `io::Error` on file read failure.
pub fn run_loop(path: &Path, duration: Duration) -> io::Result<(u64, Duration)> {
    let sql = std::fs::read_to_string(path)?;
    let start = Instant::now();
    let deadline = start + duration;
    let mut iters: u64 = 0;
    while Instant::now() < deadline {
        let lexed = crate::tokens::pg_lex(&sql);
        let mut input = Input::new(&sql, &lexed);
        let result = parse_sql_file(&mut input);
        let _ = std::hint::black_box(result);
        iters += 1;
    }
    Ok((iters, start.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn run_loop_parses_simple_select() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "SELECT 1;").unwrap();
        let (iters, elapsed) = run_loop(tmp.path(), Duration::from_millis(50)).unwrap();
        assert!(iters > 0, "expected at least one iteration");
        assert!(elapsed >= Duration::from_millis(50));
    }

    #[test]
    fn run_loop_fails_on_missing_file() {
        let err = run_loop(
            std::path::Path::new("/does/not/exist.sql"),
            Duration::from_millis(10),
        );
        assert!(err.is_err());
    }

    #[test]
    fn run_loop_accepts_empty_input() {
        // `parse_sql_file` is tolerant and returns an empty Vec for empty input,
        // so `run_loop` treats it as a successful (zero-work) fixture. The
        // orchestrator handles the "zero-work" case separately via iteration
        // counts if needed.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        let result = run_loop(tmp.path(), Duration::from_millis(10));
        assert!(result.is_ok());
    }
}
