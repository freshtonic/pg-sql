#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::literal::*;

    fn first_token_is(src: &str, kind: crate::TokenKind, expected_len: usize) -> bool {
        let lexed = crate::lex(src);
        matches!(
            lexed.tokens().next(),
            Some(token) if token.kind() == kind && token.text().len() == expected_len
        )
    }

    fn tokenizes_as(src: &str, kind: crate::TokenKind) -> bool {
        let lexed = crate::lex(src);
        let mut tokens = lexed.tokens();
        matches!(tokens.next(), Some(token) if token.kind() == kind) && tokens.next().is_none()
    }

    // --- Keyword tests ---

    #[test]
    fn keyword_select_uppercase() {
        let lexed = crate::lex("SELECT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("SELECT", crate::TokenKind::SELECT));
    }

    #[test]
    fn keyword_select_lowercase() {
        let lexed = crate::lex("select");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("select", crate::TokenKind::SELECT));
    }

    #[test]
    fn keyword_select_mixed_case() {
        let lexed = crate::lex("SeLeCt");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("SeLeCt", crate::TokenKind::SELECT));
    }

    #[test]
    fn keyword_select_not_prefix_of_identifier() {
        let lexed = crate::lex("SELECTED");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(!tokenizes_as("SELECTED", crate::TokenKind::SELECT));
    }

    #[test]
    fn keyword_bool_not_prefix_of_booleq() {
        let lexed = crate::lex("booleq");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(!tokenizes_as("booleq", crate::TokenKind::BOOL));
    }

    #[test]
    fn keyword_bool_matches_standalone() {
        let lexed = crate::lex("bool");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("bool", crate::TokenKind::BOOL));
    }

    #[test]
    fn keyword_boolean_matches() {
        let lexed = crate::lex("BOOLEAN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("BOOLEAN", crate::TokenKind::BOOLEAN));
    }

    #[test]
    fn keyword_not_matches() {
        let lexed = crate::lex("NOT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("NOT", crate::TokenKind::NOT));
    }

    // --- Punctuation tests ---

    #[test]
    fn punctuation_semicolon() {
        let lexed = crate::lex(";");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as(";", crate::TokenKind::SEMI));
    }

    #[test]
    fn punctuation_neq() {
        let lexed = crate::lex("<>");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("<>", crate::TokenKind::NEQ));
    }

    #[test]
    fn punctuation_colon_colon() {
        let lexed = crate::lex("::");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("::", crate::TokenKind::COLONCOLON));
    }

    #[test]
    fn punctuation_lte() {
        let lexed = crate::lex("<=");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("<=", crate::TokenKind::LTE));
    }

    #[test]
    fn punctuation_gte() {
        let lexed = crate::lex(">=");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as(">=", crate::TokenKind::GTE));
    }

    // --- Custom/locale operator punct tests ---

    #[test]
    fn punctuation_tilde_leq_tilde() {
        let lexed = crate::lex("~<=~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~<=~", crate::TokenKind::TILDELEQTILDE));
    }

    #[test]
    fn punctuation_tilde_geq_tilde() {
        let lexed = crate::lex("~>=~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~>=~", crate::TokenKind::TILDEGEQTILDE));
    }

    #[test]
    fn punctuation_tilde_lt_tilde() {
        let lexed = crate::lex("~<~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~<~", crate::TokenKind::TILDELTTILDE));
    }

    #[test]
    fn punctuation_tilde_gt_tilde() {
        let lexed = crate::lex("~>~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~>~", crate::TokenKind::TILDEGTTILDE));
    }

    // --- LIKE/ILIKE family operator tests ---
    //
    // Postgres uses `~~`/`!~~`/`~~*`/`!~~*` as the implementation operators
    // for LIKE/NOT LIKE/ILIKE/NOT ILIKE. They must be distinct tokens so the
    // formatter does not emit them as two adjacent `~` characters with a
    // space between (which would produce `~ ~`, a different operator
    // sequence to PG).

    #[test]
    fn punctuation_tilde_tilde() {
        let lexed = crate::lex("~~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~~", crate::TokenKind::TILDETILDE));
    }

    #[test]
    fn punctuation_bang_tilde_tilde() {
        let lexed = crate::lex("!~~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!~~", crate::TokenKind::BANGTILDETILDE));
    }

    #[test]
    fn punctuation_tilde_tilde_star() {
        let lexed = crate::lex("~~*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~~*", crate::TokenKind::TILDETILDESTAR));
    }

    #[test]
    fn punctuation_bang_tilde_tilde_star() {
        let lexed = crate::lex("!~~*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!~~*", crate::TokenKind::BANGTILDETILDESTAR));
    }

    // Disambiguation tests — the longer LIKE/ILIKE operators must win over
    // their shorter prefixes (`~`, `!~`, `~*`, `!~*`).

    #[test]
    fn tilde_tilde_wins_over_tilde() {
        // `~~` should not be consumed as two `~` tokens.
        let lexed = crate::lex("~~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~~", crate::TokenKind::TILDETILDE));
    }

    #[test]
    fn tilde_tilde_star_wins_over_tilde_star() {
        // `~~*` should not be consumed as `~` + `~*`.
        let lexed = crate::lex("~~*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~~*", crate::TokenKind::TILDETILDESTAR));
    }

    #[test]
    fn bang_tilde_tilde_wins_over_bang_tilde() {
        // `!~~` should not be consumed as `!~` + `~`.
        let lexed = crate::lex("!~~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!~~", crate::TokenKind::BANGTILDETILDE));
    }

    #[test]
    fn bang_tilde_tilde_star_wins_over_bang_tilde_star() {
        // `!~~*` should not be consumed as `!~` + `~*` or `!~~` + `*`.
        let lexed = crate::lex("!~~*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!~~*", crate::TokenKind::BANGTILDETILDESTAR));
    }

    #[test]
    fn punctuation_triple_eq() {
        let lexed = crate::lex("===");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("===", crate::TokenKind::TRIPLEEQ));
    }

    #[test]
    fn punctuation_bang_eq_eq() {
        let lexed = crate::lex("!==");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!==", crate::TokenKind::BANGEQEQ));
    }

    #[test]
    fn punctuation_hash_hash() {
        let lexed = crate::lex("##");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("##", crate::TokenKind::HASHHASH));
    }

    #[test]
    fn punctuation_at_minus_at() {
        let lexed = crate::lex("@-@");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("@-@", crate::TokenKind::ATMINUSAT));
    }

    #[test]
    fn punctuation_at_hash_at() {
        let lexed = crate::lex("@#@");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("@#@", crate::TokenKind::ATHASHAT));
    }

    #[test]
    fn punctuation_at_plus_at() {
        let lexed = crate::lex("@+@");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("@+@", crate::TokenKind::ATPLUSAT));
    }

    #[test]
    fn punctuation_bang_eq_minus() {
        let lexed = crate::lex("!=-");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!=-", crate::TokenKind::BANGEQMINUS));
    }

    // Disambiguation: longer forms must win over shorter prefixes.

    #[test]
    fn tilde_leq_tilde_wins_over_tilde() {
        // ~<=~ should not be consumed as ~ then <=~
        let lexed = crate::lex("~<=~");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("~<=~", crate::TokenKind::TILDELEQTILDE));
    }

    #[test]
    fn triple_eq_wins_over_eq() {
        let lexed = crate::lex("===");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("===", crate::TokenKind::TRIPLEEQ));
    }

    #[test]
    fn bang_eq_eq_wins_over_bang_eq() {
        let lexed = crate::lex("!==");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("!==", crate::TokenKind::BANGEQEQ));
    }

    #[test]
    fn hash_hash_wins_over_pound() {
        let lexed = crate::lex("##");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("##", crate::TokenKind::HASHHASH));
    }

    #[test]
    fn at_minus_at_wins_over_at() {
        let lexed = crate::lex("@-@");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        assert!(tokenizes_as("@-@", crate::TokenKind::ATMINUSAT));
    }

    // --- String literal tests ---

    #[test]
    fn string_literal_simple() {
        let lexed = crate::lex("'hello world'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = StringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "'hello world'");
        assert!(input.is_eof());
    }

    #[test]
    fn string_literal_with_escaped_quote() {
        let lexed = crate::lex("'it''s'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = StringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "'it''s'");
    }

    #[test]
    fn string_literal_empty() {
        let lexed = crate::lex("''");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = StringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "''");
    }

    #[test]
    fn string_literal_with_spaces() {
        let lexed = crate::lex("'   f           '");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = StringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "'   f           '");
    }

    // --- INTEGER literal tests ---

    #[test]
    fn integer_literal() {
        let lexed = crate::lex("42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "42");
    }

    #[test]
    fn integer_literal_zero() {
        let lexed = crate::lex("0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "0");
    }

    // --- NUMERIC literal tests (decimals + exponent) ---

    #[test]
    fn numeric_literal_simple_decimal() {
        let lexed = crate::lex("4.5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "4.5");
    }

    #[test]
    fn numeric_literal_leading_dot() {
        let lexed = crate::lex(".5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), ".5");
    }

    #[test]
    fn numeric_literal_exponent_int() {
        let lexed = crate::lex("2e3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "2e3");
    }

    #[test]
    fn numeric_literal_decimal_with_exponent() {
        let lexed = crate::lex("4.5e10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "4.5e10");
    }

    #[test]
    fn numeric_literal_negative_exponent() {
        let lexed = crate::lex("1.5e-5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "1.5e-5");
    }

    #[test]
    fn numeric_literal_large_exponent() {
        let lexed = crate::lex("4.4e131071");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "4.4e131071");
    }

    #[test]
    fn integer_literal_with_underscores() {
        let lexed = crate::lex("100_000_000_000_000");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "100_000_000_000_000");
    }

    // Postgres 16+ accepts non-decimal integer literal forms: `0x42F`, `0b101`,
    // `0o273` (plus uppercase prefixes and `_` digit separators). Without these,
    // `0x42F` lexes as `IntegerLit("0")` + `Ident("x42F")` — the bug this widening
    // closes.
    #[test]
    fn integer_literal_hex_lowercase_prefix() {
        let lexed = crate::lex("0x42F");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "0x42F");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn integer_literal_hex_uppercase_prefix() {
        let lexed = crate::lex("0X1A2b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "0X1A2b");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn integer_literal_hex_with_underscores() {
        let lexed = crate::lex("0xFF_FF");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "0xFF_FF");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn integer_literal_octal_prefix() {
        let lexed = crate::lex("0o273");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "0o273");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn integer_literal_binary_prefix() {
        let lexed = crate::lex("0b101");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "0b101");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// PG accepts `_` immediately after the radix prefix (`0b_…`, `0x_…`,
    /// `0o_…`) — gram.y `bininteger 0[bB](_?{bindigit})+`.
    #[test]
    fn integer_literal_radix_prefix_leading_underscore() {
        for src in ["0b_10_0101", "0x_FF", "0o_7"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let lit = IntegerLit::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert_eq!(lit.text(), src);
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    // ── lexer-arbitration test helper ─────────────────────────────────────
    //
    // The old classifier (`classify(src, 0) -> SlotState`) is gone; the logos
    // `lex` pass replaces it. These helpers expose the lexer's first-token
    // decision so the cross-token arbitration tests below — longest-match,
    // trailing-junk rejection, single-token classification — still verify the
    // same behaviour against the new model.

    // Longest-match-wins must keep `NumericLit` ahead of `IntegerLit` whenever
    // a `.` or exponent is present. These tests route through `lex` so they
    // exercise cross-token arbitration — if a future change let `IntegerLit`'s
    // regex match `0.5`, the lexer (not `NumericLit::parse` in isolation) is
    // where the wrong choice would surface.
    #[test]
    fn numeric_literal_still_wins_over_integer_with_decimal() {
        assert!(first_token_is("0.5", crate::TokenKind::NumericLit, 3));
    }

    #[test]
    fn numeric_literal_still_wins_over_integer_with_exponent() {
        assert!(first_token_is("1e10", crate::TokenKind::NumericLit, 4));
    }

    #[test]
    fn numeric_literal_with_underscores() {
        let lexed = crate::lex("1_234.567_89");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = NumericLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "1_234.567_89");
    }

    // The trailing-dot form (`1.` — empty fraction, no exponent) is a valid
    // PostgreSQL numeric literal when followed by a non-word char or EOF.
    // It must classify as a `NumericLit` spanning the whole `1.`.
    #[test]
    fn numeric_literal_trailing_dot_valid() {
        for src in ["1.", "1. ", "1.;", "1_000."] {
            let dot_end = src.find('.').unwrap() + 1;
            assert!(
                first_token_is(src, crate::TokenKind::NumericLit, dot_end),
                "{src:?} should lex as NumericLit spanning up to the dot"
            );
        }
    }

    // A numeric literal immediately followed by an identifier-start char is a
    // PostgreSQL lex error ("trailing junk after numeric literal"). The
    // trailing-dot form must NOT match across the dot for `0.a` / `1_000._5` —
    // the classifier must not return a `NumericLit` spanning the dot.
    #[test]
    fn numeric_literal_trailing_dot_junk_rejected() {
        for src in ["0.a", "1_000._5"] {
            // The `reject_trailing_word` callback makes the trailing-dot
            // numeric form reject when a word char follows. `0` alone lexing
            // as IntegerLit is fine — the `.a` / `._5` trailing junk is what
            // PostgreSQL rejects.
            if let Some(token) = crate::lex(src).tokens().next() {
                assert_ne!(
                    token.kind(),
                    crate::TokenKind::NumericLit,
                    "{src:?} must not lex as NumericLit spanning the dot"
                );
            }
        }
    }

    #[test]
    fn integer_literal_does_not_match_decimal() {
        // Bare integer still works
        let lexed = crate::lex("42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = IntegerLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "42");
    }

    // Cross-token arbitration: with the classifier installed, the non-decimal
    // `0x…` / `0o…` / `0b…` forms (and their underscore-separated variants)
    // must classify as `IntegerLit` — NOT as `IntegerLit("0")` followed by an
    // `Ident("xNNN")`. This is the umbrella analogue of the bit/hex-string
    // arbitration test above; the per-token `IntegerLit::parse` tests verify
    // the regex in isolation, this test verifies the cross-regex arbitration
    // that is the actual code path the bug lived on.
    #[test]
    fn integer_literals_classify_as_single_token() {
        for src in ["0x42F", "0b101", "0o273", "0xFF_FF", "0X1A2b"] {
            assert!(
                first_token_is(src, crate::TokenKind::IntegerLit, src.len()),
                "{src:?} must lex as a single IntegerLit token"
            );
        }
    }

    // Trailing-junk rejection: PG's flex lexer rejects a numeric literal that
    // is immediately butted up against an identifier character or a trailing
    // underscore (no whitespace separator). Examples PG rejects with
    // "trailing junk after numeric literal" or "invalid …integer at or near
    // …": `123abc`, `0xFFG`, `0b101z`, `0o7ab`, `100_`, `0xff_`, `1.5xyz`,
    // `1e10abc`, `.5xyz`, `1_000._5`.
    //
    // The fix anchors `IntegerLit` and `NumericLit` with `\b` at the end. The
    // regex crate's `\b` is a transition between a word character (`[A-Za-z_
    // 0-9]`) and a non-word character (or input boundary). Effect:
    //   - For literals ending in a digit (the common case), `\b` requires the
    //     following character to be a non-word character — `123abc` no longer
    //     matches `IntegerLit` at all (no `\b` between `3` and `a`).
    //   - For NumericLit forms ending in `.` (e.g. `1_000.`), the regex engine
    //     can backtrack to a shorter prefix when the digit-ending form fails
    //     `\b` (e.g. `1.5xyz` matches `1.`). That partial match is still
    //     enough to derail the parse: the residue (`5xyz` etc.) does not lex
    //     as any token after the `\b` anchor closes off `IntegerLit`, so the
    //     statement falls through to `Raw` and psql emits the PG error.
    //
    // These tests verify the IntegerLit case fully (no token at all) and the
    // NumericLit case as "no token that consumes the entire input" — which is
    // the property that defeats the permissive split. See
    // regress_numerology's trailing-junk fixtures.
    #[test]
    fn integer_lit_does_not_match_when_followed_by_ident_char() {
        for src in ["123abc", "0xFFG", "0b101z", "0o7ab", "100_", "0xff_"] {
            // `reject_trailing_word` rejects the numeric match; the lexer
            // emits a `TOKEN_KIND_NONE` span (or no token), never IntegerLit.
            if let Some(token) = crate::lex(src).tokens().next() {
                assert_ne!(
                    token.kind(),
                    crate::TokenKind::IntegerLit,
                    "{src:?} must not lex as IntegerLit",
                );
            }
        }
    }

    #[test]
    fn numeric_lit_does_not_match_when_followed_by_ident_char() {
        // The `\b` anchor eliminates these forms — none of them classify as
        // any NumericLit-bearing token. PG rejects each with "trailing junk
        // after numeric literal" / "invalid …" diagnostics. With our fix the
        // regex doesn't match at all, no token is produced, and the
        // statement falls to Raw so psql emits the canonical PG error.
        for src in ["1e10abc", ".5xyz"] {
            if let Some(token) = crate::lex(src).tokens().next() {
                assert_ne!(
                    token.kind(),
                    crate::TokenKind::NumericLit,
                    "{src:?} must not lex as NumericLit",
                );
            }
        }
    }

    #[test]
    fn numeric_lit_with_trailing_ident_does_not_consume_full_input() {
        // Forms where the digit-end alt fails `\b` but the regex engine
        // backtracks to the shorter dot-end alt — e.g. `1.5xyz` matches
        // `1.` instead of the previous `1.5`. The fix doesn't make these
        // forms unparseable at the regex level, but it leaves enough
        // residue (`5xyz`, etc.) that the residue does not lex as a
        // contiguous token (`5` would need `\b` to follow it for the
        // IntegerLit arm to match — but `5xyz` has no boundary between
        // `5` and `x`), so the statement still fails to parse cleanly
        // and falls to Raw. This test pins the "leaves residue" property
        // — i.e., the NumericLit match length is strictly less than the
        // PRE-fix length (which would have consumed everything up to but
        // not including the alphabetic suffix).
        // (src, max_acceptable_token_end)
        for (src, max_len) in [("1.5xyz", 2usize), ("1.5abc", 2usize)] {
            let Some(token) = crate::lex(src).tokens().next() else {
                continue; // no token at all is also acceptable
            };
            if token.kind() == crate::TokenKind::NumericLit {
                assert!(
                    token.span().end() as usize <= max_len,
                    "{src:?}: NumericLit match length {} exceeds expected \
                     max {}. reject_trailing_word must reject the longer \
                     digit-ending match.",
                    token.span().end(),
                    max_len,
                );
            }
        }
    }

    // Regression guards: legitimate terminators (whitespace, `;`, `,`, EOF)
    // must still let the IntegerLit / NumericLit regex match.
    #[test]
    fn integer_lit_still_matches_with_legitimate_terminators() {
        for (src, expected_len) in [
            ("123", 3),
            ("123 ", 3),
            ("123;", 3),
            ("123,", 3),
            ("0xFF", 4),
            ("0b101", 5),
            ("100", 3),
        ] {
            assert!(
                first_token_is(src, crate::TokenKind::IntegerLit, expected_len),
                "{src:?} should lex as IntegerLit of length {expected_len}"
            );
        }
    }

    #[test]
    fn numeric_lit_still_matches_with_legitimate_terminators() {
        for (src, expected_len) in [
            ("1.5", 3),
            ("1.5;", 3),
            ("1.5,", 3),
            ("1e10", 4),
            (".5", 2),
            ("1.5 ", 3),
        ] {
            assert!(
                first_token_is(src, crate::TokenKind::NumericLit, expected_len),
                "{src:?} should lex as NumericLit of length {expected_len}"
            );
        }
    }

    // --- Bit-string / hex-string literal tests ---
    //
    // Postgres recognises `B'10'` (bit-string) and `X'1FF'` (hex-string) as
    // single literal tokens. The body content is validated at parse-time, not
    // lex-time — the lexer accepts any non-quote characters. Quotes cannot
    // appear inside (no `''`-escape, unlike `StringLit`).

    #[test]
    fn bit_string_literal_uppercase_prefix() {
        let lexed = crate::lex("B'10'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = BitStringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "B'10'");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn bit_string_literal_lowercase_prefix() {
        let lexed = crate::lex("b'001'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = BitStringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "b'001'");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn bit_string_literal_empty() {
        let lexed = crate::lex("B''");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = BitStringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "B''");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn hex_string_literal_uppercase_prefix() {
        let lexed = crate::lex("X'1FF'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = HexStringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "X'1FF'");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn hex_string_literal_lowercase_prefix() {
        let lexed = crate::lex("x'42f'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = HexStringLit::parse(&mut input).unwrap().into_ast();
        assert_eq!(lit.text(), "x'42f'");
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    // Cross-token arbitration: with the classifier installed, `B'…'` and
    // `X'…'` must lex as the dedicated `BitStringLit` / `HexStringLit` kinds —
    // NOT as an `Ident` followed by a `StringLit`. This is the actual code
    // path where the formatter-inserted-space bug manifested (the previous
    // behaviour was two distinct tokens which the formatter then separated
    // with a space).
    #[test]
    fn bit_and_hex_string_literals_classify_as_single_token() {
        for (src, expected) in [
            ("B'10'", crate::TokenKind::BitStringLit),
            ("b'001'", crate::TokenKind::BitStringLit),
            ("B''", crate::TokenKind::BitStringLit),
            ("X'1FF'", crate::TokenKind::HexStringLit),
            ("x'42f'", crate::TokenKind::HexStringLit),
        ] {
            assert!(
                first_token_is(src, expected, src.len()),
                "{src:?} must lex as a single {expected:?} token"
            );
        }
    }

    // --- Dollar-string literal tests ---
    //
    // Postgres dollar-quoted strings (`$tag$ ... $tag$`, `$$ ... $$`) close
    // ONLY at a matching tag. The previous regex-based scanner used a non-
    // greedy regex (`\$[a-zA-Z_]*\$[\s\S]*?\$[a-zA-Z_]*\$`), but the lexer's
    // `MatchKind::All` configuration picks the LONGEST alternative — defeating
    // `*?` and causing two distinct dollar-strings in the same input to
    // collapse into one over-matched token. These tests pin the matched-tag
    // semantics through `DollarStringLit::parse` directly and through the
    // cross-token classifier path.

    #[test]
    fn dollar_string_lit_matches_only_matching_close_tag() {
        // Two distinct `$$ ... $$` strings around a `SELECT 1;` — the FIRST
        // must end at the first `$$`, not over-match to the last `$$`.
        let src = "$$ A $$ SELECT 1; $$ B $$";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let first = DollarStringLit::parse(&mut input)
            .expect("first $$...$$ must parse")
            .into_ast();
        assert_eq!(
            first.text(),
            "$$ A $$",
            "first dollar-string must end at the first matching $$; got {:?}",
            first.text(),
        );
    }

    #[test]
    fn dollar_string_lit_named_tag_closes_only_on_matching_tag() {
        // `$foo$ body $bar$ more $foo$` — the close inside (`$bar$`) does NOT
        // match the open (`$foo$`) so scanning must continue until the real
        // `$foo$` close. This is the key "back-reference" behaviour that the
        // NFA-based regex can't express.
        let src = "$foo$ body $bar$ more $foo$";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = DollarStringLit::parse(&mut input)
            .expect("named-tag dollar-string must parse")
            .into_ast();
        assert_eq!(
            lit.text(),
            "$foo$ body $bar$ more $foo$",
            "scanning must continue past the non-matching $bar$ close",
        );
    }

    #[test]
    fn dollar_string_lit_named_tag_two_distinct_strings() {
        // Two separate `$foo$...$foo$` literals — the first must end at the
        // first matching `$foo$`, not over-match into the second.
        let src = "$foo$ A $foo$ X $foo$ B $foo$";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let first = DollarStringLit::parse(&mut input)
            .expect("first $foo$...$foo$ must parse")
            .into_ast();
        assert_eq!(
            first.text(),
            "$foo$ A $foo$",
            "first named dollar-string must end at the first matching close",
        );
    }

    #[test]
    fn dollar_string_lit_with_classifier_ends_at_first_matching_close() {
        // Cross-classifier check: with the classifier installed, parsing the
        // FIRST `$$ A $$` must consume only that token, leaving ` SELECT 1;
        // $$ B $$` for the next parse. The previous regex-based scanner with
        // `MatchKind::All` over-matched, consuming both dollar-strings in one
        // 25-byte token.
        let src = "$$ A $$ SELECT 1; $$ B $$";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let first = DollarStringLit::parse(&mut input)
            .expect("first $$...$$ must parse with classifier")
            .into_ast();
        assert_eq!(
            first.text(),
            "$$ A $$",
            "first dollar-string must end at the first matching $$ under the classifier",
        );
    }

    #[test]
    fn dollar_string_lit_empty_body() {
        // `$$$$` is an empty dollar-quoted string with empty tag.
        let src = "$$$$";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let lit = DollarStringLit::parse(&mut input)
            .expect("empty $$$$ must parse")
            .into_ast();
        assert_eq!(lit.text(), "$$$$");
    }

    #[test]
    fn dollar_string_lit_rejects_digit_leading_tag() {
        // A dollar-quote tag follows unquoted-identifier rules: it cannot
        // start with a digit. `$1$...$1$` is therefore NOT a dollar-quoted
        // string — `$1` is a positional parameter (`DollarNum`) and the
        // unmatched dollar punctuation is a lexical error.
        let src = "$1$x$1$";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 1, "invalid tag must be diagnosed");
        let mut input = lexed.input();
        assert!(
            DollarStringLit::parse(&mut input).is_err(),
            "$1$...$1$ must NOT parse as a dollar-string (tag cannot start with a digit)",
        );
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 1, "invalid tag must be diagnosed");
        let mut input = lexed.input();
        let num = DollarNum::parse(&mut input)
            .expect("$1 must parse as DollarNum")
            .into_ast();
        assert_eq!(num.text(), "$1");
    }

    // --- Soft keyword tests ---

    /// A soft (non-reserved) keyword is reclaimable as an identifier when a
    /// classifier is installed: `format`, `path`, `json`, etc. classify as
    /// their keyword token, but `Ident` still accepts them through its
    /// `UnquotedIdent` admission set. A
    /// reserved keyword (`select`) must stay rejected.
    #[test]
    fn soft_keyword_parses_as_identifier_with_classifier() {
        // Non-reserved Postgres keywords — usable as ordinary identifiers.
        for word in [
            "format",
            "path",
            "json",
            "empty",
            "scalar", // SQL/JSON family
            "target",
            "source",
            "key",
            "name",
            "value",
            "data",
            "update",
            "insert",
            "type",
            "method",
            "owner",
            "action",
            "level",
            "off",
            "set",
            "nulls",
            "partition",
        ] {
            let lexed = crate::lex(word);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let id = Ident::parse(&mut input)
                .unwrap_or_else(|e| panic!("soft keyword {word:?} should parse as ident: {e}"))
                .into_ast();
            assert_eq!(id.text(), word);
            assert!(input.is_eof(), "leftover after {word:?}");
        }
    }

    #[test]
    fn reserved_keyword_rejected_as_identifier_with_classifier() {
        // Reserved keywords are not members of the general identifier
        // admission. Clause words such as SET, NULLS, and PARTITION are
        // PostgreSQL UNRESERVED keywords and remain legal identifiers; their
        // narrower exclusions belong to the grammar positions that need them.
        for word in ["select", "from", "where"] {
            let lexed = crate::lex(word);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            assert!(
                Ident::parse(&mut input).is_err(),
                "reserved/clause keyword {word:?} must not parse as an identifier"
            );
        }
    }

    // --- Identifier tests ---

    #[test]
    fn identifier_simple() {
        let lexed = crate::lex("my_table");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "my_table");
    }

    #[test]
    fn identifier_with_digits() {
        let lexed = crate::lex("f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "f1");
    }

    #[test]
    fn identifier_uppercase() {
        let lexed = crate::lex("BOOLTBL1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "BOOLTBL1");
    }

    #[test]
    fn unquoted_rejects_keyword_select() {
        let lexed = crate::lex("SELECT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Ident::parse(&mut input).is_err());
    }

    #[test]
    fn unquoted_rejects_keyword_true() {
        let lexed = crate::lex("true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Ident::parse(&mut input).is_err());
    }

    #[test]
    fn unquoted_rejects_keyword_null() {
        let lexed = crate::lex("NULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Ident::parse(&mut input).is_err());
    }

    #[test]
    fn ident_enum_rejects_keyword() {
        // The enum postcondition rejects reserved keyword input.
        let lexed = crate::lex("SELECT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Ident::parse(&mut input).is_err());
        let input2_lexed = crate::lex("SELECT");
        assert_eq!(input2_lexed.errors().count(), 0, "lex errors in input2");
        let mut input2 = input2_lexed.input();
        assert!(Ident::parse(&mut input2).is_err());
    }

    #[test]
    fn ident_accepts_rows_as_identifier() {
        // ROWS is unreserved in PostgreSQL and must be usable as a plain
        // identifier (e.g. `FROM rows`, `SELECT range FROM t`).
        for w in ["rows", "ROWS", "range", "groups"] {
            let lexed = crate::lex(w);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let id = Ident::parse(&mut input)
                .unwrap_or_else(|_| panic!("{w} should parse as Ident"))
                .into_ast();
            assert_eq!(id.text(), w);
        }
    }

    #[test]
    fn window_ref_name_rejects_clause_heads() {
        // Window `ref_name` must reject every following clause head so the
        // optional name cannot consume it before its required continuation.
        for w in [
            "partition",
            "PARTITION",
            "order",
            "ORDER",
            "rows",
            "ROWS",
            "range",
            "RANGE",
            "groups",
            "GROUPS",
        ] {
            let lexed = crate::lex(w);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            assert!(
                WindowRefNameIdent::parse(&mut input).is_err(),
                "{w} must not parse as a window ref_name"
            );
        }
    }

    #[test]
    fn window_ref_name_accepts_plain_ident() {
        let lexed = crate::lex("w1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = WindowRefNameIdent::parse(&mut input).unwrap().into_ast();
        let WindowRefNameIdent::Text(inner) = &id;
        assert_eq!(inner.text(), "w1");
    }

    #[test]
    fn ident_enum_parses_quoted() {
        let lexed = crate::lex("\"SELECT\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "\"SELECT\"");
        assert!(input.is_eof());
    }

    #[test]
    fn identifier_accepts_keyword_prefix() {
        let lexed = crate::lex("isfalse");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "isfalse");
    }

    #[test]
    fn identifier_accepts_booleq() {
        let lexed = crate::lex("booleq");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "booleq");
    }

    #[test]
    fn identifier_accepts_boolne() {
        let lexed = crate::lex("boolne");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "boolne");
    }

    #[test]
    fn identifier_accepts_isnul() {
        let lexed = crate::lex("isnul");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "isnul");
    }

    #[test]
    fn identifier_accepts_istrue() {
        let lexed = crate::lex("istrue");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "istrue");
    }

    #[test]
    fn identifier_accepts_pg_input_is_valid() {
        let lexed = crate::lex("pg_input_is_valid");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let id = Ident::parse(&mut input).unwrap().into_ast();
        assert_eq!(id.text(), "pg_input_is_valid");
    }
}
