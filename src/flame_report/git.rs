//! Capture working-copy git state — commit SHA, branch, and dirty flag — for
//! tagging flamegraph runs. Used by the orchestrator to name run directories
//! and stamp markdown headers.

use std::io;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitState {
    pub full_sha: String,
    pub short_sha: String,
    pub branch: String,
    pub dirty: bool,
}

impl GitState {
    /// Capture the current working-copy state by shelling out to `git`.
    /// Errors if `git` is missing, this is not a repo, or any required command
    /// fails. The branch lookup falls back to `"HEAD"` for detached-head state.
    pub fn capture() -> io::Result<Self> {
        let full_sha = git(&["rev-parse", "HEAD"])?;
        let short_sha = git(&["rev-parse", "--short=7", "HEAD"])?;
        let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|_| "HEAD".into());
        let status = git(&["status", "--porcelain"])?;
        let dirty = !status.trim().is_empty();
        Ok(GitState {
            full_sha,
            short_sha,
            branch,
            dirty,
        })
    }

    /// Filesystem-safe stem for naming the run directory and report file:
    /// `<short>-<timestamp>` plus a `-dirty` suffix when the working tree is
    /// dirty.
    pub fn run_stem(&self, timestamp: &str) -> String {
        if self.dirty {
            format!("{}-{}-dirty", self.short_sha, timestamp)
        } else {
            format!("{}-{}", self.short_sha, timestamp)
        }
    }
}

fn git(args: &[&str]) -> io::Result<String> {
    let out = Command::new("git").args(args).output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(io::Error::other(format!("git {}: {}", args.join(" "), err)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_succeeds_in_repo() {
        let s = GitState::capture().expect("should capture in a git repo");
        assert_eq!(
            s.short_sha.len(),
            7,
            "short sha should be 7 chars, got {:?}",
            s.short_sha
        );
        assert!(!s.full_sha.is_empty());
        assert!(!s.branch.is_empty());
    }

    #[test]
    fn short_sha_is_prefix_of_full_sha() {
        let s = GitState::capture().unwrap();
        assert!(s.full_sha.starts_with(&s.short_sha));
    }
}
