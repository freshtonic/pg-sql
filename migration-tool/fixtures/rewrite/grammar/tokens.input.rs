recursa::tokens! {
    categories { UNRESERVED, RESERVED }
    flags { bare_label }
    keywords { SELECT => r"SELECT" in RESERVED + bare_label, }
    soft_keywords { FORMAT => r"FORMAT" in UNRESERVED + bare_label, }
    punctuation { Comma => r",", }
    literals { Ident<'input>(source) => r"[a-z]+", }
    lexer_tokens { BlockComment => r"/\*" with skip_block_comment, }
    callbacks { token_kind_is_soft, reject_trailing_word, scan_dollar_string, skip_block_comment, post_lex }
    classes { bare_labels = keywords where bare_label }
    targets {
        ColId: Ident admits UNRESERVED,
        CustomOp: Ident,
        UnquotedIdent: Ident admits UNRESERVED,
        BareAliasName: Ident admits bare_labels,
        RestOfLine: Ident,
    }
}
