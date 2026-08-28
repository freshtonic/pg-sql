#[derive(recursa::Node)]
#[recursa::parser(postcondition = crate::reject)]
pub struct Bad<'input>(Word<'input>);
