//! FFI bindings to PostgreSQL 17.9's raw parser, linked as a static lib.
//!
//! See docs/plans/2026-05-21-differential-parser-testing-design.md.

mod parser;

pub use parser::{node_to_string, parse_equal, parse_ok, Equal};
