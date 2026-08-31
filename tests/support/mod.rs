//! Shared helpers for the differential parser test.

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
