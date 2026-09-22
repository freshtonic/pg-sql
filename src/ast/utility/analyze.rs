/// ANALYZE statement AST: `ANALYZE [table [(col, ...)]]`.
use crate::tokens::literal;

recursa::ast_node! {
    /// ANALYZE statement with optional qualified table name and column list.
    ///
    /// ```sql
    /// ANALYZE [VERBOSE] [table_name [(column, ...)]]
    /// ```
    #[derive(Debug)]
    #[tok(ANALYZE, this)]
    pub struct AnalyzeStmt {
        #[presence(VERBOSE)]
        /// Optional `VERBOSE` keyword (legacy bareword form).
        pub verbose: bool,
        /// Optional parenthesized options list, e.g.
        /// `(VERBOSE, SKIP_LOCKED, BUFFER_USAGE_LIMIT '512 kB')`.
        pub options: Option<AnalyzeOptions>,
        #[sep(COMMA)]
        pub targets: Option<one_or_many!(AnalyzeTarget)>,
    }
}

recursa::ast_node! {
    /// Parenthesized options owned as one comma-separated list.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct AnalyzeOptions(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(AnalyzeOption),
    );
}

recursa::ast_node! {
    /// One option inside the parenthesized `ANALYZE (...)` options list.
    ///
    /// Each option is a keyword-ish name (so we use `AliasName` to tolerate
    /// identifiers that happen to collide with keywords) followed by an optional
    /// value (string literal, integer, or ON/OFF-style AliasName).
    #[derive(Debug)]
    pub struct AnalyzeOption {
        pub name: literal::AliasName,
        pub value: Option<AnalyzeOptionValue>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum AnalyzeOptionValue {
        // Use the canonical content token from `tokens::literal`. Keeping an
        // inline lexer declaration here would create a second `StringLit`
        // definition and prevent lookahead filters from naming it uniquely.
        String(literal::StringLit),
        Integer(literal::IntegerLit),
        Name(literal::AliasName),
    }
}

recursa::ast_node! {
    /// Optional parenthesized column list on an ANALYZE target.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct AnalyzeColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
    );
}

recursa::ast_node! {
    /// `table_name [(column, ...)]` target of an ANALYZE statement.
    #[derive(Debug)]
    pub struct AnalyzeTarget {
        #[cfg(not(feature = "since-pg18"))]
        pub table_name: crate::ast::shared::names::QualifiedName,
        /// From 18 gram.y `vacuum_relation` names a `relation_expr`, so
        /// `ONLY name` and `name *` are accepted
        /// (docs/research/postgres-14-19-sql-syntax-changes.md, PostgreSQL 18,
        /// item 9; REL_18_6 gram.y `vacuum_relation`).
        #[cfg(feature = "since-pg18")]
        pub table_name: crate::ast::shared::names::RelationExpr,
        pub columns: Option<AnalyzeColumnList>,
    }
}

impl<'input> AnalyzeTarget<'input> {
    /// The name of the target relation. From 18 the target is a
    /// `relation_expr`, and this is the name inside it.
    pub fn relation_name(&self) -> &crate::ast::shared::names::QualifiedName<'input> {
        #[cfg(not(feature = "since-pg18"))]
        return &self.table_name;
        #[cfg(feature = "since-pg18")]
        return self.table_name.name();
    }
}
