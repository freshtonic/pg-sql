recursa::grammar! {
    module = crate,
    derives(Pretty, Visit, VisitMut),
    keyword_matching = ascii_insensitive,
    max_lookahead = 5,
    parser_style = table_driven,
    diagrams,
    framing(island = ast::file::SqlDocumentItem, boundary = SEMI),
}

pub mod ast;
pub mod bench_data;
pub mod document;
pub mod formatter;
pub mod tokens;

pub use document::{SqlDocument, SqlParseError, SqlRejection};
