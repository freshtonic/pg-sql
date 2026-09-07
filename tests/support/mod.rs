//! Shared helpers for the differential parser test.

// Each integration target compiles this shared baseline module as a separate
// crate and intentionally consumes a different subset of its query API.
#[allow(dead_code)]
pub mod baseline;
pub mod diff_check;

/// One SQL statement extracted from a corpus file.
///
/// `source` is the verbatim original slice of the corpus file — never
/// pg-sql's reformatted text. The differential test reformats it itself and
/// compares against this original, so corrupting it here corrupts every
/// downstream comparison.
pub struct Stmt {
    pub source: String,
}
