/// EXPLAIN statement AST.
use crate::tokens::literal;

/// An explain option value: ON, OFF, TRUE, FALSE, numeric, string, or identifier.
///
/// Per PG's `explain_option_arg` rule (gram.y), the value is an
/// `opt_boolean_or_string` / `NumericOnly`, so it accepts `ON`/`OFF`,
/// `TRUE`/`FALSE`, bare identifiers, numeric literals, and string literals
/// (e.g. `format 'json'`).
#[derive(recursa::Node, Debug)]
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

/// A single explain option: `name value` (e.g., `costs off`); gram.y
/// `utility_option_elem`, whose name is `utility_option_name`.
#[derive(recursa::Node, Debug)]
pub struct ExplainOption<'input> {
    pub name: literal::UtilityOptionName<'input>,
    pub value: Option<ExplainOptValue<'input>>,
}

/// Explain options: `(opt, ...)`.
#[derive(recursa::Node, Debug, derive_more::Deref)]
pub struct ExplainOptions<'input> {
    /// The shared parenthesis markers: after `EXPLAIN` a `(` opens either
    /// this list or a parenthesized query, and both reduce the same marker
    /// before the next token decides.
    pub open: crate::ast::shared::expr::ParenthesizedOpen,
    #[sep(COMMA)]
    #[deref]
    pub options: recursa::ArenaVec1<'input, ExplainOption<'input>>,
    pub close: crate::ast::shared::expr::ParenthesizedClose,
}

/// A statement that `EXPLAIN` accepts: gram.y `ExplainableStmt`.
///
/// Variant order mirrors `Statement`: `CREATE MATERIALIZED VIEW` before
/// `CREATE TABLE`, the DML and utility forms have disjoint leading keywords,
/// and `Query` (`SELECT`, `WITH`, `VALUES`, `TABLE`, parenthesized, and set
/// operations) comes last as the shared-prefix form. gram.y's `CreateAsStmt`
/// maps to `CreateTableStmt`, whose body carries the CTAS forms.
#[derive(recursa::Node, Debug)]
pub enum ExplainableStmt<'input> {
    CreateMaterializedView(
        recursa::ArenaBox<
            'input,
            crate::ast::ddl::materialized_view::CreateMaterializedViewStmt<'input>,
        >,
    ),
    CreateTable(recursa::ArenaBox<'input, crate::ast::ddl::table::CreateTableStmt<'input>>),
    Insert(recursa::ArenaBox<'input, crate::ast::dml::insert::InsertStmt<'input>>),
    Update(recursa::ArenaBox<'input, crate::ast::dml::update::UpdateStmt<'input>>),
    Merge(recursa::ArenaBox<'input, crate::ast::dml::merge::MergeStmt<'input>>),
    Delete(recursa::ArenaBox<'input, crate::ast::dml::delete::DeleteStmt<'input>>),
    Execute(crate::ast::tcl::prepared::ExecuteStmt<'input>),
    Refresh(crate::ast::utility::refresh::RefreshStmt<'input>),
    Declare(crate::ast::cursor::declare::DeclareStmt<'input>),
    Query(recursa::ArenaBox<'input, crate::ast::dml::values::QueryBody<'input>>),
    /// `WITH ...` before a query or a DML statement, factored as in `Statement`.
    With(recursa::ArenaBox<'input, crate::ast::shared::with_clause::WithStatement<'input>>),
}

/// An EXPLAIN option list followed by the statement being explained.
///
/// Keeping the optional prefix and required statement in one enum branch lets
/// the LR state after the option-list delimiter distinguish this form from a
/// parenthesized statement.
#[derive(recursa::Node, Debug)]
pub struct ExplainOptionsAndStatement<'input> {
    pub options: ExplainOptions<'input>,
    pub statement: recursa::ArenaBox<'input, ExplainableStmt<'input>>,
}

/// The input following `EXPLAIN`, with or without an option list.
#[derive(recursa::Node, Debug)]
pub enum ExplainInput<'input> {
    WithOptions(ExplainOptionsAndStatement<'input>),
    Statement(recursa::ArenaBox<'input, ExplainableStmt<'input>>),
}

/// EXPLAIN statement: `EXPLAIN [(options)] statement`.
#[derive(recursa::Node, Debug)]
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
