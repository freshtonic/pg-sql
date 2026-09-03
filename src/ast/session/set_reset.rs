/// SET/RESET statement AST.
use crate::tokens::literal;

/// Scope of a SET statement: `SESSION` or `LOCAL`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetScope {
    #[tok(SESSION)]
    Session,
    #[tok(LOCAL)]
    Local,
}

/// The value in a SET statement: literal, keyword, or identifier.
///
/// Variant ordering: NumericLit before IntegerLit so `77.7` is consumed as a
/// numeric literal (longest-match-wins).
#[derive(recursa::Node, Debug, Clone)]
pub enum SetValue<'input> {
    #[tok(ON)]
    On,
    #[tok(FALSE)]
    False,
    #[tok(TRUE)]
    True,
    #[tok(DEFAULT)]
    Default,
    StringLit(literal::StringLit<'input>),
    SignedNumeric(SignedNumericLit<'input>),
    NumericLit(literal::NumericLit<'input>),
    IntegerLit(literal::IntegerLit<'input>),
    Ident(crate::tokens::ColId<'input>),
}

/// A numeric literal with an optional leading sign: `-1`, `+1.5`, `2`.
///
/// Used in positions like `SET extra_float_digits = -1` where a full `Expr`
/// is overkill and would admit keywords that shouldn't be legal values.
#[derive(recursa::Node, Debug, Clone)]
pub struct SignedNumericLit<'input> {
    pub sign: NumericSign,
    pub value: UnsignedNumericLit<'input>,
}

/// Leading `-` or `+` sign of a signed numeric literal.
#[derive(recursa::Node, Debug, Clone)]
pub enum NumericSign {
    #[tok(MINUS)]
    Neg,
    #[tok(PLUS)]
    Pos,
}

/// Either a numeric (with decimal point / exponent) or an integer literal.
#[derive(recursa::Node, Debug, Clone)]
pub enum UnsignedNumericLit<'input> {
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
}

/// The separator between param and value: TO or =.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetSep {
    #[tok(TO)]
    To,
    #[tok(EQ)]
    Eq,
}

/// Plain SET statement: `SET [SESSION|LOCAL] param TO|= value [, value ...]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, this)]
pub struct SetStmt<'input> {
    pub scope: Option<SetScope>,
    pub param: crate::ast::shared::names::QualifiedName<'input>,
    pub sep: SetSep,
    #[sep(COMMA)]
    pub values: recursa::Vec1<SetValue<'input>>,
}

/// Role target in `SET ROLE`: role name, `NONE`, or `DEFAULT`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetRoleTarget<'input> {
    #[tok(DEFAULT)]
    Default,
    Role(crate::tokens::ColId<'input>),
    String(literal::StringLit<'input>),
}

/// `SET [SESSION|LOCAL] ROLE { rolename | NONE | DEFAULT }`.
///
/// The `SET ROLE TO ...` spelling is accepted through [`SetStmt`], matching
/// PostgreSQL's `generic_set` route. Keeping `TO` out of this dedicated form
/// prevents the two AST alternatives from recognizing the same token stream.
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, this)]
pub struct SetRoleStmt<'input> {
    pub scope: Option<SetScope>,
    #[tok(ROLE, this)]
    pub target: SetRoleTarget<'input>,
}

/// Role target in `SET SESSION AUTHORIZATION`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetSessionAuthTarget<'input> {
    #[tok(DEFAULT)]
    Default,
    String(literal::StringLit<'input>),
    Role(crate::tokens::ColId<'input>),
}

/// `SET [SESSION|LOCAL] SESSION AUTHORIZATION { rolename | DEFAULT }`
#[derive(recursa::Node, Debug, Clone)]
pub struct SetSessionAuthStmt<'input> {
    // Only `LOCAL` is allowed here — the `SESSION` scope keyword would
    // conflict with the `SESSION AUTHORIZATION` literal that follows.
    #[tok(SET, this, SESSION, AUTHORIZATION)]
    #[presence(LOCAL)]
    pub local: bool,
    pub target: SetSessionAuthTarget<'input>,
}

/// A signed numeric literal: `[-]numeric | [-]integer`.
///
/// Variant ordering: Numeric before Integer (longest-match-wins for `7.5`).
#[derive(recursa::Node, Debug, Clone)]
pub enum SignedNumber<'input> {
    Numeric(SignedNumeric<'input>),
    Integer(SignedInteger<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SignedNumeric<'input> {
    #[presence(MINUS)]
    pub negative: bool,
    pub value: literal::NumericLit<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct SignedInteger<'input> {
    #[presence(MINUS)]
    pub negative: bool,
    pub value: literal::IntegerLit<'input>,
}

/// Target of `SET TIME ZONE`.
///
/// Variant ordering: `LOCAL` and `DEFAULT` (keywords) before `Number` and
/// `String`. INTERVAL form is deliberately skipped.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetTimeZoneTarget<'input> {
    #[tok(LOCAL)]
    Local,
    #[tok(DEFAULT)]
    Default,
    Number(SignedNumber<'input>),
    String(literal::StringLit<'input>),
}

/// `SET [SESSION|LOCAL] TIME ZONE { signed_number | string | LOCAL | DEFAULT }`
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, this)]
pub struct SetTimeZoneStmt<'input> {
    pub scope: Option<SetScope>,
    #[tok(TIME, ZONE, this)]
    pub target: SetTimeZoneTarget<'input>,
}

/// Value of `SET XML OPTION`: `DOCUMENT` or `CONTENT`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetXmlOptionValue {
    #[tok(DOCUMENT)]
    Document,
    #[tok(CONTENT)]
    Content,
}

/// `SET XML OPTION { DOCUMENT | CONTENT }` — sets the
/// `xmloption` GUC. Special-cased in PG's gram.y (`VariableSetStmt:
/// SET set_rest_more`'s `XML OPTION document_or_content` form).
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, this)]
pub struct SetXmlOptionStmt {
    pub scope: Option<SetScope>,
    #[tok(XML, OPTION, this)]
    pub value: SetXmlOptionValue,
}

/// Target of a RESET statement.
///
/// Variant ordering: multi-token variants before single-token variants.
#[derive(recursa::Node, Debug, Clone)]
pub enum ResetTarget<'input> {
    #[tok(SESSION, AUTHORIZATION)]
    SessionAuth,
    #[tok(TIME, ZONE)]
    TimeZone,
    #[tok(ALL)]
    All,
    Ident(crate::ast::shared::names::QualifiedName<'input>),
}

/// RESET statement: `RESET { param | ALL | ROLE | SESSION AUTHORIZATION | TIME ZONE }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ResetStmt<'input> {
    #[tok(RESET, this)]
    pub target: ResetTarget<'input>,
}

// --- SHOW ---

/// Target of a SHOW statement.
///
/// Variant ordering: multi-token targets before single-token `Param`
/// fallback so the specific forms are matched first.
#[derive(recursa::Node, Debug, Clone)]
pub enum ShowTarget<'input> {
    #[tok(TRANSACTION, ISOLATION, LEVEL)]
    TransactionIsolationLevel,
    #[tok(SESSION, AUTHORIZATION)]
    SessionAuthorization,
    #[tok(TIME, ZONE)]
    TimeZone,
    #[tok(ALL)]
    All,
    Param(crate::ast::shared::names::QualifiedName<'input>),
}

/// SHOW statement: `SHOW { name | ALL | TIME ZONE | SESSION AUTHORIZATION | TRANSACTION ISOLATION LEVEL }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ShowStmt<'input> {
    #[tok(SHOW, this)]
    pub target: ShowTarget<'input>,
}

/// LOAD statement: `LOAD 'filename'` — forces loading of a shared library.
#[derive(recursa::Node, Debug, Clone)]
pub struct LoadStmt<'input> {
    #[tok(LOAD, this)]
    pub filename: literal::StringLit<'input>,
}
