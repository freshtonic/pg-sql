//! DROP/CREATE option flags shared across statement families.

recursa::ast_node! {
    /// `CASCADE | RESTRICT` drop behavior.
    #[derive(Debug)]
    pub enum DropBehavior {
        #[tok(CASCADE)]
        Cascade,
        #[tok(RESTRICT)]
        Restrict,
    }
}

recursa::ast_node! {
    /// `IF EXISTS` modifier, shared by every DROP statement that allows it.
    #[derive(Debug)]
    pub enum IfExists {
        #[tok(IF, EXISTS)]
        Value,
    }
}

recursa::ast_node! {
    /// `WITH` at a position where the name that follows may itself be spelled
    /// `time` or `ordinality`.
    ///
    /// PostgreSQL's `base_yylex` merges `WITH` with a following `TIME` or
    /// `ORDINALITY` into `WITH_LA` before the parser sees it, and gram.y spells
    /// the positions where a name may still follow as the twin `WITH | WITH_LA`
    /// (REL_17_11 gram.y 11691-11692 `any_with: WITH | WITH_LA`, 1187-1189
    /// `opt_with: WITH | WITH_LA | /*EMPTY*/`).
    ///
    /// This is a marker rule: `WITH` is its last token, so its own rule says
    /// nothing about what follows and the substitution pass, which reads
    /// `FIRST` of the rest of the rule only, cannot reach the position. The
    /// grammar therefore names the merged kind at the site, exactly as gram.y
    /// writes the twin by hand. Both arms render `with`: a merged kind is the
    /// same record read differently, and keeps the source token's spelling and
    /// span.
    #[derive(Debug)]
    pub enum AnyWith {
        #[tok(WITH)]
        Value,
        #[tok(WITH_LA)]
        Merged,
    }
}

recursa::ast_node! {
    /// `IF NOT EXISTS` modifier, shared by CREATE statements that allow it.
    #[derive(Debug)]
    pub enum IfNotExists {
        #[tok(IF, NOT, EXISTS)]
        Value,
    }
}
