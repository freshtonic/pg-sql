//! PostgreSQL grammar backed by Recursa's sole LR parser.

recursa::grammar! {
    module = crate,
    derives(Pretty, Visit, VisitMut),
    keyword_matching = ascii_insensitive,
    diagrams,
    framing(island = ast::file::SqlDocumentItem, boundary = SEMI),
}

pub mod ast;
pub mod bench_data;
pub mod document;
pub mod formatter;
pub mod tokens;

pub use document::{SqlDocument, SqlParseError, SqlRejection};
