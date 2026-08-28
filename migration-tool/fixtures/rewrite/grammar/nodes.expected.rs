
#[parse(skip)]
pub type NameList<'input> = Vec<Name<'input>>;

// The declaration comment and field order must survive the rewrite.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectStmt<'input> {
    #[tok(SELECT, LPAREN, this, RPAREN)]
    #[sep(COMMA, trailing)]
    pub items:  recursa::Vec1<Item<'input>  > ,
    #[tok(optional(AS), this)]
    pub alias: Name<'input>,
    #[sep(COMMA)]
    pub values: Vec<Value<'input> >,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub nested: Option< Vec<Value<'input> > > ,
    #[presence(UNIQUE)]
    pub unique: bool,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum Choice<'input> {
    #[tok(ONLY)]
    Only,
    Named(#[tok(AS, this)] Name<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
#[pratt]
pub enum Expr {
    #[parse(prefix, bp = 12)]
    #[tok(NOT)]
    Not(Box<Self>),
    #[parse(infix, lbp = 10, rbp = 11)]
    Add(Box<Self>, #[tok(PLUS)] Box<Self>),
    #[parse(postfix, bp = 20)]
    Factorial(#[tok(this, BANG)] Box<Self>),
}
