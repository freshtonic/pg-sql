//! Data Manipulation Language statements: SELECT, INSERT, UPDATE, DELETE, MERGE, VALUES.

pub mod delete;
pub mod insert;
// Added in 15: research, PostgreSQL 15, "New statements" (REL_15_19 gram.y
// `MergeStmt`). REL_14_24 gram.y has no MERGE.
#[cfg(feature = "since-pg15")]
pub mod merge;
pub mod select;
pub mod update;
pub mod values;
