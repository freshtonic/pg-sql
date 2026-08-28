use recursa::seq::{Seq0, Seq1, OptionalTrailing};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

#[recursa::ast]
pub type NameList<'input> = Vec<Name<'input>>;

// The declaration comment and field order must survive the rewrite.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules)]
pub struct SelectStmt<'input> {
    pub select: SELECT,
    pub items: Surrounded<punct::LParen, Seq1<Item<'input>, punct::Comma, OptionalTrailing>, punct::RParen>,
    pub r#as: Option<AS>,
    pub alias: Name<'input>,
    pub values: Seq0<Value<'input>, punct::Comma>,
    pub nested: Option<Surrounded<punct::LParen, Seq0<Value<'input>, punct::Comma>, punct::RParen>> ,
    pub unique: Option<UNIQUE>,
}

#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum Choice<'input> {
    Only(ONLY),
    Named((AS, Name<'input>)),
}

#[derive(FormatTokens, Visit, Transform, Debug, Clone)]
#[recursa::parser(rules = SqlRules, pratt)]
pub enum Expr {
    #[parse(prefix, bp = 12)]
    Not(NOT, Box<Self>),
    #[parse(infix, bp = 10)]
    Add((Box<Self>, punct::Plus, Box<Self>)),
    #[parse(postfix, bp = 20)]
    Factorial((Box<Self>, punct::Bang)),
}
