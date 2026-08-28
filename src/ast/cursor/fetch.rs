//! FETCH / MOVE / CLOSE cursor statements.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::literal;

/// `FROM` or `IN` cursor-source keyword in FETCH/MOVE.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchSource {
    From(FROM),
    In(IN),
}

/// `ABSOLUTE n` form. `n` is a `SignedIconst` per gram.y's
/// `fetch_args: ABSOLUTE_P SignedIconst opt_from_in cursor_name` — so a
/// leading sign (e.g. `ABSOLUTE -1`) is accepted.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FetchAbsolute<'input> {
    pub absolute: ABSOLUTE,
    pub count: crate::ast::shared::numbers::SignedIconst<'input>,
}

/// `RELATIVE n` form. `n` is a `SignedIconst` (see [`FetchAbsolute`]).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FetchRelative<'input> {
    pub relative: RELATIVE,
    pub count: crate::ast::shared::numbers::SignedIconst<'input>,
}

/// `FORWARD [n|ALL]` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FetchForward<'input> {
    pub forward: FORWARD,
    pub count: Option<FetchCountOrAll<'input>>,
}

/// `BACKWARD [n|ALL]` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FetchBackward<'input> {
    pub backward: BACKWARD,
    pub count: Option<FetchCountOrAll<'input>>,
}

/// A count or `ALL` marker following `FORWARD`/`BACKWARD`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchCountOrAll<'input> {
    All(ALL),
    Count(literal::IntegerLit<'input>),
}

/// FETCH/MOVE direction clause.
///
/// Variant ordering: multi-token forms (`ABSOLUTE n`, `RELATIVE n`,
/// `FORWARD [...]`, `BACKWARD [...]`) before single-keyword directions.
/// `Count` (bare integer) listed last since it has no keyword prefix.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchDirection<'input> {
    Absolute(FetchAbsolute<'input>),
    Relative(FetchRelative<'input>),
    Forward(FetchForward<'input>),
    Backward(FetchBackward<'input>),
    Next(NEXT),
    Prior(PRIOR),
    First(FIRST),
    Last(LAST),
    All(ALL),
    Count(literal::IntegerLit<'input>),
}

/// ```sql
/// FETCH [direction] [FROM|IN] cursor_name
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct FetchStmt<'input> {
    pub fetch: FETCH,
    pub direction: Option<FetchDirection<'input>>,
    pub source: Option<FetchSource>,
    pub cursor: literal::AliasName<'input>,
}

/// Target of a `CLOSE` statement: a named cursor or `ALL`.
///
/// Variant ordering: `All` (the `ALL` keyword) before `Cursor` so the
/// reserved word is not swallowed as a cursor name.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CloseTarget<'input> {
    All(ALL),
    Cursor(literal::AliasName<'input>),
}

/// ```sql
/// CLOSE { cursor_name | ALL }
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct CloseStmt<'input> {
    pub close: CLOSE,
    pub target: CloseTarget<'input>,
}

/// ```sql
/// MOVE [direction] [FROM|IN] cursor_name
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct MoveStmt<'input> {
    pub r#move: MOVE,
    pub direction: Option<FetchDirection<'input>>,
    pub source: Option<FetchSource>,
    pub cursor: literal::AliasName<'input>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn close_cursor_is_modelled() {
        let stmt: CloseStmt = parse_stmt("CLOSE foo1");
        assert!(matches!(stmt.target, CloseTarget::Cursor(_)));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE foo1"), "CLOSE foo1");
    }

    #[test]
    fn close_all_is_modelled() {
        let stmt: CloseStmt = parse_stmt("CLOSE ALL");
        assert!(matches!(stmt.target, CloseTarget::All(_)));
        assert_eq!(roundtrip::<CloseStmt>("CLOSE ALL"), "CLOSE ALL");
    }
}
