//! PostgreSQL grammar backed by Recursa's sole LR parser.
//!
//! The `spans` Cargo feature retains occurrence provenance during ordinary
//! parsing. Generated `parse_without_spans` entry points always provide
//! semantic values without that provenance cost. Without the feature,
//! ordinary parsing also skips provenance and the exact-source document
//! interface is unavailable.

recursa::grammar! {
    module = crate,
    arena_ast,
    derives(Pretty, Visit, VisitMut),
    keyword_matching = ascii_insensitive,
    diagrams,
    framing(island = ast::file::SqlDocumentItem, boundary = SEMI),
}

pub mod ast;
pub mod bench_data;
#[cfg(feature = "spans")]
pub mod document;
pub mod formatter;
pub mod tokens;

#[cfg(feature = "spans")]
pub use document::{SqlDocument, SqlParseError, SqlRejection};
