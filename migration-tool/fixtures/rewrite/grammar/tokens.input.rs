recursa::grammar! {
    module = crate::grammar,
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
    }
    // Soft words retain their declaration position when folded into keywords.
    soft_keywords { FORMAT => r"FORMAT" in UNRESERVED + bare_label, }
    punctuation { Comma => r",", }
    literals {
        DollarStringLit<'input>(source) => r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$" with scan_dollar_string,
        NumericLit<'input>(source) => r"(?:(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\.)" with reject_trailing_word,
        IntegerLit<'input>(source) => r"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)" with reject_trailing_word,
        DollarNum<'input>(source) => r"\$[0-9]+" with reject_trailing_word,
    }
    lexer_tokens {
        // Nested comments are excluded from the significant stream at this phase;
        // Recursa #93 retains their immutable gap records before the public document seam.
        BlockComment => r"/\*" with skip_block_comment,
        CustomOp => r"([-+*/<>=~!@#%^&|?]*[~!@#%^&|?][-+*/<>=~!@#%^&|?]*|[-+*/<>=]+[*/<>=])",
    }
    classes { bare_label_keywords = keywords where bare_label }
    targets {
        ColId: literal::Ident admits UNRESERVED, COL_NAME,
        type_function_name: literal::Ident admits UNRESERVED, TYPE_FUNC_NAME,
        NonReservedWord: literal::Ident admits UNRESERVED, COL_NAME, TYPE_FUNC_NAME,
        ColLabel: literal::Ident admits UNRESERVED, COL_NAME, TYPE_FUNC_NAME, RESERVED,
        BareColLabel: literal::Ident admits bare_label_keywords,
    }
}

#[derive(recursa::Node)]
pub struct SavepointStmt<'input> {
    pub name: ColId<'input>,
}

#[derive(recursa::Node)]
pub struct NamedFuncParam<'input> {
    pub name: type_function_name<'input>,
}

#[derive(recursa::Node)]
pub struct RoleSpec<'input> {
    pub name: NonReservedWord<'input>,
}

#[derive(recursa::Node)]
pub struct DefElem<'input> {
    pub name: ColLabel<'input>,
}

#[derive(recursa::Node)]
pub enum DeleteTableAlias<'input> {
    Bare(BareColLabel<'input>),
}

#[derive(recursa::Node)]
pub enum Ident<'input> {
    Unquoted(UnquotedIdent<'input>),
}

#[derive(recursa::Node)]
pub enum AliasName<'input> {
    Bare(BareAliasName<'input>),
}

#[derive(recursa::Node)]
pub struct InlineWindowSpec<'input> {
    pub ref_name: Option<WindowRefNameIdent<'input>>,
}

#[derive(recursa::Node)]
pub struct PsqlVariable<'input> {
    #[tok(COLON)]
    pub colon: Colon,
    pub name: PsqlVariableName<'input>,
}

#[derive(recursa::Node)]
pub struct DoStmt<'input> {
    pub body: DollarStringLit<'input>,
}

#[derive(recursa::Node)]
pub struct NumericValue<'input> {
    pub value: NumericLit<'input>,
}

#[derive(recursa::Node)]
pub struct IntegerValue<'input> {
    pub value: IntegerLit<'input>,
}

#[derive(recursa::Node)]
pub struct PositionalParam<'input> {
    pub value: DollarNum<'input>,
}

#[derive(recursa::Node)]
pub struct OperatorExpr<'input> {
    pub operator: CustomOp<'input>,
}
