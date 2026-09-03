/// SELECT statement AST.
use crate::ast::dml::values::Subquery;
use crate::ast::shared::expr::{
    CastType, DirectSubquery, Expr, FunctionApplicationExpr, FunctionCallApplication, JsonEncoding,
    JsonOnBehavior, JsonPassing, JsonQuotes, JsonWrapper, ParenthesizedClose, ParenthesizedOpen,
    XmlPassingBy,
};
use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

/// A bare wildcard token used by target lists and inherited table names.
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectStar {
    #[tok(STAR)]
    Value,
}

/// An expression target with its optional output alias.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectExprItem<'input> {
    pub expr: Expr<'input>,
    pub alias: Option<Alias<'input>>,
}

/// A SELECT/RETURNING target item.
///
/// PostgreSQL gives bare `*` its own `target_el` production rather than
/// admitting it as an expression. The enum also makes `* AS alias`
/// unconstructible while preserving aliases for expression targets.
#[derive(recursa::Node, Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "keep the public parser AST variants inline and source-compatible"
)]
pub enum SelectItem<'input> {
    Star(SelectStar),
    Expr(SelectExprItem<'input>),
}

/// Alias with explicit AS keyword: `AS name [UESCAPE 'c']`.
/// Uses AliasName so keywords are accepted (e.g., `SELECT 1 AS true`).
/// The optional UESCAPE suffix applies when the alias is a unicode-quoted
/// identifier (`U&"..."`).
#[derive(recursa::Node, Debug, Clone)]
pub struct AsAlias<'input> {
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
    pub uescape: Option<crate::ast::shared::expr::UescapeSuffix<'input>>,
}

/// AS alias clause, or bare alias.
///
/// Variant ordering: WithAs (`AS name`) has a longer first_pattern than
/// Bare (`ident`), so longest-match-wins picks it when AS is present.
#[derive(recursa::Node, Debug, Clone)]
pub enum Alias<'input> {
    WithAs(AsAlias<'input>),
    Bare(literal::SelectBareAliasName<'input>),
}

impl<'input> Alias<'input> {
    /// Returns the alias name regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            Alias::WithAs(a) => a.name.text(),
            Alias::Bare(ident) => ident.text(),
        }
    }
}

/// FROM clause: `FROM table [, table ...]`.
///
/// PostgreSQL's `from_list` is nonempty: `FROM` commits to at least one
/// `TableRef` (`SELECT FROM` must fail; only the target list may be empty).
#[derive(recursa::Node, Debug, Clone)]
#[tok(FROM, this)]
pub struct FromClause<'input> {
    #[sep(COMMA)]
    pub tables: recursa::Vec1<TableRef<'input>>,
}

/// Table name with inheritance marker and optional alias: `person* p`
#[derive(recursa::Node, Debug, Clone)]
pub struct InheritedTable<'input> {
    pub name: QualifiedName<'input>,
    #[tok(STAR, this)]
    pub alias: Option<literal::Ident<'input>>,
}

/// `AS name [(col1, col2)]` table alias form.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableAliasWithAs<'input> {
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
    pub columns: Option<TableAliasColumnList<'input>>,
}

/// Bare `name [(col1, col2)]` table alias form. PostgreSQL's `alias_clause`
/// admits `ColId`; the narrower category keeps join and clause starters from
/// being consumed as aliases.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableAliasBare<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub columns: Option<TableAliasColumnList<'input>>,
}

/// Parenthesized column aliases attached to a table alias.
///
/// The delimiters wrap the list as a whole. Attaching them to the repeated
/// field would instead require a fresh pair of parentheses around every
/// element after a comma.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct TableAliasColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<literal::AliasName<'input>>,
);

/// Table alias: `AS name [(col1, col2)]` or bare `name [(col1, col2)]`.
///
/// Variant ordering: `WithAs` (`AS`) before `Bare` (ident).
#[derive(recursa::Node, Debug, Clone)]
pub enum TableAlias<'input> {
    WithAs(TableAliasWithAs<'input>),
    Bare(TableAliasBare<'input>),
}

/// Subquery in FROM: `(SELECT ...) AS alias`
#[derive(recursa::Node, Debug, Clone)]
pub struct SubqueryRef<'input> {
    pub open: SelectLParen,
    pub query: Box<Subquery<'input>>,
    pub close: SelectRParen,
    pub alias: Option<TableAlias<'input>>,
}

/// Parenthesized join tree in FROM: `(t1 CROSS JOIN t2) AS alias`.
///
/// Distinguished from `SubqueryRef` by what the `(` contains: a subquery
/// starts with `SELECT` / `VALUES` / `TABLE` / `WITH` (all keywords),
/// whereas a parenthesized join tree starts with a table name (ident).
#[derive(recursa::Node, Debug, Clone)]
pub struct ParenJoinRef<'input> {
    pub open: SelectLParen,
    pub table: Box<TableRef<'input>>,
    pub close: SelectRParen,
    pub alias: Option<PlainTableAlias<'input>>,
}

/// Parenthesized FROM source with the delimiters and trailing alias shared by
/// query and join bodies.
#[derive(recursa::Node, Debug, Clone)]
pub struct ParenTableRef<'input> {
    pub open: SelectLParen,
    pub body: ParenTableBody<'input>,
    pub close: SelectRParen,
    pub alias: Option<PlainTableAlias<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum ParenTableBody<'input> {
    Query(Box<DirectSubquery<'input>>),
    Table(Box<TableRef<'input>>),
}

pub type SelectLParen = ParenthesizedOpen;

pub type SelectRParen = ParenthesizedClose;

/// `LATERAL (subquery) [alias]` — the parenthesized-subquery LATERAL form.
#[derive(recursa::Node, Debug, Clone)]
pub struct LateralSubquery<'input> {
    #[tok(LPAREN, this, RPAREN)]
    pub query: Box<Subquery<'input>>,
    pub alias: Option<PlainTableAlias<'input>>,
}

/// The table reference a `LATERAL` prefixes: a parenthesized subquery, a
/// function table, or an `XMLTABLE` / `JSON_TABLE`.
///
/// Variant ordering: `JsonTable` / `XmlTable` (soft keyword) before `Func`,
/// which would otherwise reclaim them as ordinary function names.
#[derive(recursa::Node, Debug, Clone)]
pub enum LateralBody<'input> {
    Subquery(LateralSubquery<'input>),
    JsonTable(Box<JsonTableRef<'input>>),
    XmlTable(Box<XmlTableRef<'input>>),
    Func(Box<FuncTableRef<'input>>),
}

/// `LATERAL` table reference in FROM: `LATERAL (VALUES(...)) v`,
/// `LATERAL func(...)`, `LATERAL XMLTABLE(...)`, `LATERAL JSON_TABLE(...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct LateralRef<'input> {
    #[tok(LATERAL, this)]
    pub body: LateralBody<'input>,
}

/// Plain table reference with optional alias: `[ONLY] tablename [AS] alias`
///
/// `ONLY` means do not recurse into inheritance children (the opposite
/// of the `table *` `InheritedTable` form).
#[derive(recursa::Node, Debug, Clone)]
pub struct PlainTable<'input> {
    #[presence(ONLY)]
    pub only: bool,
    pub name: QualifiedName<'input>,
    pub alias: Option<PlainTableAlias<'input>>,
}

/// Alias of a plain table reference in FROM: `[AS] name [(col, col, ...)]`.
///
/// Unlike `TableAlias` (which is used for subqueries, function tables, etc.,
/// where an alias is mandatory), this one uses `literal::Ident` for the bare
/// form so that SQL keywords like `WHERE`, `ORDER`, `GROUP` are not swallowed
/// as the alias name when the alias is absent. The `WithAs` variant can still
/// use `literal::AliasName` because the `AS` keyword disambiguates.
///
/// Variant ordering: `WithAs` (starts with `AS`) must be listed before `Bare`
/// so longest-match-wins picks it when `AS` is present.
#[derive(recursa::Node, Debug, Clone)]
pub enum PlainTableAlias<'input> {
    WithAs(PlainTableAliasWithAs<'input>),
    Bare(PlainTableAliasBare<'input>),
}

/// `AS name [(col, ...)]` form.
#[derive(recursa::Node, Debug, Clone)]
pub struct PlainTableAliasWithAs<'input> {
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
    pub columns: Option<TableAliasColumnList<'input>>,
}

/// Bare `name [(col, ...)]` form using PostgreSQL's `ColId` alias category.
#[derive(recursa::Node, Debug, Clone)]
pub struct PlainTableAliasBare<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub columns: Option<TableAliasColumnList<'input>>,
}

/// A column definition inside a function-table column-def-list:
/// `name type` (e.g., `a int`).
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableColumnDef<'input> {
    pub name: literal::AliasName<'input>,
    pub type_name: crate::ast::shared::expr::CastType<'input>,
}

/// `[AS] alias (col type, ...)` or just `(col type, ...)` -- the
/// column definition list form for table-returning functions.
#[derive(recursa::Node, Debug, Clone)]
pub struct ColumnDefList<'input> {
    #[tok(optional(AS), this)]
    pub name: Option<literal::AliasName<'input>>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns: Vec<FuncTableColumnDef<'input>>,
}

/// Alias of a function table reference.
///
/// The alias head is parsed once. Parenthesized columns retain an optional
/// type per item, representing both ordinary alias columns and a function
/// column-definition list without two alternatives competing on the same
/// `AS name (` or `name (` prefix.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncTableAlias<'input> {
    WithAs(FuncTableAliasWithAs<'input>),
    Named(FuncTableAliasNamed<'input>),
    Columns(FuncTableAliasColumns<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableAliasWithAs<'input> {
    pub as_keyword: SelectAsKeyword,
    pub body: FuncTableAliasAfterAs<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SelectAsKeyword {
    #[tok(AS)]
    Value,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum FuncTableAliasAfterAs<'input> {
    Named(FuncTableAliasAsNamed<'input>),
    Columns(FuncTableAliasColumns<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableAliasAsNamed<'input> {
    pub name: literal::AliasName<'input>,
    pub columns: Option<FuncTableAliasColumns<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableAliasNamed<'input> {
    pub name: literal::Ident<'input>,
    pub columns: Option<FuncTableAliasColumns<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableAliasColumns<'input> {
    pub open: SelectLParen,
    #[sep(COMMA)]
    pub columns: recursa::Vec1<FuncTableAliasColumn<'input>>,
    pub close: SelectRParen,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableAliasColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub type_name: Option<CastType<'input>>,
}

/// Function call used as table reference with optional WITH ORDINALITY and alias.
#[derive(recursa::Node, Debug, Clone)]
pub struct FuncTableRef<'input> {
    pub func: FunctionApplicationExpr<'input>,
    #[presence(WITH, ORDINALITY)]
    pub ordinality: bool,
    pub alias: Option<FuncTableAlias<'input>>,
}

/// One of the SQL-standard special-form function expressions allowed in a
/// `FROM` clause (PG `func_expr_common_subexpr` reachable from `func_table`).
///
/// gram.y `func_table → func_expr_windowless` admits both `func_application`
/// (modelled by `FuncTableRef`) and `func_expr_common_subexpr` — the
/// special-form atoms with their own keyword-led grammar (CAST, COALESCE,
/// COLLATION FOR, …). pg-sql models several of these as dedicated `Expr`
/// atoms (`CastCall`, `CollationForCall`, …); a `FROM`-clause variant
/// re-uses those types so the special forms can appear as table sources.
///
/// Currently covers the forms exercised by the create_view regression
/// corpus: `CAST(expr AS type)` and `COLLATION FOR (expr)`. Extend this
/// enum as new corpus statements demand additional special forms.
///
/// Variant ordering: keyword-led forms in declaration order; each variant's
/// leading token is distinct, so the first-set tree dispatches cleanly.
#[derive(recursa::Node, Debug, Clone)]
pub enum SpecialFuncTableExpr<'input> {
    /// `CAST(expr AS type [COLLATE "c"])`.
    Cast(crate::ast::shared::expr::CastCall<'input>),
    /// `COLLATION FOR (expr)`.
    CollationFor(crate::ast::shared::expr::CollationForCall<'input>),
    #[tok(USER)]
    /// `USER` — the reserved-keyword spelling of `CURRENT_USER`. Used as a
    /// zero-arg function reference in FROM (`SELECT * FROM USER`). The
    /// other reserved-feeling spellings (CURRENT_TIMESTAMP, LOCALTIMESTAMP,
    /// ...) lex as `UnquotedIdent` and parse as `PlainTable.name`; only
    /// `USER` is a hard keyword in pg-sql and so needs an explicit variant.
    User,
}

/// `FROM`-clause special-form function expression with optional alias.
#[derive(recursa::Node, Debug, Clone)]
pub struct SpecialFuncTableRef<'input> {
    pub func: SpecialFuncTableExpr<'input>,
    #[presence(WITH, ORDINALITY)]
    pub ordinality: bool,
    pub alias: Option<FuncTableAlias<'input>>,
}

// --- JSON_TABLE table reference ---
//
// `JSON_TABLE(...)` is a SQL/JSON table function: it appears in FROM (and
// JOIN) and projects a jsonpath match into rows/columns. A grammar
// construct with its own `COLUMNS ( ... )` clause, NESTED paths and
// per-column behaviors — modeled as a dedicated `SimpleTableRef` variant.

/// `[AS] ‹name›` — a path-variable name (after the JSON_TABLE path, or on a
/// `NESTED PATH`).
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTablePathName<'input> {
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
}

/// `PATH '‹jsonpath›'` clause on a JSON_TABLE column.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTableColumnPath<'input> {
    #[tok(PATH, this)]
    pub path_str: literal::StringLit<'input>,
}

/// `FOR ORDINALITY` — the row-counter column kind.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonTableOrdinality {
    #[tok(FOR, ORDINALITY)]
    Value,
}

/// The typed-column tail: `‹type› [EXISTS] [FORMAT JSON ...] [PATH '...']
/// [wrapper] [quotes] [behavior ON EMPTY] [behavior ON ERROR]`.
///
/// `EXISTS` columns and regular columns are merged — `exists` is just an
/// optional marker — and clauses are parsed permissively.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTableTypedColumn<'input> {
    pub ty: CastType<'input>,
    #[presence(EXISTS)]
    pub exists: bool,
    pub format: Option<JsonTableFormat<'input>>,
    pub path: Option<JsonTableColumnPath<'input>>,
    pub wrapper: Option<JsonWrapper>,
    pub quotes: Option<JsonQuotes>,
    pub on_empty_behaviour: Option<JsonOnBehavior<'input>>,
    pub on_error_behaviour: Option<JsonOnBehavior<'input>>,
}

/// `FORMAT JSON [ENCODING name]` within JSON_TABLE.
///
/// The required FORMAT/JSON pair wraps the complete optional-encoding body.
/// This keeps the clause present even when ENCODING is absent.
#[derive(recursa::Node, Debug, Clone)]
#[tok(FORMAT, JSON, this)]
pub struct JsonTableFormat<'input> {
    pub encoding: Option<JsonEncoding<'input>>,
}

/// The tail of a non-`NESTED` column, after its name.
///
/// `Ordinality` leads with `FOR`, `Typed` with a type — distinct first
/// tokens, so the enum dispatches cleanly.
#[derive(recursa::Node, Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum JsonTableColumnKind<'input> {
    Ordinality(JsonTableOrdinality),
    Typed(JsonTableTypedColumn<'input>),
}

/// A non-`NESTED` JSON_TABLE column: `‹name› {FOR ORDINALITY | ‹type› ...}`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTableValuedColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub kind: JsonTableColumnKind<'input>,
}

/// `NESTED [PATH] '‹jsonpath›' [AS ‹name›] COLUMNS ( ... )` — projects a
/// nested jsonpath into additional columns.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTableNestedColumn<'input> {
    #[tok(NESTED, optional(PATH), this)]
    pub path_str: literal::StringLit<'input>,
    pub as_name: Option<JsonTablePathName<'input>>,
    pub columns: JsonTableColumnList<'input>,
}

/// One column of a JSON_TABLE `COLUMNS ( ... )` list.
///
/// Variant ordering: `Nested` (leads with `NESTED`) before `Valued` — a
/// column literally named `nested` would also match `Valued`'s
/// keyword-permissive `AliasName`, so `Nested` is tried first and falls
/// through on non-NESTED syntax.
#[derive(recursa::Node, Debug, Clone)]
pub enum JsonTableColumn<'input> {
    Nested(JsonTableNestedColumn<'input>),
    Valued(JsonTableValuedColumn<'input>),
}

/// `COLUMNS ( ‹column› [, ...] )` — the JSON_TABLE column list (may be empty).
#[derive(recursa::Node, Debug, Clone)]
#[tok(COLUMNS, LPAREN, this, RPAREN)]
pub struct JsonTableColumnList<'input> {
    #[sep(COMMA)]
    pub list: Vec<JsonTableColumn<'input>>,
}

/// Inner contents of `JSON_TABLE ( ‹ctx› , ‹path› [AS name] [PASSING ...]
/// COLUMNS ( ... ) [behavior ON ERROR] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTableInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonTableFormat<'input>>,
    #[tok(COMMA, this)]
    pub path: Box<Expr<'input>>,
    pub path_name: Option<JsonTablePathName<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub column_list: JsonTableColumnList<'input>,
    pub on_error: Option<JsonOnBehavior<'input>>,
}

/// The `JSON_TABLE ( ... )` construct.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTable<'input> {
    #[tok(JSON_TABLE, LPAREN, this, RPAREN)]
    pub inner: JsonTableInner<'input>,
}

/// `JSON_TABLE(...)` as a table reference, with an optional table alias.
#[derive(recursa::Node, Debug, Clone)]
pub struct JsonTableRef<'input> {
    pub table: JsonTable<'input>,
    pub alias: Option<TableAlias<'input>>,
}

// --- XMLTABLE table reference ---
//
// `XMLTABLE(...)` is the XML analogue of `JSON_TABLE` — an XML table
// function in FROM/JOIN, projecting an XPath match into rows and columns.

/// One entry of an `XMLNAMESPACES(...)` list: `‹uri› AS ‹prefix›`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlNamespaceNamed<'input> {
    pub uri: Box<Expr<'input>>,
    #[tok(AS, this)]
    pub prefix: literal::AliasName<'input>,
}

/// The `DEFAULT ‹uri›` entry of an `XMLNAMESPACES(...)` list.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlNamespaceDefault<'input> {
    #[tok(DEFAULT, this)]
    pub uri: Box<Expr<'input>>,
}

/// One namespace declaration: a `DEFAULT ‹uri›` or a `‹uri› AS ‹prefix›`.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlNamespaceItem<'input> {
    Default(XmlNamespaceDefault<'input>),
    Named(XmlNamespaceNamed<'input>),
}

/// `XMLNAMESPACES ( ‹item› [, ...] ) ,` — the optional namespace prefix of
/// `XMLTABLE`. The trailing comma separates it from the row expression.
#[derive(recursa::Node, Debug, Clone)]
#[tok(XMLNAMESPACES, LPAREN, this, RPAREN, COMMA)]
pub struct XmlTableNamespaces<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<XmlNamespaceItem<'input>>,
}

/// `PATH '‹xpath›'` clause on an XMLTABLE column.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTableColumnPath<'input> {
    #[tok(PATH, this)]
    pub xpath: Box<Expr<'input>>,
}

/// `DEFAULT ‹expr›` clause on an XMLTABLE column.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTableColumnDefault<'input> {
    #[tok(DEFAULT, this)]
    pub value: Box<Expr<'input>>,
}

/// `NOT NULL` / `NULL` nullability marker on an XMLTABLE column.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlTableColumnNull {
    #[tok(NOT, NULL)]
    NotNull,
    #[tok(NULL)]
    Null,
}

/// `‹type› [PATH '...'] [DEFAULT expr] [NOT NULL|NULL]` — the typed-column tail.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTableTypedColumn<'input> {
    pub ty: CastType<'input>,
    pub path: Option<XmlTableColumnPath<'input>>,
    pub default: Option<XmlTableColumnDefault<'input>>,
    pub null: Option<XmlTableColumnNull>,
}

/// The tail of an XMLTABLE column, after its name: `FOR ORDINALITY` or a type.
#[derive(recursa::Node, Debug, Clone)]
pub enum XmlTableColumnKind<'input> {
    Ordinality(JsonTableOrdinality),
    Typed(XmlTableTypedColumn<'input>),
}

/// One column of an XMLTABLE `COLUMNS` list: `‹name› ‹kind›`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTableColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub kind: XmlTableColumnKind<'input>,
}

/// `COLUMNS ‹column› [, ...]` — the non-empty XMLTABLE column list.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(COLUMNS, this)]
pub struct XmlTableColumnList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<XmlTableColumn<'input>>,
);

/// Inner contents of `XMLTABLE ( [XMLNAMESPACES(...),] ‹row_xpath›
/// PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] COLUMNS ‹col› [, ...] )`.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTableInner<'input> {
    pub namespaces: Option<XmlTableNamespaces<'input>>,
    pub row_expr: Box<Expr<'input>>,
    pub passing: XmlTablePassing<'input>,
    pub by_after: Option<XmlPassingBy>,
    pub column_list: XmlTableColumnList<'input>,
}

/// Mandatory `PASSING` clause with an optional `BY` mode.
#[derive(recursa::Node, Debug, Clone)]
#[tok(PASSING, this)]
pub struct XmlTablePassing<'input> {
    pub by: Option<XmlPassingBy>,
    pub doc: Box<Expr<'input>>,
}

/// The `XMLTABLE ( ... )` construct.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTable<'input> {
    #[tok(XMLTABLE, LPAREN, this, RPAREN)]
    pub inner: XmlTableInner<'input>,
}

/// `XMLTABLE(...)` as a table reference, with an optional table alias.
#[derive(recursa::Node, Debug, Clone)]
pub struct XmlTableRef<'input> {
    pub table: XmlTable<'input>,
    pub alias: Option<TableAlias<'input>>,
}

// --- ROWS FROM (...) table reference ---

/// `AS ( col type [, ...] )` column-definition list on a `ROWS FROM` item.
#[derive(recursa::Node, Debug, Clone)]
#[tok(AS, LPAREN, this, RPAREN)]
pub struct RowsFromColDef<'input> {
    #[sep(COMMA)]
    pub columns: Vec<FuncTableColumnDef<'input>>,
}

/// One function entry of a `ROWS FROM (...)` list: a function call with an
/// optional `AS (coldef, ...)` column-definition list.
#[derive(recursa::Node, Debug, Clone)]
pub struct RowsFromItem<'input> {
    pub func: FunctionApplicationExpr<'input>,
    pub coldef: Option<RowsFromColDef<'input>>,
}

/// `ROWS FROM ( func [, ...] ) [WITH ORDINALITY] [alias]` — the multi-function
/// table reference, evaluating several set-returning functions in parallel.
#[derive(recursa::Node, Debug, Clone)]
pub struct RowsFromRef<'input> {
    pub items: RowsFromItemList<'input>,
    #[presence(WITH, ORDINALITY)]
    pub ordinality: bool,
    pub alias: Option<FuncTableAlias<'input>>,
}

/// The parenthesized function list inside `ROWS FROM`.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(ROWS, FROM, LPAREN, this, RPAREN)]
pub struct RowsFromItemList<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::Vec1<RowsFromItem<'input>>,
);

/// Function/table name admission used in FROM.
///
/// `COLLATION` is held back from the unqualified route so the exact
/// `COLLATION FOR (...)` special form can dispatch without competing with a
/// generic named relation. Qualified names retain PostgreSQL's ordinary
/// `ColId` admission.
#[derive(recursa::Node, Debug, Clone)]
pub enum TableFunctionName<'input> {
    Qualified(crate::ast::shared::expr::FuncCallQualifiedName<'input>),
    Name(crate::tokens::table_function_name<'input>),
}

/// Identifier-led FROM source. PostgreSQL's callable-name admission covers
/// ordinary identifiers and every qualified `ColId` name. Parsing that name
/// once lets `(`, `*`, an alias, or the absence of a suffix select the source
/// shape without comparing arbitrarily long dotted names.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedTableRef<'input> {
    pub name: TableFunctionName<'input>,
    pub tail: Option<NamedTableRefTail<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum NamedTableRefTail<'input> {
    Function(Box<NamedFunctionTableTail<'input>>),
    Inherited(NamedInheritedTail<'input>),
    Alias(PlainTableAlias<'input>),
}

/// Function-call tail after the shared function/table name.
#[derive(recursa::Node, Debug, Clone)]
pub struct NamedFunctionTableTail<'input> {
    pub application: FunctionCallApplication<'input>,
    #[presence(WITH, ORDINALITY)]
    pub ordinality: bool,
    pub alias: Option<FuncTableAlias<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct NamedInheritedTail<'input> {
    pub star: SelectStar,
    pub alias: Option<PlainTableAlias<'input>>,
}

/// `ONLY name [alias]`, separated from identifier-led sources by its required
/// keyword.
#[derive(recursa::Node, Debug, Clone)]
pub struct OnlyTableRef<'input> {
    pub only: SelectOnly,
    pub name: QualifiedName<'input>,
    pub alias: Option<PlainTableAlias<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum SelectOnly {
    #[tok(ONLY)]
    Value,
}

/// An unqualified `COL_NAME` keyword used as a relation name. These are the
/// only valid table-name starters not covered by `TableFunctionName`; keeping them
/// relation-only prevents XML/JSON special forms from re-entering the generic
/// function-table grammar.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColNameTableName {
    #[tok(VALUES)]
    Values,
    #[tok(JSON)]
    Json,
    #[tok(JSON_VALUE)]
    JsonValue,
    #[tok(JSON_QUERY)]
    JsonQuery,
    #[tok(JSON_EXISTS)]
    JsonExists,
    #[tok(JSON_OBJECT)]
    JsonObject,
    #[tok(JSON_ARRAY)]
    JsonArray,
    #[tok(JSON_OBJECTAGG)]
    JsonObjectAgg,
    #[tok(JSON_ARRAYAGG)]
    JsonArrayAgg,
    #[tok(JSON_SERIALIZE)]
    JsonSerialize,
    #[tok(JSON_SCALAR)]
    JsonScalar,
    #[tok(JSON_TABLE)]
    JsonTable,
    #[tok(BOOLEAN)]
    Boolean,
    #[tok(INT)]
    Int,
    #[tok(SETOF)]
    Setof,
    #[tok(EXISTS)]
    Exists,
    #[tok(ROW)]
    Row,
    #[tok(INTEGER)]
    Integer,
    #[tok(NUMERIC)]
    Numeric,
    #[tok(VARCHAR)]
    Varchar,
    #[tok(BETWEEN)]
    Between,
    #[tok(TIMESTAMP)]
    Timestamp,
    #[tok(TIME)]
    Time,
    #[tok(NONE)]
    None,
    #[tok(XMLELEMENT)]
    XmlElement,
    #[tok(XMLATTRIBUTES)]
    XmlAttributes,
    #[tok(XMLFOREST)]
    XmlForest,
    #[tok(XMLPI)]
    XmlPi,
    #[tok(OUT)]
    Out,
    #[tok(INOUT)]
    InOut,
    #[tok(TRIM)]
    Trim,
    #[tok(SUBSTRING)]
    Substring,
    #[tok(POSITION)]
    Position,
    #[tok(OVERLAY)]
    Overlay,
    #[tok(EXTRACT)]
    Extract,
    #[tok(GROUPING)]
    Grouping,
    #[tok(INTERVAL)]
    Interval,
    #[tok(PRECISION)]
    Precision,
    #[tok(BIT)]
    Bit,
    #[tok(CHARACTER)]
    Character,
    #[tok(XMLSERIALIZE)]
    XmlSerialize,
    #[tok(XMLROOT)]
    XmlRoot,
    #[tok(XMLEXISTS)]
    XmlExists,
    #[tok(XMLTABLE)]
    XmlTable,
    #[tok(XMLPARSE)]
    XmlParse,
    #[tok(XMLNAMESPACES)]
    XmlNamespaces,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct ColNameTableRef<'input> {
    pub name: ColNameTableName,
    pub tail: Option<ColNameTableTail<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub enum ColNameTableTail<'input> {
    Inherited(NamedInheritedTail<'input>),
    Alias(PlainTableAlias<'input>),
}

/// A single table reference (no joins). Used as building block for JoinTableRef.
///
/// Keyword-led special forms have distinct FIRST sets. Identifier-led table,
/// inherited-table, alias, and function-application forms share one
/// [`NamedTableRef`] prefix and select their continuation after the name.
#[derive(recursa::Node, Debug, Clone)]
pub enum SimpleTableRef<'input> {
    Lateral(LateralRef<'input>),
    JsonTable(Box<JsonTableRef<'input>>),
    XmlTable(Box<XmlTableRef<'input>>),
    RowsFrom(Box<RowsFromRef<'input>>),
    /// `CAST(expr AS type) [alias]`, `COLLATION FOR (expr) [alias]`. Each
    /// special form is keyword-led so the first-set tree disambiguates
    /// against `Func` and `Table` cleanly.
    SpecialFunc(Box<SpecialFuncTableRef<'input>>),
    Paren(ParenTableRef<'input>),
    Only(OnlyTableRef<'input>),
    ColName(ColNameTableRef<'input>),
    Named(Box<NamedTableRef<'input>>),
}

/// Join head, including the required JOIN keyword.
///
/// Every alternative is non-nullable. This lets plain JOIN participate in the
/// same deterministic prefix decision as its qualified forms.
#[derive(recursa::Node, Debug, Clone)]
pub enum JoinType {
    #[tok(LEFT, OUTER, JOIN)]
    LeftOuter,
    #[tok(LEFT, JOIN)]
    Left,
    #[tok(RIGHT, OUTER, JOIN)]
    RightOuter,
    #[tok(RIGHT, JOIN)]
    Right,
    #[tok(FULL, OUTER, JOIN)]
    FullOuter,
    #[tok(FULL, JOIN)]
    Full,
    #[tok(INNER, JOIN)]
    Inner,
    #[tok(CROSS, JOIN)]
    Cross,
    #[tok(JOIN)]
    Plain,
}

/// JOIN condition: ON expr or USING (col, ...)
#[derive(recursa::Node, Debug, Clone)]
pub enum JoinCondition<'input> {
    On(JoinOn<'input>),
    Using(JoinUsing<'input>),
}

/// ON condition for JOIN
#[derive(recursa::Node, Debug, Clone)]
pub struct JoinOn<'input> {
    #[tok(ON, this)]
    pub condition: Box<Expr<'input>>,
}

/// `AS alias` form of a JOIN USING alias.
#[derive(recursa::Node, Debug, Clone)]
pub struct JoinUsingAliasWithAs<'input> {
    #[tok(AS, this)]
    pub name: literal::AliasName<'input>,
}

/// `[AS] alias` suffix on a JOIN ... USING column list.
///
/// Variant ordering: `WithAs` (`AS name`) is longer than `Bare`
/// (`ident`); list it first.
#[derive(recursa::Node, Debug, Clone)]
pub enum JoinUsingAlias<'input> {
    WithAs(JoinUsingAliasWithAs<'input>),
    Bare(literal::Ident<'input>),
}

/// Parenthesized comma-separated column list in a JOIN USING clause.
#[derive(recursa::Node, Debug, Clone, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct JoinUsingColumns<'input>(
    #[deref]
    #[sep(COMMA)]
    pub Vec<literal::AliasName<'input>>,
);

/// USING clause for JOIN: `USING (col, ...) [[AS] alias]`
#[derive(recursa::Node, Debug, Clone)]
#[tok(USING, this)]
pub struct JoinUsing<'input> {
    pub columns: JoinUsingColumns<'input>,
    pub alias: Option<JoinUsingAlias<'input>>,
}

/// A single join suffix:
/// `[NATURAL] [LEFT|RIGHT|FULL|INNER|CROSS] [OUTER] JOIN table [ON expr | USING (...)]`.
///
/// `OUTER` is optional after `LEFT`/`RIGHT`/`FULL`; the exact JoinType variants
/// keep those longer spellings deterministic without admitting it after INNER
/// or CROSS.
#[derive(recursa::Node, Debug, Clone)]
pub struct JoinSuffix<'input> {
    #[presence(NATURAL)]
    pub natural: bool,
    pub join_type: JoinType,
    /// PostgreSQL assigns an unparenthesized joined table recursively to the
    /// right operand, preserving each condition at the grammar level that
    /// owns it.
    pub table: Box<TableRef<'input>>,
    pub condition: Option<JoinCondition<'input>>,
}

/// TABLESAMPLE clause: `TABLESAMPLE method (args) [REPEATABLE (seed)]`.
/// Attached to a single table reference (not to joined results).
#[derive(recursa::Node, Debug, Clone)]
pub struct TableSampleClause<'input> {
    #[tok(TABLESAMPLE, this)]
    pub method: literal::AliasName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args: Vec<Expr<'input>>,
    pub repeatable: Option<TableSampleRepeatable<'input>>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct TableSampleRepeatable<'input> {
    #[tok(REPEATABLE, LPAREN, this, RPAREN)]
    pub seed: Expr<'input>,
}

/// A table reference that may have zero or more JOIN suffixes.
#[derive(recursa::Node, Debug, Clone)]
pub struct TableRef<'input> {
    pub base: SimpleTableRef<'input>,
    pub tablesample: Option<TableSampleClause<'input>>,
    pub joins: Vec<JoinSuffix<'input>>,
}

/// WHERE-clause body: either a normal expression or the cursor-current
/// row filter `CURRENT OF cursor_name` (used by positioned UPDATE/DELETE).
///
/// Variant ordering: `CurrentOf` must come before `Expr` since `CURRENT`
/// is a specific keyword lead-in.
#[derive(recursa::Node, Debug, Clone)]
pub enum WhereCondition<'input> {
    CurrentOf(WhereCurrentOf<'input>),
    Expr(Expr<'input>),
}

/// `CURRENT OF cursor_name` filter.
#[derive(recursa::Node, Debug, Clone)]
pub struct WhereCurrentOf<'input> {
    #[tok(CURRENT, OF, this)]
    pub cursor: literal::AliasName<'input>,
}

/// WHERE clause: `WHERE expr` or `WHERE CURRENT OF cursor`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WhereClause<'input> {
    #[tok(WHERE, this)]
    pub condition: WhereCondition<'input>,
}

/// USING operator in ORDER BY: `USING > | USING < | USING ~<~ | ...`
///
/// Variant ordering: longer (4-char) locale operators before shorter (3-char),
/// then single-char `>` / `<` last.
#[derive(recursa::Node, Debug, Clone)]
pub enum UsingOp<'input> {
    #[tok(TILDELEQTILDE)]
    TildeLeqTilde,
    #[tok(TILDEGEQTILDE)]
    TildeGeqTilde,
    #[tok(TILDELTTILDE)]
    TildeLtTilde,
    #[tok(TILDEGTTILDE)]
    TildeGtTilde,
    #[tok(GT)]
    Gt,
    #[tok(LT)]
    Lt,
    Custom(#[lex(matcher)] literal::CustomOp<'input>),
}

/// USING clause in ORDER BY: `USING op`
#[derive(recursa::Node, Debug, Clone)]
pub struct UsingClause<'input> {
    #[tok(USING, this)]
    pub op: UsingOp<'input>,
}

/// Sort direction: ASC or DESC.
#[derive(recursa::Node, Debug, Clone)]
pub enum SortDir {
    #[tok(ASC)]
    Asc,
    #[tok(DESC)]
    Desc,
}

/// NULLS FIRST or NULLS LAST.
#[derive(recursa::Node, Debug, Clone)]
pub enum NullsOrder {
    #[tok(NULLS, FIRST)]
    First,
    #[tok(NULLS, LAST)]
    Last,
}

/// A single ORDER BY item: `expr [ASC|DESC] [USING op] [NULLS FIRST|LAST]`
#[derive(recursa::Node, Debug, Clone)]
pub struct OrderByItem<'input> {
    pub expr: Expr<'input>,
    pub dir: Option<SortDir>,
    pub using: Option<UsingClause<'input>>,
    pub nulls: Option<NullsOrder>,
}

/// ORDER BY clause: `ORDER BY item [, item ...]`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ORDER, BY, this)]
pub struct OrderByClause<'input> {
    #[sep(COMMA)]
    pub items: Vec<OrderByItem<'input>>,
}

/// OFFSET clause: `OFFSET expr`
#[derive(recursa::Node, Debug, Clone)]
pub struct OffsetClause<'input> {
    #[tok(OFFSET, this)]
    pub count: Expr<'input>,
}

/// LIMIT clause: `LIMIT expr`
#[derive(recursa::Node, Debug, Clone)]
pub struct LimitClause<'input> {
    #[tok(LIMIT, this)]
    pub count: Expr<'input>,
}

/// `FIRST` or `NEXT` keyword in FETCH clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchFirstOrNext {
    #[tok(FIRST)]
    First,
    #[tok(NEXT)]
    Next,
}

/// `ROW` or `ROWS` keyword in FETCH clause.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchRowOrRows {
    #[tok(ROWS)]
    Rows,
    #[tok(ROW)]
    Row,
}

/// `ONLY` or `WITH TIES` — FETCH clause termination mode.
///
/// `WithTies` declared first since it's longer and both start after a keyword.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchMode {
    #[tok(WITH, TIES)]
    WithTies,
    #[tok(ONLY)]
    Only,
}

/// FETCH without count: `{ ROW | ROWS } { ONLY | WITH TIES }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchNoCount {
    pub row_or_rows: FetchRowOrRows,
    pub mode: FetchMode,
}

/// FETCH with count: `expr { ROW | ROWS } { ONLY | WITH TIES }`.
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchWithCount<'input> {
    pub count: Box<Expr<'input>>,
    pub row_or_rows: FetchRowOrRows,
    pub mode: FetchMode,
}

/// Body of FETCH clause: either `{ ROW | ROWS } mode` (no count)
/// or `expr { ROW | ROWS } mode` (with count).
///
/// `NoCount` (peeks `ROW`/`ROWS`) must come first so it's tried before
/// `WithCount` which would greedily consume `ROWS` as an identifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum FetchFirstBody<'input> {
    NoCount(FetchNoCount),
    WithCount(FetchWithCount<'input>),
}

/// `FETCH { FIRST | NEXT } [count] { ROW | ROWS } { ONLY | WITH TIES }`
#[derive(recursa::Node, Debug, Clone)]
pub struct FetchFirstClause<'input> {
    #[tok(FETCH, this)]
    pub first_or_next: FetchFirstOrNext,
    pub body: FetchFirstBody<'input>,
}

/// A limiting clause: `LIMIT expr` or `FETCH FIRST ...`. PostgreSQL's
/// `select_limit` production admits at most one limiting clause per query,
/// so `Offset` is deliberately not part of this enum.
///
/// `FetchFirst` and `Limit` start on distinct keywords (`FETCH` vs `LIMIT`).
#[derive(recursa::Node, Debug, Clone)]
pub enum LimitingClause<'input> {
    FetchFirst(FetchFirstClause<'input>),
    Limit(LimitClause<'input>),
}

/// A limiting clause followed by an optional `OFFSET` clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct LimitThenOffset<'input> {
    pub limit: LimitingClause<'input>,
    #[pretty(break_before = soft)]
    pub offset: Option<Box<OffsetClause<'input>>>,
}

/// An `OFFSET` clause followed by an optional limiting clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct OffsetThenLimit<'input> {
    pub offset: OffsetClause<'input>,
    #[pretty(break_before = soft)]
    pub limit: Option<Box<LimitingClause<'input>>>,
}

/// The limit/offset tail of a query, restricted to the clause orders
/// PostgreSQL's `select_limit` production accepts: a limiting clause with an
/// optional `OFFSET` after it, or an `OFFSET` clause with an optional
/// limiting clause after it. Duplicate same-kind clauses and `LIMIT` mixed
/// with `FETCH FIRST` are structurally unrepresentable, and rendering
/// preserves the written order via the variant.
#[derive(recursa::Node, Debug, Clone)]
pub enum LimitOffsetClause<'input> {
    LimitOffset(LimitThenOffset<'input>),
    OffsetLimit(OffsetThenLimit<'input>),
}

/// FOR UPDATE / FOR SHARE / FOR NO KEY UPDATE / FOR KEY SHARE locking clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForUpdateClause<'input> {
    #[tok(FOR, this)]
    pub mode: LockingMode,
    /// Optional `OF table[, ...]` restricting the lock to a subset of the
    /// FROM-list entries (plain tables or table aliases).
    pub of: Option<ForUpdateOf<'input>>,
    /// Optional wait-behavior modifier: `NOWAIT` or `SKIP LOCKED`.
    pub wait: Option<ForUpdateWait>,
}

/// `OF name[, ...]` in a `FOR UPDATE` locking clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct ForUpdateOf<'input> {
    #[tok(OF, this)]
    #[sep(COMMA)]
    pub names: Vec<crate::tokens::ColId<'input>>,
}

/// `NOWAIT | SKIP LOCKED` suffix on a `FOR UPDATE` clause.
///
/// Variant ordering: `SkipLocked` (two tokens) before `Nowait` (one token).
#[derive(recursa::Node, Debug, Clone)]
pub enum ForUpdateWait {
    #[tok(SKIP, LOCKED)]
    SkipLocked,
    #[tok(NOWAIT)]
    Nowait,
}

/// Lock strength for `SELECT ... FOR ...` locking clauses.
///
/// Variant ordering: longer (`NO KEY UPDATE`, `KEY SHARE`) before shorter
/// (`UPDATE`, `SHARE`) so longest-match wins.
#[derive(recursa::Node, Debug, Clone)]
pub enum LockingMode {
    #[tok(NO, KEY, UPDATE)]
    NoKeyUpdate,
    #[tok(KEY, SHARE)]
    KeyShare,
    #[tok(UPDATE)]
    Update,
    #[tok(SHARE)]
    Share,
}

/// GROUP BY clause: `GROUP BY item, ...` where each item is an expression
/// or one of the grouping primitives (GROUPING SETS, ROLLUP, CUBE).
#[derive(recursa::Node, Debug, Clone)]
#[tok(GROUP, BY, this)]
pub struct GroupByClause<'input> {
    /// Optional `DISTINCT` / `ALL` modifier (Postgres 16+).
    pub modifier: Option<GroupByModifier>,
    #[sep(COMMA)]
    pub items: Vec<GroupByItem<'input>>,
}

/// `GROUP BY [DISTINCT|ALL]` modifier.
#[derive(recursa::Node, Debug, Clone)]
pub enum GroupByModifier {
    #[tok(DISTINCT)]
    Distinct,
    #[tok(ALL)]
    All,
}

/// `GROUPING SETS ( item, ... )`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(GROUPING, SETS, LPAREN, this, RPAREN)]
pub struct GroupingSetsItem<'input> {
    #[sep(COMMA)]
    pub groups: Vec<Box<GroupByItem<'input>>>,
}

/// `ROLLUP ( item, ... )`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ROLLUP, LPAREN, this, RPAREN)]
pub struct RollupItem<'input> {
    #[sep(COMMA)]
    pub items: Vec<Box<GroupByItem<'input>>>,
}

/// `CUBE ( item, ... )`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(CUBE, LPAREN, this, RPAREN)]
pub struct CubeItem<'input> {
    #[sep(COMMA)]
    pub items: Vec<Box<GroupByItem<'input>>>,
}

/// A single element in a GROUP BY clause.
///
/// Variant ordering: two-keyword primitives first (`GROUPING SETS`), then
/// single-keyword primitives (`ROLLUP`, `CUBE`), then the catch-all `Expr`
/// which also handles `(a, b)` row-style groupings.
#[derive(recursa::Node, Debug, Clone)]
pub enum GroupByItem<'input> {
    GroupingSets(GroupingSetsItem<'input>),
    Empty(EmptyGroupingSet),
    Expr(Box<Expr<'input>>),
}

/// The empty grouping set `()`, used inside `GROUPING SETS` and also valid as
/// a top-level grouping item.
#[derive(recursa::Node, Debug, Clone)]
pub enum EmptyGroupingSet {
    #[tok(LPAREN, RPAREN)]
    Value,
}

/// HAVING clause: `HAVING expr`
#[derive(recursa::Node, Debug, Clone)]
pub struct HavingClause<'input> {
    #[tok(HAVING, this)]
    pub condition: Expr<'input>,
}

/// A single named window definition: `name AS (inline_window_spec)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct WindowDef<'input> {
    pub name: crate::tokens::ColId<'input>,
    #[tok(AS, LPAREN, this, RPAREN)]
    pub spec: crate::ast::shared::expr::InlineWindowSpec<'input>,
}

/// `WINDOW name AS (...)[, name AS (...), ...]` clause in SELECT.
#[derive(recursa::Node, Debug, Clone)]
#[tok(WINDOW, this)]
pub struct WindowClause<'input> {
    #[sep(COMMA)]
    pub defs: recursa::Vec1<WindowDef<'input>>,
}

/// `INTO [TEMP|TEMPORARY|UNLOGGED] [TABLE] target` clause for the
/// Postgres `SELECT ... INTO new_table` statement form.
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectIntoPersistence {
    #[tok(TEMP)]
    Temp,
    #[tok(TEMPORARY)]
    Temporary,
    #[tok(UNLOGGED)]
    Unlogged,
}

/// `INTO [TEMP|TEMPORARY|UNLOGGED] [TABLE] target` clause for the
/// Postgres `SELECT ... INTO new_table` statement form.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectIntoClause<'input> {
    #[tok(INTO, this)]
    pub persistence: Option<SelectIntoPersistence>,
    #[tok(optional(TABLE), this)]
    pub target: crate::ast::shared::names::QualifiedName<'input>,
    pub using: Option<crate::ast::ddl::table::UsingAccessMethodClause<'input>>,
}

/// `DISTINCT` or `DISTINCT ON (exprs)` qualifier on a SELECT.
///
/// Variant ordering: `On` (longer, starts with `DISTINCT ON`) before `All`
/// (just `DISTINCT`).
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectDistinct<'input> {
    On(SelectDistinctOn<'input>),
    #[tok(DISTINCT)]
    All,
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(DISTINCT, ON, LPAREN, this, RPAREN)]
pub struct SelectDistinctOn<'input> {
    #[sep(COMMA)]
    pub exprs: Vec<crate::ast::shared::expr::Expr<'input>>,
}

/// SELECT statement.
#[derive(recursa::Node, Debug, Clone)]
#[pretty(group = consistent)]
#[tok(SELECT, this)]
pub struct SelectStmt<'input> {
    #[pretty(break_before = soft)]
    pub head: SelectHead<'input>,
    #[pretty(break_before = soft)]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    #[pretty(break_before = soft)]
    pub group_by: Option<Box<GroupByClause<'input>>>,
    #[pretty(break_before = soft)]
    pub having: Option<Box<HavingClause<'input>>>,
    #[pretty(break_before = soft)]
    pub window: Option<Box<WindowClause<'input>>>,
    #[pretty(break_before = soft)]
    pub order_by: Option<Box<OrderByClause<'input>>>,
    /// LIMIT / OFFSET / FETCH FIRST tail. Postgres allows one limiting
    /// clause (`LIMIT` or `FETCH FIRST`) and one `OFFSET`, in either order.
    #[pretty(break_before = soft)]
    pub limit_offset: Option<Box<LimitOffsetClause<'input>>>,
    #[pretty(break_before = soft)]
    pub for_update: Option<Box<ForUpdateClause<'input>>>,
}

/// The DISTINCT ON, bare DISTINCT, or unqualified head of a SELECT statement.
///
/// Each qualified alternative owns its fixed prefix directly. This keeps the
/// prefix nonnullable while sharing the complete target grammar after it.
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectHead<'input> {
    DistinctOn(SelectDistinctOnTargets<'input>),
    Distinct(SelectDistinctTargets<'input>),
    Plain(SelectTargets<'input>),
}

/// A required `DISTINCT ON (...)` qualifier followed by the SELECT targets.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectDistinctOnTargets<'input> {
    pub qualifier: SelectDistinctOn<'input>,
    #[pretty(break_before = soft)]
    pub targets: SelectTargets<'input>,
}

/// A required bare `DISTINCT` prefix followed by the SELECT targets.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DISTINCT, this)]
pub struct SelectDistinctTargets<'input> {
    #[pretty(break_before = soft)]
    pub targets: SelectTargets<'input>,
}

/// SELECT targets together with their optional INTO/FROM clauses.
///
/// `FROM` is a reserved exact prefix for the zero-target PostgreSQL form.
/// Keeping that alternative beside the required nonempty target list avoids a
/// nullable expression-list decision on the same token.
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectTargets<'input> {
    Empty(FromClause<'input>),
    Items(SelectTargetList<'input>),
}

/// A nonempty SELECT target list and the clauses that immediately follow it.
#[derive(recursa::Node, Debug, Clone)]
pub struct SelectTargetList<'input> {
    #[sep(COMMA)]
    #[pretty(group = consistent, indent)]
    pub items: recursa::Vec1<SelectItem<'input>>,
    #[pretty(break_before = soft)]
    pub into: Option<Box<SelectIntoClause<'input>>>,
    #[pretty(break_before = soft)]
    pub from_clause: Option<Box<FromClause<'input>>>,
}

impl<'input> SelectStmt<'input> {
    /// Return an owned semantic projection of the optional DISTINCT qualifier.
    pub fn distinct(&self) -> Option<SelectDistinct<'input>> {
        match &self.head {
            SelectHead::DistinctOn(head) => Some(SelectDistinct::On(head.qualifier.clone())),
            SelectHead::Distinct(_) => Some(SelectDistinct::All),
            SelectHead::Plain(_) => None,
        }
    }

    /// Return the targets from either SELECT-head form.
    pub fn targets(&self) -> &SelectTargets<'input> {
        match &self.head {
            SelectHead::DistinctOn(head) => &head.targets,
            SelectHead::Distinct(head) => &head.targets,
            SelectHead::Plain(targets) => targets,
        }
    }

    /// Number of items in the SELECT list (zero if the list is empty,
    /// e.g. the regression-test form `SELECT FROM tbl`).
    pub fn item_count(&self) -> usize {
        match self.targets() {
            SelectTargets::Empty(_) => 0,
            SelectTargets::Items(targets) => targets.items.len(),
        }
    }

    /// Iterate over the SELECT items.
    pub fn items(&self) -> impl Iterator<Item = &SelectItem<'input>> {
        match self.targets() {
            SelectTargets::Empty(_) => None,
            SelectTargets::Items(targets) => Some(targets.items.as_slice()),
        }
        .into_iter()
        .flatten()
    }

    /// Return the FROM clause from either target-list form.
    pub fn from_clause(&self) -> Option<&FromClause<'input>> {
        match self.targets() {
            SelectTargets::Empty(from_clause) => Some(from_clause),
            SelectTargets::Items(targets) => targets.from_clause.as_deref(),
        }
    }
}

/// A SELECT body that can appear in subqueries -- WITH, SELECT, or VALUES.
/// WithBody must come before Select so `WITH ... SELECT` matches before bare `SELECT`.
/// SelectStmt must come before ValuesStmt so `SELECT` keyword wins over ambiguity.
#[derive(recursa::Node, Debug, Clone)]
pub enum SelectBody<'input> {
    WithBody(Box<crate::ast::shared::with_clause::WithStatement<'input>>),
    Select(Box<SelectStmt<'input>>),
    Values(ValuesBody<'input>),
}

#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct ValuesRow<'input> {
    #[sep(COMMA)]
    pub values: Vec<Expr<'input>>,
}

/// VALUES body: `VALUES (expr, ...), (expr, ...)`
/// Can appear standalone or inside subqueries.
#[derive(recursa::Node, Debug, Clone)]
#[tok(VALUES, this)]
pub struct ValuesBody<'input> {
    #[sep(COMMA)]
    pub rows: Vec<ValuesRow<'input>>,
}
