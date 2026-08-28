impl<'input> Parse<'input> for OtherRestOfLine<'input> {
    fn parse(input: &mut Input<'input>) -> Result<Self, ParseError> {
        parse_raw_source_remainder(input)
    }
}
