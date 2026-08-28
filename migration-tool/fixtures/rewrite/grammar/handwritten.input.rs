use crate::__firstset::*;
pub struct SqlRules;
pub mod __firstset { include!("generated/first_set.rs"); }

// Four legacy Parse implementations have reviewed declarations. The generic
// RestOfLine parser is removed; scanner-correct psql directives belong to #12.
// The surrounding explanation remains in place.
impl Parse for StringLitSeq0 { fn parse() {} }
impl<'input> Parse<'input> for CustomOp<'input> { fn parse() {} }
impl<'input> Parse<'input> for UnquotedIdent<'input> { fn parse() {} }
impl<'input> Parse<'input> for BareAliasName<'input> { fn parse() {} }
impl<'input> Parse<'input> for RestOfLine<'input> { fn parse() {} }

pub fn retained_helper() {}
