pub struct Commented<'input> {
    #[sep(COMMA)]
    pub seq0: Vec<Value<'input> // seq0-line
        >,
    #[sep(COMMA, trailing)]
    pub seq1: recursa::Vec1<Item<'input> /* seq1-item */  /* seq1-path */  /* trailing */ >,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub surrounded:  /* surrounded-inner */ Vec<Value<'input> > ,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub nested: Option</* nested-left */  Vec<Value<'input> /* nested-separator */ > /* nested-right */ >,
}
