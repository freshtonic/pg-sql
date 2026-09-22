//! The target version of this build (ADR 0009).
//!
//! A pg-sql build reproduces the raw parser of exactly one PostgreSQL major
//! version. The Cargo version feature selects it at build time: `pg14`,
//! `pg15`, `pg16`, `pg17` (the default), `pg18` or `pg19-beta`. There is no
//! run-time version selection; [`crate::TARGET_VERSION`] only reports the
//! choice.

/// A PostgreSQL major version that a pg-sql build can target.
///
/// Each target version is pinned to one exact release: the latest minor
/// release of that major, or a named commit for a pre-release major.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetVersion {
    /// PostgreSQL 14 (feature `pg14`).
    Pg14,
    /// PostgreSQL 15 (feature `pg15`).
    Pg15,
    /// PostgreSQL 16 (feature `pg16`).
    Pg16,
    /// PostgreSQL 17 (feature `pg17`, the default).
    Pg17,
    /// PostgreSQL 18 (feature `pg18`).
    Pg18,
    /// PostgreSQL 19 before its first release (feature `pg19-beta`). It
    /// becomes `Pg19` when PostgreSQL 19.0 is tagged.
    Pg19Beta,
}

impl TargetVersion {
    /// The PostgreSQL major version number, for example `17`.
    pub const fn major(self) -> u32 {
        match self {
            Self::Pg14 => 14,
            Self::Pg15 => 15,
            Self::Pg16 => 16,
            Self::Pg17 => 17,
            Self::Pg18 => 18,
            Self::Pg19Beta => 19,
        }
    }

    /// The Cargo version feature that selects this target version, for
    /// example `"pg17"`.
    pub const fn feature(self) -> &'static str {
        match self {
            Self::Pg14 => "pg14",
            Self::Pg15 => "pg15",
            Self::Pg16 => "pg16",
            Self::Pg17 => "pg17",
            Self::Pg18 => "pg18",
            Self::Pg19Beta => "pg19-beta",
        }
    }
}
