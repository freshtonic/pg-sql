/// SET/RESET statement AST.
use recursa::seq::Seq0;

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Scope of a SET statement: `SESSION` or `LOCAL`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetScope {
    #[tok(SESSION)] Session,
    #[tok(LOCAL)] Local,
}

/// The value in a SET statement: literal, keyword, or identifier.
///
/// Variant ordering: NumericLit before IntegerLit so `77.7` is consumed as a
/// numeric literal (longest-match-wins).
#[derive(recursa::Node, Debug, Clone)]
pub enum SetValue<'input> {
    #[tok(ON)] On,
    #[tok(OFF)] Off,
    #[tok(FALSE)] False,
    #[tok(TRUE)] True,
    #[tok(DEFAULT)] Default,
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
    #[tok(MINUS)] Neg,
    #[tok(PLUS)] Pos,
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
    #[tok(TO)] To,
    #[tok(EQ)] Eq,
}

/// Plain SET statement: `SET [SESSION|LOCAL] param TO|= value [, value ...]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetStmt<'input> {
    #[tok(SET, this)]
    pub scope: Option<SetScope>,
    pub param: crate::ast::shared::names::QualifiedName<'input>,
    pub sep: SetSep,
    #[sep(COMMA)]
    pub values: Vec<SetValue<'input> >,
}

/// Role target in `SET ROLE`: role name, `NONE`, or `DEFAULT`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetRoleTarget<'input> {
    #[tok(NONE)] None,
    #[tok(DEFAULT)] Default,
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
#[derive(recursa::Node, Debug, Clone)]
pub struct SetRoleStmt<'input> {
    #[tok(SET, this)]
    pub scope: Option<SetScope>,
    #[tok(ROLE, optional(TO), this)]
    pub target: SetRoleTarget<'input>,
}

/// Role target in `SET SESSION AUTHORIZATION`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetSessionAuthTarget<'input> {
    #[tok(DEFAULT)] Default,
    String(literal::StringLit<'input>),
    Role(literal::AliasName<'input>),
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
    #[tok(LOCAL)] Local,
    #[tok(DEFAULT)] Default,
    Number(SignedNumber<'input>),
    String(literal::StringLit<'input>),
}

/// `SET [SESSION|LOCAL] TIME ZONE { signed_number | string | LOCAL | DEFAULT }`
#[derive(recursa::Node, Debug, Clone)]
pub struct SetTimeZoneStmt<'input> {
    #[tok(SET, this)]
    pub scope: Option<SetScope>,
    #[tok(TIME, ZONE, this)]
    pub target: SetTimeZoneTarget<'input>,
}

/// Value of `SET XML OPTION`: `DOCUMENT` or `CONTENT`.
#[derive(recursa::Node, Debug, Clone)]
pub enum SetXmlOptionValue {
    #[tok(DOCUMENT)] Document,
    #[tok(CONTENT)] Content,
}

/// `SET XML OPTION { DOCUMENT | CONTENT }` — sets the
/// `xmloption` GUC. Special-cased in PG's gram.y (`VariableSetStmt:
/// SET set_rest_more`'s `XML OPTION document_or_content` form).
#[derive(recursa::Node, Debug, Clone)]
pub struct SetXmlOptionStmt {
    #[tok(SET, this)]
    pub scope: Option<SetScope>,
    #[tok(XML, OPTION, this)]
    pub value: SetXmlOptionValue,
}

/// Target of a RESET statement.
///
/// Variant ordering: multi-token variants before single-token variants.
#[derive(recursa::Node, Debug, Clone)]
pub enum ResetTarget<'input> {
    #[tok(SESSION, AUTHORIZATION)] SessionAuth,
    #[tok(TIME, ZONE)] TimeZone,
    #[tok(ROLE)] Role,
    #[tok(ALL)] All,
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
    #[tok(TRANSACTION, ISOLATION, LEVEL)] TransactionIsolationLevel,
    #[tok(SESSION, AUTHORIZATION)] SessionAuthorization,
    #[tok(TIME, ZONE)] TimeZone,
    #[tok(ALL)] All,
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

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::session::set_reset::{
        ResetStmt, SetRoleStmt, SetSessionAuthStmt, SetStmt, SetTimeZoneStmt, ShowStmt,
    };

    #[test]
    fn parse_set_to() {
        let lexed = crate::tokens::lex("SET enable_seqscan TO off");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.param.object(), "enable_seqscan");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_eq() {
        let lexed = crate::tokens::lex("SET enable_sort = false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.param.object(), "enable_sort");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_integer_value() {
        let lexed = crate::tokens::lex("SET work_mem = 4096");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.values.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_numeric_value() {
        let lexed = crate::tokens::lex("SET seq_page_cost = 1.5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.values.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_multi_value() {
        let lexed = crate::tokens::lex("SET search_path TO public, pg_catalog");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.values.len(), 2);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_scope() {
        let lexed = crate::tokens::lex("SET SESSION enable_seqscan TO off");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = SetStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.scope.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset() {
        let lexed = crate::tokens::lex("RESET enable_seqscan");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let _ = stmt;
    }

    #[test]
    fn parse_reset_all() {
        let lexed = crate::tokens::lex("RESET ALL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_role() {
        let lexed = crate::tokens::lex("RESET ROLE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_session_authorization() {
        let lexed = crate::tokens::lex("RESET SESSION AUTHORIZATION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_reset_time_zone() {
        let lexed = crate::tokens::lex("RESET TIME ZONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ResetStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_default() {
        let lexed = crate::tokens::lex("SET ROLE DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_none() {
        let lexed = crate::tokens::lex("SET ROLE NONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_role_name() {
        let lexed = crate::tokens::lex("SET ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_local_role() {
        let lexed = crate::tokens::lex("SET LOCAL ROLE alice");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetRoleStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_authorization_default() {
        let lexed = crate::tokens::lex("SET SESSION AUTHORIZATION DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetSessionAuthStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_session_authorization_string() {
        let lexed = crate::tokens::lex("SET SESSION AUTHORIZATION 'alice'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetSessionAuthStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_string() {
        let lexed = crate::tokens::lex("SET TIME ZONE 'UTC'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_negative() {
        let lexed = crate::tokens::lex("SET TIME ZONE -8");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_default() {
        let lexed = crate::tokens::lex("SET TIME ZONE DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_param() {
        let lexed = crate::tokens::lex("SHOW TimeZone");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_ident() {
        let lexed = crate::tokens::lex("SHOW transaction_read_only");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_all() {
        let lexed = crate::tokens::lex("SHOW ALL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_time_zone() {
        let lexed = crate::tokens::lex("SHOW TIME ZONE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_show_transaction_isolation_level() {
        let lexed = crate::tokens::lex("SHOW TRANSACTION ISOLATION LEVEL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = ShowStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_set_time_zone_local() {
        let lexed = crate::tokens::lex("SET TIME ZONE LOCAL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SetTimeZoneStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
