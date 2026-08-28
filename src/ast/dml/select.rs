/// SELECT statement AST.
use recursa::seq::{OptionalTrailing, Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::ast::dml::values::Subquery;
use crate::ast::shared::expr::{
    CastType, Expr, FuncCall, JsonFormat, JsonOnBehavior, JsonPassing, JsonQuotes, JsonWrapper,
    XmlPassingBy,
};
use crate::ast::shared::names::QualifiedName;
use crate::tokens::{literal, punct};

use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
/// A single item in the SELECT list: `expr [AS alias]` or `expr alias`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SelectItem<'input> {
    pub expr: Expr<'input>,
    pub alias: Option<Alias<'input>>,
}

/// Alias with explicit AS keyword: `AS name [UESCAPE 'c']`.
/// Uses AliasName so keywords are accepted (e.g., `SELECT 1 AS true`).
/// The optional UESCAPE suffix applies when the alias is a unicode-quoted
/// identifier (`U&"..."`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AsAlias<'input> {
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
    pub uescape: Option<crate::ast::shared::expr::UescapeSuffix<'input>>,
}

/// AS alias clause, or bare alias.
///
/// Variant ordering: WithAs (`AS name`) has a longer first_pattern than
/// Bare (`ident`), so longest-match-wins picks it when AS is present.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum Alias<'input> {
    WithAs(AsAlias<'input>),
    Bare(literal::Ident<'input>),
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FromClause<'input> {
    pub from: FROM,
    pub tables: Seq0<TableRef<'input>, punct::Comma>,
}

/// Table name with inheritance marker and optional alias: `person* p`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct InheritedTable<'input> {
    pub name: QualifiedName<'input>,
    pub star: punct::Star,
    pub alias: Option<literal::Ident<'input>>,
}

/// `AS name [(col1, col2)]` table alias form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableAliasWithAs<'input> {
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
}

/// Bare `name [(col1, col2)]` table alias form. Bare alias names must not
/// be reserved keywords (uses `Ident`, not `AliasName`), otherwise clauses
/// like `FROM unnest(a) ORDER BY 1` would consume `ORDER` as an alias.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableAliasBare<'input> {
    pub name: literal::Ident<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
}

/// Table alias: `AS name [(col1, col2)]` or bare `name [(col1, col2)]`.
///
/// Variant ordering: `WithAs` (`AS`) before `Bare` (ident).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum TableAlias<'input> {
    WithAs(TableAliasWithAs<'input>),
    Bare(TableAliasBare<'input>),
}

/// Subquery in FROM: `(SELECT ...) AS alias`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubqueryRef<'input> {
    pub lparen: punct::LParen,
    pub query: Box<Subquery<'input>>,
    pub rparen: punct::RParen,
    pub alias: Option<TableAlias<'input>>,
}

/// Parenthesized join tree in FROM: `(t1 CROSS JOIN t2) AS alias`.
///
/// Distinguished from `SubqueryRef` by what the `(` contains: a subquery
/// starts with `SELECT` / `VALUES` / `TABLE` / `WITH` (all keywords),
/// whereas a parenthesized join tree starts with a table name (ident).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ParenJoinRef<'input> {
    pub lparen: punct::LParen,
    pub table: Box<TableRef<'input>>,
    pub rparen: punct::RParen,
    pub alias: Option<PlainTableAlias<'input>>,
}

/// `LATERAL (subquery) [alias]` — the parenthesized-subquery LATERAL form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LateralSubquery<'input> {
    pub query: Surrounded<punct::LParen, Box<Subquery<'input>>, punct::RParen>,
    pub alias: Option<PlainTableAlias<'input>>,
}

/// The table reference a `LATERAL` prefixes: a parenthesized subquery, a
/// function table, or an `XMLTABLE` / `JSON_TABLE`.
///
/// Variant ordering: `JsonTable` / `XmlTable` (soft keyword) before `Func`,
/// which would otherwise reclaim them as ordinary function names.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LateralBody<'input> {
    Subquery(LateralSubquery<'input>),
    JsonTable(Box<JsonTableRef<'input>>),
    XmlTable(Box<XmlTableRef<'input>>),
    Func(Box<FuncTableRef<'input>>),
}

/// `LATERAL` table reference in FROM: `LATERAL (VALUES(...)) v`,
/// `LATERAL func(...)`, `LATERAL XMLTABLE(...)`, `LATERAL JSON_TABLE(...)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LateralRef<'input> {
    pub lateral: LATERAL,
    pub body: LateralBody<'input>,
}

/// Plain table reference with optional alias: `[ONLY] tablename [AS] alias`
///
/// `ONLY` means do not recurse into inheritance children (the opposite
/// of the `table *` `InheritedTable` form).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PlainTable<'input> {
    pub only: Option<ONLY>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum PlainTableAlias<'input> {
    WithAs(PlainTableAliasWithAs<'input>),
    Bare(PlainTableAliasBare<'input>),
}

/// `AS name [(col, ...)]` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PlainTableAliasWithAs<'input> {
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
}

/// Bare `name [(col, ...)]` form. Uses `literal::Ident` to reject keywords.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct PlainTableAliasBare<'input> {
    pub name: literal::Ident<'input>,
    pub columns: Option<
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    >,
}

/// A column definition inside a function-table column-def-list:
/// `name type` (e.g., `a int`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncTableColumnDef<'input> {
    pub name: literal::AliasName<'input>,
    pub type_name: crate::ast::shared::expr::CastType<'input>,
}

/// `[AS] alias (col type, ...)` or just `(col type, ...)` -- the
/// column definition list form for table-returning functions.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ColumnDefList<'input> {
    pub r#as: Option<AS>,
    pub name: Option<literal::AliasName<'input>>,
    pub columns:
        Surrounded<punct::LParen, Seq0<FuncTableColumnDef<'input>, punct::Comma>, punct::RParen>,
}

/// Alias of a function table reference: either a regular `TableAlias`
/// or a column-definition list form.
///
/// Variant ordering: `ColumnDefList` is more specific (its inner uses
/// `name type` pairs requiring at least one type token after each name)
/// so list it first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FuncTableAlias<'input> {
    ColumnDefList(ColumnDefList<'input>),
    Plain(TableAlias<'input>),
}

/// Function call used as table reference with optional WITH ORDINALITY and alias.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FuncTableRef<'input> {
    pub func: FuncCall<'input>,
    pub ordinality: Option<(WITH, ORDINALITY)>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SpecialFuncTableExpr<'input> {
    /// `CAST(expr AS type [COLLATE "c"])`.
    Cast(crate::ast::shared::expr::CastCall<'input>),
    /// `COLLATION FOR (expr)`.
    CollationFor(crate::ast::shared::expr::CollationForCall<'input>),
    /// `USER` — the reserved-keyword spelling of `CURRENT_USER`. Used as a
    /// zero-arg function reference in FROM (`SELECT * FROM USER`). The
    /// other reserved-feeling spellings (CURRENT_TIMESTAMP, LOCALTIMESTAMP,
    /// ...) lex as `UnquotedIdent` and parse as `PlainTable.name`; only
    /// `USER` is a hard keyword in pg-sql and so needs an explicit variant.
    User(USER),
}

/// `FROM`-clause special-form function expression with optional alias.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SpecialFuncTableRef<'input> {
    pub func: SpecialFuncTableExpr<'input>,
    pub ordinality: Option<(WITH, ORDINALITY)>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTablePathName<'input> {
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
}

/// `PATH '‹jsonpath›'` clause on a JSON_TABLE column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableColumnPath<'input> {
    pub path: PATH,
    pub path_str: literal::StringLit<'input>,
}

/// `FOR ORDINALITY` — the row-counter column kind.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableOrdinality {
    pub r#for: FOR,
    pub ordinality: ORDINALITY,
}

/// The typed-column tail: `‹type› [EXISTS] [FORMAT JSON ...] [PATH '...']
/// [wrapper] [quotes] [behavior ON EMPTY] [behavior ON ERROR]`.
///
/// `EXISTS` columns and regular columns are merged — `exists` is just an
/// optional marker — and clauses are parsed permissively.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableTypedColumn<'input> {
    pub ty: CastType<'input>,
    pub exists: Option<EXISTS>,
    pub format: Option<JsonFormat<'input>>,
    pub path: Option<JsonTableColumnPath<'input>>,
    pub wrapper: Option<JsonWrapper>,
    pub quotes: Option<JsonQuotes>,
    pub on_empty_behaviour: Option<JsonOnBehavior<'input>>,
    pub on_error_behaviour: Option<JsonOnBehavior<'input>>,
}

/// The tail of a non-`NESTED` column, after its name.
///
/// `Ordinality` leads with `FOR`, `Typed` with a type — distinct first
/// tokens, so the enum dispatches cleanly.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
#[allow(clippy::large_enum_variant)]
pub enum JsonTableColumnKind<'input> {
    Ordinality(JsonTableOrdinality),
    Typed(JsonTableTypedColumn<'input>),
}

/// A non-`NESTED` JSON_TABLE column: `‹name› {FOR ORDINALITY | ‹type› ...}`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableValuedColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub kind: JsonTableColumnKind<'input>,
}

/// `NESTED [PATH] '‹jsonpath›' [AS ‹name›] COLUMNS ( ... )` — projects a
/// nested jsonpath into additional columns.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableNestedColumn<'input> {
    pub nested: NESTED,
    pub path: Option<PATH>,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JsonTableColumn<'input> {
    Nested(JsonTableNestedColumn<'input>),
    Valued(JsonTableValuedColumn<'input>),
}

/// `COLUMNS ( ‹column› [, ...] )` — the JSON_TABLE column list (may be empty).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableColumnList<'input> {
    pub columns: COLUMNS,
    pub list: Surrounded<punct::LParen, Seq0<JsonTableColumn<'input>, punct::Comma>, punct::RParen>,
}

/// Inner contents of `JSON_TABLE ( ‹ctx› , ‹path› [AS name] [PASSING ...]
/// COLUMNS ( ... ) [behavior ON ERROR] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableInner<'input> {
    pub context: Box<Expr<'input>>,
    pub context_format: Option<JsonFormat<'input>>,
    pub comma: punct::Comma,
    pub path: Box<Expr<'input>>,
    pub path_name: Option<JsonTablePathName<'input>>,
    pub passing: Option<JsonPassing<'input>>,
    pub column_list: JsonTableColumnList<'input>,
    pub on_error: Option<JsonOnBehavior<'input>>,
}

/// The `JSON_TABLE ( ... )` construct.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTable<'input> {
    pub kw: JSON_TABLE,
    pub inner: Surrounded<punct::LParen, JsonTableInner<'input>, punct::RParen>,
}

/// `JSON_TABLE(...)` as a table reference, with an optional table alias.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JsonTableRef<'input> {
    pub table: JsonTable<'input>,
    pub alias: Option<TableAlias<'input>>,
}

// --- XMLTABLE table reference ---
//
// `XMLTABLE(...)` is the XML analogue of `JSON_TABLE` — an XML table
// function in FROM/JOIN, projecting an XPath match into rows and columns.

/// One entry of an `XMLNAMESPACES(...)` list: `‹uri› AS ‹prefix›`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlNamespaceNamed<'input> {
    pub uri: Box<Expr<'input>>,
    pub r#as: AS,
    pub prefix: literal::AliasName<'input>,
}

/// The `DEFAULT ‹uri›` entry of an `XMLNAMESPACES(...)` list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlNamespaceDefault<'input> {
    pub default: DEFAULT,
    pub uri: Box<Expr<'input>>,
}

/// One namespace declaration: a `DEFAULT ‹uri›` or a `‹uri› AS ‹prefix›`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlNamespaceItem<'input> {
    Default(XmlNamespaceDefault<'input>),
    Named(XmlNamespaceNamed<'input>),
}

/// `XMLNAMESPACES ( ‹item› [, ...] ) ,` — the optional namespace prefix of
/// `XMLTABLE`. The trailing comma separates it from the row expression.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableNamespaces<'input> {
    pub kw: XMLNAMESPACES,
    pub items:
        Surrounded<punct::LParen, Seq1<XmlNamespaceItem<'input>, punct::Comma>, punct::RParen>,
    pub comma: punct::Comma,
}

/// `PATH '‹xpath›'` clause on an XMLTABLE column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableColumnPath<'input> {
    pub path: PATH,
    pub xpath: Box<Expr<'input>>,
}

/// `DEFAULT ‹expr›` clause on an XMLTABLE column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableColumnDefault<'input> {
    pub default: DEFAULT,
    pub value: Box<Expr<'input>>,
}

/// `NOT NULL` / `NULL` nullability marker on an XMLTABLE column.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlTableColumnNull {
    NotNull((NOT, NULL)),
    Null(NULL),
}

/// `‹type› [PATH '...'] [DEFAULT expr] [NOT NULL|NULL]` — the typed-column tail.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableTypedColumn<'input> {
    pub ty: CastType<'input>,
    pub path: Option<XmlTableColumnPath<'input>>,
    pub default: Option<XmlTableColumnDefault<'input>>,
    pub null: Option<XmlTableColumnNull>,
}

/// The tail of an XMLTABLE column, after its name: `FOR ORDINALITY` or a type.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum XmlTableColumnKind<'input> {
    Ordinality(JsonTableOrdinality),
    Typed(XmlTableTypedColumn<'input>),
}

/// One column of an XMLTABLE `COLUMNS` list: `‹name› ‹kind›`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableColumn<'input> {
    pub name: literal::AliasName<'input>,
    pub kind: XmlTableColumnKind<'input>,
}

/// Inner contents of `XMLTABLE ( [XMLNAMESPACES(...),] ‹row_xpath›
/// PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] COLUMNS ‹col› [, ...] )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableInner<'input> {
    pub namespaces: Option<XmlTableNamespaces<'input>>,
    pub row_expr: Box<Expr<'input>>,
    pub passing: PASSING,
    pub by_before: Option<XmlPassingBy>,
    pub doc: Box<Expr<'input>>,
    pub by_after: Option<XmlPassingBy>,
    pub columns: COLUMNS,
    pub column_list: Seq1<XmlTableColumn<'input>, punct::Comma>,
}

/// The `XMLTABLE ( ... )` construct.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTable<'input> {
    pub kw: XMLTABLE,
    pub inner: Surrounded<punct::LParen, XmlTableInner<'input>, punct::RParen>,
}

/// `XMLTABLE(...)` as a table reference, with an optional table alias.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct XmlTableRef<'input> {
    pub table: XmlTable<'input>,
    pub alias: Option<TableAlias<'input>>,
}

// --- ROWS FROM (...) table reference ---

/// `AS ( col type [, ...] )` column-definition list on a `ROWS FROM` item.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RowsFromColDef<'input> {
    pub r#as: AS,
    pub columns:
        Surrounded<punct::LParen, Seq0<FuncTableColumnDef<'input>, punct::Comma>, punct::RParen>,
}

/// One function entry of a `ROWS FROM (...)` list: a function call with an
/// optional `AS (coldef, ...)` column-definition list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RowsFromItem<'input> {
    pub func: FuncCall<'input>,
    pub coldef: Option<RowsFromColDef<'input>>,
}

/// `ROWS FROM ( func [, ...] ) [WITH ORDINALITY] [alias]` — the multi-function
/// table reference, evaluating several set-returning functions in parallel.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RowsFromRef<'input> {
    pub rows: ROWS,
    pub from: FROM,
    pub items: Surrounded<punct::LParen, Seq1<RowsFromItem<'input>, punct::Comma>, punct::RParen>,
    pub ordinality: Option<(WITH, ORDINALITY)>,
    pub alias: Option<FuncTableAlias<'input>>,
}

/// A single table reference (no joins). Used as building block for JoinTableRef.
///
/// Variant ordering matters for disambiguation via longest-match-wins:
/// - Lateral before Func: both match `keyword(` pattern length, Lateral
///   wins via declaration order since LATERAL is a keyword (not an Ident).
/// - JsonTable / XmlTable before Func: `JSON_TABLE` / `XMLTABLE` are soft
///   keywords that `FuncCall` would otherwise reclaim as function names.
/// - Func before Inherited/Table: FuncCall's `ident(` pattern is longer
///   than bare ident.
/// - Inherited before Table: `person*` matches longer than `person`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SimpleTableRef<'input> {
    Lateral(LateralRef<'input>),
    JsonTable(Box<JsonTableRef<'input>>),
    XmlTable(Box<XmlTableRef<'input>>),
    RowsFrom(Box<RowsFromRef<'input>>),
    /// `CAST(expr AS type) [alias]`, `COLLATION FOR (expr) [alias]`. Each
    /// special form is keyword-led so the first-set tree disambiguates
    /// against `Func` and `Table` cleanly.
    SpecialFunc(Box<SpecialFuncTableRef<'input>>),
    Func(Box<FuncTableRef<'input>>),
    // Subquery must come before ParenJoin: both start with `(`, but a
    // subquery body begins with a keyword (`SELECT`/`VALUES`/`TABLE`/`WITH`)
    // while a parenthesized join tree begins with an identifier. The parser
    // forks and tries in declaration order, so try the more restrictive
    // (keyword-leading) form first.
    Subquery(SubqueryRef<'input>),
    ParenJoin(ParenJoinRef<'input>),
    Inherited(InheritedTable<'input>),
    Table(PlainTable<'input>),
}

/// Join type: LEFT, RIGHT, FULL, INNER, CROSS, or plain JOIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JoinType {
    Left(LEFT),
    Right(RIGHT),
    Full(FULL),
    Inner(INNER),
    Cross(CROSS),
}

/// JOIN condition: ON expr or USING (col, ...)
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JoinCondition<'input> {
    On(JoinOn<'input>),
    Using(JoinUsing<'input>),
}

/// ON condition for JOIN
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JoinOn<'input> {
    pub on: ON,
    pub condition: Box<Expr<'input>>,
}

/// `AS alias` form of a JOIN USING alias.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JoinUsingAliasWithAs<'input> {
    pub r#as: AS,
    pub name: literal::AliasName<'input>,
}

/// `[AS] alias` suffix on a JOIN ... USING column list.
///
/// Variant ordering: `WithAs` (`AS name`) is longer than `Bare`
/// (`ident`); list it first.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum JoinUsingAlias<'input> {
    WithAs(JoinUsingAliasWithAs<'input>),
    Bare(literal::Ident<'input>),
}

/// USING clause for JOIN: `USING (col, ...) [[AS] alias]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JoinUsing<'input> {
    pub using: USING,
    pub columns:
        Surrounded<punct::LParen, Seq0<literal::AliasName<'input>, punct::Comma>, punct::RParen>,
    pub alias: Option<JoinUsingAlias<'input>>,
}

/// A single join suffix:
/// `[NATURAL] [LEFT|RIGHT|FULL|INNER|CROSS] [OUTER] JOIN table [ON expr | USING (...)]`.
///
/// `OUTER` is allowed (and traditionally written) after `LEFT`/`RIGHT`/`FULL`.
/// Postgres accepts but does not require it; the grammar accepts it after any
/// join type for simplicity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct JoinSuffix<'input> {
    pub natural: Option<NATURAL>,
    pub join_type: Option<JoinType>,
    pub outer: Option<OUTER>,
    pub join: JOIN,
    /// The join's right operand — a full (recursive) `TableRef`, not a
    /// bare `SimpleTableRef`. This is what makes deferred `ON` clauses
    /// work: `a JOIN b JOIN c ON x ON y` parses as
    /// `a JOIN (b JOIN c ON x) ON y` because this `table` recursively
    /// consumes `b JOIN c ON x` (its inner `Seq0<JoinSuffix>` stops at the
    /// `ON`, which is not a join keyword), leaving `ON y` for `condition`.
    pub table: Box<TableRef<'input>>,
    pub condition: Option<JoinCondition<'input>>,
}

/// TABLESAMPLE clause: `TABLESAMPLE method (args) [REPEATABLE (seed)]`.
/// Attached to a single table reference (not to joined results).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableSampleClause<'input> {
    pub tablesample: TABLESAMPLE,
    pub method: literal::AliasName<'input>,
    pub args: Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
    pub repeatable: Option<TableSampleRepeatable<'input>>,
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableSampleRepeatable<'input> {
    pub repeatable: REPEATABLE,
    pub seed: Surrounded<punct::LParen, Expr<'input>, punct::RParen>,
}

/// A table reference that may have zero or more JOIN suffixes.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct TableRef<'input> {
    pub base: SimpleTableRef<'input>,
    pub tablesample: Option<TableSampleClause<'input>>,
    pub joins: Seq0<JoinSuffix<'input>, (), OptionalTrailing>,
}

/// WHERE-clause body: either a normal expression or the cursor-current
/// row filter `CURRENT OF cursor_name` (used by positioned UPDATE/DELETE).
///
/// Variant ordering: `CurrentOf` must come before `Expr` since `CURRENT`
/// is a specific keyword lead-in.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum WhereCondition<'input> {
    CurrentOf(WhereCurrentOf<'input>),
    Expr(Expr<'input>),
}

/// `CURRENT OF cursor_name` filter.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WhereCurrentOf<'input> {
    pub current: CURRENT,
    pub of: OF,
    pub cursor: literal::AliasName<'input>,
}

/// WHERE clause: `WHERE expr` or `WHERE CURRENT OF cursor`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WhereClause<'input> {
    pub r#where: WHERE,
    pub condition: WhereCondition<'input>,
}

/// USING operator in ORDER BY: `USING > | USING < | USING ~<~ | ...`
///
/// Variant ordering: longer (4-char) locale operators before shorter (3-char),
/// then single-char `>` / `<` last.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum UsingOp<'input> {
    TildeLeqTilde(punct::TildeLeqTilde),
    TildeGeqTilde(punct::TildeGeqTilde),
    TildeLtTilde(punct::TildeLtTilde),
    TildeGtTilde(punct::TildeGtTilde),
    Gt(punct::Gt),
    Lt(punct::Lt),
    Custom(literal::CustomOp<'input>),
}

/// USING clause in ORDER BY: `USING op`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct UsingClause<'input> {
    pub using: USING,
    pub op: UsingOp<'input>,
}

/// Sort direction: ASC or DESC.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SortDir {
    Asc(ASC),
    Desc(DESC),
}

/// NULLS FIRST or NULLS LAST.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum NullsOrder {
    First((NULLS, FIRST)),
    Last((NULLS, LAST)),
}

/// A single ORDER BY item: `expr [ASC|DESC] [USING op] [NULLS FIRST|LAST]`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OrderByItem<'input> {
    pub expr: Expr<'input>,
    pub dir: Option<SortDir>,
    pub using: Option<UsingClause<'input>>,
    pub nulls: Option<NullsOrder>,
}

/// ORDER BY clause: `ORDER BY item [, item ...]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OrderByClause<'input> {
    pub order: ORDER,
    pub by: BY,
    pub items: Seq0<OrderByItem<'input>, punct::Comma>,
}

/// OFFSET clause: `OFFSET expr`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct OffsetClause<'input> {
    pub offset: OFFSET,
    pub count: Expr<'input>,
}

/// LIMIT clause: `LIMIT expr`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct LimitClause<'input> {
    pub limit: LIMIT,
    pub count: Expr<'input>,
}

/// `FIRST` or `NEXT` keyword in FETCH clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchFirstOrNext {
    First(FIRST),
    Next(NEXT),
}

/// `ROW` or `ROWS` keyword in FETCH clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchRowOrRows {
    Rows(ROWS),
    Row(ROW),
}

/// `ONLY` or `WITH TIES` — FETCH clause termination mode.
///
/// `WithTies` declared first since it's longer and both start after a keyword.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchMode {
    WithTies((WITH, TIES)),
    Only(ONLY),
}

/// FETCH without count: `{ ROW | ROWS } { ONLY | WITH TIES }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FetchNoCount {
    pub row_or_rows: FetchRowOrRows,
    pub mode: FetchMode,
}

/// FETCH with count: `expr { ROW | ROWS } { ONLY | WITH TIES }`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum FetchFirstBody<'input> {
    NoCount(FetchNoCount),
    WithCount(FetchWithCount<'input>),
}

/// `FETCH { FIRST | NEXT } [count] { ROW | ROWS } { ONLY | WITH TIES }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct FetchFirstClause<'input> {
    pub fetch: FETCH,
    pub first_or_next: FetchFirstOrNext,
    pub body: FetchFirstBody<'input>,
}

/// An item in the limit/offset/fetch sequence. Allows any order of
/// LIMIT, OFFSET, and FETCH FIRST clauses.
///
/// `FetchFirst` must come before `Offset` and `Limit` since `FETCH` is
/// a keyword that doesn't overlap with LIMIT/OFFSET.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LimitOffsetItem<'input> {
    FetchFirst(FetchFirstClause<'input>),
    Limit(LimitClause<'input>),
    Offset(OffsetClause<'input>),
}

/// FOR UPDATE / FOR SHARE / FOR NO KEY UPDATE / FOR KEY SHARE locking clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForUpdateClause<'input> {
    pub r#for: FOR,
    pub mode: LockingMode,
    /// Optional `OF table[, ...]` restricting the lock to a subset of the
    /// FROM-list entries (plain tables or table aliases).
    pub of: Option<ForUpdateOf<'input>>,
    /// Optional wait-behavior modifier: `NOWAIT` or `SKIP LOCKED`.
    pub wait: Option<ForUpdateWait>,
}

/// `OF name[, ...]` in a `FOR UPDATE` locking clause.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ForUpdateOf<'input> {
    pub of: OF,
    pub names: Seq0<crate::tokens::ColId<'input>, punct::Comma>,
}

/// `NOWAIT | SKIP LOCKED` suffix on a `FOR UPDATE` clause.
///
/// Variant ordering: `SkipLocked` (two tokens) before `Nowait` (one token).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum ForUpdateWait {
    SkipLocked((SKIP, LOCKED)),
    Nowait(NOWAIT),
}

/// Lock strength for `SELECT ... FOR ...` locking clauses.
///
/// Variant ordering: longer (`NO KEY UPDATE`, `KEY SHARE`) before shorter
/// (`UPDATE`, `SHARE`) so longest-match wins.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum LockingMode {
    NoKeyUpdate((NO, KEY, UPDATE)),
    KeyShare((KEY, SHARE)),
    Update(UPDATE),
    Share(SHARE),
}

/// GROUP BY clause: `GROUP BY item, ...` where each item is an expression
/// or one of the grouping primitives (GROUPING SETS, ROLLUP, CUBE).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GroupByClause<'input> {
    pub group: GROUP,
    pub by: BY,
    /// Optional `DISTINCT` / `ALL` modifier (Postgres 16+).
    pub modifier: Option<GroupByModifier>,
    pub items: Seq0<GroupByItem<'input>, punct::Comma>,
}

/// `GROUP BY [DISTINCT|ALL]` modifier.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum GroupByModifier {
    Distinct(DISTINCT),
    All(ALL),
}

/// `GROUPING SETS ( item, ... )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct GroupingSetsItem<'input> {
    pub grouping_sets: (GROUPING, SETS),
    pub groups:
        Surrounded<punct::LParen, Seq0<Box<GroupByItem<'input>>, punct::Comma>, punct::RParen>,
}

/// `ROLLUP ( item, ... )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct RollupItem<'input> {
    pub rollup: ROLLUP,
    pub items:
        Surrounded<punct::LParen, Seq0<Box<GroupByItem<'input>>, punct::Comma>, punct::RParen>,
}

/// `CUBE ( item, ... )`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CubeItem<'input> {
    pub cube: CUBE,
    pub items:
        Surrounded<punct::LParen, Seq0<Box<GroupByItem<'input>>, punct::Comma>, punct::RParen>,
}

/// A single element in a GROUP BY clause.
///
/// Variant ordering: two-keyword primitives first (`GROUPING SETS`), then
/// single-keyword primitives (`ROLLUP`, `CUBE`), then the catch-all `Expr`
/// which also handles `(a, b)` row-style groupings.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum GroupByItem<'input> {
    GroupingSets(GroupingSetsItem<'input>),
    Rollup(RollupItem<'input>),
    Cube(CubeItem<'input>),
    Expr(Box<Expr<'input>>),
}

/// HAVING clause: `HAVING expr`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct HavingClause<'input> {
    pub having: HAVING,
    pub condition: Expr<'input>,
}

/// A single named window definition: `name AS (inline_window_spec)`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WindowDef<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub r#as: AS,
    pub spec: Surrounded<
        punct::LParen,
        crate::ast::shared::expr::InlineWindowSpec<'input>,
        punct::RParen,
    >,
}

/// `WINDOW name AS (...)[, name AS (...), ...]` clause in SELECT.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct WindowClause<'input> {
    pub window: WINDOW,
    pub defs: Seq1<WindowDef<'input>, punct::Comma>,
}

/// `INTO [TEMP|TEMPORARY|UNLOGGED] [TABLE] target` clause for the
/// Postgres `SELECT ... INTO new_table` statement form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SelectIntoClause<'input> {
    pub into: INTO,
    pub temp: Option<crate::ast::ddl::table::TempKw>,
    pub unlogged: Option<UNLOGGED>,
    pub table: Option<TABLE>,
    pub target: crate::ast::shared::names::QualifiedName<'input>,
    pub using: Option<crate::ast::ddl::table::UsingAccessMethodClause<'input>>,
}

/// `DISTINCT` or `DISTINCT ON (exprs)` qualifier on a SELECT.
///
/// Variant ordering: `On` (longer, starts with `DISTINCT ON`) before `All`
/// (just `DISTINCT`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SelectDistinct<'input> {
    On(SelectDistinctOn<'input>),
    All(DISTINCT),
}

#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SelectDistinctOn<'input> {
    pub distinct: DISTINCT,
    pub on: ON,
    pub exprs: Surrounded<
        punct::LParen,
        Seq0<crate::ast::shared::expr::Expr<'input>, punct::Comma>,
        punct::RParen,
    >,
}

/// SELECT statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["dql"])]
#[format_tokens(group(consistent))]
pub struct SelectStmt<'input> {
    pub select: SELECT,
    pub distinct: Option<Box<SelectDistinct<'input>>>,
    /// SELECT item list. Wrapped in `Option` (with `Seq1` inside) so
    /// the parser fork-and-tries the items via `Option::parse`. This both
    /// allows the empty form `SELECT FROM tbl` (a regression-test case) and
    /// avoids over-eager peeks in `Expr` consuming the next clause keyword.
    #[format_tokens(group(consistent), indent, break(flat = " ", broken = "\n"))]
    pub items: Option<Box<Seq1<SelectItem<'input>, punct::Comma>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub into: Option<Box<SelectIntoClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub from_clause: Option<Box<FromClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub where_clause: Option<Box<WhereClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub group_by: Option<Box<GroupByClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub having: Option<Box<HavingClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub window: Option<Box<WindowClause<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub order_by: Option<Box<OrderByClause<'input>>>,
    /// First LIMIT / OFFSET / FETCH FIRST clause. Postgres allows both
    /// `LIMIT x OFFSET y` and `OFFSET y LIMIT x` and `FETCH FIRST n ROWS ONLY`.
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub limit_offset_1: Option<Box<LimitOffsetItem<'input>>>,
    /// Optional second clause (e.g. OFFSET after LIMIT, or vice versa).
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub limit_offset_2: Option<Box<LimitOffsetItem<'input>>>,
    #[format_tokens(break(flat = " ", broken = "\n"))]
    pub for_update: Option<Box<ForUpdateClause<'input>>>,
}

impl<'input> SelectStmt<'input> {
    /// Number of items in the SELECT list (zero if the list is empty,
    /// e.g. the regression-test form `SELECT FROM tbl`).
    pub fn item_count(&self) -> usize {
        self.items.as_deref().map_or(0, |s| s.len())
    }

    /// Iterate over the SELECT items.
    pub fn items(&self) -> impl Iterator<Item = &SelectItem<'input>> {
        self.items.as_deref().into_iter().flat_map(|s| s.iter())
    }
}

/// A SELECT body that can appear in subqueries -- WITH, SELECT, or VALUES.
/// WithBody must come before Select so `WITH ... SELECT` matches before bare `SELECT`.
/// SelectStmt must come before ValuesStmt so `SELECT` keyword wins over ambiguity.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SelectBody<'input> {
    WithBody(Box<crate::ast::shared::with_clause::WithStatement<'input>>),
    Select(Box<SelectStmt<'input>>),
    Values(ValuesBody<'input>),
}

/// VALUES body: `VALUES (expr, ...), (expr, ...)`
/// Can appear standalone or inside subqueries.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct ValuesBody<'input> {
    pub values: VALUES,
    pub rows: Seq0<
        Surrounded<punct::LParen, Seq0<Expr<'input>, punct::Comma>, punct::RParen>,
        punct::Comma,
    >,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::dml::select::SelectStmt;

    /// Parse `src` as a complete `SELECT` through the logos lex pass.
    fn parse_select_classified(src: &'static str) {
        let mut input = crate::tokens::test_input(src);
        SelectStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
        assert!(
            input.is_empty(),
            "leftover parsing {src:?}: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// A JOIN's `ON`/`USING` may be deferred: `a JOIN b JOIN c ON x ON y`
    /// parses as `a JOIN (b JOIN c ON x) ON y`. The left-deep form must be
    /// unchanged, and the recursion must also work inside parentheses.
    #[test]
    fn parse_stacked_on_joins() {
        for src in [
            "SELECT * FROM a JOIN b JOIN c ON x ON y", // deferred ON
            "SELECT * FROM a JOIN b ON x JOIN c ON y", // left-deep
            "SELECT * FROM t1 LEFT JOIN \
             (t2 LEFT JOIN t3 FULL JOIN t4 ON p ON q) \
             LEFT JOIN t5 ON r ON s", // parenthesised
            "SELECT * FROM a CROSS JOIN b JOIN c ON x", // unqualified mixed in
        ] {
            parse_select_classified(src);
        }
    }

    #[test]
    fn parse_rows_from() {
        for src in [
            "SELECT * FROM ROWS FROM(f(1), g(2)) WITH ORDINALITY AS z(a, b, c, ord)",
            "SELECT * FROM ROWS FROM(getf(1) AS (id int, nm text)) AS z(a, b)",
        ] {
            parse_select_classified(src);
        }
    }

    #[test]
    fn parse_xmltable_and_lateral() {
        for src in [
            "SELECT * FROM XMLTABLE('/r' PASSING d COLUMNS \
             a int PATH '@id', o FOR ORDINALITY, n text PATH 'N' NOT NULL, \
             p text DEFAULT 'x') AS f (x, y)",
            "SELECT * FROM XMLTABLE(XMLNAMESPACES('http://x' AS zz), '/zz:r' \
             PASSING BY REF d COLUMNS a int PATH 'zz:a')",
            // LATERAL now wraps XMLTABLE / a function table / a subquery.
            "SELECT * FROM d, LATERAL XMLTABLE('/r' PASSING data COLUMNS a int) jt",
            "SELECT * FROM t, LATERAL generate_series(1, t.n) g",
            "SELECT * FROM t, LATERAL (SELECT 1) s",
        ] {
            parse_select_classified(src);
        }
    }

    #[test]
    fn parse_json_table() {
        for src in [
            // empty COLUMNS, ordinality, typed, EXISTS, behaviors
            "SELECT * FROM JSON_TABLE(NULL, '$' COLUMNS ())",
            "SELECT * FROM JSON_TABLE('[]', 'strict $.a' COLUMNS (js2 int PATH '$') ERROR ON ERROR)",
            "SELECT * FROM JSON_TABLE(jsonb '1', '$' COLUMNS \
             (id FOR ORDINALITY, c int EXISTS PATH '$.a' UNKNOWN ON ERROR))",
            // FORMAT JSON / wrapper / quotes on columns
            "SELECT * FROM JSON_TABLE(js, 'lax $[*]' COLUMNS \
             (jsb jsonb FORMAT JSON PATH '$' OMIT QUOTES, jw json PATH '$' WITH WRAPPER))",
            // path-name, PASSING, NESTED, table alias with column aliases
            "SELECT * FROM JSON_TABLE(js, '$' AS root PASSING 1 AS a \
             COLUMNS (a int, NESTED PATH '$.b' AS nb COLUMNS (c int PATH '$'))) AS jt (x, y)",
        ] {
            parse_select_classified(src);
        }
    }

    #[test]
    fn parse_simple_select() {
        let mut input = crate::tokens::test_input("SELECT 1 AS one");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.item_count(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_empty_items() {
        let mut input = crate::tokens::test_input("SELECT FROM emp");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.item_count(), 0);
        assert!(stmt.from_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_paren_join_cross() {
        let mut input = crate::tokens::test_input("SELECT * FROM (a CROSS JOIN b) AS tx");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_paren_join_using() {
        let mut input = crate::tokens::test_input("SELECT * FROM (a JOIN b USING (i)) AS x");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_paren_join_with_col_aliases() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM (a t1 (x, y) CROSS JOIN b t2 (p, q)) AS tx (a, b, c, d)",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// `CAST(...)` and `COLLATION FOR (...)` are PG `func_expr_common_subexpr`
    /// forms reachable from `func_table` (gram.y), so they can appear in a
    /// `FROM` clause alongside ordinary function-style table references.
    /// The create_view regression exercises `from coalesce(1,2) as c,
    /// collation for ('x'::text) col, ..., cast(1+2 as int4) as i4` —
    /// modelled via the new `SimpleTableRef::SpecialFunc` variant.
    #[test]
    fn parse_from_special_func_table() {
        for src in [
            "SELECT * FROM collation for ('x'::text)",
            "SELECT * FROM collation for ('x'::text) col",
            "SELECT * FROM cast(1+2 as int4)",
            "SELECT * FROM cast(1+2 as int4) as i4",
            "SELECT * FROM coalesce(1,2) as c, collation for ('x'::text) col, cast(1+2 as int4) as i4",
        ] {
            let mut input = crate::tokens::test_input(src);
            let _stmt =
                SelectStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_select_from_where() {
        let mut input = crate::tokens::test_input("SELECT f1 FROM BOOLTBL1 WHERE f1 = true");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.item_count(), 1);
        assert!(stmt.from_clause.is_some());
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn parse_select_star() {
        let mut input = crate::tokens::test_input("SELECT * FROM t");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.item_count(), 1);
    }

    #[test]
    fn parse_select_with_alias_keyword() {
        let mut input = crate::tokens::test_input("SELECT 1 AS true");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        let first = stmt.items().next().unwrap();
        let alias = first.alias.as_ref().unwrap();
        assert_eq!(alias.name(), "true");
    }

    #[test]
    fn parse_select_order_by() {
        let mut input = crate::tokens::test_input("SELECT f1 FROM t ORDER BY f1");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
    }

    #[test]
    fn parse_select_from_function() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM pg_input_error_info('junk', 'bool')");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
    }

    // --- ORDER BY enhancements ---

    #[test]
    fn parse_order_by_using() {
        let mut input = crate::tokens::test_input("SELECT f1 FROM t ORDER BY f1 using >");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_order_by_asc() {
        let mut input = crate::tokens::test_input("SELECT * FROM t ORDER BY f1 ASC");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_order_by_desc() {
        let mut input = crate::tokens::test_input("SELECT * FROM t ORDER BY f1 DESC");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_order_by_nulls_first() {
        let mut input = crate::tokens::test_input("SELECT * FROM t ORDER BY f1 NULLS FIRST");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_order_by_desc_nulls_last() {
        let mut input = crate::tokens::test_input("SELECT * FROM t ORDER BY f1 DESC NULLS LAST");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.order_by.is_some());
        assert!(input.is_empty());
    }

    // --- OFFSET/LIMIT ---

    #[test]
    fn parse_select_offset() {
        let mut input = crate::tokens::test_input("SELECT 1 OFFSET 0");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.limit_offset_1.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_limit() {
        let mut input = crate::tokens::test_input("SELECT 1 LIMIT 1");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.limit_offset_1.is_some());
        assert!(input.is_empty());
    }

    // --- FOR UPDATE ---

    #[test]
    fn parse_select_from_only() {
        let mut input = crate::tokens::test_input("SELECT f1 FROM ONLY t");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_from_only_with_alias() {
        let mut input = crate::tokens::test_input("SELECT f1 FROM ONLY t AS x");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_from_qualified_name() {
        let mut input = crate::tokens::test_input("SELECT * FROM myschema.mytable");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_window_clause_standalone() {
        use super::WindowClause;
        let mut input = crate::tokens::test_input("WINDOW w AS (PARTITION BY y ORDER BY z)");
        let wc = WindowClause::parse(&mut input).unwrap();
        assert_eq!(wc.defs.len(), 1);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_window_clause() {
        let mut input = crate::tokens::test_input(
            "SELECT sum(x) OVER w FROM t WINDOW w AS (PARTITION BY y ORDER BY z)",
        );
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.window.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_frame_rows_between() {
        let mut input = crate::tokens::test_input(
            "SELECT sum(x) OVER (ORDER BY y ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM t",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_over_named() {
        let mut input = crate::tokens::test_input("SELECT sum(x) OVER w FROM t");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_from_alias_with_column_list() {
        let mut input = crate::tokens::test_input("SELECT * FROM tbl AS t (a, b, c)");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_from_qualified_name_with_alias() {
        let mut input = crate::tokens::test_input("SELECT * FROM s.t AS x");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.from_clause.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_join_using_alias() {
        let mut input = crate::tokens::test_input("SELECT * FROM a JOIN b USING (i) AS x");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_join_using_alias_where() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM a JOIN b USING (i) AS x WHERE x.i = 1");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_func_with_ordinality() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM rngfunct(1) WITH ORDINALITY AS z(a, b, ord)");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_func_column_def_list() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM test_ret_set_rec_dyn(1500) AS (a int, b int, c int)",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// Corpus: `select * from json_populate_recordset(row(0::int),'[...]') q (a text, b text)`
    /// — function-call FROM source with a bare-alias-name column-def list (no AS).
    ///
    /// DEFERRED: the `FuncTableAlias` enum disambiguates via first-token kind
    /// only, and both `ColumnDefList` and `Plain(TableAlias)` start with Ident
    /// (since `ColumnDefList::name` is `Option<AliasName>`). The codegen
    /// dispatcher commits on `Plain` for any non-`AS` first token, so the bare-
    /// name + column-def-list form (`q (a text, b text)`) falls into
    /// `Plain(TableAlias::Bare)` and the columns survive as leftover. Fixing
    /// this needs either a recursa-level longest-match-wins fallback for
    /// overlapping enum first-sets, or restructuring `FuncTableAlias` so each
    /// alternative has a distinct prefix (e.g. require the LParen first).
    /// Affects 8 `json_populate_recordset(...) q (a text, b text)` PG-accepts
    /// fallbacks across json.sql / jsonb.sql.
    #[test]
    #[ignore]
    fn parse_select_func_table_bare_alias_col_def() {
        let mut input = crate::tokens::test_input(
            "select * from json_populate_recordset(row(0::int),'[{\"a\":\"1\"}]') q (a text, b text)",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "leftover at {}: {:?}",
            input.byte_offset(),
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_select_func_with_ordinality_unnest() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM unnest(array['a','b']) WITH ORDINALITY AS z(a, ord)",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_natural_join() {
        let mut input = crate::tokens::test_input("SELECT * FROM a NATURAL JOIN b");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_natural_left_join() {
        let mut input = crate::tokens::test_input("SELECT * FROM a NATURAL LEFT JOIN b");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_left_outer_join_using() {
        let mut input = crate::tokens::test_input("SELECT * FROM a LEFT OUTER JOIN b USING (i)");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_full_outer_join_using() {
        let mut input = crate::tokens::test_input("SELECT * FROM a FULL OUTER JOIN b USING (i)");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_right_outer_join_on() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM a RIGHT OUTER JOIN b ON a.i = b.i");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_paren_join_simple() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM a LEFT JOIN (b JOIN c ON b.x = c.x) ON a.y = b.y",
        );
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_paren_join_with_subquery_inside() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM a LEFT JOIN (b JOIN (SELECT 1 AS x) s ON b.x = s.x) ON a.y = b.y",
        );
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_paren_join_leading_subquery() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM a LEFT JOIN ((SELECT * FROM b) s LEFT JOIN c ON s.x = c.x) ON a.y = s.y",
        );
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_group_by_grouping_sets_simple() {
        let mut input = crate::tokens::test_input(
            "SELECT sum(c) FROM t GROUP BY GROUPING SETS ((), (a), (a,b))",
        );
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.group_by.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_group_by_rollup() {
        let mut input = crate::tokens::test_input("SELECT sum(c) FROM t GROUP BY ROLLUP (a, b)");
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_group_by_cube() {
        let mut input = crate::tokens::test_input("SELECT sum(c) FROM t GROUP BY CUBE (a, b)");
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_group_by_grouping_sets_nested() {
        let mut input = crate::tokens::test_input(
            "SELECT sum(c) FROM t GROUP BY GROUPING SETS (ROLLUP(a), CUBE(b))",
        );
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_group_by_mixed_primitives() {
        let mut input =
            crate::tokens::test_input("SELECT sum(c) FROM t GROUP BY a, ROLLUP(b), CUBE(c)");
        SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_group_by_distinct_modifier() {
        // Regression: groupingsets.sql uses `GROUP BY DISTINCT ROLLUP(a, b), ROLLUP(a, c)`
        // and `GROUP BY ALL ROLLUP(a, b), ROLLUP(a, c)`.
        for src in [
            "SELECT a FROM t GROUP BY DISTINCT ROLLUP(a, b), ROLLUP(a, c)",
            "SELECT a FROM t GROUP BY ALL ROLLUP(a, b), ROLLUP(a, c)",
        ] {
            let mut input = crate::tokens::test_input(src);
            SelectStmt::parse(&mut input).unwrap();
            assert!(input.is_empty(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_select_for_update() {
        let mut input = crate::tokens::test_input("SELECT f1 FROM t FOR UPDATE");
        let stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(stmt.for_update.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_locking_variants() {
        // Regression: matview.sql uses `FOR SHARE`. Postgres also supports
        // `FOR NO KEY UPDATE` and `FOR KEY SHARE`.
        for src in [
            "SELECT * FROM t FOR SHARE",
            "SELECT * FROM t FOR NO KEY UPDATE",
            "SELECT * FROM t FOR KEY SHARE",
        ] {
            let mut input = crate::tokens::test_input(src);
            let stmt = SelectStmt::parse(&mut input).unwrap();
            assert!(stmt.for_update.is_some(), "no locking clause: {src:?}");
            assert!(input.is_empty(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_order_by_using_locale_ops() {
        // Postgres allows custom operators in ORDER BY ... USING <op>.
        // The locale-aware operators ~<~, ~>~, ~<=~, ~>=~ are the main ones.
        for src in [
            "SELECT * FROM t ORDER BY c USING ~<~",
            "SELECT * FROM t ORDER BY c USING ~>~",
            "SELECT * FROM t ORDER BY c USING ~<=~",
            "SELECT * FROM t ORDER BY c USING ~>=~",
        ] {
            let mut input = crate::tokens::test_input(src);
            let _stmt = SelectStmt::parse(&mut input).unwrap_or_else(|e| panic!("{src}: {e}"));
            assert!(input.is_empty(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_select_unicode_alias() {
        let mut input =
            crate::tokens::test_input(r#"SELECT U&'d\0061t\+000061' AS U&"d\0061t\+000061""#);
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_select_unicode_alias_uescape() {
        let mut input = crate::tokens::test_input(
            r#"SELECT U&'d!0061t\+000061' UESCAPE '!' AS U&"d*0061t\+000061" UESCAPE '*'"#,
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    // --- LIMIT / OFFSET / FETCH FIRST ---

    #[test]
    fn parse_offset_before_limit() {
        // Standard SQL order: OFFSET before LIMIT.
        let mut input = crate::tokens::test_input("SELECT * FROM t ORDER BY x OFFSET 10 LIMIT 5");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_limit_before_offset() {
        // Postgres order: LIMIT before OFFSET.
        let mut input = crate::tokens::test_input("SELECT * FROM t ORDER BY x LIMIT 5 OFFSET 10");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_fetch_first_rows_only() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM t ORDER BY x FETCH FIRST 5 ROWS ONLY");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_fetch_first_row_with_ties() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM t ORDER BY x FETCH FIRST 2 ROW WITH TIES");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_fetch_first_rows_no_count() {
        // FETCH FIRST ROWS WITH TIES — count is omitted (defaults to 1).
        let mut input =
            crate::tokens::test_input("SELECT * FROM t ORDER BY x FETCH FIRST ROWS WITH TIES");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_fetch_next_row_only() {
        let mut input =
            crate::tokens::test_input("SELECT * FROM t ORDER BY x FETCH NEXT 1 ROW ONLY");
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_offset_then_fetch() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM t ORDER BY x OFFSET 10 FETCH FIRST 5 ROWS ONLY",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_fetch_then_offset() {
        let mut input = crate::tokens::test_input(
            "SELECT * FROM t ORDER BY x FETCH FIRST 5 ROWS WITH TIES OFFSET 10",
        );
        let _stmt = SelectStmt::parse(&mut input).unwrap();
        assert!(
            input.is_empty(),
            "remaining: {}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// PG accepts `"normalize"('abc', 'def')` as a function call — the
    /// quoted ident escapes NORMALIZE as a user-defined function name.
    /// Without `Expr::QuotedFunc`, the Pratt nud kind-match for
    /// `QuotedIdent` would commit to `Expr::ColumnRef` and strand the
    /// trailing `(args)`. The new atom registers under the `QuotedIdent`
    /// kind ahead of `ColumnRef`, so the function-call form wins.
    #[test]
    fn parse_quoted_keyword_function_name() {
        for src in [
            "SELECT \"normalize\"('abc', 'def')",
            "SELECT \"select\"('a')",
            "SELECT \"trim\"('abc')",
            "SELECT \"any\"('a')",
        ] {
            let mut input = crate::tokens::test_input(src);
            let _stmt =
                SelectStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(
                input.is_empty(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }
}
