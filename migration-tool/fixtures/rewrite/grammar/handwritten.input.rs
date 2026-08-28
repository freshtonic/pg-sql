use crate::__firstset::*;
pub struct SqlRules;
pub mod __firstset { include!("generated/first_set.rs"); }

// These five legacy Parse implementations are replaced by reviewed Node and
// token-target declarations; the surrounding explanation remains in place.
impl Parse for StringLitSeq0 { fn parse() {} }
impl<'input> Parse<'input> for CustomOp<'input> { fn parse() {} }
impl<'input> Parse<'input> for UnquotedIdent<'input> { fn parse() {} }
impl<'input> Parse<'input> for BareAliasName<'input> { fn parse() {} }
impl<'input> Parse<'input> for RestOfLine<'input> { fn parse() {} }

pub fn retained_helper() {}
