//! Host probe: platform-gated capture of hardware/OS metadata used in the
//! flamegraph diagnostic report. Individual probes that fail fall back to
//! `"unknown"` (or `None` for optional fields) rather than aborting — the
//! diagnostic pipeline must keep running even if one shell-out fails.

use std::process::Command;

#[derive(Debug, Clone)]
pub struct Host {
    pub model: String,
    pub chip: Option<String>,
    pub cores: String,
    pub memory: String,
    pub os: String,
    pub arch: String,
    pub power_source: Option<String>,
    pub rustc: String,
    pub cargo_flamegraph: String,
}

impl Host {
    /// Probe the current host. Platform-gated: macOS and Linux each pull from
    /// their canonical sources; unknown platforms produce an all-`unknown`
    /// struct (minus `arch`, which is always available from `std::env::consts`).
    pub fn probe() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::probe_macos()
        }
        #[cfg(target_os = "linux")]
        {
            Self::probe_linux()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Self::unknown()
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn unknown() -> Self {
        Host {
            model: "unknown".into(),
            chip: None,
            cores: "unknown".into(),
            memory: "unknown".into(),
            os: "unknown".into(),
            arch: std::env::consts::ARCH.into(),
            power_source: None,
            rustc: rustc_version().unwrap_or_else(|| "unknown".into()),
            cargo_flamegraph: cargo_flamegraph_version().unwrap_or_else(|| "unknown".into()),
        }
    }

    #[cfg(target_os = "macos")]
    fn probe_macos() -> Self {
        let model = sysctl("hw.model").unwrap_or_else(|| "unknown".into());
        let chip = sysctl("machdep.cpu.brand_string");
        // `sysctl -n hw.ncpu` returns just the total core count (e.g. "10").
        // The plan accepts that minimal shape; a P+E breakdown would require
        // additional probes (`hw.perflevel0.physicalcpu` etc.) and is out of
        // scope for this task.
        let cores = sysctl("hw.ncpu").unwrap_or_else(|| "unknown".into());
        let memory = sysctl("hw.memsize")
            .and_then(|b| b.parse::<u64>().ok())
            .map(|b| format!("{} GB", b / (1024 * 1024 * 1024)))
            .unwrap_or_else(|| "unknown".into());
        let os_ver = run("sw_vers", &["-productVersion"]).unwrap_or_else(|| "unknown".into());
        let kernel = run("uname", &["-r"]).unwrap_or_else(|| "?".into());
        let os = format!("macOS {os_ver} (Darwin {kernel})");
        let power_source = macos_power_source();
        Host {
            model,
            chip,
            cores,
            memory,
            os,
            arch: std::env::consts::ARCH.into(),
            power_source,
            rustc: rustc_version().unwrap_or_else(|| "unknown".into()),
            cargo_flamegraph: cargo_flamegraph_version().unwrap_or_else(|| "unknown".into()),
        }
    }

    #[cfg(target_os = "linux")]
    fn probe_linux() -> Self {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        let model = cpuinfo
            .lines()
            .find_map(|l| {
                l.strip_prefix("model name")
                    .and_then(|s| s.split(':').nth(1))
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        let cores = num_cpus_from_cpuinfo(&cpuinfo).unwrap_or_else(|| "unknown".into());
        let meminfo = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let memory = memory_from_meminfo(&meminfo).unwrap_or_else(|| "unknown".into());
        let os = linux_os_release().unwrap_or_else(|| "Linux".into());
        Host {
            model,
            chip: None,
            cores,
            memory,
            os,
            arch: std::env::consts::ARCH.into(),
            power_source: None,
            rustc: rustc_version().unwrap_or_else(|| "unknown".into()),
            cargo_flamegraph: cargo_flamegraph_version().unwrap_or_else(|| "unknown".into()),
        }
    }

    /// Render as a GitHub-flavoured markdown table. Optional fields (`chip`,
    /// `power_source`) are omitted entirely when `None` — no empty rows.
    pub fn render_markdown(&self) -> String {
        let mut s = String::from("## Host\n\n| Field | Value |\n|---|---|\n");
        s.push_str(&format!("| Model | {} |\n", self.model));
        if let Some(chip) = &self.chip {
            s.push_str(&format!("| Chip | {chip} |\n"));
        }
        s.push_str(&format!("| Cores | {} |\n", self.cores));
        s.push_str(&format!("| Memory | {} |\n", self.memory));
        s.push_str(&format!("| OS | {} |\n", self.os));
        s.push_str(&format!("| Architecture | {} |\n", self.arch));
        if let Some(ps) = &self.power_source {
            s.push_str(&format!("| Power source | {ps} |\n"));
        }
        s.push_str(&format!("| rustc | {} |\n", self.rustc));
        s.push_str(&format!(
            "| cargo-flamegraph | {} |\n",
            self.cargo_flamegraph
        ));
        s
    }
}

#[cfg(target_os = "macos")]
fn sysctl(key: &str) -> Option<String> {
    run("sysctl", &["-n", key])
}

/// Shell out and capture trimmed stdout on success. Returns `None` for any
/// failure mode (spawn error, non-zero exit, non-UTF8 output) — callers treat
/// every failure as "probe unavailable" and fall back to `"unknown"`.
fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    Some(s.trim().to_string())
}

#[cfg(target_os = "macos")]
fn macos_power_source() -> Option<String> {
    // `pmset -g batt` first line looks like:
    //   "Now drawing from 'AC Power'"      (plugged in)
    //   "Now drawing from 'Battery Power'" (unplugged)
    // NOTE: brittle to non-English locales; acceptable for English-only dev
    // machines. Revisit if we start running on localized systems.
    let out = run("pmset", &["-g", "batt"])?;
    if out.contains("'AC Power'") {
        Some("AC".into())
    } else if out.contains("'Battery Power'") {
        Some("Battery".into())
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn num_cpus_from_cpuinfo(cpuinfo: &str) -> Option<String> {
    let count = cpuinfo
        .lines()
        .filter(|l| l.starts_with("processor"))
        .count();
    if count == 0 {
        None
    } else {
        Some(count.to_string())
    }
}

#[cfg(target_os = "linux")]
fn memory_from_meminfo(meminfo: &str) -> Option<String> {
    let line = meminfo.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(format!("{} GB", kb / (1024 * 1024)))
}

#[cfg(target_os = "linux")]
fn linux_os_release() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    let pretty = text
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))?
        .trim_matches('"')
        .to_string();
    let kernel = run("uname", &["-r"]).unwrap_or_else(|| "?".into());
    Some(format!("{pretty} (Linux {kernel})"))
}

fn rustc_version() -> Option<String> {
    run("rustc", &["--version"]).map(|s| s.trim_start_matches("rustc ").to_string())
}

fn cargo_flamegraph_version() -> Option<String> {
    // `cargo flamegraph --version` prints e.g. "cargo-flamegraph 0.6.5".
    run("cargo", &["flamegraph", "--version"])
        .map(|s| s.trim_start_matches("cargo-flamegraph ").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_non_default_on_real_system() {
        // Smoke test: probing the real host should populate *something*.
        let h = Host::probe();
        // We can't assert exact values (varies by machine), but the probe
        // should not return an all-unknown struct on a supported OS.
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        assert_ne!(
            h.cores, "unknown",
            "cores should be detectable on macOS/Linux, got {h:?}"
        );
    }

    #[test]
    fn render_table_has_expected_fields() {
        let h = Host {
            model: "Test Model".into(),
            chip: Some("Apple Test".into()),
            cores: "10".into(),
            memory: "64 GB".into(),
            os: "macOS 14.5 (Darwin 24.5.0)".into(),
            arch: "arm64".into(),
            power_source: Some("AC".into()),
            rustc: "1.83.0 (stable)".into(),
            cargo_flamegraph: "0.6.5".into(),
        };
        let md = h.render_markdown();
        assert!(md.contains("| Model | Test Model |"));
        assert!(md.contains("| Chip | Apple Test |"));
        assert!(md.contains("| Memory | 64 GB |"));
        assert!(md.contains("| Power source | AC |"));
    }

    #[test]
    fn render_omits_none_only_fields() {
        // Linux has no "chip" or "power source" — they should not render.
        let h = Host {
            model: "Intel".into(),
            chip: None,
            cores: "20".into(),
            memory: "32 GB".into(),
            os: "Linux".into(),
            arch: "x86_64".into(),
            power_source: None,
            rustc: "1.83.0".into(),
            cargo_flamegraph: "0.6.5".into(),
        };
        let md = h.render_markdown();
        assert!(!md.contains("Chip"));
        assert!(!md.contains("Power source"));
    }
}
