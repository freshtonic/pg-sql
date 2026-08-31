recursa::grammar! {
    module = crate,
    derives(Pretty, Visit, VisitMut),
    keyword_matching = ascii_insensitive,
    max_lookahead = 5,
    diagrams,
}

pub mod ast;
pub mod bench_data;
pub mod formatter;
pub mod tokens;
