//! FFI bindings to the raw parser of the target version's pinned PostgreSQL
//! release, linked as a static lib.
//!
//! The version feature (`pg14` ... `pg19-beta`, default `pg17`) selects the
//! release from `pins.tsv` (ADR 0009). See
//! docs/plans/2026-05-21-differential-parser-testing-design.md.

mod parser;

pub use parser::{node_to_string, parse_equal, parse_ok, Equal};

/// The Git ref (a release tag, or a branch for a pre-release major) of the
/// PostgreSQL release that this oracle is built from.
pub const POSTGRES_REF: &str = env!("PG_ORACLE_POSTGRES_REF");

/// The Git commit of the PostgreSQL release that this oracle is built from.
pub const POSTGRES_COMMIT: &str = env!("PG_ORACLE_POSTGRES_COMMIT");
