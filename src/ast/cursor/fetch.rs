//! FETCH / MOVE / CLOSE cursor statements.

use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::literal;

/// `FROM` or `IN` cursor-source keyword in FETCH/MOVE.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchSource {
    #[tok(FROM)] From,
    #[tok(IN)] In,
}

/// `ABSOLUTE n` form. `n` is a `SignedIconst` per gram.y's
/// `fetch_args: ABSOLUTE_P SignedIconst opt_from_in cursor_name` — so a
/// leading sign (e.g. `ABSOLUTE -1`) is accepted.
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchAbsolute<'input> {
    #[tok(ABSOLUTE, this)]
    pub count: crate::ast::shared::numbers::SignedIconst<'input>,
}

/// `RELATIVE n` form. `n` is a `SignedIconst` (see [`FetchAbsolute`]).
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchRelative<'input> {
    #[tok(RELATIVE, this)]
    pub count: crate::ast::shared::numbers::SignedIconst<'input>,
}

/// `FORWARD [n|ALL]` form.
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchForward<'input> {
    #[tok(FORWARD, this)]
    pub count: Option<FetchCountOrAll<'input>>,
}

/// `BACKWARD [n|ALL]` form.
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchBackward<'input> {
    #[tok(BACKWARD, this)]
    pub count: Option<FetchCountOrAll<'input>>,
}

/// A count or `ALL` marker following `FORWARD`/`BACKWARD`.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchCountOrAll<'input> {
    #[tok(ALL)] All,
    Count(literal::IntegerLit<'input>),
}

/// FETCH/MOVE direction clause.
///
/// Variant ordering: multi-token forms (`ABSOLUTE n`, `RELATIVE n`,
/// `FORWARD [...]`, `BACKWARD [...]`) before single-keyword directions.
/// `Count` (bare integer) listed last since it has no keyword prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchDirection<'input> {
    Absolute(FetchAbsolute<'input>),
    Relative(FetchRelative<'input>),
    Forward(FetchForward<'input>),
    Backward(FetchBackward<'input>),
    #[tok(NEXT)] Next,
    #[tok(PRIOR)] Prior,
    #[tok(FIRST)] First,
    #[tok(LAST)] Last,
    #[tok(ALL)] All,
    Count(literal::IntegerLit<'input>),
}

/// ```sql
/// FETCH [direction] [FROM|IN] cursor_name
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchStmt<'input> {
    #[tok(FETCH, this)]
    pub direction: Option<FetchDirection<'input>>,
    pub source: Option<FetchSource>,
    pub cursor: literal::AliasName<'input>,
}

/// Target of a `CLOSE` statement: a named cursor or `ALL`.
///
/// Variant ordering: `All` (the `ALL` keyword) before `Cursor` so the
/// reserved word is not swallowed as a cursor name.
#[derive(recursa::Node, Debug, Clone)]
pub enum CloseTarget<'input> {
    #[tok(ALL)] All,
    Cursor(literal::AliasName<'input>),
}

/// ```sql
/// CLOSE { cursor_name | ALL }
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct CloseStmt<'input> {
    #[tok(CLOSE, this)]
    pub target: CloseTarget<'input>,
}

/// ```sql
/// MOVE [direction] [FROM|IN] cursor_name
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct MoveStmt<'input> {
    #[tok(MOVE, this)]
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
