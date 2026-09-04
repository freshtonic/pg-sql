/// EXPLAIN statement AST.
use crate::tokens::literal;

/// An explain option value: ON, OFF, TRUE, FALSE, numeric, string, or identifier.
///
/// Per PG's `explain_option_arg` rule (gram.y), the value is an
/// `opt_boolean_or_string` / `NumericOnly`, so it accepts `ON`/`OFF`,
/// `TRUE`/`FALSE`, bare identifiers, numeric literals, and string literals
/// (e.g. `format 'json'`).
#[derive(recursa::Node, Debug, Clone)]
pub enum ExplainOptValue<'input> {
    #[tok(ON)]
    On,
    #[tok(TRUE)]
    True,
    #[tok(FALSE)]
    False,
    // Numeric literal (e.g. `WAL on, ROWS 100`). `NumericLit` requires a
    // decimal/exponent (longer match), so it must come before `IntegerLit`.
    Numeric(literal::NumericLit<'input>),
    Integer(literal::IntegerLit<'input>),
    // Quoted string (e.g. `FORMAT 'json'`).
    String(literal::StringLit<'input>),
    Ident(crate::tokens::ColId<'input>),
}

/// A single explain option: `name value` (e.g., `costs off`).
#[derive(recursa::Node, Debug, Clone)]
pub struct ExplainOption<'input> {
    pub name: literal::AliasName<'input>,
    pub value: Option<ExplainOptValue<'input>>,
}

/// Explain options: `(opt, ...)`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct ExplainOptions<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<ExplainOption<'input>>,
);

/// A statement that `EXPLAIN` accepts: gram.y `ExplainableStmt`.
///
/// Variant order mirrors `Statement`: `CREATE MATERIALIZED VIEW` before
/// `CREATE TABLE`, the DML and utility forms have disjoint leading keywords,
/// and `Query` (`SELECT`, `WITH`, `VALUES`, `TABLE`, parenthesized, and set
/// operations) comes last as the shared-prefix form. gram.y's `CreateAsStmt`
/// maps to `CreateTableStmt`, whose body carries the CTAS forms.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExplainableStmt<'input> {
    CreateMaterializedView(
        Box<crate::ast::ddl::materialized_view::CreateMaterializedViewStmt<'input>>,
    ),
    CreateTable(Box<crate::ast::ddl::table::CreateTableStmt<'input>>),
    Insert(Box<crate::ast::dml::insert::InsertStmt<'input>>),
    Update(Box<crate::ast::dml::update::UpdateStmt<'input>>),
    Merge(Box<crate::ast::dml::merge::MergeStmt<'input>>),
    Delete(Box<crate::ast::dml::delete::DeleteStmt<'input>>),
    Execute(crate::ast::tcl::prepared::ExecuteStmt<'input>),
    Refresh(crate::ast::utility::refresh::RefreshStmt<'input>),
    Declare(crate::ast::cursor::declare::DeclareStmt<'input>),
    Query(Box<crate::ast::dml::values::Subquery<'input>>),
}

/// An EXPLAIN option list followed by the statement being explained.
///
/// Keeping the optional prefix and required statement in one enum branch lets
/// Recursa distinguish this form from a parenthesized statement by the token
/// following the balanced option-list delimiter.
#[derive(recursa::Node, Debug, Clone)]
pub struct ExplainOptionsAndStatement<'input> {
    pub options: ExplainOptions<'input>,
    pub statement: Box<ExplainableStmt<'input>>,
}

/// The input following `EXPLAIN`, with or without an option list.
#[derive(recursa::Node, Debug, Clone)]
pub enum ExplainInput<'input> {
    WithOptions(ExplainOptionsAndStatement<'input>),
    Statement(Box<ExplainableStmt<'input>>),
}

/// EXPLAIN statement: `EXPLAIN [(options)] statement`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(EXPLAIN, this)]
pub struct ExplainStmt<'input> {
    pub input: ExplainInput<'input>,
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
