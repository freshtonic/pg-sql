//! FETCH / MOVE / CLOSE cursor statements.

use crate::tokens::literal;

/// `FROM` or `IN` cursor-source keyword in FETCH/MOVE.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchSource {
    #[tok(FROM)]
    From,
    #[tok(IN)]
    In,
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
#[tok(FORWARD, this)]
pub struct FetchForward<'input> {
    pub count: Option<FetchCountOrAll<'input>>,
}

/// `BACKWARD [n|ALL]` form.
#[derive(recursa::Node, Debug, Clone)]
#[tok(BACKWARD, this)]
pub struct FetchBackward<'input> {
    pub count: Option<FetchCountOrAll<'input>>,
}

/// A count or `ALL` marker following `FORWARD`/`BACKWARD`.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchCountOrAll<'input> {
    #[tok(ALL)]
    All,
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
    #[tok(NEXT)]
    Next,
    #[tok(PRIOR)]
    Prior,
    #[tok(FIRST)]
    First,
    #[tok(LAST)]
    Last,
    #[tok(ALL)]
    All,
    Count(literal::IntegerLit<'input>),
}

/// ```sql
/// FETCH [direction] [FROM|IN] cursor_name
/// ```
#[derive(recursa::Node, Debug, Clone)]
#[tok(FETCH, this)]
pub struct FetchStmt<'input> {
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
    #[tok(ALL)]
    All,
    Cursor(literal::Ident<'input>),
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
#[tok(MOVE, this)]
pub struct MoveStmt<'input> {
    pub direction: Option<FetchDirection<'input>>,
    pub source: Option<FetchSource>,
    pub cursor: literal::AliasName<'input>,
}
