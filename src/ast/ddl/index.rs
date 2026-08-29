/// CREATE INDEX / DROP INDEX statement AST.
use recursa::seq::{Seq0, Seq1};
use recursa_diagram::railroad;

pub use crate::ast::shared::flags::{DropBehavior, IfExists, IfNotExists};

use crate::ast::dml::select::{NullsOrder, SortDir, WhereClause};
use crate::ast::session::set_reset::SetValue;
use crate::ast::shared::expr::{Expr, FuncCall, JsonFuncExpr};
use crate::tokens::{literal, punct};

use crate::tokens::keyword::*;
// ---------------------------------------------------------------------------
// Additional imports for the ALTER/DROP types appended to this file as part
// of the DDL physical-extraction migration. Glob imports keep cross-batch
// type references resolvable regardless of migration order; a polish pass
// will tighten these once the migration completes.
use crate::ast::ddl::database::SetTablespaceClause;
use crate::ast::ddl::statistics::SetStatisticsValue;
use crate::ast::ddl::trigger::DependsOnExtension;
#[allow(unused_imports)]
use crate::ast::shared::expr::*;
#[allow(unused_imports)]
use crate::ast::shared::flags::*;
#[allow(unused_imports)]
use crate::ast::shared::names::*;
#[allow(unused_imports)]
use crate::ast::shared::numbers::*;
#[allow(unused_imports)]
use crate::tokens::soft_keyword::*;
// ---------------------------------------------------------------------------
/// Index access method: `USING method_name`.
///
/// The method name can be an identifier or one of the built-in method
/// keywords (`btree`, `gin`, ...). We accept `literal::AliasName` so both
/// identifiers and keywords are allowed in this position.
#[derive(recursa::Node, Debug, Clone)]
pub struct UsingMethod<'input> {
    #[tok(USING, this)]
    pub method: literal::AliasName<'input>,
}

/// A single opclass option: `name = value`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassOption<'input> {
    pub name: literal::AliasName<'input>,
    #[tok(EQ, this)]
    pub value: Expr<'input>,
}

/// Parenthesized opclass option list: `(name = value, ...)`.
#[derive(Debug, Clone, FormatTokens, Visit, Transform, derive_more::Deref)]
pub struct OpclassOptions<'input>(
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    #[deref] pub  Vec<OpclassOption<'input> > ,
);

/// Opclass name plus optional options: `int4_ops [(opt = val, ...)]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct OpclassSpec<'input> {
    pub name: crate::tokens::ColId<'input>,
    pub options: Option<OpclassOptions<'input>>,
}

/// A storage parameter entry: `name [= value]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParam<'input> {
    pub name: StorageParamName<'input>,
    pub value: Option<StorageParamValue<'input>>,
}

/// Storage parameter name: either a bare word or `namespace.word` (for
/// namespaced reloptions like `toast.vacuum_truncate` and
/// `some_ns.fillfactor`). Modeled as a dedicated type so existing grammar
/// that depends on `StorageParam::name` being a single token is unaffected
/// beyond the inner shape.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParamName<'input> {
    pub namespace: Option<StorageParamNamespace<'input>>,
    pub name: literal::AliasName<'input>,
}

/// `namespace.` prefix on a storage parameter name.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParamNamespace<'input> {
    #[tok(this, DOT)]
    pub namespace: literal::AliasName<'input>,
}

/// `= value` suffix for a storage parameter.
///
/// The value is a permissive SetValue (keywords like `off`, `on`, string/numeric
/// literals, identifiers) rather than a full `Expr` — storage param values are
/// simple literals and `Expr::ColumnRef` rejects keywords like `off`.
#[derive(recursa::Node, Debug, Clone)]
pub struct StorageParamValue<'input> {
    #[tok(EQ, this)]
    pub value: SetValue<'input>,
}

/// `WITH (name = value, ...)` storage parameters clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct WithStorage<'input> {
    #[tok(WITH, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params:  Vec<StorageParam<'input> > ,
}

/// `INCLUDE (col, ...)` covering-index clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct IncludeClause<'input> {
    #[tok(INCLUDE, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns:
         Vec<crate::tokens::ColId<'input> > ,
}

/// Index column target: a parenthesized expression, a bare SQL/JSON
/// function expression, a bare function call (e.g., `lower(fruit)`), or a
/// plain column identifier. Postgres allows any `func_expr_windowless` as a
/// bare index element — that includes the SQL/JSON functions.
///
/// Variant ordering:
/// - `Expr` (`(`) starts with a different token than the others.
/// - `Json` before `Func`: a JSON function keyword is soft and `Func`
///   would otherwise reclaim it as an ordinary function name.
/// - `Func` (`ident(`) must come before `Col` (`ident`) so longest-match
///   prefers the function call form.
#[derive(recursa::Node, Debug, Clone)]
pub enum IndexTarget<'input> {
    Expr(#[tok(LPAREN, this, RPAREN)]  Box<Expr<'input>> ),
    Json(Box<JsonFuncExpr<'input>>),
    Func(Box<FuncCall<'input>>),
    Col(crate::tokens::ColId<'input>),
}

/// `COLLATE "name"` on an index element.
#[derive(recursa::Node, Debug, Clone)]
pub struct IndexCollate<'input> {
    #[tok(COLLATE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// An index element:
/// `column_or_expr [COLLATE "name"] [opclass [(options)]] [ASC|DESC] [NULLS FIRST|LAST]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct IndexElem<'input> {
    pub target: IndexTarget<'input>,
    pub collate: Option<IndexCollate<'input>>,
    pub opclass: Option<OpclassSpec<'input>>,
    pub dir: Option<SortDir>,
    pub nulls: Option<NullsOrder>,
}

/// CREATE INDEX statement.
///
/// ```sql
/// CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] [name]
///        ON table [USING method] (index_elem, ...)
///        [INCLUDE (col, ...)]
///        [WITH (storage_param = value, ...)]
///        [WHERE predicate]
/// ```
///
/// The index name is optional (Postgres allows it to be omitted).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateIndexStmt<'input> {
    #[tok(CREATE, this, INDEX)]
    #[presence(UNIQUE)]
    pub unique: bool,
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub if_not_exists: Option<IfNotExists>,
    pub name: Option<crate::tokens::ColId<'input>>,
    #[tok(ON, this)]
    #[presence(ONLY)]
    /// Optional `ONLY` modifier — restricts the index to the named table
    /// without descending into inheritance children (partitioned tables).
    pub only: bool,
    pub table_name: crate::ast::shared::names::QualifiedName<'input>,
    pub using: Option<Box<UsingMethod<'input>>>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub columns:  Vec<IndexElem<'input> > ,
    pub include: Option<Box<IncludeClause<'input>>>,
    pub nulls_distinct: Option<NullsDistinctClause>,
    pub with_storage: Option<Box<WithStorage<'input>>>,
    pub tablespace: Option<crate::ast::ddl::table::TablespaceClause<'input>>,
    pub where_clause: Option<Box<WhereClause<'input>>>,
}

/// `NULLS [NOT] DISTINCT` modifier on a unique index.
///
/// Variant ordering: `NotDistinct` (`NULLS NOT DISTINCT`, longer) before
/// `Distinct` (`NULLS DISTINCT`, shorter).
#[derive(recursa::Node, Debug, Clone)]
pub enum NullsDistinctClause {
    #[tok(NULLS, NOT, DISTINCT)] NotDistinct,
    #[tok(NULLS, DISTINCT)] Distinct,
}

/// DROP INDEX statement:
///
/// ```sql
/// DROP INDEX [CONCURRENTLY] [IF EXISTS] name [, name ...] [CASCADE | RESTRICT]
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct DropIndexStmt<'input> {
    #[tok(DROP, INDEX, this)]
    #[presence(CONCURRENTLY)]
    pub concurrently: bool,
    pub if_exists: Option<IfExists>,
    #[sep(COMMA)]
    pub names: Vec<crate::ast::shared::names::QualifiedName<'input> >,
    pub behavior: Option<DropBehavior>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use crate::ast::ddl::index::{CreateIndexStmt, DropIndexStmt};

    #[test]
    fn parse_create_unique_index_nulls_distinct() {
        let lexed = crate::tokens::lex("CREATE UNIQUE INDEX i ON t (i) NULLS NOT DISTINCT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let lexed = crate::tokens::lex("CREATE UNIQUE INDEX i ON t (i) NULLS DISTINCT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.as_ref().unwrap().text(), "fooi");
        assert_eq!(stmt.table_name.object(), "foo");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_with_desc() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo (f1 DESC)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_desc_nulls_last() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo (f1 DESC NULLS LAST)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_if_not_exists() {
        let lexed = crate::tokens::lex("CREATE INDEX IF NOT EXISTS fooi ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_concurrently() {
        let lexed = crate::tokens::lex("CREATE INDEX CONCURRENTLY fooi ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.concurrently.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_on_only() {
        let lexed = crate::tokens::lex("CREATE INDEX idx ON ONLY ptif_test (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_unnamed() {
        let lexed = crate::tokens::lex("CREATE INDEX ON foo (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.name.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_using_btree() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo USING btree (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.using.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_using_gin() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo USING gin (f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_opclass() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo (f1 int4_ops)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let _ = stmt;
    }

    #[test]
    fn parse_create_index_opclass_desc() {
        let lexed = crate::tokens::lex("CREATE INDEX fooi ON foo (f1 text_pattern_ops DESC)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_expr_column() {
        let lexed = crate::tokens::lex("CREATE INDEX i ON t ((lower(name)))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// A bare SQL/JSON function is a valid index element (Postgres allows
    /// any `func_expr_windowless`). It must not require extra parentheses.
    #[test]
    fn parse_create_index_bare_json_expr() {
        let sql = "CREATE INDEX ON t (JSON_QUERY(js, '$' PASSING 1 AS x))";
        let lexed = crate::tokens::lex(sql);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt =
            CreateIndexStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {sql:?}: {e}")).into_ast();
        assert!(
            input.is_eof(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn parse_create_index_include() {
        let lexed = crate::tokens::lex("CREATE INDEX i ON t (a) INCLUDE (b, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.include.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_where_predicate() {
        let lexed = crate::tokens::lex("CREATE INDEX i ON t (a) WHERE a > 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_with_storage() {
        let lexed = crate::tokens::lex("CREATE INDEX i ON t (a) WITH (fillfactor = 70)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.with_storage.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_keyword_value_off() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::tokens::lex("CREATE TABLE target (tid integer, balance integer) WITH (autovacuum_enabled=off)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_string_value() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::tokens::lex("CREATE TABLE t (a int) WITH (foo = 'bar')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_signed_numeric_value() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::tokens::lex("CREATE TABLE t (a int) WITH (fillfactor = -30.1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_table_with_storage_signed_integer_value() {
        use crate::ast::ddl::table::CreateTableStmt;
        let lexed = crate::tokens::lex("CREATE TABLE t (a int) WITH (fillfactor = +30)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// Corpus regression for `reloptions.sql`: `(i INT) WITH (fillfactor=-30.1)`.
    /// Requires both the signed-numeric storage-param value and the PG
    /// operator-boundary rule on the lexer (`=-` lexes as Eq + Minus, not
    /// as one 2-char CustomOp).
    #[test]
    fn parse_create_table_with_storage_no_space_signed_numeric() {
        use crate::ast::ddl::table::CreateTableStmt;
        for src in [
            "CREATE TABLE t (i INT) WITH (fillfactor=-30.1)",
            "CREATE TABLE reloptions_test2(i INT) WITH (fillfactor=-30.1)",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = CreateTableStmt::parse(&mut input).unwrap().into_ast();
            assert!(
                input.is_eof(),
                "{src:?}: leftover at offset {}: {:?}",
                input.byte_offset(),
                &src[input.byte_offset()..]
            );
        }
    }

    #[test]
    fn parse_create_unique_index() {
        let lexed = crate::tokens::lex("CREATE UNIQUE INDEX i ON t (a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.unique.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_full_kitchen_sink() {
        let lexed = crate::tokens::lex("CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS idx ON t USING btree (a int4_ops ASC, (lower(b))) INCLUDE (c) WITH (fillfactor = 70) WHERE c > 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.unique.is_some());
        assert!(stmt.concurrently.is_some());
        assert!(stmt.if_not_exists.is_some());
        assert!(stmt.using.is_some());
        assert!(stmt.include.is_some());
        assert!(stmt.with_storage.is_some());
        assert!(stmt.where_clause.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_opclass_on_second_col() {
        let lexed = crate::tokens::lex("create unique index op_index_key on insertconflicttest(key, fruit text_pattern_ops)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_collate() {
        let lexed = crate::tokens::lex("create unique index collation_index_key on insertconflicttest(key, fruit collate \"C\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_collate_and_opclass() {
        let lexed = crate::tokens::lex("create unique index both_index_key on insertconflicttest(key, fruit collate \"C\" text_pattern_ops)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_index_func_target_collate_opclass() {
        let lexed = crate::tokens::lex("create unique index both_index_expr_key on insertconflicttest(key, lower(fruit) collate \"C\" text_pattern_ops)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index() {
        let lexed = crate::tokens::lex("DROP INDEX fooi");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_if_exists() {
        let lexed = crate::tokens::lex("DROP INDEX IF EXISTS fooi");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_concurrently() {
        let lexed = crate::tokens::lex("DROP INDEX CONCURRENTLY fooi");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.concurrently.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_multiple() {
        let lexed = crate::tokens::lex("DROP INDEX a, b, c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.names.len(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_index_cascade() {
        let lexed = crate::tokens::lex("DROP INDEX fooi CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropIndexStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }
}

// =========================================================================
// ALTER/DROP INDEX — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `SET (storage_param = value, ...)` action shared by ALTER INDEX /
/// ALTER VIEW / ALTER MATERIALIZED VIEW / ALTER TABLE — modifies storage
/// parameters. Differs from `WithStorage` (`WITH (...)` on CREATE) only in
/// the leading keyword.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetReloptions<'input> {
    #[tok(SET, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params:

        recursa::Vec1<crate::ast::ddl::index::StorageParam<'input> >

    ,
}

/// `RESET (param_name [= value], ...)` action shared by ALTER INDEX /
/// ALTER VIEW / ALTER MATERIALIZED VIEW / ALTER TABLE — removes storage
/// parameters. Postgres' gram.y `reloption_elem` allows
/// `ColLabel [. ColLabel] [= def_arg]`, so the syntax accepts `name = value`
/// in RESET too (PG ignores the value semantically). Modeled via the same
/// `StorageParam` type used by `WITH (...)`.
#[derive(recursa::Node, Debug, Clone)]
pub struct ResetReloptions<'input> {
    #[tok(RESET, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params:

        recursa::Vec1<crate::ast::ddl::index::StorageParam<'input> >

    ,
}

/// `ATTACH PARTITION qualified_name` — Postgres' `index_partition_cmd` (the
/// single ALTER INDEX form that takes a partition operation).
#[derive(recursa::Node, Debug, Clone)]
pub struct AttachPartitionClause<'input> {
    #[tok(ATTACH, PARTITION, this)]
    pub name: QualifiedName<'input>,
}

/// A column reference inside `ALTER INDEX … ALTER COLUMN col_ref …`:
/// either an integer column position (`SignedIconst`) or a column name
/// (`Ident`). Postgres' gram.y has two productions; we union them as one
/// enum so the surrounding action struct can be derived.
///
/// Variant ordering: `Number` first (lex token kind disjoint from
/// `Ident`), then `Name`.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColumnRef<'input> {
    Number(SignedIconst<'input>),
    Name(crate::tokens::ColId<'input>),
}

/// `ALTER [COLUMN] col_ref SET STATISTICS …` — Postgres' alter_table_cmd
/// for adjusting per-column statistics targets. Used in ALTER INDEX / ALTER
/// TABLE / ALTER MATERIALIZED VIEW.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnSetStatistics<'input> {
    #[tok(ALTER, optional(COLUMN), this)]
    pub col_ref: ColumnRef<'input>,
    #[tok(SET, STATISTICS, this)]
    pub value: SetStatisticsValue<'input>,
}

/// `ALTER [COLUMN] col_ref SET (param = value, ...)` — Postgres'
/// alter_table_cmd for adjusting per-column reloptions. Used in ALTER
/// INDEX (`n_distinct`, etc.).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterColumnSetReloptions<'input> {
    #[tok(ALTER, optional(COLUMN), this)]
    pub col_ref: ColumnRef<'input>,
    pub set: SetReloptions<'input>,
}

/// One `ALTER COLUMN …` cmd on ALTER INDEX. The two forms (`SET
/// STATISTICS` and `SET (params)`) both start with `ALTER … SET`; the
/// disambiguation token is `STATISTICS` vs `(`.
///
/// Variant ordering: `Stats` (the keyword `STATISTICS`) before
/// `Reloptions` (the `(` start) is for clarity only — they peek on
/// distinct tokens after `SET`.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterColumnIndexCmd<'input> {
    Stats(AlterColumnSetStatistics<'input>),
    Reloptions(AlterColumnSetReloptions<'input>),
}

/// One action on a single-target `ALTER INDEX [IF EXISTS] name action` —
/// the corpus-exercised subset of `alter_table_cmds` plus `RenameStmt` and
/// `AlterObjectDependsStmt` and `index_partition_cmd`.
///
/// Variant ordering:
/// - `SetTablespace` (`SET TABLESPACE`) and `SetReloptions` (`SET (`) and
///   `ResetReloptions` (`RESET …`) — second tokens are disjoint, so order
///   is for clarity.
/// - `AlterColumn` starts with the `ALTER` token, distinct from all
///   `SET`/`RESET`/`ATTACH`/`RENAME`/`NO`/`DEPENDS` first tokens.
/// - `Depends` allows a bare `DEPENDS …` (without `NO`), and `NoDepends`
///   is reached via the `Depends` arm since both share the
///   `DependsOnExtension` type (with `NO` as an `Option`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterIndexAction<'input> {
    SetTablespace(SetTablespaceClause<'input>),
    SetReloptions(SetReloptions<'input>),
    ResetReloptions(ResetReloptions<'input>),
    Attach(AttachPartitionClause<'input>),
    AlterColumn(AlterColumnIndexCmd<'input>),
    Depends(DependsOnExtension<'input>),
    Rename(RenameTo<'input>),
}

/// `ALL IN TABLESPACE name [OWNED BY role_list] SET TABLESPACE new
/// [NOWAIT]` — Postgres' bulk-relocate action on ALTER INDEX (and ALTER
/// MATERIALIZED VIEW). Moves every index in the named tablespace to a new
/// tablespace, optionally filtered by owner role(s).
#[derive(recursa::Node, Debug, Clone)]
pub struct AllInTablespaceBody<'input> {
    #[tok(ALL, IN, TABLESPACE, this)]
    pub source: crate::tokens::ColId<'input>,
    pub owned_by: Option<OwnedByRoles<'input>>,
    pub set_tablespace: SetTablespaceClause<'input>,
    #[presence(NOWAIT)]
    pub nowait: bool,
}

/// `OWNED BY role_list` — owner filter on the bulk `ALL IN TABLESPACE`
/// action.
#[derive(recursa::Node, Debug, Clone)]
pub struct OwnedByRoles<'input> {
    #[tok(OWNED, BY, this)]
    pub roles: RoleList<'input>,
}

/// `ALTER INDEX [IF EXISTS] name action` plus the bulk `ALTER INDEX ALL IN
/// TABLESPACE …` form. The two top-level shapes share the leading `ALTER
/// INDEX` keywords, so they sit on either side of a single enum to
/// preserve dispatcher commitment.
///
/// Variant ordering: `All` (starts with `ALL`) before `Single`
/// (starts with `[IF EXISTS] qualified_name` — never `ALL`).
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterIndexBody<'input> {
    All(AllInTablespaceBody<'input>),
    Single(AlterIndexSingle<'input>),
}

/// `[IF EXISTS] name action` — the per-index branch of ALTER INDEX.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterIndexSingle<'input> {
    pub if_exists: Option<IfExists>,
    pub name: QualifiedName<'input>,
    pub action: AlterIndexAction<'input>,
}

/// `ALTER INDEX [IF EXISTS] name action`
/// `ALTER INDEX ALL IN TABLESPACE name [OWNED BY role_list] SET TABLESPACE
///   new [NOWAIT]` — the two top-level shapes of Postgres' `AlterTableStmt`
/// branches that begin with `ALTER INDEX …`, plus the index branches of
/// `RenameStmt` / `AlterObjectDependsStmt` (single form).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterIndexStmt<'input> {
    #[tok(ALTER, INDEX, this)]
    pub body: AlterIndexBody<'input>,
}
