//! Identifier folding (`pg_sql::ident`), rule by rule.
//!
//! The rules are PostgreSQL 17.11's, read from the vendored scanner:
//! `scan.l` (`{identifier}`, `<xd>{xdstop}`, `<xui>{dquote}`), `scansup.c`
//! (`downcase_identifier`, `truncate_identifier`) and `parser.c`
//! (`base_yylex` `case UIDENT`, `str_udeescape`, `check_uescapechar`). They do
//! not exercise the grammar, so they live here and not under
//! `embedded-tests/`.

use std::borrow::Cow;

use pg_sql::ident::{
    FoldError, Folded, NAMEDATALEN, fold_identifier, fold_identifier_with_uescape,
};

fn folded(raw: &str) -> Folded<'_> {
    fold_identifier(raw).unwrap_or_else(|error| panic!("{raw}: {error}"))
}

#[test]
fn folds_each_identifier_form() {
    // (source text, folded name, borrowed from the source)
    let cases = [
        // `downcase_identifier`: ASCII `A`-`Z` only.
        ("foo", "foo", true),
        ("Foo", "foo", false),
        ("FOO_Bar9", "foo_bar9", false),
        ("_x", "_x", true),
        // A multibyte encoding leaves every byte with the high bit set alone.
        ("Ärger", "Ärger", true),
        ("ÄRGER", "Ärger", false),
        ("straße", "straße", true),
        // A keyword in an identifier position: its lowercase spelling.
        ("VALUE", "value", false),
        ("Coalesce", "coalesce", false),
        ("value", "value", true),
        // `<xd>`: strip the quotes, `""` is `"`, no downcasing.
        ("\"Foo\"", "Foo", true),
        ("\"foo bar\"", "foo bar", true),
        ("\"Foo\"\"Bar\"", "Foo\"Bar", false),
        ("\"\"\"\"", "\"", false),
        ("\"SELECT\"", "SELECT", true),
        ("\"Ärger\"", "Ärger", true),
        // A backslash means nothing in a plain quoted identifier.
        ("\"d\\0061t\"", "d\\0061t", true),
        // `<xui>` and `str_udeescape`.
        ("U&\"d\\0061t\\+000061\"", "data", false),
        ("u&\"d\\0061t\\+000061\"", "data", false),
        ("U&\"Plain\"", "Plain", true),
        ("U&\"a\\\\b\"", "a\\b", false),
        ("U&\"\\00E4rger\"", "ärger", false),
        ("U&\"\\00e4\"", "ä", false),
        ("U&\"a\"\"\\0062\"", "a\"b", false),
        // A UTF-16 surrogate pair, in each spelling: U+1F600.
        ("U&\"\\D83D\\DE00\"", "\u{1F600}", false),
        ("U&\"\\+00D83D\\+00DE00\"", "\u{1F600}", false),
        ("U&\"\\D83D\\+00DE00\"", "\u{1F600}", false),
        ("U&\"\\+01F600\"", "\u{1F600}", false),
    ];
    for (raw, name, borrowed) in cases {
        let folded = folded(raw);
        assert_eq!(folded.name, name, "{raw}");
        assert!(!folded.truncated, "{raw}");
        assert_eq!(
            matches!(folded.name, Cow::Borrowed(_)),
            borrowed,
            "{raw}: borrowed"
        );
    }
}

#[test]
fn applies_a_uescape_character() {
    let cases = [
        ("U&\"d!0061t!+000061\"", "!", "data"),
        ("U&\"d!!t\"", "!", "d!t"),
        // Once another character is the escape, a backslash is text.
        ("U&\"d\\0061t\"", "!", "d\\0061t"),
        ("U&\"d#0061t\"", "#", "dat"),
        // The clause has no effect on the other forms.
        ("\"d!0061t\"", "!", "d!0061t"),
        ("Foo", "!", "foo"),
    ];
    for (raw, uescape, name) in cases {
        let folded = fold_identifier_with_uescape(raw, uescape)
            .unwrap_or_else(|error| panic!("{raw} UESCAPE '{uescape}': {error}"));
        assert_eq!(folded.name, name, "{raw} UESCAPE '{uescape}'");
    }
}

#[test]
fn rejects_what_check_uescapechar_rejects() {
    for uescape in [
        "", "ab", "0", "9", "a", "F", "+", "'", "''", "\"", " ", "\t", "\n", "\r", "\u{b}",
        "\u{c}", "ä",
    ] {
        assert_eq!(
            fold_identifier_with_uescape("U&\"x\"", uescape),
            Err(FoldError::InvalidEscapeCharacter),
            "UESCAPE {uescape:?}"
        );
    }
    assert_eq!(
        FoldError::InvalidEscapeCharacter.to_string(),
        "invalid Unicode escape character"
    );
    // `g` is the first letter that is not a hexadecimal digit.
    assert_eq!(
        fold_identifier_with_uescape("U&\"dg0061t\"", "g")
            .unwrap()
            .name,
        "dat"
    );
}

#[test]
fn reports_postgresql_errors_for_bad_escapes() {
    let cases = [
        // Not `XXXX`, `+XXXXXX` or a doubled escape character.
        ("U&\"\\\"", FoldError::InvalidEscape),
        ("U&\"\\006\"", FoldError::InvalidEscape),
        ("U&\"\\006g\"", FoldError::InvalidEscape),
        ("U&\"\\+00006\"", FoldError::InvalidEscape),
        ("U&\"\\x0061\"", FoldError::InvalidEscape),
        ("U&\"a\\\"", FoldError::InvalidEscape),
        // `check_unicode_value`: 0 and everything above U+10FFFF.
        ("U&\"\\0000\"", FoldError::InvalidEscapeValue),
        ("U&\"\\+000000\"", FoldError::InvalidEscapeValue),
        ("U&\"\\+110000\"", FoldError::InvalidEscapeValue),
        ("U&\"\\+FFFFFF\"", FoldError::InvalidEscapeValue),
        // A surrogate without its partner.
        ("U&\"\\D83D\"", FoldError::InvalidSurrogatePair),
        ("U&\"\\D83Dx\"", FoldError::InvalidSurrogatePair),
        ("U&\"\\D83D\\\\\"", FoldError::InvalidSurrogatePair),
        ("U&\"\\D83D\\0061\"", FoldError::InvalidSurrogatePair),
        ("U&\"\\D83D\\D83D\"", FoldError::InvalidSurrogatePair),
        ("U&\"\\DE00\"", FoldError::InvalidSurrogatePair),
        ("U&\"x\\DE00\\D83D\"", FoldError::InvalidSurrogatePair),
    ];
    for (raw, error) in cases {
        assert_eq!(fold_identifier(raw), Err(error), "{raw}");
    }
    assert_eq!(
        FoldError::InvalidEscape.to_string(),
        "invalid Unicode escape"
    );
    assert_eq!(
        FoldError::InvalidEscapeValue.to_string(),
        "invalid Unicode escape value"
    );
    assert_eq!(
        FoldError::InvalidSurrogatePair.to_string(),
        "invalid Unicode surrogate pair"
    );
}

/// `scan.l` raises "zero-length delimited identifier". pg-sql's lexer accepts
/// `""`, so the fold is where a consumer learns of it.
#[test]
fn rejects_a_zero_length_delimited_identifier() {
    for raw in ["\"\"", "U&\"\"", "u&\"\""] {
        assert_eq!(fold_identifier(raw), Err(FoldError::ZeroLength), "{raw}");
    }
    assert_eq!(
        FoldError::ZeroLength.to_string(),
        "zero-length delimited identifier"
    );
    // `""""` is one quote character, not an empty name.
    assert_eq!(fold_identifier("\"\"\"\"").unwrap().name, "\"");
}

#[test]
fn truncates_to_namedatalen_minus_one_bytes() {
    assert_eq!(NAMEDATALEN, 64);

    // 63 bytes is a whole name.
    let fits = "a".repeat(63);
    let folded_fits = folded(&fits);
    assert_eq!(folded_fits.name, fits);
    assert!(!folded_fits.truncated);

    // 64 bytes is one too many, in each form.
    let long = "a".repeat(64);
    let quoted = format!("\"{long}\"");
    let unicode = format!("U&\"{}\\0061\"", "a".repeat(63));
    let upper = "A".repeat(64);
    for raw in [&long, &quoted, &unicode, &upper] {
        let folded = folded(raw);
        assert_eq!(folded.name, fits, "{raw}");
        assert!(folded.truncated, "{raw}");
    }
    // Nothing but the extent changed, so the cut name still borrows.
    assert!(matches!(folded(&long).name, Cow::Borrowed(_)));
    assert!(matches!(folded(&quoted).name, Cow::Borrowed(_)));

    // `pg_mbcliplen` never cuts inside a character: `é` is two bytes, at
    // bytes 62 and 63, so the name ends at byte 62.
    let straddle = format!("{}é", "a".repeat(62));
    assert_eq!(straddle.len(), 64);
    for raw in [straddle.clone(), format!("\"{straddle}\"")] {
        let folded = folded(&raw);
        assert_eq!(folded.name, "a".repeat(62), "{raw}");
        assert!(folded.truncated, "{raw}");
    }
    // A four-byte character that starts at byte 61 goes whole.
    let wide = format!("\"{}\u{1F600}\"", "a".repeat(61));
    let folded_wide = folded(&wide);
    assert_eq!(folded_wide.name, "a".repeat(61));
    assert!(folded_wide.truncated);

    // The cut comes last: the quotes and the escapes do not count.
    let escaped = format!("U&\"{}\"", "\\0061".repeat(63));
    let folded_escaped = folded(&escaped);
    assert_eq!(folded_escaped.name, fits);
    assert!(!folded_escaped.truncated);
    let doubled = format!("\"{}\"", "\"\"".repeat(63));
    let folded_doubled = folded(&doubled);
    assert_eq!(folded_doubled.name, "\"".repeat(63));
    assert!(!folded_doubled.truncated);
}

#[test]
fn folds_a_keyword_from_its_token_kind() {
    use pg_sql::TokenKind;
    use pg_sql::ident::fold_keyword;

    for (kind, name) in [
        (TokenKind::VALUE, "value"),
        (TokenKind::COALESCE, "coalesce"),
        (TokenKind::JSON_VALUE, "json_value"),
        (TokenKind::SELECT, "select"),
    ] {
        let folded = fold_keyword(kind).unwrap_or_else(|| panic!("{kind} is a keyword"));
        assert_eq!(folded.name, name);
        assert!(!folded.truncated);
    }
    // Punctuation has a fixed spelling but is not a word; a content token
    // has no fixed spelling.
    assert_eq!(fold_keyword(TokenKind::LPAREN), None);
    assert_eq!(fold_keyword(TokenKind::COLONCOLON), None);
    assert_eq!(fold_keyword(TokenKind::ColIdText), None);
}

/// The identifier nodes call the free function on their own source text.
#[test]
fn identifier_nodes_fold_their_own_text() {
    use pg_sql::tokens::literal::Ident;
    use pg_sql::tokens::{BareColLabel, ColId, ColLabel, NonReservedWord, type_function_name};

    macro_rules! folded_name {
        ($node:ident, $source:expr) => {{
            let lexed = pg_sql::lex($source);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {}", $source);
            let mut input = lexed.input();
            let parsed = $node::parse(&mut input).unwrap_or_else(|e| panic!("{}: {e}", $source));
            assert!(input.is_eof(), "{} not fully consumed", $source);
            parsed.ast().folded().map(|folded| folded.name.into_owned())
        }};
    }

    assert_eq!(folded_name!(ColId, "\"Foo\"\"Bar\"").unwrap(), "Foo\"Bar");
    assert_eq!(folded_name!(ColId, "VALUE").unwrap(), "value");
    assert_eq!(folded_name!(ColLabel, "SELECT").unwrap(), "select");
    assert_eq!(folded_name!(BareColLabel, "Coalesce").unwrap(), "coalesce");
    assert_eq!(
        folded_name!(NonReservedWord, "U&\"d\\0061t\"").unwrap(),
        "dat"
    );
    assert_eq!(folded_name!(type_function_name, "Lower").unwrap(), "lower");
    assert_eq!(folded_name!(Ident, "MyTable").unwrap(), "mytable");
    assert_eq!(folded_name!(ColId, "\"\""), Err(FoldError::ZeroLength));
}

/// PostgreSQL's own scanner, through the `pg-oracle` FFI bridge, agrees with
/// the fold. `raw_parser` prints a column reference's folded name, reports a
/// scanner error as no tree at all, and truncates a long name itself.
///
/// Run with `cargo test --features postgres-oracle --test ident`.
#[cfg(feature = "postgres-oracle")]
#[test]
fn postgresql_scanner_agrees_with_the_fold() {
    fn column_name(select_item: &str) -> Option<String> {
        let tree = pg_oracle::node_to_string(&format!("SELECT {select_item}"))?;
        let start = tree.find(":fields (\"")? + ":fields (\"".len();
        let end = start + tree[start..].find("\") :location")?;
        Some(tree[start..end].to_owned())
    }

    let long = "a".repeat(64);
    let straddle = format!("\"{}é\"", "a".repeat(62));
    let accepted = [
        "Foo",
        "ÄRGER",
        "\"Foo\"",
        "\"Foo\"\"Bar\"",
        "\"SELECT\"",
        "U&\"d\\0061t\\+000061\"",
        "u&\"d\\0061t\\+000061\"",
        "U&\"\\00E4rger\"",
        "U&\"\\D83D\\DE00\"",
        "U&\"\\+01F600\"",
        "U&\"\\D83D\\+00DE00\"",
        "U&\"a\"\"\\0062\"",
        long.as_str(),
        straddle.as_str(),
    ];
    for raw in accepted {
        let folded = fold_identifier(raw).unwrap_or_else(|error| panic!("{raw}: {error}"));
        assert_eq!(column_name(raw).as_deref(), Some(&*folded.name), "{raw}");
    }
    for (raw, uescape) in [("U&\"d!0061t!+000061\"", "!"), ("U&\"d#0061t##\"", "#")] {
        let folded = fold_identifier_with_uescape(raw, uescape).unwrap();
        assert_eq!(
            column_name(&format!("{raw} UESCAPE '{uescape}'")).as_deref(),
            Some(&*folded.name),
            "{raw} UESCAPE '{uescape}'"
        );
    }

    let rejected = [
        "\"\"",
        "U&\"\"",
        "U&\"\\006\"",
        "U&\"\\+00006\"",
        "U&\"\\0000\"",
        "U&\"\\+110000\"",
        "U&\"\\D83D\"",
        "U&\"\\D83Dx\"",
        "U&\"\\D83D\\\\\"",
        "U&\"\\DE00\"",
        "U&\"\\D83D\\0061\"",
    ];
    for raw in rejected {
        assert!(fold_identifier(raw).is_err(), "{raw}");
        assert_eq!(column_name(raw), None, "{raw}");
    }
    for uescape in ["0", "a", "+", "\"", " ", "ab", ""] {
        let raw = "U&\"x\"";
        assert!(fold_identifier_with_uescape(raw, uescape).is_err());
        assert_eq!(
            column_name(&format!("{raw} UESCAPE '{uescape}'")),
            None,
            "{uescape:?}"
        );
    }
}
