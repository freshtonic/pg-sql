// Shared test helpers for the relocated embedded test bodies.

use recursa::{Parse, Pretty, PrettyConfig};

use crate::formatter::format_tokens_sql;

/// Parses one complete statement as `T` through the generated public seam.
pub fn parse_stmt<T>(src: &'static str) -> T
where
    T: Parse<crate::Input<'static>, Output = recursa::Parsed<'static, T>>,
{
    let lexed = crate::lex(src);
    assert_eq!(lexed.errors().count(), 0, "lex errors in input");
    let mut input = lexed.input();
    let stmt = T::parse(&mut input).unwrap().into_ast();
    assert!(input.is_eof(), "unconsumed input after parsing {src:?}");
    stmt
}

/// Asserts that generated parsing and formatting reach a fixed point.
pub fn reparse_stable<T>(src: &'static str)
where
    T: Parse<crate::Input<'static>, Output = recursa::Parsed<'static, T>> + Pretty,
{
    let lexed = crate::lex(src);
    assert_eq!(lexed.errors().count(), 0, "lex errors in input");
    let mut input = lexed.input();
    let stmt = T::parse(&mut input).unwrap().into_ast();
    assert!(input.is_eof(), "unconsumed input after parsing {src:?}");
    let once = format_tokens_sql(&stmt, PrettyConfig::default());
    let leaked: &'static str = Box::leak(once.clone().into_boxed_str());
    let relexed = crate::lex(leaked);
    assert_eq!(relexed.errors().count(), 0, "lex errors in reinput");
    let mut reinput = relexed.input();
    let restmt = T::parse(&mut reinput).unwrap().into_ast();
    assert!(
        reinput.is_eof(),
        "unconsumed input after re-parsing {once:?}"
    );
    let twice = format_tokens_sql(&restmt, PrettyConfig::default());
    assert_eq!(once, twice, "formatter output is not a fixed point");
}

/// Parses and formats one complete generated statement.
pub fn roundtrip<T>(src: &'static str) -> String
where
    T: Parse<crate::Input<'static>, Output = recursa::Parsed<'static, T>> + Pretty,
{
    let lexed = crate::lex(src);
    assert_eq!(lexed.errors().count(), 0, "lex errors in input");
    let mut input = lexed.input();
    let stmt = T::parse(&mut input).unwrap().into_ast();
    assert!(input.is_eof(), "unconsumed input after parsing {src:?}");
    format_tokens_sql(&stmt, PrettyConfig::default())
}
