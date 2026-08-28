//! Orchestrator-side logic for the flame_report binary. Exposed as a library
//! module so each component (host probe, git state, markdown rendering,
//! orchestration) is unit-testable without going through the binary.

pub mod git;
pub mod host;
pub mod orchestrator;
pub mod profiler;
pub mod render;
