pub struct Commented<'input> {
    pub seq0: Seq0<Value<'input>, // seq0-line
        punct::Comma>,
    pub seq1: Seq1<Item<'input> /* seq1-item */, punct /* seq1-path */ ::Comma, /* trailing */ OptionalTrailing>,
    pub surrounded: Surrounded<punct::LParen, /* surrounded-inner */ Seq0<Value<'input>, punct::Comma>, punct::RParen>,
    pub nested: Option<Surrounded</* nested-left */ punct::LParen, Seq0<Value<'input>, /* nested-separator */ punct::Comma>, /* nested-right */ punct::RParen>>,
}
