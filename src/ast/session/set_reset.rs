/// SET/RESET statement AST.
use recursa::seq::Seq0;
use recursa::{FormatTokens, Transform, Visit};

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Scope of a SET statement: `SESSION` or `LOCAL`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetScope {
    Session(SESSION),
    Local(LOCAL),
}

/// The value in a SET statement: literal, keyword, or identifier.
///
/// Variant ordering: NumericLit before IntegerLit so `77.7` is consumed as a
/// numeric literal (longest-match-wins).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetValue<'input> {
    On(ON),
    Off(OFF),
    False(FALSE),
    True(TRUE),
    Default(DEFAULT),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SignedNumericLit<'input> {
    pub sign: NumericSign,
    pub value: UnsignedNumericLit<'input>,
}

/// Leading `-` or `+` sign of a signed numeric literal.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NumericSign {
    Neg(punct::Minus),
    Pos(punct::Plus),
}

/// Either a numeric (with decimal point / exponent) or an integer literal.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum UnsignedNumericLit<'input> {
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
}

/// The separator between param and value: TO or =.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetSep {
    To(TO),
    Eq(punct::Eq),
}

/// Plain SET statement: `SET [SESSION|LOCAL] param TO|= value [, value ...]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct SetStmt<'input> {
    pub set: SET,
    pub scope: Option<SetScope>,
    pub param: crate::ast::shared::names::QualifiedName<'input>,
    pub sep: SetSep,
    pub values: Seq0<SetValue<'input>, punct::Comma>,
}

/// Role target in `SET ROLE`: role name, `NONE`, or `DEFAULT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetRoleTarget<'input> {
    None(NONE),
    Default(DEFAULT),
    Role(literal::AliasName<'input>),
    String(literal::StringLit<'input>),
}

/// `SET [SESSION|LOCAL] ROLE [TO] { rolename | NONE | DEFAULT }`.
///
/// The `TO` is accepted because Postgres' grammar reaches this form via
/// `generic_set: var_name TO var_list` when `ROLE` is treated as an
/// unreserved keyword (it's `UNRESERVED_KEYWORD` in `kwlist.h`). pg-sql
/// commits on the specific `SET … ROLE` path before the generic form, so
/// the `TO` is modelled here as an explicit `Option<TO>`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct SetRoleStmt<'input> {
    pub set: SET,
    pub scope: Option<SetScope>,
    pub role: ROLE,
    pub to: Option<TO>,
    pub target: SetRoleTarget<'input>,
}

/// Role target in `SET SESSION AUTHORIZATION`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetSessionAuthTarget<'input> {
    Default(DEFAULT),
    String(literal::StringLit<'input>),
    Role(literal::AliasName<'input>),
}

/// `SET [SESSION|LOCAL] SESSION AUTHORIZATION { rolename | DEFAULT }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct SetSessionAuthStmt<'input> {
    pub set: SET,
    // Only `LOCAL` is allowed here — the `SESSION` scope keyword would
    // conflict with the `SESSION AUTHORIZATION` literal that follows.
    pub local: Option<LOCAL>,
    pub session: SESSION,
    pub authorization: AUTHORIZATION,
    pub target: SetSessionAuthTarget<'input>,
}

/// A signed numeric literal: `[-]numeric | [-]integer`.
///
/// Variant ordering: Numeric before Integer (longest-match-wins for `7.5`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SignedNumber<'input> {
    Numeric(SignedNumeric<'input>),
    Integer(SignedInteger<'input>),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SignedNumeric<'input> {
    pub minus: Option<punct::Minus>,
    pub value: literal::NumericLit<'input>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SignedInteger<'input> {
    pub minus: Option<punct::Minus>,
    pub value: literal::IntegerLit<'input>,
}

/// Target of `SET TIME ZONE`.
///
/// Variant ordering: `LOCAL` and `DEFAULT` (keywords) before `Number` and
/// `String`. INTERVAL form is deliberately skipped.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetTimeZoneTarget<'input> {
    Local(LOCAL),
    Default(DEFAULT),
    Number(SignedNumber<'input>),
    String(literal::StringLit<'input>),
}

/// `SET [SESSION|LOCAL] TIME ZONE { signed_number | string | LOCAL | DEFAULT }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct SetTimeZoneStmt<'input> {
    pub set: SET,
    pub scope: Option<SetScope>,
    pub time: TIME,
    pub zone: ZONE,
    pub target: SetTimeZoneTarget<'input>,
}

/// Value of `SET XML OPTION`: `DOCUMENT` or `CONTENT`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SetXmlOptionValue {
    Document(DOCUMENT),
    Content(CONTENT),
}

/// `SET XML OPTION { DOCUMENT | CONTENT }` — sets the
/// `xmloption` GUC. Special-cased in PG's gram.y (`VariableSetStmt:
/// SET set_rest_more`'s `XML OPTION document_or_content` form).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct SetXmlOptionStmt {
    pub set: SET,
    pub scope: Option<SetScope>,
    pub xml: XML,
    pub option: OPTION,
    pub value: SetXmlOptionValue,
}

/// Target of a RESET statement.
///
/// Variant ordering: multi-token variants before single-token variants.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ResetTarget<'input> {
    SessionAuth((SESSION, AUTHORIZATION)),
    TimeZone((TIME, ZONE)),
    Role(ROLE),
    All(ALL),
    Ident(crate::ast::shared::names::QualifiedName<'input>),
}

/// RESET statement: `RESET { param | ALL | ROLE | SESSION AUTHORIZATION | TIME ZONE }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct ResetStmt<'input> {
    pub reset: RESET,
    pub target: ResetTarget<'input>,
}

// --- SHOW ---

/// Target of a SHOW statement.
///
/// Variant ordering: multi-token targets before single-token `Param`
/// fallback so the specific forms are matched first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ShowTarget<'input> {
    TransactionIsolationLevel((TRANSACTION, ISOLATION, LEVEL)),
    SessionAuthorization((SESSION, AUTHORIZATION)),
    TimeZone((TIME, ZONE)),
    All(ALL),
    Param(crate::ast::shared::names::QualifiedName<'input>),
}

/// SHOW statement: `SHOW { name | ALL | TIME ZONE | SESSION AUTHORIZATION | TRANSACTION ISOLATION LEVEL }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["session"])]
pub struct ShowStmt<'input> {
    pub show: SHOW,
    pub target: ShowTarget<'input>,
}

/// LOAD statement: `LOAD 'filename'` — forces loading of a shared library.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct LoadStmt<'input> {
    pub load: LOAD,
    pub filename: literal::StringLit<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::session::set_reset::{
        ResetStmt, SetRoleStmt, SetSessionAuthStmt, SetStmt, SetTimeZoneStmt, ShowStmt,
    };

    #[test]
    fn parse_set_to() {
        let mut input = crate::tokens::test_input("SET enable_seqscan TO off");
        let stmt = SetStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.param.object(), "enable_seqscan");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_eq() {
        let mut input = crate::tokens::test_input("SET enable_sort = false");
        let stmt = SetStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.param.object(), "enable_sort");
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_integer_value() {
        let mut input = crate::tokens::test_input("SET work_mem = 4096");
        let stmt = SetStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.values.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_numeric_value() {
        let mut input = crate::tokens::test_input("SET seq_page_cost = 1.5");
        let stmt = SetStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.values.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_multi_value() {
        let mut input = crate::tokens::test_input("SET search_path TO public, pg_catalog");
        let stmt = SetStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.values.len(), 2);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_session_scope() {
        let mut input = crate::tokens::test_input("SET SESSION enable_seqscan TO off");
        let stmt = SetStmt::parse(&mut input).unwrap();
        assert!(stmt.scope.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_reset() {
        let mut input = crate::tokens::test_input("RESET enable_seqscan");
        let stmt = ResetStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
        let _ = stmt;
    }

    #[test]
    fn parse_reset_all() {
        let mut input = crate::tokens::test_input("RESET ALL");
        let _stmt = ResetStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_reset_role() {
        let mut input = crate::tokens::test_input("RESET ROLE");
        let _stmt = ResetStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_reset_session_authorization() {
        let mut input = crate::tokens::test_input("RESET SESSION AUTHORIZATION");
        let _stmt = ResetStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_reset_time_zone() {
        let mut input = crate::tokens::test_input("RESET TIME ZONE");
        let _stmt = ResetStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_role_default() {
        let mut input = crate::tokens::test_input("SET ROLE DEFAULT");
        let _stmt = SetRoleStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_role_none() {
        let mut input = crate::tokens::test_input("SET ROLE NONE");
        let _stmt = SetRoleStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_role_name() {
        let mut input = crate::tokens::test_input("SET ROLE alice");
        let _stmt = SetRoleStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_local_role() {
        let mut input = crate::tokens::test_input("SET LOCAL ROLE alice");
        let _stmt = SetRoleStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_session_authorization_default() {
        let mut input = crate::tokens::test_input("SET SESSION AUTHORIZATION DEFAULT");
        let _stmt = SetSessionAuthStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_session_authorization_string() {
        let mut input = crate::tokens::test_input("SET SESSION AUTHORIZATION 'alice'");
        let _stmt = SetSessionAuthStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_time_zone_string() {
        let mut input = crate::tokens::test_input("SET TIME ZONE 'UTC'");
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_time_zone_negative() {
        let mut input = crate::tokens::test_input("SET TIME ZONE -8");
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_time_zone_default() {
        let mut input = crate::tokens::test_input("SET TIME ZONE DEFAULT");
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_show_param() {
        let mut input = crate::tokens::test_input("SHOW TimeZone");
        let _stmt = ShowStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_show_ident() {
        let mut input = crate::tokens::test_input("SHOW transaction_read_only");
        let _stmt = ShowStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_show_all() {
        let mut input = crate::tokens::test_input("SHOW ALL");
        let _stmt = ShowStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_show_time_zone() {
        let mut input = crate::tokens::test_input("SHOW TIME ZONE");
        let _stmt = ShowStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_show_transaction_isolation_level() {
        let mut input = crate::tokens::test_input("SHOW TRANSACTION ISOLATION LEVEL");
        let _stmt = ShowStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_set_time_zone_local() {
        let mut input = crate::tokens::test_input("SET TIME ZONE LOCAL");
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }
}
