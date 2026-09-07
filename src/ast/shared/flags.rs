//! DROP/CREATE option flags shared across statement families.

/// `CASCADE | RESTRICT` drop behavior.
#[derive(recursa::Node, Debug, Clone)]
pub enum DropBehavior {
    #[tok(CASCADE)]
    Cascade,
    #[tok(RESTRICT)]
    Restrict,
}

/// `IF EXISTS` modifier, shared by every DROP statement that allows it.
#[derive(recursa::Node, Debug, Clone)]
pub enum IfExists {
    #[tok(IF, EXISTS)]
    Value,
}

/// `WITH` at a position where the name that follows may itself be spelled
/// `time` or `ordinality`.
///
/// PostgreSQL's `base_yylex` merges `WITH` with a following `TIME` or
/// `ORDINALITY` into `WITH_LA` before the parser sees it, and gram.y spells
/// the positions where a name may still follow as the twin `WITH | WITH_LA`
/// (`with_clause`, `any_with`, `opt_with`). recursa's lookahead-filter
/// lowering emits that twin for this marker rule.
#[derive(recursa::Node, Debug, Clone)]
pub enum AnyWith {
    #[tok(WITH)]
    Value,
}

/// `IF NOT EXISTS` modifier, shared by CREATE statements that allow it.
#[derive(recursa::Node, Debug, Clone)]
pub enum IfNotExists {
    #[tok(IF, NOT, EXISTS)]
    Value,
}
