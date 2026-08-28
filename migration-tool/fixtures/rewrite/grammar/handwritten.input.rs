use crate::__firstset::*;
pub struct SqlRules;
pub mod __firstset { include!("generated/first_set.rs"); }

// These five legacy Parse implementations are replaced by reviewed Node and
// token-target declarations; the surrounding explanation remains in place.
impl Parse for StringLitSeq0 { fn parse() {} }
impl Parse for CustomOp { fn parse() {} }
impl Parse for UnquotedIdent { fn parse() {} }
impl Parse for BareAliasName { fn parse() {} }
impl Parse for RestOfLine { fn parse() {} }

pub fn retained_helper() {}
