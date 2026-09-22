/// SET/RESET statement AST.
use crate::tokens::literal;

recursa::ast_node! {
    /// The value in a SET statement: literal, keyword, or identifier.
    ///
    /// Variant ordering: NumericLit before IntegerLit so `77.7` is consumed as a
    /// numeric literal (longest-match-wins).
    #[derive(Debug)]
    pub enum SetValue {
        #[tok(ON)]
        On,
        #[tok(FALSE)]
        False,
        #[tok(TRUE)]
        True,
        #[tok(DEFAULT)]
        Default,
        StringLit(literal::StringLit),
        SignedNumeric(SignedNumericLit),
        NumericLit(literal::NumericLit),
        IntegerLit(literal::IntegerLit),
        Ident(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// A numeric literal with an optional leading sign: `-1`, `+1.5`, `2`.
    ///
    /// Used in positions like `SET extra_float_digits = -1` where a full `Expr`
    /// is overkill and would admit keywords that shouldn't be legal values.
    #[derive(Debug)]
    pub struct SignedNumericLit {
        pub sign: NumericSign,
        pub value: UnsignedNumericLit,
    }
}

recursa::ast_node! {
    /// Leading `-` or `+` sign of a signed numeric literal.
    #[derive(Debug)]
    pub enum NumericSign {
        #[tok(MINUS)]
        Neg,
        #[tok(PLUS)]
        Pos,
    }
}

recursa::ast_node! {
    /// Either a numeric (with decimal point / exponent) or an integer literal.
    #[derive(Debug)]
    pub enum UnsignedNumericLit {
        Numeric(literal::NumericLit),
        Integer(literal::IntegerLit),
    }
}

recursa::ast_node! {
    /// The separator between param and value: TO or =.
    #[derive(Debug)]
    pub enum SetSep {
        #[tok(TO)]
        To,
        #[tok(EQ)]
        Eq,
    }
}

recursa::ast_node! {
    /// The generic `set_rest_more` form shared by top-level and nested `SET`
    /// statements.
    ///
    /// This deliberately has no leading `SET` or scope. PostgreSQL's
    /// `VariableSetStmt` owns those literal prefixes, before it enters
    /// `set_rest`; keeping the same boundary prevents `SET SESSION
    /// CHARACTERISTICS` from reducing `SESSION` as a scope before
    /// `CHARACTERISTICS` can decide the rest.
    #[derive(Debug)]
    pub struct GenericSetRest {
        pub param: crate::ast::shared::names::QualifiedName,
        pub sep: SetSep,
        #[sep(COMMA)]
        pub values: one_or_many!(SetValue),
    }
}

#[cfg(feature = "since-pg19")]
recursa::ast_node! {
    /// `param TO NULL` / `param = NULL` — the two `generic_set` arms that 19
    /// adds (gram.y b73d13c:1727, 1737; research PostgreSQL 19, "Changes to
    /// existing statements", commit ff4597acd). `NULL` is one value, never
    /// an element of a list.
    #[derive(Debug)]
    pub struct GenericSetNull {
        pub param: crate::ast::shared::names::QualifiedName,
        #[tok(this, NULL)]
        pub sep: SetSep,
    }
}

#[cfg(feature = "since-pg19")]
recursa::ast_node! {
    /// Nested `SET param TO NULL`, the 19 twin of [`SetStmt`].
    #[derive(Debug)]
    #[tok(SET, this)]
    pub struct SetNullStmt {
        pub rest: GenericSetNull,
    }
}

recursa::ast_node! {
    /// Generic nested `SET`: `SET param TO|= value [, value ...]`.
    ///
    /// `FunctionSetResetClause` and `SetResetClause` use PostgreSQL's
    /// `set_rest_more`, which does not permit `LOCAL` or `SESSION`.
    #[derive(Debug)]
    #[tok(SET, this)]
    pub struct SetStmt {
        pub rest: GenericSetRest,
    }
}

recursa::ast_node! {
    /// Role target in `SET ROLE`: role name, `NONE`, or `DEFAULT`.
    #[derive(Debug)]
    pub enum SetRoleTarget {
        #[tok(DEFAULT)]
        Default,
        Role(crate::tokens::ColId),
        String(literal::StringLit),
    }
}

recursa::ast_node! {
    /// `ROLE { rolename | NONE | DEFAULT }`, the `set_rest_more` role form.
    ///
    /// The `SET ROLE TO ...` spelling is accepted through [`SetStmt`], matching
    /// PostgreSQL's `generic_set` route. Keeping `TO` out of this dedicated form
    /// prevents the two AST alternatives from recognizing the same token stream.
    #[derive(Debug)]
    pub struct SetRoleStmt {
        #[tok(ROLE, this)]
        pub target: SetRoleTarget,
    }
}

recursa::ast_node! {
    /// Role target in `SET SESSION AUTHORIZATION`.
    #[derive(Debug)]
    pub enum SetSessionAuthTarget {
        #[tok(DEFAULT)]
        Default,
        String(literal::StringLit),
        Role(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// `SESSION AUTHORIZATION { rolename | DEFAULT }`, the `set_rest_more`
    /// session-authorisation form.
    ///
    /// The enclosing [`VariableSetStmt`] supplies any `LOCAL` or `SESSION`
    /// prefix. This also retains PostgreSQL's `SET SESSION SESSION
    /// AUTHORIZATION` spelling.
    #[derive(Debug)]
    pub struct SetSessionAuthStmt {
        #[tok(SESSION, AUTHORIZATION, this)]
        pub target: SetSessionAuthTarget,
    }
}

recursa::ast_node! {
    /// A signed numeric literal: `[-]numeric | [-]integer`.
    ///
    /// Variant ordering: Numeric before Integer (longest-match-wins for `7.5`).
    #[derive(Debug)]
    pub enum SignedNumber {
        Numeric(SignedNumeric),
        Integer(SignedInteger),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SignedNumeric {
        #[presence(MINUS)]
        pub negative: bool,
        pub value: literal::NumericLit,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SignedInteger {
        #[presence(MINUS)]
        pub negative: bool,
        pub value: literal::IntegerLit,
    }
}

recursa::ast_node! {
    /// Target of `SET TIME ZONE`.
    ///
    /// Variant ordering: `LOCAL` and `DEFAULT` (keywords) before `Number` and
    /// `String`. INTERVAL form is deliberately skipped.
    #[derive(Debug)]
    pub enum SetTimeZoneTarget {
        #[tok(LOCAL)]
        Local,
        #[tok(DEFAULT)]
        Default,
        Number(SignedNumber),
        String(literal::StringLit),
    }
}

recursa::ast_node! {
    /// `TIME ZONE { signed_number | string | LOCAL | DEFAULT }`, the
    /// `set_rest_more` time-zone form.
    #[derive(Debug)]
    pub struct SetTimeZoneStmt {
        #[tok(TIME, ZONE, this)]
        pub target: SetTimeZoneTarget,
    }
}

recursa::ast_node! {
    /// Value of `SET XML OPTION`: `DOCUMENT` or `CONTENT`.
    #[derive(Debug)]
    pub enum SetXmlOptionValue {
        #[tok(DOCUMENT)]
        Document,
        #[tok(CONTENT)]
        Content,
    }
}

recursa::ast_node! {
    /// `SET XML OPTION { DOCUMENT | CONTENT }` — sets the
    /// `xmloption` GUC. Special-cased in PG's gram.y (`VariableSetStmt:
    /// SET set_rest_more`'s `XML OPTION document_or_content` form).
    #[derive(Debug)]
    pub struct SetXmlOptionStmt {
        #[tok(XML, OPTION, this)]
        pub value: SetXmlOptionValue,
    }
}

recursa::ast_node! {
    /// PostgreSQL's `set_rest`: the statements allowed after each literal
    /// `SET`, `SET LOCAL`, or `SET SESSION` prefix.
    #[derive(Debug)]
    pub enum VariableSetRest {
        Transaction(crate::ast::tcl::transaction::SetTransactionRest),
        SessionCharacteristics(crate::ast::tcl::transaction::SetSessionCharacteristicsRest),
        Role(SetRoleStmt),
        SessionAuthorization(SetSessionAuthStmt),
        TimeZone(SetTimeZoneStmt),
        XmlOption(SetXmlOptionStmt),
        // `QualifiedName` admits several special-form keywords, so keep this
        // fallback after their literal-leading branches.
        Generic(GenericSetRest),
        /// Added in 19: `generic_set: var_name TO NULL_P` (b73d13c:1727).
        #[cfg(feature = "since-pg19")]
        GenericNull(GenericSetNull),
    }
}

recursa::ast_node! {
    /// PostgreSQL's `VariableSetStmt`.
    ///
    /// The three prefixes intentionally own their literal scope keywords rather
    /// than reducing through a `SetScope` nonterminal. This is gram.y's shape:
    /// after `SET SESSION`, `CHARACTERISTICS` can continue the unscoped
    /// `set_rest`, while every other following token enters the scoped one.
    #[derive(Debug)]
    pub enum VariableSetStmt {
        Unscoped(SetUnscoped),
        Local(SetLocal),
        Session(SetSession),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SetUnscoped {
        #[tok(SET, this)]
        pub rest: VariableSetRest,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SetLocal {
        #[tok(SET, LOCAL, this)]
        pub rest: VariableSetRest,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct SetSession {
        #[tok(SET, SESSION, this)]
        pub rest: VariableSetRest,
    }
}

recursa::ast_node! {
    /// Target of a RESET statement.
    ///
    /// Variant ordering: multi-token variants before single-token variants.
    #[derive(Debug)]
    pub enum ResetTarget {
        #[tok(SESSION, AUTHORIZATION)]
        SessionAuth,
        #[tok(TIME, ZONE)]
        TimeZone,
        #[tok(ALL)]
        All,
        Ident(crate::ast::shared::names::QualifiedName),
    }
}

recursa::ast_node! {
    /// RESET statement: `RESET { param | ALL | ROLE | SESSION AUTHORIZATION | TIME ZONE }`.
    #[derive(Debug)]
    pub struct ResetStmt {
        #[tok(RESET, this)]
        pub target: ResetTarget,
    }
}

// --- SHOW ---

recursa::ast_node! {
    /// Target of a SHOW statement.
    ///
    /// Variant ordering: multi-token targets before single-token `Param`
    /// fallback so the specific forms are matched first.
    #[derive(Debug)]
    pub enum ShowTarget {
        #[tok(TRANSACTION, ISOLATION, LEVEL)]
        TransactionIsolationLevel,
        #[tok(SESSION, AUTHORIZATION)]
        SessionAuthorization,
        #[tok(TIME, ZONE)]
        TimeZone,
        #[tok(ALL)]
        All,
        Param(crate::ast::shared::names::QualifiedName),
    }
}

recursa::ast_node! {
    /// SHOW statement: `SHOW { name | ALL | TIME ZONE | SESSION AUTHORIZATION | TRANSACTION ISOLATION LEVEL }`.
    #[derive(Debug)]
    pub struct ShowStmt {
        #[tok(SHOW, this)]
        pub target: ShowTarget,
    }
}

recursa::ast_node! {
    /// LOAD statement: `LOAD 'filename'` — forces loading of a shared library.
    #[derive(Debug)]
    pub struct LoadStmt {
        #[tok(LOAD, this)]
        pub filename: literal::StringLit,
    }
}
