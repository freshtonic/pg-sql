/// EXPLAIN statement AST.
use crate::tokens::literal;

recursa::ast_node! {
    /// An explain option value: ON, OFF, TRUE, FALSE, numeric, string, or identifier.
    ///
    /// Per PG's `explain_option_arg` rule (gram.y), the value is an
    /// `opt_boolean_or_string` / `NumericOnly`, so it accepts `ON`/`OFF`,
    /// `TRUE`/`FALSE`, bare identifiers, numeric literals, and string literals
    /// (e.g. `format 'json'`).
    #[derive(Debug)]
    pub enum ExplainOptValue {
        #[tok(ON)]
        On,
        #[tok(TRUE)]
        True,
        #[tok(FALSE)]
        False,
        // Numeric literal (e.g. `WAL on, ROWS 100`). `NumericLit` requires a
        // decimal/exponent (longer match), so it must come before `IntegerLit`.
        Numeric(literal::NumericLit),
        Integer(literal::IntegerLit),
        // Quoted string (e.g. `FORMAT 'json'`).
        String(literal::StringLit),
        Ident(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// A single explain option: `name value` (e.g., `costs off`); gram.y
    /// `utility_option_elem`, whose name is `utility_option_name`.
    #[derive(Debug)]
    pub struct ExplainOption {
        pub name: literal::UtilityOptionName,
        pub value: Option<ExplainOptValue>,
    }
}

recursa::ast_node! {
    /// Explain options: `(opt, ...)`.
    #[derive(Debug, derive_more :: Deref)]
    pub struct ExplainOptions {
        /// The shared parenthesis markers: after `EXPLAIN` a `(` opens either
        /// this list or a parenthesized query, and both reduce the same marker
        /// before the next token decides.
        pub open: crate::ast::shared::expr::ParenthesizedOpen,
        #[sep(COMMA)]
        #[deref]
        pub options: one_or_many!(ExplainOption),
        pub close: crate::ast::shared::expr::ParenthesizedClose,
    }
}

recursa::ast_node! {
    /// A statement that `EXPLAIN` accepts: gram.y `ExplainableStmt`.
    ///
    /// Variant order mirrors `Statement`: `CREATE MATERIALIZED VIEW` before
    /// `CREATE TABLE`, the DML and utility forms have disjoint leading keywords,
    /// and `Query` (`SELECT`, `WITH`, `VALUES`, `TABLE`, parenthesized, and set
    /// operations) comes last as the shared-prefix form. gram.y's `CreateAsStmt`
    /// maps to `CreateTableStmt`, whose body carries the CTAS forms.
    #[derive(Debug)]
    pub enum ExplainableStmt {
        CreateMaterializedView(
            boxed!(crate::ast::ddl::materialized_view::CreateMaterializedViewStmt),
        ),
        CreateTable(boxed!(crate::ast::ddl::table::CreateTableStmt)),
        Insert(boxed!(crate::ast::dml::insert::InsertStmt)),
        Update(boxed!(crate::ast::dml::update::UpdateStmt)),
        Merge(boxed!(crate::ast::dml::merge::MergeStmt)),
        Delete(boxed!(crate::ast::dml::delete::DeleteStmt)),
        Execute(crate::ast::tcl::prepared::ExecuteStmt),
        Refresh(crate::ast::utility::refresh::RefreshStmt),
        Declare(crate::ast::cursor::declare::DeclareStmt),
        Query(boxed!(crate::ast::dml::values::QueryBody)),
        /// `WITH ...` before a query or a DML statement, factored as in `Statement`.
        With(boxed!(crate::ast::shared::with_clause::WithStatement)),
    }
}

recursa::ast_node! {
    /// An EXPLAIN option list followed by the statement being explained.
    ///
    /// Keeping the optional prefix and required statement in one enum branch lets
    /// the LR state after the option-list delimiter distinguish this form from a
    /// parenthesized statement.
    #[derive(Debug)]
    pub struct ExplainOptionsAndStatement {
        pub options: ExplainOptions,
        pub statement: boxed!(ExplainableStmt),
    }
}

recursa::ast_node! {
    /// The input following `EXPLAIN`, with or without an option list.
    #[derive(Debug)]
    pub enum ExplainInput {
        WithOptions(ExplainOptionsAndStatement),
        Statement(boxed!(ExplainableStmt)),
    }
}

recursa::ast_node! {
    /// EXPLAIN statement: `EXPLAIN [(options)] statement`.
    #[derive(Debug)]
    #[tok(EXPLAIN, this)]
    pub struct ExplainStmt {
        pub input: ExplainInput,
    }
}

impl<'input> ExplainStmt<'input> {
    /// Returns the option list when the statement includes one.
    pub const fn options(&self) -> Option<&ExplainOptions<'input>> {
        match &self.input {
            ExplainInput::WithOptions(value) => Some(&value.options),
            ExplainInput::Statement(_) => None,
        }
    }

    /// Returns the statement being explained.
    pub fn statement(&self) -> &ExplainableStmt<'input> {
        match &self.input {
            ExplainInput::WithOptions(value) => &value.statement,
            ExplainInput::Statement(statement) => statement,
        }
    }
}
