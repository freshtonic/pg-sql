//! DO anonymous code block.

use crate::tokens::literal;

// --- DO ---

recursa::ast_node! {
    /// `DO [LANGUAGE lang] $$ ... $$` anonymous code block.
    #[derive(Debug)]
    #[tok(DO, this)]
    pub struct DoStmt {
        pub language: Option<DoLanguage>,
        #[lex(matcher)]
        pub body: literal::DollarStringLit,
        pub trailing_language: Option<DoLanguage>,
    }
}

recursa::ast_node! {
    /// `LANGUAGE lang` clause on a `DO` block (may appear before or after body).
    #[derive(Debug)]
    pub struct DoLanguage {
        #[tok(LANGUAGE, this)]
        pub name: crate::tokens::ColId,
    }
}
