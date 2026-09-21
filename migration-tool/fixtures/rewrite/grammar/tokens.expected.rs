recursa::grammar! {
    module = crate::grammar,
    keyword_matching = ascii_insensitive,
    max_lookahead = 5,
}

recursa::tokens! {
    categories { UNRESERVED, COL_NAME, TYPE_FUNC_NAME, RESERVED }
    flags { bare_label }
    keywords {
        SELECT => r"SELECT" in RESERVED + bare_label,
        NULL => r"NULL" in RESERVED + bare_label,
        TRUE => r"TRUE" in RESERVED + bare_label,
        FALSE => r"FALSE" in RESERVED + bare_label,
        ROWS => r"ROWS" in UNRESERVED + bare_label,
        RANGE => r"RANGE" in UNRESERVED + bare_label,
        GROUPS => r"GROUPS" in UNRESERVED + bare_label,
    // Soft words retain their declaration position when folded into keywords.
        FORMAT => r"FORMAT" in UNRESERVED + bare_label,
    }
    punctuation { Comma => r",", }
    matchers {
        DollarStringLit => same_delimiter(opener = r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$"),
        NumericLit => next_exclusion(pattern = r"(?:(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\.)", excluded = r"[A-Za-z0-9_]"),
        IntegerLit => next_exclusion(pattern = r"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)", excluded = r"[A-Za-z0-9_]"),
        DollarNum => next_exclusion(pattern = r"\$[0-9]+", excluded = r"[A-Za-z0-9_]"),
        CustomOp => operator_run(
            characters = "-+*/<>=~!@#%^&|?",
            fences = ["/*", "--"],
            trailing = "+-",
            qualifying = "~!@#%^&|?"
        ),
    }
    ignore {
        // Nested comments are excluded from the significant stream at this phase;
        // Recursa #93 retains their immutable gap records before the public document seam.
        BlockComment => nested(opener = "/*", closer = "*/"),
    }
    admissions {
        AllWordKinds = keywords,
        ColId = UNRESERVED + COL_NAME,
        type_function_name = UNRESERVED + TYPE_FUNC_NAME,
        NonReservedWord = UNRESERVED + COL_NAME + TYPE_FUNC_NAME,
        ColLabel = UNRESERVED + COL_NAME + TYPE_FUNC_NAME + RESERVED,
        BareColLabel = bare_label,
        WindowRefName = ColId - { ROWS, RANGE, GROUPS },
        PsqlVariableName = AllWordKinds - { NULL, TRUE, FALSE },
        UnquotedIdent = NonReservedWord,
        BareAliasName = AllWordKinds,
    }
}

#[derive(recursa::Node)]
pub struct SavepointStmt<'input> {
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(ColId))]
    pub name: ColId<'input>,
}

#[derive(recursa::Node)]
pub struct NamedFuncParam<'input> {
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(type_function_name))]
    pub name: type_function_name<'input>,
}

#[derive(recursa::Node)]
pub struct RoleSpec<'input> {
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(NonReservedWord))]
    pub name: NonReservedWord<'input>,
}

#[derive(recursa::Node)]
pub struct DefElem<'input> {
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(ColLabel))]
    pub name: ColLabel<'input>,
}

#[derive(recursa::Node)]
pub enum DeleteTableAlias<'input> {
    Bare(#[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(BareColLabel))] BareColLabel<'input>),
}

#[derive(recursa::Node)]
pub enum Ident<'input> {
    Unquoted(#[lex(pattern = r"[A-Za-z_][A-Za-z0-9_]*", admits(UnquotedIdent))] UnquotedIdent<'input>),
}

#[derive(recursa::Node)]
pub enum AliasName<'input> {
    Bare(#[lex(pattern = r"[A-Za-z_][A-Za-z0-9_]*", admits(BareAliasName))] BareAliasName<'input>),
}

#[derive(recursa::Node)]
pub struct InlineWindowSpec<'input> {
    pub ref_name: Option<WindowRefNameIdent<'input>>,
}

#[derive(recursa::Node)]
pub struct PsqlVariable<'input> {
    #[tok(COLON)]
    pub colon: Colon,
    #[lex(pattern = r#"(?:[A-Za-z_][A-Za-z0-9_]*|'[^']*'|"[^"]*")"#, admits(PsqlVariableName))]
    pub name: PsqlVariableName<'input>,
}

#[derive(recursa::Node)]
pub struct DoStmt<'input> {
    #[lex(matcher)]
    pub body: DollarStringLit<'input>,
}

#[derive(recursa::Node)]
pub struct NumericValue<'input> {
    #[lex(matcher)]
    pub value: NumericLit<'input>,
}

#[derive(recursa::Node)]
pub struct IntegerValue<'input> {
    #[lex(matcher)]
    pub value: IntegerLit<'input>,
}

#[derive(recursa::Node)]
pub struct PositionalParam<'input> {
    #[lex(matcher)]
    pub value: DollarNum<'input>,
}

#[derive(recursa::Node)]
pub struct OperatorExpr<'input> {
    #[lex(matcher)]
    pub operator: CustomOp<'input>,
}
