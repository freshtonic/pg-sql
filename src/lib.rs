recursa::grammar! {
    module = crate,
    keyword_matching = ascii_insensitive,
    max_lookahead = 5,
}

pub mod ast;
pub mod bench_data;
pub mod formatter;
pub mod tokens;

#[cfg(feature = "arbitrary")]
pub use arbitrary;
