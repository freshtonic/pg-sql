/// SELECT statement AST.
use crate::ast::dml::values::Subquery;
use crate::ast::shared::expr::{
    CastType, Expr, FunctionApplicationExpr, FunctionCallApplication, ParenthesizedClose,
    ParenthesizedOpen, XmlPassingBy,
};
// The SQL/JSON query clauses exist from 17, as `JSON_TABLE` does.
#[cfg(feature = "since-pg17")]
use crate::ast::shared::expr::{
    JsonBehaviorClause, JsonEncoding, JsonOnBehavior, JsonPassing, JsonQuotes, JsonWrapper,
};
use crate::ast::shared::names::QualifiedName;
use crate::tokens::literal;

recursa::ast_node! {
    /// A bare wildcard token used by target lists and inherited table names.
    #[derive(Debug)]
    pub enum SelectStar {
        #[tok(STAR)]
        Value,
    }
}

recursa::ast_node! {
    /// An expression target with its optional output alias.
    #[derive(Debug)]
    pub struct SelectExprItem {
        pub expr: Expr,
        pub alias: Option<Alias>,
    }
}

recursa::ast_node! {
    /// A SELECT/RETURNING target item.
    ///
    /// PostgreSQL gives bare `*` its own `target_el` production rather than
    /// admitting it as an expression. The enum also makes `* AS alias`
    /// unconstructible while preserving aliases for expression targets.
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum SelectItem {
        Star(SelectStar),
        Expr(SelectExprItem),
    }
}

recursa::ast_node! {
    /// Alias with explicit AS keyword: `AS name [UESCAPE 'c']`.
    /// Uses AliasName so keywords are accepted (e.g., `SELECT 1 AS true`).
    /// The optional UESCAPE suffix applies when the alias is a unicode-quoted
    /// identifier (`U&"..."`).
    #[derive(Debug)]
    pub struct AsAlias {
        #[tok(AS, this)]
        pub name: literal::AliasName,
        pub uescape: Option<crate::ast::shared::expr::UescapeSuffix>,
    }
}

recursa::ast_node! {
    /// AS alias clause, or bare alias.
    ///
    /// Variant ordering: WithAs (`AS name`) has a longer first_pattern than
    /// Bare (`ident`), so longest-match-wins picks it when AS is present.
    #[derive(Debug)]
    pub enum Alias {
        WithAs(AsAlias),
        Bare(literal::SelectBareAliasName),
    }
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

recursa::ast_node! {
    /// FROM clause: `FROM table [, table ...]`.
    ///
    /// PostgreSQL's `from_list` is nonempty: `FROM` commits to at least one
    /// `TableRef` (`SELECT FROM` must fail; only the target list may be empty).
    ///
    /// The clause owns a fit-or-break group so that `FROM <list>` is measured on
    /// its own. A statement that breaks its target list keeps `FROM` attached to
    /// the last item while it still fits, and moves `FROM` onto its own line only
    /// when the clause itself is too wide.
    #[derive(Debug)]
    #[pretty(group = consistent)]
    #[tok(FROM, this)]
    pub struct FromClause {
        #[sep(COMMA)]
        pub tables: one_or_many!(TableRef),
    }
}

recursa::ast_node! {
    /// Table name with inheritance marker and optional alias: `person* p`
    #[derive(Debug)]
    pub struct InheritedTable {
        pub name: QualifiedName,
        #[tok(STAR, this)]
        pub alias: Option<literal::Ident>,
    }
}

recursa::ast_node! {
    /// `AS name [(col1, col2)]` table alias form.
    #[derive(Debug)]
    pub struct TableAliasWithAs {
        /// gram.y `alias_clause: AS ColId ...`.
        #[tok(AS, this)]
        pub name: crate::tokens::ColId,
        pub columns: Option<TableAliasColumnList>,
    }
}

recursa::ast_node! {
    /// Bare `name [(col1, col2)]` table alias form. PostgreSQL's `alias_clause`
    /// admits `ColId`; the narrower category keeps join and clause starters from
    /// being consumed as aliases.
    #[derive(Debug)]
    pub struct TableAliasBare {
        pub name: crate::tokens::ColId,
        pub columns: Option<TableAliasColumnList>,
    }
}

recursa::ast_node! {
    /// Parenthesized column aliases attached to a table alias.
    ///
    /// The delimiters wrap the list as a whole. Attaching them to the repeated
    /// field would instead require a fresh pair of parentheses around every
    /// element after a comma.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct TableAliasColumnList(
        /// gram.y `name_list`.
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(crate::tokens::ColId),
    );
}

recursa::ast_node! {
    /// Table alias: `AS name [(col1, col2)]` or bare `name [(col1, col2)]`.
    ///
    /// Variant ordering: `WithAs` (`AS`) before `Bare` (ident).
    #[derive(Debug)]
    pub enum TableAlias {
        WithAs(TableAliasWithAs),
        Bare(TableAliasBare),
    }
}

recursa::ast_node! {
    /// Subquery in FROM: `(SELECT ...) AS alias`
    #[derive(Debug)]
    pub struct SubqueryRef {
        pub open: SelectLParen,
        pub query: boxed!(Subquery),
        pub close: SelectRParen,
        pub alias: Option<TableAlias>,
    }
}

recursa::ast_node! {
    /// A parenthesized FROM source: gram.y `select_with_parens
    /// opt_alias_clause` or `'(' joined_table ')' alias_clause`. The
    /// parentheses hold a query or a join, never a lone relation: `FROM (t)`
    /// is a syntax error.
    ///
    /// Variant ordering: both lead with `(` and part on what follows it.
    #[derive(Debug)]
    pub enum ParenTableRef {
        Query(ParenQueryRef),
        Join(ParenJoinRef),
    }
}

recursa::ast_node! {
    /// `( query ) [alias]`.
    #[derive(Debug)]
    pub struct ParenQueryRef {
        pub open: SelectLParen,
        pub body: ParenTableBody,
        pub close: SelectRParen,
        pub alias: Option<PlainTableAlias>,
    }
}

recursa::ast_node! {
    /// The query inside a parenthesized FROM source. It is an enum of one
    /// variant so that the scoped resolutions against [`SelectWithParens`]
    /// have a variant to stand on.
    ///
    /// [`SelectWithParens`]: crate::ast::dml::values::SelectWithParens
    #[derive(Debug)]
    pub enum ParenTableBody {
        #[parse(lr_conflict(
        action = reduce,
        against = ast::dml::values::SelectWithParens,
        lookahead = { RPAREN },
        expect = 1
    ))]
        #[parse(lr_conflict(
        action = shift,
        against = ast::dml::values::SelectWithParens,
        lookahead = { RPAREN_SELECT_LA },
        expect = 1
    ))]
        Query(boxed!(Subquery)),
    }
}

recursa::ast_node! {
    /// `( joined_table ) [alias]`: gram.y `'(' joined_table ')' alias_clause`,
    /// and with no alias the `joined_table: '(' joined_table ')'` that stands
    /// as the operand of a further join.
    #[derive(Debug)]
    pub struct ParenJoinRef {
        pub group: ParenJoinGroup,
        pub alias: Option<PlainTableAlias>,
    }
}

recursa::ast_node! {
    /// gram.y `joined_table` (gram.y:13529): `'(' joined_table ')'`, or a
    /// `table_ref` with at least one join after it.
    ///
    /// Variant ordering: both can lead with `(`. A group is a `joined_table`
    /// when `)` follows it, and the base of a chain when a join does.
    #[derive(Debug)]
    pub enum JoinedTable {
        Chain(boxed!(JoinChain)),
        Group(ParenJoinGroup),
    }
}

recursa::ast_node! {
    /// `table_ref join ...`: one table reference and one or more joins.
    #[derive(Debug)]
    pub struct JoinChain {
        pub base: SimpleTableRef,
        pub joins: one_or_many!(JoinSuffix),
    }
}

recursa::ast_node! {
    /// `'(' joined_table ')'`, with no alias. [`ParenJoinRef`] and
    /// [`JoinedTable::Group`] share it, so the parser decides between them on
    /// the token after the `)` and not before it.
    #[derive(Debug)]
    pub struct ParenJoinGroup {
        pub open: SelectLParen,
        pub joined: boxed!(JoinedTable),
        pub close: SelectRParen,
    }
}

pub type SelectLParen = ParenthesizedOpen;

pub type SelectRParen = ParenthesizedClose;

recursa::ast_node! {
    /// `LATERAL (subquery) [alias]` — the parenthesized-subquery LATERAL form.
    #[derive(Debug)]
    pub struct LateralSubquery {
        #[tok(LPAREN, this, RPAREN)]
        pub query: boxed!(Subquery),
        pub alias: Option<PlainTableAlias>,
    }
}

recursa::ast_node! {
    /// The table reference a `LATERAL` prefixes: a parenthesized subquery, a
    /// function table, or an `XMLTABLE` / `JSON_TABLE`.
    ///
    /// Variant ordering: `JsonTable` / `XmlTable` (soft keyword) before `Func`,
    /// which would otherwise reclaim them as ordinary function names.
    #[derive(Debug)]
    pub enum LateralBody {
        Subquery(LateralSubquery),
        // Added in 17: see `JsonTableRef`.
        #[cfg(feature = "since-pg17")]
        JsonTable(boxed!(JsonTableRef)),
        XmlTable(boxed!(XmlTableRef)),
        Func(boxed!(FuncTableRef)),
    }
}

recursa::ast_node! {
    /// `LATERAL` table reference in FROM: `LATERAL (VALUES(...)) v`,
    /// `LATERAL func(...)`, `LATERAL XMLTABLE(...)`, `LATERAL JSON_TABLE(...)`.
    #[derive(Debug)]
    pub struct LateralRef {
        #[tok(LATERAL, this)]
        pub body: LateralBody,
    }
}

recursa::ast_node! {
    /// gram.y `relation_expr_opt_alias`, the target of `MERGE INTO`: a
    /// `relation_expr` with an optional alias.
    #[derive(Debug)]
    pub struct PlainTable {
        pub relation: crate::ast::shared::names::RelationExpr,
        pub alias: Option<PlainTableAlias>,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum PlainTableAlias {
        WithAs(PlainTableAliasWithAs),
        Bare(PlainTableAliasBare),
    }
}

recursa::ast_node! {
    /// `AS name [(col, ...)]` form.
    #[derive(Debug)]
    pub struct PlainTableAliasWithAs {
        /// gram.y `alias_clause: AS ColId ...`.
        #[tok(AS, this)]
        pub name: crate::tokens::ColId,
        pub columns: Option<TableAliasColumnList>,
    }
}

recursa::ast_node! {
    /// Bare `name [(col, ...)]` form using PostgreSQL's `ColId` alias category.
    #[derive(Debug)]
    pub struct PlainTableAliasBare {
        pub name: crate::tokens::ColId,
        pub columns: Option<TableAliasColumnList>,
    }
}

recursa::ast_node! {
    /// A column definition inside a function-table column-def-list:
    /// `name type` (e.g., `a int`).
    #[derive(Debug)]
    pub struct FuncTableColumnDef {
        /// gram.y `TableFuncElement: ColId Typename opt_collate_clause`.
        pub name: crate::tokens::ColId,
        pub type_name: crate::ast::shared::expr::CastType,
        pub collate: Option<crate::ast::ddl::table::CollateClause>,
    }
}

recursa::ast_node! {
    /// `[AS] alias (col type, ...)` or just `(col type, ...)` -- the
    /// column definition list form for table-returning functions.
    #[derive(Debug)]
    pub struct ColumnDefList {
        /// gram.y `func_alias_clause: [AS] ColId '(' TableFuncElementList ')'`.
        #[tok(optional(AS), this)]
        pub name: Option<crate::tokens::ColId>,
        #[tok(LPAREN, this, RPAREN)]
        #[sep(COMMA)]
        pub columns: zero_or_many!(FuncTableColumnDef),
    }
}

recursa::ast_node! {
    /// Alias of a function table reference.
    ///
    /// gram.y:13694 `func_alias_clause: alias_clause | AS '('
    /// TableFuncElementList ')' | AS ColId '(' TableFuncElementList ')' | ColId
    /// '(' TableFuncElementList ')'`, where `alias_clause` is `[AS] ColId ['('
    /// name_list ')']`. So a list needs `AS` or a name before it, a named list
    /// holds names or definitions ([`FuncTableAliasColumnList`]), and a list
    /// with `AS` and no name holds definitions only.
    #[derive(Debug)]
    pub enum FuncTableAlias {
        WithAs(FuncTableAliasWithAs),
        Named(FuncTableAliasNamed),
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct FuncTableAliasWithAs {
        pub as_keyword: SelectAsKeyword,
        pub body: FuncTableAliasAfterAs,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SelectAsKeyword {
        #[tok(AS)]
        Value,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum FuncTableAliasAfterAs {
        Named(FuncTableAliasAsNamed),
        /// `AS ( definitions )`: with no alias name the list is a
        /// `TableFuncElementList`, never a `name_list`.
        Definitions(FuncTableDefinitionList),
    }
}

recursa::ast_node! {
    /// `'(' TableFuncElementList ')'`.
    #[derive(Debug)]
    pub struct FuncTableDefinitionList {
        pub open: SelectLParen,
        #[sep(COMMA)]
        pub columns: one_or_many!(FuncTableColumnDef),
        pub close: SelectRParen,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct FuncTableAliasAsNamed {
        /// gram.y `alias_clause: AS ColId ...`.
        pub name: crate::tokens::ColId,
        pub columns: Option<FuncTableAliasColumns>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct FuncTableAliasNamed {
        /// gram.y `alias_clause: ColId ...`; `Ident` (every non-reserved word)
        /// also admitted `left`, `join` and the other `type_func_name`
        /// keywords that continue a join chain.
        pub name: crate::tokens::ColId,
        pub columns: Option<FuncTableAliasColumns>,
    }
}

recursa::ast_node! {
    /// The parenthesized list of a function table alias: gram.y's `'('
    /// name_list ')'` or `'(' TableFuncElementList ')'`.
    #[derive(Debug)]
    pub struct FuncTableAliasColumns {
        pub open: SelectLParen,
        pub columns: FuncTableAliasColumnList,
        pub close: SelectRParen,
    }
}

recursa::ast_node! {
    /// Column names, or column definitions, and never a mix: PostgreSQL
    /// rejects `AS x (a int, b)`.
    ///
    /// Both lists lead with a `ColId`, and the parser tells them apart on the
    /// token after it: a `,` or `)` ends a name, anything else starts a type.
    #[derive(Debug)]
    pub enum FuncTableAliasColumnList {
        /// gram.y `name_list`.
        Names(#[sep(COMMA)] one_or_many!(crate::tokens::ColId)),
        /// gram.y `TableFuncElementList`.
        Definitions(#[sep(COMMA)] one_or_many!(FuncTableColumnDef)),
    }
}

recursa::ast_node! {
    /// Function call used as table reference with optional WITH ORDINALITY and alias.
    #[derive(Debug)]
    pub struct FuncTableRef {
        pub func: FunctionApplicationExpr,
        #[presence(WITH, ORDINALITY)]
        pub ordinality: bool,
        pub alias: Option<FuncTableAlias>,
    }
}

recursa::ast_node! {
    /// `FROM`-clause special-form function expression with optional alias.
    #[derive(Debug)]
    pub struct SpecialFuncTableRef {
        /// gram.y `func_table: func_expr_windowless opt_ordinality`, its
        /// `func_expr_common_subexpr` half. The field holds an [`Expr`]: the
        /// node `SELECT coalesce(a, b)` builds for the same text.
        pub func: boxed!(crate::ast::shared::expr::FuncExprCommonSubexpr),
        #[presence(WITH, ORDINALITY)]
        pub ordinality: bool,
        pub alias: Option<FuncTableAlias>,
    }
}

// --- JSON_TABLE table reference ---
//
// `JSON_TABLE(...)` is a SQL/JSON table function: it appears in FROM (and
// JOIN) and projects a jsonpath match into rows/columns. A grammar
// construct with its own `COLUMNS ( ... )` clause, NESTED paths and
// per-column behaviors — modeled as a dedicated `SimpleTableRef` variant.

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `[AS] ‹name›` — a path-variable name (after the JSON_TABLE path, or on a
    /// `NESTED PATH`).
    #[derive(Debug)]
    pub struct JsonTablePathName {
        #[tok(AS, this)]
        pub name: literal::AliasName,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `PATH '‹jsonpath›'` clause on a JSON_TABLE column.
    #[derive(Debug)]
    pub struct JsonTableColumnPath {
        #[tok(PATH, this)]
        pub path_str: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `FOR ORDINALITY` — the row-counter column kind.
    #[derive(Debug)]
    pub enum JsonTableOrdinality {
        #[tok(FOR, ORDINALITY)]
        Value,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// The typed-column tail: `‹type› [EXISTS] [FORMAT JSON ...] [PATH '...']
    /// [wrapper] [quotes] [behavior ON EMPTY] [behavior ON ERROR]`.
    ///
    /// `EXISTS` columns and regular columns are merged — `exists` is just an
    /// optional marker — and clauses are parsed permissively.
    #[derive(Debug)]
    pub struct JsonTableTypedColumn {
        pub ty: CastType,
        #[presence(EXISTS)]
        pub exists: bool,
        pub format: Option<JsonTableFormat>,
        pub path: Option<JsonTableColumnPath>,
        pub wrapper: Option<JsonWrapper>,
        pub quotes: Option<JsonQuotes>,
        /// gram.y `json_behavior_clause_opt`.
        pub on_behavior: Option<JsonBehaviorClause>,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `FORMAT JSON [ENCODING name]` within JSON_TABLE.
    ///
    /// The required FORMAT/JSON pair wraps the complete optional-encoding body.
    /// This keeps the clause present even when ENCODING is absent.
    #[derive(Debug)]
    #[tok(FORMAT, JSON, this)]
    pub struct JsonTableFormat {
        pub encoding: Option<JsonEncoding>,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// The tail of a non-`NESTED` column, after its name.
    ///
    /// `Ordinality` leads with `FOR`, `Typed` with a type — distinct first
    /// tokens, so the enum dispatches cleanly.
    #[derive(Debug)]
    #[allow(clippy::large_enum_variant)]
    pub enum JsonTableColumnKind {
        Ordinality(JsonTableOrdinality),
        Typed(JsonTableTypedColumn),
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// A non-`NESTED` JSON_TABLE column: `‹name› {FOR ORDINALITY | ‹type› ...}`.
    #[derive(Debug)]
    pub struct JsonTableValuedColumn {
        pub name: literal::AliasName,
        pub kind: JsonTableColumnKind,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `NESTED [PATH] '‹jsonpath›' [AS ‹name›] COLUMNS ( ... )` — projects a
    /// nested jsonpath into additional columns.
    #[derive(Debug)]
    pub struct JsonTableNestedColumn {
        #[tok(NESTED, optional(PATH), this)]
        pub path_str: literal::StringLit,
        pub as_name: Option<JsonTablePathName>,
        pub columns: JsonTableColumnList,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// One column of a JSON_TABLE `COLUMNS ( ... )` list.
    ///
    /// Variant ordering: `Nested` (leads with `NESTED`) before `Valued` — a
    /// column literally named `nested` would also match `Valued`'s
    /// keyword-permissive `AliasName`, so `Nested` is tried first and falls
    /// through on non-NESTED syntax.
    #[derive(Debug)]
    pub enum JsonTableColumn {
        Nested(JsonTableNestedColumn),
        Valued(JsonTableValuedColumn),
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `COLUMNS ( ‹column› [, ...] )` — the JSON_TABLE column list (may be empty).
    #[derive(Debug)]
    #[tok(COLUMNS, LPAREN, this, RPAREN)]
    pub struct JsonTableColumnList {
        #[sep(COMMA)]
        pub list: zero_or_many!(JsonTableColumn),
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// Inner contents of `JSON_TABLE ( ‹ctx› , ‹path› [AS name] [PASSING ...]
    /// COLUMNS ( ... ) [behavior ON ERROR] )`.
    #[derive(Debug)]
    pub struct JsonTableInner {
        pub context: boxed!(Expr),
        pub context_format: Option<JsonTableFormat>,
        #[tok(COMMA, this)]
        pub path: boxed!(Expr),
        pub path_name: Option<JsonTablePathName>,
        pub passing: Option<JsonPassing>,
        pub column_list: JsonTableColumnList,
        pub on_error: Option<JsonOnBehavior>,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// The `JSON_TABLE ( ... )` construct.
    #[derive(Debug)]
    pub struct JsonTable {
        #[tok(JSON_TABLE, LPAREN, this, RPAREN)]
        pub inner: JsonTableInner,
    }
}

// Added in 17: `JSON_TABLE` (research, PostgreSQL 17, "Queries and expressions";
// REL_17_11 gram.y 14138-14283, `table_ref` 13519).
#[cfg(feature = "since-pg17")]
recursa::ast_node! {
    /// `JSON_TABLE(...)` as a table reference, with an optional table alias.
    #[derive(Debug)]
    pub struct JsonTableRef {
        pub table: JsonTable,
        pub alias: Option<TableAlias>,
    }
}

// --- XMLTABLE table reference ---
//
// `XMLTABLE(...)` is the XML analogue of `JSON_TABLE` — an XML table
// function in FROM/JOIN, projecting an XPath match into rows and columns.

recursa::ast_node! {
    /// One entry of an `XMLNAMESPACES(...)` list: `‹uri› AS ‹prefix›`.
    #[derive(Debug)]
    pub struct XmlNamespaceNamed {
        /// gram.y `xml_namespace_el: b_expr AS ColLabel` (exclusions as in
        /// `PositionInner`).
        pub uri: boxed!(crate::ast::shared::expr::BExpr),
        #[tok(AS, this)]
        pub prefix: literal::AliasName,
    }
}

recursa::ast_node! {
    /// The `DEFAULT ‹uri›` entry of an `XMLNAMESPACES(...)` list.
    #[derive(Debug)]
    pub struct XmlNamespaceDefault {
        /// gram.y `xml_namespace_el: DEFAULT b_expr`.
        #[tok(DEFAULT, this)]
        pub uri: boxed!(crate::ast::shared::expr::BExpr),
    }
}

recursa::ast_node! {
    /// One namespace declaration: a `DEFAULT ‹uri›` or a `‹uri› AS ‹prefix›`.
    #[derive(Debug)]
    pub enum XmlNamespaceItem {
        Default(XmlNamespaceDefault),
        Named(XmlNamespaceNamed),
    }
}

recursa::ast_node! {
    /// `XMLNAMESPACES ( ‹item› [, ...] ) ,` — the optional namespace prefix of
    /// `XMLTABLE`. The trailing comma separates it from the row expression.
    #[derive(Debug)]
    #[tok(XMLNAMESPACES, LPAREN, this, RPAREN, COMMA)]
    pub struct XmlTableNamespaces {
        #[sep(COMMA)]
        pub items: one_or_many!(XmlNamespaceItem),
    }
}

recursa::ast_node! {
    /// `PATH '‹xpath›'` clause on an XMLTABLE column.
    #[derive(Debug)]
    pub struct XmlTableColumnPath {
        /// Before 17, `path` is no keyword and the option is REL_16_15 gram.y
        /// 13883 `xmltable_column_option_el: IDENT b_expr`. The action of
        /// `xmltable_column_el` (13805-13858) then rejects every name other
        /// than `path`; pg-sql accepts any `IDENT` here. Research, PostgreSQL
        /// 17, "Keywords" and "Queries and expressions" (`XMLTABLE`).
        #[cfg(not(feature = "since-pg17"))]
        pub option: crate::tokens::literal::IdentOnly,
        /// gram.y `xmltable_column_option_el: PATH b_expr` (exclusions as in
        /// `PositionInner`), so the path ends before a following `NOT NULL`.
        #[cfg_attr(feature = "since-pg17", tok(PATH, this))]
        pub xpath: boxed!(crate::ast::shared::expr::BExpr),
    }
}

recursa::ast_node! {
    /// `DEFAULT ‹expr›` clause on an XMLTABLE column.
    #[derive(Debug)]
    pub struct XmlTableColumnDefault {
        /// gram.y `xmltable_column_option_el: DEFAULT b_expr` (exclusions as in
        /// `PositionInner`), so the default ends before a following `NOT NULL`.
        #[tok(DEFAULT, this)]
        pub value: boxed!(crate::ast::shared::expr::BExpr),
    }
}

recursa::ast_node! {
    /// `NOT NULL` / `NULL` nullability marker on an XMLTABLE column.
    #[derive(Debug)]
    pub enum XmlTableColumnNull {
        #[tok(NOT, NULL)]
        NotNull,
        #[tok(NULL)]
        Null,
    }
}

recursa::ast_node! {
    /// `‹type› [PATH '...'] [DEFAULT expr] [NOT NULL|NULL]` — the typed-column tail.
    #[derive(Debug)]
    pub struct XmlTableTypedColumn {
        pub ty: CastType,
        pub path: Option<XmlTableColumnPath>,
        pub default: Option<XmlTableColumnDefault>,
        pub null: Option<XmlTableColumnNull>,
    }
}

recursa::ast_node! {
    /// The tail of an XMLTABLE column, after its name: `FOR ORDINALITY` or a type.
    #[derive(Debug)]
    pub enum XmlTableColumnKind {
        Ordinality(JsonTableOrdinality),
        Typed(XmlTableTypedColumn),
    }
}

recursa::ast_node! {
    /// One column of an XMLTABLE `COLUMNS` list: `‹name› ‹kind›`.
    #[derive(Debug)]
    pub struct XmlTableColumn {
        pub name: literal::AliasName,
        pub kind: XmlTableColumnKind,
    }
}

recursa::ast_node! {
    /// `COLUMNS ‹column› [, ...]` — the non-empty XMLTABLE column list.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(COLUMNS, this)]
    pub struct XmlTableColumnList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(XmlTableColumn),
    );
}

recursa::ast_node! {
    /// Inner contents of `XMLTABLE ( [XMLNAMESPACES(...),] ‹row_xpath›
    /// PASSING [BY {REF|VALUE}] ‹doc› [BY {REF|VALUE}] COLUMNS ‹col› [, ...] )`.
    #[derive(Debug)]
    pub struct XmlTableInner {
        pub namespaces: Option<XmlTableNamespaces>,
        pub row_expr: boxed!(Expr),
        pub passing: XmlTablePassing,
        pub by_after: Option<XmlPassingBy>,
        pub column_list: XmlTableColumnList,
    }
}

recursa::ast_node! {
    /// Mandatory `PASSING` clause with an optional `BY` mode.
    #[derive(Debug)]
    #[tok(PASSING, this)]
    pub struct XmlTablePassing {
        pub by: Option<XmlPassingBy>,
        pub doc: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// The `XMLTABLE ( ... )` construct.
    #[derive(Debug)]
    pub struct XmlTable {
        #[tok(XMLTABLE, LPAREN, this, RPAREN)]
        pub inner: XmlTableInner,
    }
}

recursa::ast_node! {
    /// `XMLTABLE(...)` as a table reference, with an optional table alias.
    #[derive(Debug)]
    pub struct XmlTableRef {
        pub table: XmlTable,
        pub alias: Option<TableAlias>,
    }
}

// --- ROWS FROM (...) table reference ---

recursa::ast_node! {
    /// `AS ( col type [, ...] )` column-definition list on a `ROWS FROM` item.
    #[derive(Debug)]
    #[tok(AS, LPAREN, this, RPAREN)]
    pub struct RowsFromColDef {
        #[sep(COMMA)]
        pub columns: zero_or_many!(FuncTableColumnDef),
    }
}

recursa::ast_node! {
    /// The function of a `rowsfrom_item`: gram.y:13891 `func_expr_windowless
    /// opt_col_def_list`. The two variants lead with disjoint words: a
    /// `COL_NAME` keyword is never a `func_application` name.
    #[derive(Debug)]
    pub enum RowsFromFunc {
        /// A `func_expr_common_subexpr`, held as an [`Expr`].
        Special(boxed!(crate::ast::shared::expr::FuncExprCommonSubexpr)),
        Func(FunctionApplicationExpr),
    }
}

recursa::ast_node! {
    /// One function entry of a `ROWS FROM (...)` list: a function call with an
    /// optional `AS (coldef, ...)` column-definition list.
    #[derive(Debug)]
    pub struct RowsFromItem {
        pub func: RowsFromFunc,
        pub coldef: Option<RowsFromColDef>,
    }
}

recursa::ast_node! {
    /// `ROWS FROM ( func [, ...] ) [WITH ORDINALITY] [alias]` — the multi-function
    /// table reference, evaluating several set-returning functions in parallel.
    #[derive(Debug)]
    pub struct RowsFromRef {
        pub items: RowsFromItemList,
        #[presence(WITH, ORDINALITY)]
        pub ordinality: bool,
        pub alias: Option<FuncTableAlias>,
    }
}

recursa::ast_node! {
    /// The parenthesized function list inside `ROWS FROM`.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(ROWS, FROM, LPAREN, this, RPAREN)]
    pub struct RowsFromItemList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(RowsFromItem),
    );
}

recursa::ast_node! {
    /// Function/table name admission used in FROM.
    ///
    /// `COLLATION` is held back from the unqualified route so the exact
    /// `COLLATION FOR (...)` special form can dispatch without competing with a
    /// generic named relation. Qualified names retain PostgreSQL's ordinary
    /// `ColId` admission.
    #[derive(Debug)]
    pub enum TableFunctionName {
        Qualified(crate::ast::shared::expr::FuncCallQualifiedName),
        Name(crate::tokens::table_function_name),
    }
}

recursa::ast_node! {
    /// Identifier-led FROM source. PostgreSQL's callable-name admission covers
    /// ordinary identifiers and every qualified `ColId` name. Parsing that name
    /// once lets `(`, `*`, an alias, or the absence of a suffix select the source
    /// shape without comparing arbitrarily long dotted names.
    #[derive(Debug)]
    pub struct NamedTableRef {
        pub name: TableFunctionName,
        pub tail: Option<NamedTableRefTail>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum NamedTableRefTail {
        Function(boxed!(NamedFunctionTableTail)),
        Inherited(NamedInheritedTail),
        Alias(SampledTableAlias),
        /// `TABLESAMPLE ...` with no alias before it.
        Sample(TableSampleClause),
    }
}

recursa::ast_node! {
    /// Function-call tail after the shared function/table name.
    #[derive(Debug)]
    pub struct NamedFunctionTableTail {
        pub application: FunctionCallApplication,
        #[presence(WITH, ORDINALITY)]
        pub ordinality: bool,
        pub alias: Option<FuncTableAlias>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct NamedInheritedTail {
        pub star: SelectStar,
        pub alias: Option<PlainTableAlias>,
        pub tablesample: Option<TableSampleClause>,
    }
}

recursa::ast_node! {
    /// `ONLY name [alias]`, separated from identifier-led sources by its required
    /// keyword.
    #[derive(Debug)]
    pub struct OnlyTableRef {
        pub only: SelectOnly,
        pub name: OnlyRelationName,
        pub alias: Option<PlainTableAlias>,
        pub tablesample: Option<TableSampleClause>,
    }
}

recursa::ast_node! {
    /// gram.y `extended_relation_expr: ONLY qualified_name | ONLY '('
    /// qualified_name ')'`.
    #[derive(Debug)]
    pub enum OnlyRelationName {
        Parens(#[tok(LPAREN, this, RPAREN)] QualifiedName),
        Plain(QualifiedName),
    }
}

recursa::ast_node! {
    /// gram.y `opt_alias_clause tablesample_clause` after a relation name: the
    /// alias, then the sample that only a relation can have.
    #[derive(Debug)]
    pub struct SampledTableAlias {
        pub alias: PlainTableAlias,
        pub tablesample: Option<TableSampleClause>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum SelectOnly {
        #[tok(ONLY)]
        Value,
    }
}

recursa::ast_node! {
    /// An unqualified `COL_NAME` keyword used as a relation name. These are the
    /// only valid table-name starters not covered by `TableFunctionName`; keeping them
    /// relation-only prevents XML/JSON special forms from re-entering the generic
    /// function-table grammar.
    #[derive(Debug)]
    pub enum ColNameTableName {
        #[tok(VALUES)]
        Values,
        // `json` is `COL_NAME` from 17 (research, PostgreSQL 17, "Keywords"). In 16
        // it is unreserved, so `TableFunctionName` admits it.
        #[cfg(feature = "since-pg17")]
        #[tok(JSON)]
        Json,
        // Added in 17: research, PostgreSQL 17, "Keywords".
        #[cfg(feature = "since-pg17")]
        #[tok(JSON_VALUE)]
        JsonValue,
        // Added in 17: research, PostgreSQL 17, "Keywords".
        #[cfg(feature = "since-pg17")]
        #[tok(JSON_QUERY)]
        JsonQuery,
        // Added in 17: research, PostgreSQL 17, "Keywords".
        #[cfg(feature = "since-pg17")]
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
        // Added in 17: research, PostgreSQL 17, "Keywords".
        #[cfg(feature = "since-pg17")]
        #[tok(JSON_SERIALIZE)]
        JsonSerialize,
        // Added in 17: research, PostgreSQL 17, "Keywords".
        #[cfg(feature = "since-pg17")]
        #[tok(JSON_SCALAR)]
        JsonScalar,
        // Added in 17: research, PostgreSQL 17, "Keywords".
        #[cfg(feature = "since-pg17")]
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
        #[tok(COALESCE)]
        Coalesce,
        #[tok(GREATEST)]
        Greatest,
        #[tok(LEAST)]
        Least,
        #[tok(NULLIF)]
        NullIf,
        #[tok(NORMALIZE)]
        Normalize,
        #[tok(XMLCONCAT)]
        XmlConcat,
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
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct ColNameTableRef {
        pub name: ColNameTableName,
        pub tail: Option<ColNameTableTail>,
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    pub enum ColNameTableTail {
        Inherited(NamedInheritedTail),
        Alias(SampledTableAlias),
        /// `TABLESAMPLE ...` with no alias before it.
        Sample(TableSampleClause),
    }

}

recursa::ast_node! {
    /// A single table reference (no joins). Used as building block for JoinTableRef.
    ///
    /// Keyword-led special forms have distinct FIRST sets. Identifier-led table,
    /// inherited-table, alias, and function-application forms share one
    /// [`NamedTableRef`] prefix and select their continuation after the name.
    #[derive(Debug)]
    pub enum SimpleTableRef {
        Lateral(LateralRef),
        // Added in 17: see `JsonTableRef`.
        #[cfg(feature = "since-pg17")]
        JsonTable(boxed!(JsonTableRef)),
        XmlTable(boxed!(XmlTableRef)),
        RowsFrom(boxed!(RowsFromRef)),
        /// `CAST(expr AS type) [alias]`, `COLLATION FOR (expr) [alias]`. Each
        /// special form is keyword-led, giving it distinct LR actions from
        /// `Func` and `Table`.
        SpecialFunc(boxed!(SpecialFuncTableRef)),
        Paren(ParenTableRef),
        Only(OnlyTableRef),
        ColName(ColNameTableRef),
        Named(boxed!(NamedTableRef)),
    }
}

recursa::ast_node! {
    /// Join head, including the required JOIN keyword.
    ///
    /// Every alternative is non-nullable. This lets plain JOIN participate in the
    /// same deterministic prefix decision as its qualified forms.
    #[derive(Debug)]
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
        #[tok(JOIN)]
        Plain,
    }
}

recursa::ast_node! {
    /// JOIN condition: ON expr or USING (col, ...)
    #[derive(Debug)]
    pub enum JoinCondition {
        On(JoinOn),
        Using(JoinUsing),
    }
}

recursa::ast_node! {
    /// ON condition for JOIN
    #[derive(Debug)]
    pub struct JoinOn {
        #[tok(ON, this)]
        pub condition: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `AS alias` suffix on a JOIN ... USING column list.
    ///
    /// gram.y `opt_alias_clause_for_join_using` is `AS ColId | /*EMPTY*/`: there
    /// is no bare-identifier spelling here, unlike every other alias position.
    /// Keeping `AS` mandatory is what lets a following `JOIN` (or `LEFT`,
    /// `NATURAL`, ...) continue the join chain instead of being taken as the
    /// alias name — those words are `type_func_name` keywords, which a bare
    /// `Ident` would admit.
    #[derive(Debug)]
    #[tok(AS, this)]
    pub struct JoinUsingAlias {
        /// gram.y `USING '(' name_list ')' opt_alias_clause`: `AS ColId`.
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Parenthesized comma-separated column list in a JOIN USING clause.
    #[derive(Debug, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct JoinUsingColumns(
        /// gram.y `name_list`: one or more `ColId`.
        #[deref]
        #[sep(COMMA)]
        pub one_or_many!(crate::tokens::ColId),
    );
}

recursa::ast_node! {
    /// USING clause for JOIN: `USING (col, ...) [AS alias]`
    #[derive(Debug)]
    #[tok(USING, this)]
    pub struct JoinUsing {
        pub columns: JoinUsingColumns,
        pub alias: Option<JoinUsingAlias>,
    }
}

recursa::ast_node! {
    /// A single join suffix:
    /// `[NATURAL] [LEFT|RIGHT|FULL|INNER|CROSS] [OUTER] JOIN table [ON expr | USING (...)]`.
    ///
    /// `OUTER` is optional after `LEFT`/`RIGHT`/`FULL`; the exact JoinType variants
    /// keep those longer spellings deterministic without admitting it after INNER
    /// or CROSS.
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum JoinSuffix {
        /// `CROSS JOIN table_ref` and `NATURAL [join_type] JOIN table_ref`.
        Unqualified(UnqualifiedJoin),
        /// `[join_type] JOIN table_ref join_qual`.
        Qualified(QualifiedJoin),
    }
}

recursa::ast_node! {
    /// gram.y `joined_table`'s forms without a `join_qual`. Its precedence
    /// declaration `%left JOIN CROSS LEFT FULL RIGHT INNER_P NATURAL` makes a
    /// chain of them left-associative (`a CROSS JOIN b CROSS JOIN c` is
    /// `(a CROSS JOIN b) CROSS JOIN c`), so the right operand is one table
    /// reference and never a join; a parenthesized join is a
    /// the parenthesized `SimpleTableRef` variants.
    #[derive(Debug)]
    pub struct UnqualifiedJoin {
        pub kind: UnqualifiedJoinKind,
        pub table: SimpleTableRef,
    }
}

recursa::ast_node! {
    /// `CROSS JOIN` or `NATURAL [join_type] JOIN`.
    #[derive(Debug)]
    pub enum UnqualifiedJoinKind {
        #[tok(CROSS, JOIN)]
        Cross,
        Natural(NaturalJoin),
    }
}

recursa::ast_node! {
    /// `NATURAL [join_type] JOIN` — gram.y `NATURAL join_type JOIN | NATURAL JOIN`.
    #[derive(Debug)]
    pub struct NaturalJoin {
        #[tok(NATURAL, this)]
        pub join_type: JoinType,
    }
}

recursa::ast_node! {
    /// gram.y `table_ref join_type JOIN table_ref join_qual` and `table_ref JOIN
    /// table_ref join_qual`: the qualification is required, so `a JOIN b JOIN c
    /// ON x ON y` nests to the right (the inner join must own an `ON` before the
    /// outer one can) and `a JOIN b` alone is rejected, as PostgreSQL rejects it.
    /// PostgreSQL assigns an unparenthesized joined table recursively to the
    /// right operand, preserving each condition at the grammar level that owns
    /// it.
    #[derive(Debug)]
    pub struct QualifiedJoin {
        pub join_type: JoinType,
        pub table: boxed!(TableRef),
        pub condition: JoinCondition,
    }
}

recursa::ast_node! {
    /// TABLESAMPLE clause: `TABLESAMPLE method (args) [REPEATABLE (seed)]`.
    ///
    /// gram.y attaches it to one `table_ref` form only, gram.y:13451
    /// `relation_expr opt_alias_clause tablesample_clause`: a table name, with
    /// `*` or `ONLY` or an alias, and never a subquery, a function, a
    /// parenthesized join, `LATERAL` or `ROWS FROM`. So the relation forms hold
    /// it ([`NamedTableRefTail`], [`ColNameTableTail`], [`NamedInheritedTail`],
    /// [`OnlyTableRef`]) and [`TableRef`] does not. It follows the alias, and a
    /// join operand takes it because the operand is a `table_ref`.
    #[derive(Debug)]
    pub struct TableSampleClause {
        /// gram.y `func_name`: the method may be schema-qualified.
        #[tok(TABLESAMPLE, this)]
        pub method: crate::ast::shared::expr::FuncCallName,
        /// gram.y `tablesample_clause: TABLESAMPLE func_name '(' expr_list ')'`.
        #[tok(LPAREN, this, RPAREN)]
        pub args: TableSampleArgs,
        pub repeatable: Option<TableSampleRepeatable>,
    }
}

recursa::ast_node! {
    /// gram.y `expr_list`: one or more expressions.
    #[derive(Debug, derive_more :: Deref)]
    pub struct TableSampleArgs(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(Expr),
    );
}

recursa::ast_node! {
    #[derive(Debug)]
    pub struct TableSampleRepeatable {
        #[tok(REPEATABLE, LPAREN, this, RPAREN)]
        pub seed: Expr,
    }
}

recursa::ast_node! {
    /// A table reference that may have zero or more JOIN suffixes.
    #[derive(Debug)]
    pub struct TableRef {
        pub base: SimpleTableRef,
        pub joins: zero_or_many!(JoinSuffix),
    }
}

recursa::ast_node! {
    /// WHERE-clause body: either a normal expression or the cursor-current
    /// row filter `CURRENT OF cursor_name` (used by positioned UPDATE/DELETE).
    ///
    /// Variant ordering: `CurrentOf` must come before `Expr` since `CURRENT`
    /// is a specific keyword lead-in.
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum WhereCondition {
        CurrentOf(WhereCurrentOf),
        Expr(Expr),
    }
}

recursa::ast_node! {
    /// `CURRENT OF cursor_name` filter.
    #[derive(Debug)]
    pub struct WhereCurrentOf {
        #[tok(CURRENT, OF, this)]
        pub cursor: literal::AliasName,
    }
}

recursa::ast_node! {
    /// WHERE clause: `WHERE expr` or `WHERE CURRENT OF cursor`.
    #[derive(Debug)]
    pub struct WhereClause {
        #[tok(WHERE, this)]
        pub condition: WhereCondition,
    }
}

recursa::ast_node! {
    /// gram.y:16498 `qual_all_Op: all_Op | OPERATOR '(' any_operator ')'`, the
    /// operator of `sortby: a_expr USING qual_all_Op` (gram.y:13095).
    ///
    /// `all_Op` is `Op | MathOp`, so every operator spelling is legal here,
    /// `+`, `=` and `<>` included. Whether the operator orders anything is
    /// for parse analysis: PostgreSQL parses `ORDER BY x USING +` and then
    /// reports "operator + is not a valid ordering operator".
    ///
    /// Variant ordering: `Decorated` leads with `OPERATOR`, `Plain` with an
    /// operator token.
    #[derive(Debug)]
    pub enum UsingOp {
        Decorated(crate::ast::shared::expr::DecoratedOperator),
        Plain(crate::ast::shared::names::OperatorName),
    }
}

recursa::ast_node! {
    /// USING clause in ORDER BY: `USING op`
    #[derive(Debug)]
    pub struct UsingClause {
        #[tok(USING, this)]
        pub op: UsingOp,
    }
}

recursa::ast_node! {
    /// Sort direction: ASC or DESC.
    #[derive(Debug)]
    pub enum SortDir {
        #[tok(ASC)]
        Asc,
        #[tok(DESC)]
        Desc,
    }
}

recursa::ast_node! {
    /// NULLS FIRST or NULLS LAST.
    #[derive(Debug)]
    pub enum NullsOrder {
        #[tok(NULLS, FIRST)]
        First,
        #[tok(NULLS, LAST)]
        Last,
    }
}

recursa::ast_node! {
    /// A single ORDER BY item, gram.y:13095 `sortby: a_expr USING qual_all_Op
    /// opt_nulls_order | a_expr opt_asc_desc opt_nulls_order`.
    ///
    /// The order is `ASC`, `DESC` or `USING op`, one of them at most: `ORDER
    /// BY x ASC USING <` is a syntax error.
    #[derive(Debug)]
    pub struct OrderByItem {
        pub expr: Expr,
        pub order: Option<SortOrder>,
        pub nulls: Option<NullsOrder>,
    }
}

recursa::ast_node! {
    /// How a `sortby` orders: `opt_asc_desc`'s two words, or `USING
    /// qual_all_Op`.
    ///
    /// Variant ordering: the three lead with `ASC`, `DESC` and `USING`.
    #[derive(Debug)]
    pub enum SortOrder {
        Dir(SortDir),
        Using(UsingClause),
    }
}

recursa::ast_node! {
    /// ORDER BY clause: `ORDER BY item [, item ...]`.
    #[derive(Debug)]
    #[tok(ORDER, BY, this)]
    pub struct OrderByClause {
        /// gram.y `sortby_list`: one or more items.
        #[sep(COMMA)]
        pub items: one_or_many!(OrderByItem),
    }
}

recursa::ast_node! {
    /// OFFSET clause: `OFFSET expr`
    #[derive(Debug)]
    pub struct OffsetClause {
        #[tok(OFFSET, this)]
        pub count: Expr,
    }
}

recursa::ast_node! {
    /// LIMIT clause: `LIMIT expr`
    #[derive(Debug)]
    pub struct LimitClause {
        #[tok(LIMIT, this)]
        pub count: Expr,
    }
}

recursa::ast_node! {
    /// `FIRST` or `NEXT` keyword in FETCH clause.
    #[derive(Debug)]
    pub enum FetchFirstOrNext {
        #[tok(FIRST)]
        First,
        #[tok(NEXT)]
        Next,
    }
}

recursa::ast_node! {
    /// `ROW` or `ROWS` keyword in FETCH clause.
    #[derive(Debug)]
    pub enum FetchRowOrRows {
        #[tok(ROWS)]
        Rows,
        #[tok(ROW)]
        Row,
    }
}

recursa::ast_node! {
    /// `ONLY` or `WITH TIES` — FETCH clause termination mode.
    ///
    /// `WithTies` declared first since it's longer and both start after a keyword.
    #[derive(Debug)]
    pub enum FetchMode {
        #[tok(WITH, TIES)]
        WithTies,
        #[tok(ONLY)]
        Only,
    }
}

recursa::ast_node! {
    /// FETCH without count: `{ ROW | ROWS } { ONLY | WITH TIES }`.
    #[derive(Debug)]
    pub struct FetchNoCount {
        pub row_or_rows: FetchRowOrRows,
        pub mode: FetchMode,
    }
}

recursa::ast_node! {
    /// FETCH with count: `expr { ROW | ROWS } { ONLY | WITH TIES }`.
    #[derive(Debug)]
    pub struct FetchWithCount {
        pub count: boxed!(Expr),
        pub row_or_rows: FetchRowOrRows,
        pub mode: FetchMode,
    }
}

recursa::ast_node! {
    /// Body of FETCH clause: either `{ ROW | ROWS } mode` (no count)
    /// or `expr { ROW | ROWS } mode` (with count).
    ///
    /// `NoCount` (peeks `ROW`/`ROWS`) must come first so it's tried before
    /// `WithCount` which would greedily consume `ROWS` as an identifier.
    #[derive(Debug)]
    pub enum FetchFirstBody {
        NoCount(FetchNoCount),
        WithCount(FetchWithCount),
    }
}

recursa::ast_node! {
    /// `FETCH { FIRST | NEXT } [count] { ROW | ROWS } { ONLY | WITH TIES }`
    #[derive(Debug)]
    pub struct FetchFirstClause {
        #[tok(FETCH, this)]
        pub first_or_next: FetchFirstOrNext,
        pub body: FetchFirstBody,
    }
}

recursa::ast_node! {
    /// A limiting clause: `LIMIT expr` or `FETCH FIRST ...`. PostgreSQL's
    /// `select_limit` production admits at most one limiting clause per query,
    /// so `Offset` is deliberately not part of this enum.
    ///
    /// `FetchFirst` and `Limit` start on distinct keywords (`FETCH` vs `LIMIT`).
    #[derive(Debug)]
    #[allow(
        clippy::large_enum_variant,
        reason = "keep the public parser AST variants inline and source-compatible"
    )]
    pub enum LimitingClause {
        FetchFirst(FetchFirstClause),
        Limit(LimitClause),
    }
}

recursa::ast_node! {
    /// A limiting clause followed by an optional `OFFSET` clause.
    #[derive(Debug)]
    pub struct LimitThenOffset {
        pub limit: LimitingClause,
        #[pretty(break_before = soft)]
        pub offset: Option<boxed!(OffsetClause)>,
    }
}

recursa::ast_node! {
    /// An `OFFSET` clause followed by an optional limiting clause.
    #[derive(Debug)]
    pub struct OffsetThenLimit {
        pub offset: OffsetClause,
        #[pretty(break_before = soft)]
        pub limit: Option<boxed!(LimitingClause)>,
    }
}

recursa::ast_node! {
    /// The limit/offset tail of a query, restricted to the clause orders
    /// PostgreSQL's `select_limit` production accepts: a limiting clause with an
    /// optional `OFFSET` after it, or an `OFFSET` clause with an optional
    /// limiting clause after it. Duplicate same-kind clauses and `LIMIT` mixed
    /// with `FETCH FIRST` are structurally unrepresentable, and rendering
    /// preserves the written order via the variant.
    #[derive(Debug)]
    pub enum LimitOffsetClause {
        LimitOffset(LimitThenOffset),
        OffsetLimit(OffsetThenLimit),
    }
}

recursa::ast_node! {
    /// The limit and locking tails of a query, in the two orders gram.y's
    /// `select_no_parens` has (gram.y:12707-12715):
    ///
    /// ```text
    /// select_clause opt_sort_clause for_locking_clause opt_select_limit
    /// select_clause opt_sort_clause select_limit opt_for_locking_clause
    /// ```
    ///
    /// so `ORDER BY 1 FOR UPDATE LIMIT 1` and `ORDER BY 1 LIMIT 1 FOR UPDATE`
    /// are both queries, and `FOR UPDATE LIMIT 1 FOR SHARE` is not. The variant
    /// keeps the written order for rendering.
    ///
    /// Variant ordering: `LockingFirst` leads with `FOR`, `LimitFirst` with
    /// `LIMIT`, `OFFSET` or `FETCH`.
    #[derive(Debug)]
    pub enum LimitLockingClause {
        LockingFirst(LockingThenLimit),
        LimitFirst(LimitThenLocking),
    }
}

recursa::ast_node! {
    /// `for_locking_clause opt_select_limit`.
    #[derive(Debug)]
    pub struct LockingThenLimit {
        pub locking: ForLockingClause,
        #[pretty(break_before = soft)]
        pub limit_offset: Option<boxed!(LimitOffsetClause)>,
    }
}

recursa::ast_node! {
    /// `select_limit opt_for_locking_clause`.
    #[derive(Debug)]
    pub struct LimitThenLocking {
        pub limit_offset: boxed!(LimitOffsetClause),
        #[pretty(break_before = soft)]
        pub locking: Option<ForLockingClause>,
    }
}

recursa::ast_node! {
    /// gram.y:13362 `for_locking_clause: for_locking_items | FOR READ ONLY`.
    ///
    /// `for_locking_items` is a list with no separator (gram.y:13372): `FOR
    /// SHARE OF a FOR UPDATE OF b` holds two items, and PostgreSQL applies
    /// each one. `FOR READ ONLY` locks nothing; PostgreSQL reads it as no
    /// locking clause at all.
    ///
    /// Variant ordering: `ReadOnly` is three fixed tokens; an item's second
    /// token is never `READ`.
    #[derive(Debug)]
    pub enum ForLockingClause {
        #[tok(FOR, READ, ONLY)]
        ReadOnly,
        Items(one_or_many!(ForUpdateClause)),
    }
}

impl<'input> LimitLockingClause<'input> {
    /// The `select_limit` of either order.
    pub fn limit_offset(&self) -> Option<&LimitOffsetClause<'input>> {
        match self {
            Self::LockingFirst(tail) => tail.limit_offset.as_deref(),
            Self::LimitFirst(tail) => Some(&tail.limit_offset),
        }
    }

    /// The `for_locking_clause` of either order.
    pub fn locking(&self) -> Option<&ForLockingClause<'input>> {
        match self {
            Self::LockingFirst(tail) => Some(&tail.locking),
            Self::LimitFirst(tail) => tail.locking.as_ref(),
        }
    }
}

impl<'input> ForLockingClause<'input> {
    /// The locking items, in source order; none for `FOR READ ONLY`.
    pub fn items(&self) -> &[ForUpdateClause<'input>] {
        match self {
            Self::ReadOnly => &[],
            Self::Items(items) => items.as_slice(),
        }
    }
}

recursa::ast_node! {
    /// gram.y:13377 `for_locking_item: for_locking_strength locked_rels_list
    /// opt_nowait_or_skip`: FOR UPDATE / FOR SHARE / FOR NO KEY UPDATE / FOR KEY
    /// SHARE.
    #[derive(Debug)]
    pub struct ForUpdateClause {
        #[tok(FOR, this)]
        pub mode: LockingMode,
        /// Optional `OF table[, ...]` restricting the lock to a subset of the
        /// FROM-list entries (plain tables or table aliases).
        pub of: Option<ForUpdateOf>,
        /// Optional wait-behavior modifier: `NOWAIT` or `SKIP LOCKED`.
        pub wait: Option<ForUpdateWait>,
    }
}

recursa::ast_node! {
    /// `OF name[, ...]` in a `FOR UPDATE` locking clause.
    #[derive(Debug)]
    #[tok(OF, this)]
    pub struct ForUpdateOf {
        /// gram.y:13396 `locked_rels_list: OF qualified_name_list`: one or more
        /// names, each a `qualified_name`. PostgreSQL's parse analysis rejects a
        /// qualified one ("FOR UPDATE must specify unqualified relation names"),
        /// and the grammar accepts it.
        ///
        /// `OF` is on the node, not on this field: on the list field it took the
        /// separator's place and `OF a, b` did not parse.
        #[sep(COMMA)]
        pub names: one_or_many!(crate::ast::shared::names::QualifiedName),
    }
}

recursa::ast_node! {
    /// `NOWAIT | SKIP LOCKED` suffix on a `FOR UPDATE` clause.
    ///
    /// Variant ordering: `SkipLocked` (two tokens) before `Nowait` (one token).
    #[derive(Debug)]
    pub enum ForUpdateWait {
        #[tok(SKIP, LOCKED)]
        SkipLocked,
        #[tok(NOWAIT)]
        Nowait,
    }
}

recursa::ast_node! {
    /// Lock strength for `SELECT ... FOR ...` locking clauses.
    ///
    /// Variant ordering: longer (`NO KEY UPDATE`, `KEY SHARE`) before shorter
    /// (`UPDATE`, `SHARE`) so longest-match wins.
    #[derive(Debug)]
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
}

recursa::ast_node! {
    /// GROUP BY clause: `GROUP BY item, ...` where each item is an expression
    /// or one of the grouping primitives (GROUPING SETS, ROLLUP, CUBE).
    #[derive(Debug)]
    #[tok(GROUP, BY, this)]
    pub struct GroupByClause {
        /// Optional `DISTINCT` / `ALL` modifier (Postgres 16+).
        pub modifier: Option<GroupByModifier>,
        /// gram.y `group_by_list`: one or more items.
        #[sep(COMMA)]
        pub items: one_or_many!(GroupByItem),
    }
}

recursa::ast_node! {
    /// `GROUP BY [DISTINCT|ALL]` modifier.
    #[derive(Debug)]
    pub enum GroupByModifier {
        #[tok(DISTINCT)]
        Distinct,
        #[tok(ALL)]
        All,
    }
}

recursa::ast_node! {
    /// `GROUPING SETS ( item, ... )`.
    #[derive(Debug)]
    #[tok(GROUPING, SETS, LPAREN, this, RPAREN)]
    pub struct GroupingSetsItem {
        #[sep(COMMA)]
        pub groups: zero_or_many!(boxed!(GroupByItem)),
    }
}

recursa::ast_node! {
    /// gram.y:13336 `rollup_clause: ROLLUP '(' expr_list ')'`, PostgreSQL's
    /// `GroupingSet` of kind `GROUPING_SET_ROLLUP`.
    ///
    /// `ROLLUP` is an unreserved keyword, so `rollup(a, b)` is also the
    /// spelling of a function call. gram.y settles it with `%nonassoc CUBE
    /// ROLLUP` below `'('` (gram.y:871-873, 13330-13334): as a
    /// `group_by_item` the word followed by `(` is always this clause. A
    /// parenthesized `(rollup(a, b))` and a qualified `s.rollup(a, b)` stay
    /// function calls, because neither starts a `group_by_item` with the word.
    #[derive(Debug)]
    #[tok(ROLLUP, LPAREN, this, RPAREN)]
    pub struct RollupItem {
        #[sep(COMMA)]
        pub items: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// gram.y:13343 `cube_clause: CUBE '(' expr_list ')'`, PostgreSQL's
    /// `GroupingSet` of kind `GROUPING_SET_CUBE`. [`RollupItem`] describes how
    /// it wins over the function call of the same spelling.
    #[derive(Debug)]
    #[tok(CUBE, LPAREN, this, RPAREN)]
    pub struct CubeItem {
        #[sep(COMMA)]
        pub items: one_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// A single element in a GROUP BY clause.
    ///
    /// gram.y:13315 `group_by_item: a_expr | empty_grouping_set | cube_clause |
    /// rollup_clause | grouping_sets_clause`. `Expr` also handles `(a, b)`
    /// row-style groupings.
    #[derive(Debug)]
    pub enum GroupByItem {
        GroupingSets(GroupingSetsItem),
        Rollup(RollupItem),
        Cube(CubeItem),
        Empty(EmptyGroupingSet),
        Expr(boxed!(Expr)),
    }
}

recursa::ast_node! {
    /// The empty grouping set `()`, used inside `GROUPING SETS` and also valid as
    /// a top-level grouping item.
    #[derive(Debug)]
    pub enum EmptyGroupingSet {
        #[tok(LPAREN, RPAREN)]
        Value,
    }
}

recursa::ast_node! {
    /// HAVING clause: `HAVING expr`
    #[derive(Debug)]
    pub struct HavingClause {
        #[tok(HAVING, this)]
        pub condition: Expr,
    }
}

recursa::ast_node! {
    /// A single named window definition: `name AS (inline_window_spec)`.
    #[derive(Debug)]
    pub struct WindowDef {
        pub name: crate::tokens::ColId,
        #[tok(AS, LPAREN, this, RPAREN)]
        pub spec: crate::ast::shared::expr::InlineWindowSpec,
    }
}

recursa::ast_node! {
    /// `WINDOW name AS (...)[, name AS (...), ...]` clause in SELECT.
    #[derive(Debug)]
    #[tok(WINDOW, this)]
    pub struct WindowClause {
        #[sep(COMMA)]
        pub defs: one_or_many!(WindowDef),
    }
}

recursa::ast_node! {
    /// `INTO [TEMP|TEMPORARY|UNLOGGED] [TABLE] target` clause for the
    /// Postgres `SELECT ... INTO new_table` statement form.
    #[derive(Debug)]
    pub enum SelectIntoPersistence {
        #[tok(TEMP)]
        Temp,
        #[tok(TEMPORARY)]
        Temporary,
        #[tok(UNLOGGED)]
        Unlogged,
    }
}

recursa::ast_node! {
    /// `INTO [TEMP|TEMPORARY|UNLOGGED] [TABLE] target` clause for the
    /// Postgres `SELECT ... INTO new_table` statement form.
    #[derive(Debug)]
    #[tok(INTO, this)]
    pub struct SelectIntoClause {
        pub persistence: Option<SelectIntoPersistence>,
        #[tok(optional(TABLE), this)]
        pub target: crate::ast::shared::names::QualifiedName,
        pub using: Option<crate::ast::ddl::table::UsingAccessMethodClause>,
    }
}

recursa::ast_node! {
    /// `DISTINCT` or `DISTINCT ON (exprs)` qualifier on a SELECT.
    ///
    /// Variant ordering: `On` (longer, starts with `DISTINCT ON`) before `All`
    /// (just `DISTINCT`).
    #[derive(Debug)]
    pub enum SelectDistinct {
        On(SelectDistinctOn),
        #[tok(DISTINCT)]
        All,
    }
}

/// Borrowed projection of a SELECT's optional DISTINCT qualifier.
#[derive(Debug)]
pub enum SelectDistinctRef<'view, 'input> {
    On(&'view SelectDistinctOn<'input>),
    All,
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(DISTINCT, ON, LPAREN, this, RPAREN)]
    pub struct SelectDistinctOn {
        #[sep(COMMA)]
        pub exprs: zero_or_many!(crate::ast::shared::expr::Expr),
    }
}

recursa::ast_node! {
    /// SELECT statement.
    ///
    /// `indent` covers the whole clause body, so every break the statement is
    /// forced to take — after `SELECT`, between target items, and before the
    /// trailing clauses — lands one level in from column zero.
    #[derive(Debug)]
    #[flat(pool)]
    #[pretty(group = consistent, indent)]
    #[tok(SELECT, this)]
    pub struct SelectStmt {
        /// gram.y `simple_select: SELECT opt_all_clause opt_target_list ...`.
        /// `opt_target_list` is nullable, so a bare `SELECT` with no targets,
        /// no `INTO` and no `FROM` is a complete `simple_select` (`select;`).
        ///
        #[pretty(break_before = soft)]
        pub head: Option<SelectHead>,
        #[pretty(break_before = soft)]
        pub where_clause: Option<boxed!(WhereClause)>,
        #[pretty(break_before = soft)]
        pub group_by: Option<boxed!(GroupByClause)>,
        #[pretty(break_before = soft)]
        pub having: Option<boxed!(HavingClause)>,
        #[pretty(break_before = soft)]
        pub window: Option<boxed!(WindowClause)>,
    }
}

recursa::ast_node! {
    /// The DISTINCT ON, bare DISTINCT, or unqualified head of a SELECT statement.
    ///
    /// Each qualified alternative owns its fixed prefix directly. This keeps the
    /// prefix nonnullable while sharing the complete target grammar after it.
    #[derive(Debug)]
    pub enum SelectHead {
        DistinctOn(SelectDistinctOnTargets),
        Distinct(SelectDistinctTargets),
        All(SelectAllTargets),
        Plain(SelectTargets),
    }
}

recursa::ast_node! {
    /// `ALL` followed by the optional SELECT targets: gram.y:12791 `SELECT
    /// opt_all_clause opt_target_list` with `opt_all_clause: ALL`
    /// (gram.y:13071). `ALL` is a noise word, and unlike `DISTINCT` it keeps
    /// the nullable `opt_target_list`: `SELECT ALL FROM t` and a bare `SELECT
    /// ALL` are complete.
    #[derive(Debug)]
    #[tok(ALL, this)]
    pub struct SelectAllTargets {
        #[pretty(break_before = soft)]
        pub targets: Option<SelectTargets>,
    }
}

recursa::ast_node! {
    /// A required `DISTINCT ON (...)` qualifier followed by the SELECT targets.
    ///
    /// gram.y `simple_select: SELECT distinct_clause target_list into_clause
    /// ...`: the list is `target_list`, not the nullable `opt_target_list` of
    /// the `opt_all_clause` form, so `SELECT DISTINCT ON (x) FROM t` is a
    /// syntax error.
    #[derive(Debug)]
    pub struct SelectDistinctOnTargets {
        pub qualifier: SelectDistinctOn,
        #[pretty(break_before = soft)]
        pub targets: SelectTargetList,
    }
}

recursa::ast_node! {
    /// A required bare `DISTINCT` prefix followed by the SELECT targets.
    ///
    /// The list is gram.y's `target_list`, as [`SelectDistinctOnTargets`]
    /// describes: `SELECT DISTINCT FROM t` is a syntax error (errors.sql).
    #[derive(Debug)]
    #[tok(DISTINCT, this)]
    pub struct SelectDistinctTargets {
        #[pretty(break_before = soft)]
        pub targets: SelectTargetList,
    }
}

recursa::ast_node! {
    /// SELECT targets together with their optional INTO/FROM clauses.
    ///
    /// `INTO` and `FROM` are reserved exact prefixes for the zero-target
    /// PostgreSQL forms. Keeping those alternatives beside the required nonempty
    /// target list avoids a nullable expression-list decision on the same token:
    /// a target item can never start with either keyword, so the three
    /// alternatives stay disjoint on their first token.
    #[derive(Debug)]
    pub enum SelectTargets {
        Into(SelectIntoTargets),
        Empty(FromClause),
        Items(SelectTargetList),
    }
}

recursa::ast_node! {
    /// A zero-target `SELECT INTO tbl [FROM ...]`.
    ///
    /// gram.y reaches this through the nullable `opt_target_list` followed by a
    /// present `into_clause`; `create_am.sql` exercises the spelling.
    #[derive(Debug)]
    pub struct SelectIntoTargets {
        pub into: boxed!(SelectIntoClause),
        /// No break hint here: `FromClause` owns the break before `FROM` inside
        /// its own group, so the boundary is measured with the clause it belongs
        /// to.
        pub from_clause: Option<boxed!(FromClause)>,
    }
}

recursa::ast_node! {
    /// A nonempty SELECT target list and the clauses that immediately follow it.
    ///
    /// The group covers the target list together with the clauses that follow it,
    /// so the decision to put one target per line is made against the width of
    /// `<targets> FROM <tables>` rather than the targets alone.
    #[derive(Debug)]
    #[pretty(group = consistent)]
    pub struct SelectTargetList {
        #[sep(COMMA)]
        pub items: one_or_many!(SelectItem),
        #[pretty(break_before = soft)]
        pub into: Option<boxed!(SelectIntoClause)>,
        /// No break hint here: `FromClause` owns the break before `FROM` inside
        /// its own group, so the boundary is measured with the clause it belongs
        /// to.
        pub from_clause: Option<boxed!(FromClause)>,
    }
}

impl<'input> SelectStmt<'input> {
    /// Return an owned semantic projection of the optional DISTINCT qualifier.
    pub fn distinct(&self) -> Option<SelectDistinctRef<'_, 'input>> {
        match self.head.as_ref()? {
            SelectHead::DistinctOn(head) => Some(SelectDistinctRef::On(&head.qualifier)),
            SelectHead::Distinct(_) => Some(SelectDistinctRef::All),
            SelectHead::All(_) | SelectHead::Plain(_) => None,
        }
    }

    /// Return the nonempty target list of any SELECT-head form, or `None`
    /// for a zero-target `SELECT`, which only the head without `DISTINCT` has.
    pub fn target_list(&self) -> Option<&SelectTargetList<'input>> {
        match self.head.as_ref()? {
            SelectHead::DistinctOn(head) => Some(&head.targets),
            SelectHead::Distinct(head) => Some(&head.targets),
            SelectHead::All(_) | SelectHead::Plain(_) => match self.plain_targets()? {
                SelectTargets::Items(targets) => Some(targets),
                SelectTargets::Into(_) | SelectTargets::Empty(_) => None,
            },
        }
    }

    /// Return the targets of a head without `DISTINCT`, the only kind that
    /// has the zero-target forms.
    fn plain_targets(&self) -> Option<&SelectTargets<'input>> {
        match self.head.as_ref()? {
            SelectHead::All(head) => head.targets.as_ref(),
            SelectHead::Plain(targets) => Some(targets),
            SelectHead::DistinctOn(_) | SelectHead::Distinct(_) => None,
        }
    }

    /// Number of items in the SELECT list (zero if the list is empty,
    /// e.g. the regression-test form `SELECT FROM tbl`).
    pub fn item_count(&self) -> usize {
        self.target_list().map_or(0, |targets| targets.items.len())
    }

    /// Iterate over the SELECT items.
    pub fn items(&self) -> impl Iterator<Item = &SelectItem<'input>> {
        self.target_list()
            .map(|targets| targets.items.as_slice())
            .into_iter()
            .flatten()
    }

    /// Return the FROM clause from any target-list form.
    pub fn from_clause(&self) -> Option<&FromClause<'input>> {
        if let Some(targets) = self.target_list() {
            return targets.from_clause.as_deref();
        }
        match self.plain_targets()? {
            SelectTargets::Into(targets) => targets.from_clause.as_deref(),
            SelectTargets::Empty(from_clause) => Some(from_clause),
            SelectTargets::Items(_) => None,
        }
    }

    /// Return the `INTO` clause from any target-list form.
    pub fn into_clause(&self) -> Option<&SelectIntoClause<'input>> {
        if let Some(targets) = self.target_list() {
            return targets.into.as_deref();
        }
        match self.plain_targets()? {
            SelectTargets::Into(targets) => Some(&targets.into),
            SelectTargets::Empty(_) | SelectTargets::Items(_) => None,
        }
    }
}

recursa::ast_node! {
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ValuesRow {
        #[sep(COMMA)]
        pub values: zero_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// VALUES body: `VALUES (expr, ...), (expr, ...)` — gram.y `values_clause`,
    /// which has at least one row. Can appear standalone or inside subqueries;
    /// the expression-level subquery forms share it, so `(VALUES (1))` has one
    /// row grammar wherever it occurs.
    #[derive(Debug)]
    #[tok(VALUES, this)]
    pub struct ValuesBody {
        #[sep(COMMA)]
        pub rows: one_or_many!(ValuesRow),
    }
}
