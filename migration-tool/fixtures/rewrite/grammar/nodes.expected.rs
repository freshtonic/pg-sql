
#[parse(skip)]
pub type NameList<'input> = Vec<Name<'input>>;

// The declaration comment and field order must survive the rewrite.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectStmt<'input> {
    #[kwd(SELECT)]
    #[surrounded(LPAREN, this, RPAREN)]
    #[sep(COMMA, trailing)]
    pub items:  recursa::Vec1<Item<'input>  > ,
    pub alias: Alias<'input>,
    #[sep(COMMA)]
    pub values: Vec<Value<'input> >,
    #[surrounded(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub nested: Option< Vec<Value<'input> > > ,
    #[kwd(UNIQUE)]
    pub unique: bool,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum Choice<'input> {
    #[kwd(ONLY)]
    Only,
    #[kwd(AS)]
    Named(Name<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
#[pratt]
pub enum Expr {
    #[parse(prefix, bp = 12)]
    #[kwd(NOT)]
    Not(Box<Self>),
    #[parse(infix, lbp = 10, rbp = 11)]
    Add(Box<Self>, #[tok(PLUS)] Box<Self>),
    #[parse(postfix, bp = 20)]
    Factorial(#[tok(BANG)] Box<Self>),
}
