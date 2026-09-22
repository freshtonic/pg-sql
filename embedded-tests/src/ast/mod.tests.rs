#[cfg(test)]
mod tests {
    use super::*;

    /// `2 !=-- comment` (create_operator.sql) — PG's scan.l splits `!=--`
    /// into the `!=` comparison and a `-- …` line comment. logos would
    /// otherwise greedily take the 4-char `!=--` (CustomOp) operator and
    /// leave the comment body as stray identifier tokens; `pg_lex`'s
    /// `split_bang_eq_minus_before_dash_comment` pass undoes that.
    #[test]
    fn parse_bang_eq_minus_line_comment_split() {
        let src = "SELECT 2 !=-- comment to be removed by psql\n  1";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = Statement::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
            ;
        let _stmt = _stmt_parsed.ast();
        let cursor = input.cursor();
        assert!(input.is_eof(), "parser cursor: {cursor}");
    }

    /// Operator-form `LIKE` / `NOT LIKE` / `ILIKE` / `NOT ILIKE` — PG's
    /// `~~` / `!~~` / `~~*` / `!~~*` (gram.y 14860/14874/14888/14897) are
    /// the operator-equivalent spellings of the LIKE family. Used as
    /// ordinary infix Pratt operators on any a_expr.
    #[test]
    fn parse_like_operator_aliases() {
        for src in [
            "SELECT ROW('a','b') ~~ ROW('a','b') AS like_op",
            "SELECT 'foo' !~~ 'bar'",
            "SELECT 'foo' ~~* 'bar'",
            "SELECT 'foo' !~~* 'bar'",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    /// `UPDATE arrtest SET c[1:NULL] = '{…}'` — slice with a SQL keyword
    /// (NULL) as the upper bound, which is just gram.y's `opt_slice_bound`
    /// holding an `a_expr`.
    #[test]
    fn parse_subscript_assign_slice_null_bound() {
        let src = "UPDATE arrtest SET c[1:NULL] = '{16,25}' WHERE array_dims(c) is null";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = Statement::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
            ;
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// `IS [NFC|NFD|NFKC|NFKD] NORMALIZED` and `IS NOT [NFC|NFD|NFKC|NFKD]
    /// NORMALIZED` — gram.y `a_expr IS [NOT] [unicode_normal_form] NORMALIZED`.
    /// The bare form (no NF-prefix) tests for default-form NFC normalization.
    #[test]
    fn parse_is_normalized() {
        for src in [
            "SELECT 'abc' IS NORMALIZED",
            "SELECT 'abc' IS NOT NORMALIZED",
            "SELECT 'abc' IS NFC NORMALIZED",
            "SELECT 'abc' IS NFD NORMALIZED",
            "SELECT 'abc' IS NFKC NORMALIZED",
            "SELECT 'abc' IS NFKD NORMALIZED",
            "SELECT 'abc' IS NOT NFC NORMALIZED",
            "SELECT U&'\\00E4\\24D1c' IS NFC NORMALIZED AS test_nfc",
            "SELECT U&'\\00E4\\24D1c' IS NORMALIZED AS test_default",
            // Restricting the bare-alias admission must not restrict the
            // explicit PostgreSQL `AS ColLabel` form.
            "SELECT 1 AS is",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    /// `SET` is `unreserved_keyword` per kwlist.h — PG accepts a function
    /// named `set`, both as a call site (`SELECT set('t')`) and at function
    /// definition / drop sites. pg-sql keeps `SET` as a hard keyword to
    /// preserve `UPDATE … SET …` disambiguation, but reclaims it explicitly
    /// in function-name positions (see `FuncCallName::Set`).
    #[test]
    fn parse_set_as_function_name() {
        for src in [
            "SELECT set('t')",
            "CREATE FUNCTION set(tabname name) RETURNS VOID AS $$ BEGIN END; $$ LANGUAGE plpgsql",
            "DROP FUNCTION set(name)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _stmt = _stmt_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    /// Regression guard: keep the top-level statement enums small enough that the
    /// generated parser fits in the default test thread stack.
    /// Prior to boxing the largest variants, `Statement` was 1480 bytes and
    /// fixture-parsing tests required `RUST_MIN_STACK=16777216`.
    #[test]
    fn statement_size_is_bounded() {
        use std::mem::size_of;
        let stmt = size_of::<Statement<'_>>();
        assert!(
            stmt <= 128,
            "Statement grew to {stmt} bytes — Box the largest variants",
        );
    }

    /// Print sizes of major AST node types. Run with `--nocapture` to see output.
    /// `#[ignore]` so it doesn't run by default but stays available for diagnosis.
    #[test]
    #[ignore]
    fn report_ast_sizes() {
        use std::mem::size_of;
        let mut sizes: Vec<(&'static str, usize)> = vec![
            ("TerminatedStatement", size_of::<TerminatedStatement<'_>>()),
            ("Statement", size_of::<Statement<'_>>()),
            ("Expr", size_of::<crate::ast::shared::expr::Expr<'_>>()),
            (
                "CaseSearched",
                size_of::<crate::ast::shared::expr::CaseSearched<'_>>(),
            ),
            (
                "CaseSimple",
                size_of::<crate::ast::shared::expr::CaseSimple<'_>>(),
            ),
            (
                "IntervalLit",
                size_of::<crate::ast::shared::expr::IntervalLit<'_>>(),
            ),
            (
                "TimestampLit",
                size_of::<crate::ast::shared::expr::TimestampLit<'_>>(),
            ),
            (
                "TypeCastFunc",
                size_of::<crate::ast::shared::expr::TypeCastFunc<'_>>(),
            ),
            (
                "XmlElement",
                size_of::<crate::ast::shared::expr::XmlElement<'_>>(),
            ),
            (
                "XmlForest",
                size_of::<crate::ast::shared::expr::XmlForest<'_>>(),
            ),
            (
                "XmlAttributes",
                size_of::<crate::ast::shared::expr::XmlAttributes<'_>>(),
            ),
            ("XmlPi", size_of::<crate::ast::shared::expr::XmlPi<'_>>()),
            (
                "ArrayExpr",
                size_of::<crate::ast::shared::expr::ArrayExpr<'_>>(),
            ),
            (
                "QualifiedRef",
                size_of::<crate::ast::shared::expr::QualifiedRef<'_>>(),
            ),
            (
                "ParenthesizedExpr",
                size_of::<crate::ast::shared::expr::ParenthesizedExpr<'_>>(),
            ),
            (
                "ExistsExpr",
                size_of::<crate::ast::shared::expr::ExistsExpr<'_>>(),
            ),
            (
                "ArrayBracket",
                size_of::<crate::ast::shared::expr::ArrayBracket<'_>>(),
            ),
            (
                "RowExpr",
                size_of::<crate::ast::shared::expr::RowExpr<'_>>(),
            ),
            (
                "CastType",
                size_of::<crate::ast::shared::expr::CastType<'_>>(),
            ),
            (
                "ExtractCall",
                size_of::<crate::ast::shared::expr::ExtractCall<'_>>(),
            ),
            (
                "NotInSuffix",
                size_of::<crate::ast::shared::expr::NotInSuffix<'_>>(),
            ),
            (
                "InContent",
                size_of::<crate::ast::shared::expr::InContent<'_>>(),
            ),
            ("InList", size_of::<crate::ast::shared::expr::InList<'_>>()),
            (
                "SubstringCall",
                size_of::<crate::ast::shared::expr::SubstringCall<'_>>(),
            ),
            (
                "OverlayCall",
                size_of::<crate::ast::shared::expr::OverlayCall<'_>>(),
            ),
            (
                "TrimCall",
                size_of::<crate::ast::shared::expr::TrimCall<'_>>(),
            ),
            (
                "PositionCall",
                size_of::<crate::ast::shared::expr::PositionCall<'_>>(),
            ),
            (
                "SelectStmt",
                size_of::<crate::ast::dml::select::SelectStmt<'_>>(),
            ),
            (
                "CreateTableStmt",
                size_of::<crate::ast::ddl::table::CreateTableStmt<'_>>(),
            ),
            (
                "CreateFunctionStmt",
                size_of::<crate::ast::ddl::function::CreateFunctionStmt<'_>>(),
            ),
            (
                "InsertStmt",
                size_of::<crate::ast::dml::insert::InsertStmt<'_>>(),
            ),
            (
                "UpdateStmt",
                size_of::<crate::ast::dml::update::UpdateStmt<'_>>(),
            ),
            (
                "DeleteStmt",
                size_of::<crate::ast::dml::delete::DeleteStmt<'_>>(),
            ),
            (
                "MergeStmt",
                size_of::<crate::ast::dml::merge::MergeStmt<'_>>(),
            ),
            (
                "ExplainStmt",
                size_of::<crate::ast::utility::explain::ExplainStmt<'_>>(),
            ),
            (
                "CompoundQuery",
                size_of::<crate::ast::dml::values::Subquery<'_>>(),
            ),
            (
                "WithStatement",
                size_of::<crate::ast::shared::with_clause::WithStatement<'_>>(),
            ),
            (
                "FuncCall",
                size_of::<crate::ast::shared::expr::FuncCall<'_>>(),
            ),
            (
                "ColumnDef",
                size_of::<crate::ast::ddl::table::ColumnDef<'_>>(),
            ),
            (
                "ConflictAction",
                size_of::<crate::ast::dml::insert::ConflictAction<'_>>(),
            ),
            (
                "DoUpdateAction",
                size_of::<crate::ast::dml::insert::DoUpdateAction<'_>>(),
            ),
            (
                "GroupByItem",
                size_of::<crate::ast::dml::select::GroupByItem<'_>>(),
            ),
            (
                "FuncArg",
                size_of::<crate::ast::shared::expr::FuncArg<'_>>(),
            ),
            (
                "AlterTableStmt",
                size_of::<crate::ast::ddl::table::AlterTableStmt<'_>>(),
            ),
            (
                "CreateTriggerStmt",
                size_of::<crate::ast::ddl::trigger::CreateTriggerStmt<'_>>(),
            ),
            (
                "CreateRuleStmt",
                size_of::<crate::ast::ddl::rule::CreateRuleStmt<'_>>(),
            ),
            (
                "CreateForeignStmt",
                size_of::<crate::ast::ddl::foreign::CreateForeignStmt<'_>>(),
            ),
            (
                "CreateMaterializedViewStmt",
                size_of::<crate::ast::ddl::materialized_view::CreateMaterializedViewStmt<'_>>(),
            ),
            (
                "AlterMaterializedViewStmt",
                size_of::<crate::ast::ddl::materialized_view::AlterMaterializedViewStmt<'_>>(),
            ),
            (
                "CopyStmt",
                size_of::<crate::ast::utility::copy::CopyStmt<'_>>(),
            ),
            (
                "VacuumStmt",
                size_of::<crate::ast::utility::vacuum::VacuumStmt<'_>>(),
            ),
            (
                "ReindexStmt",
                size_of::<crate::ast::utility::reindex::ReindexStmt<'_>>(),
            ),
            (
                "ClusterStmt",
                size_of::<crate::ast::utility::cluster::ClusterStmt<'_>>(),
            ),
            (
                "GrantStmt",
                size_of::<crate::ast::utility::grant::GrantStmt<'_>>(),
            ),
            (
                "RevokeStmt",
                size_of::<crate::ast::utility::grant::RevokeStmt<'_>>(),
            ),
            ("DoStmt", size_of::<crate::ast::utility::r#do::DoStmt<'_>>()),
            (
                "CreateRoleStmt",
                size_of::<crate::ast::ddl::role::CreateRoleStmt<'_>>(),
            ),
            (
                "CreateAggregateStmt",
                size_of::<crate::ast::ddl::aggregate::CreateAggregateStmt<'_>>(),
            ),
            (
                "CreateOperatorStmt",
                size_of::<crate::ast::ddl::operator::CreateOperatorStmt<'_>>(),
            ),
            (
                "AnalyzeStmt",
                size_of::<crate::ast::utility::analyze::AnalyzeStmt<'_>>(),
            ),
            (
                "CreateIndexStmt",
                size_of::<crate::ast::ddl::index::CreateIndexStmt<'_>>(),
            ),
            (
                "CreateViewStmt",
                size_of::<crate::ast::ddl::view::CreateViewStmt<'_>>(),
            ),
            (
                "DropTableStmt",
                size_of::<crate::ast::ddl::table::DropTableStmt<'_>>(),
            ),
            (
                "CreateUserMappingStmt",
                size_of::<crate::ast::ddl::role::CreateUserMappingStmt<'_>>(),
            ),
            (
                "AlterUserMappingStmt",
                size_of::<crate::ast::ddl::role::AlterUserMappingStmt<'_>>(),
            ),
            (
                "DropUserMappingStmt",
                size_of::<crate::ast::ddl::role::DropUserMappingStmt<'_>>(),
            ),
            (
                "AlterOperatorClassStmt",
                size_of::<crate::ast::ddl::operator::AlterOperatorClassStmt<'_>>(),
            ),
            (
                "CreateProcedureStmt",
                size_of::<crate::ast::ddl::procedure::CreateProcedureStmt<'_>>(),
            ),
            (
                "CreateTablespaceStmt",
                size_of::<crate::ast::ddl::tablespace::CreateTablespaceStmt<'_>>(),
            ),
            (
                "DropFunctionStmt",
                size_of::<crate::ast::ddl::function::DropFunctionStmt<'_>>(),
            ),
            (
                "CreateEventTriggerStmt",
                size_of::<crate::ast::ddl::trigger::CreateEventTriggerStmt<'_>>(),
            ),
            (
                "CreateAccessMethodStmt",
                size_of::<crate::ast::ddl::access_method::CreateAccessMethodStmt<'_>>(),
            ),
            (
                "CreateLanguageStmt",
                size_of::<crate::ast::ddl::language::CreateLanguageStmt<'_>>(),
            ),
            (
                "CreateDatabaseStmt",
                size_of::<crate::ast::ddl::database::CreateDatabaseStmt<'_>>(),
            ),
            (
                "CreateUserStmt",
                size_of::<crate::ast::ddl::role::CreateUserStmt<'_>>(),
            ),
            (
                "CreateSchemaStmt",
                size_of::<crate::ast::ddl::schema::CreateSchemaStmt<'_>>(),
            ),
            (
                "CreateSequenceStmt",
                size_of::<crate::ast::ddl::sequence::CreateSequenceStmt<'_>>(),
            ),
            (
                "CreateTypeStmt",
                size_of::<crate::ast::ddl::r#type::CreateTypeStmt<'_>>(),
            ),
            (
                "CreateDomainStmt",
                size_of::<crate::ast::ddl::domain::CreateDomainStmt<'_>>(),
            ),
            (
                "CreateCastStmt",
                size_of::<crate::ast::ddl::cast::CreateCastStmt<'_>>(),
            ),
            (
                "CreateCollationStmt",
                size_of::<crate::ast::ddl::collation::CreateCollationStmt<'_>>(),
            ),
            (
                "CreateExtensionStmt",
                size_of::<crate::ast::ddl::extension::CreateExtensionStmt<'_>>(),
            ),
            (
                "CreatePolicyStmt",
                size_of::<crate::ast::ddl::policy::CreatePolicyStmt<'_>>(),
            ),
            (
                "CreateStatisticsStmt",
                size_of::<crate::ast::ddl::statistics::CreateStatisticsStmt<'_>>(),
            ),
            (
                "CreatePublicationStmt",
                size_of::<crate::ast::ddl::publication::CreatePublicationStmt<'_>>(),
            ),
            (
                "CreateSubscriptionStmt",
                size_of::<crate::ast::ddl::subscription::CreateSubscriptionStmt<'_>>(),
            ),
            (
                "CreateConversionStmt",
                size_of::<crate::ast::ddl::conversion::CreateConversionStmt<'_>>(),
            ),
            (
                "CreateServerStmt",
                size_of::<crate::ast::ddl::foreign::CreateServerStmt<'_>>(),
            ),
            (
                "CreateGroupStmt",
                size_of::<crate::ast::ddl::role::CreateGroupStmt<'_>>(),
            ),
            (
                "AlterIndexStmt",
                size_of::<crate::ast::ddl::index::AlterIndexStmt<'_>>(),
            ),
            (
                "AlterViewStmt",
                size_of::<crate::ast::ddl::view::AlterViewStmt<'_>>(),
            ),
            (
                "AlterFunctionStmt",
                size_of::<crate::ast::ddl::function::AlterFunctionStmt<'_>>(),
            ),
            (
                "AlterDatabaseStmt",
                size_of::<crate::ast::ddl::database::AlterDatabaseStmt<'_>>(),
            ),
            (
                "AlterDomainStmt",
                size_of::<crate::ast::ddl::domain::AlterDomainStmt<'_>>(),
            ),
            (
                "AlterEventTriggerStmt",
                size_of::<crate::ast::ddl::trigger::AlterEventTriggerStmt<'_>>(),
            ),
            (
                "AlterTriggerStmt",
                size_of::<crate::ast::ddl::trigger::AlterTriggerStmt<'_>>(),
            ),
            (
                "AlterSequenceStmt",
                size_of::<crate::ast::ddl::sequence::AlterSequenceStmt<'_>>(),
            ),
            (
                "ImportForeignSchemaStmt",
                size_of::<crate::ast::ddl::foreign::ImportForeignSchemaStmt<'_>>(),
            ),
            (
                "CommentStmt",
                size_of::<crate::ast::utility::comment::CommentStmt<'_>>(),
            ),
            (
                "SecurityLabelStmt",
                size_of::<crate::ast::utility::comment::SecurityLabelStmt<'_>>(),
            ),
            (
                "PrepareStmt",
                size_of::<crate::ast::tcl::prepared::PrepareStmt<'_>>(),
            ),
            (
                "TableRef",
                size_of::<crate::ast::dml::select::TableRef<'_>>(),
            ),
            (
                "SimpleTableRef",
                size_of::<crate::ast::dml::select::SimpleTableRef<'_>>(),
            ),
            (
                "CompoundQuery (if any)",
                size_of::<crate::ast::dml::values::Subquery<'_>>(),
            ),
        ];
        sizes.sort_by_key(|b| std::cmp::Reverse(b.1));
        eprintln!("\n=== AST sizes (bytes) ===");
        for (name, size) in &sizes {
            eprintln!("{size:>6}  {name}");
        }
        eprintln!();
    }

    #[test]
    fn parse_statement_select() {
        let lexed = crate::lex("SELECT 1 AS one");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        // All query forms share the Subquery path so SELECT, VALUES, TABLE,
        // WITH, parentheses, and set operations share one Subquery path.
        assert!(matches!(stmt, Statement::Query(_)));
    }

    #[test]
    fn parse_statement_create_table() {
        let lexed = crate::lex("CREATE TABLE t (f1 bool)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::CreateTable(_)));
    }

    #[test]
    fn parse_statement_insert() {
        let lexed = crate::lex("INSERT INTO t (f1) VALUES (true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::Insert(_)));
    }

    #[test]
    fn parse_statement_delete() {
        let lexed = crate::lex("DELETE FROM t WHERE a > 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::Delete(_)));
    }

    #[test]
    fn parse_statement_drop_table() {
        let lexed = crate::lex("DROP TABLE t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::DropTable(_)));
    }

    #[test]
    fn parse_strict_statement_to_end_of_input() {
        let lexed = crate::lex("SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::Query(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn psql_directive_is_not_a_strict_statement() {
        let lexed = crate::lex("\\pset null '(null)'\n");
        let mut input = lexed.input();
        assert!(Statement::parse(&mut input).is_err());
    }

    #[test]
    fn parse_select_with_where_and_bool_test() {
        let lexed = crate::lex("SELECT f1 FROM BOOLTBL1 WHERE f1 IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::Query(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_full_insert_with_type_cast() {
        let lexed = crate::lex("INSERT INTO BOOLTBL1 (f1) VALUES (bool 't')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = Statement::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert!(matches!(stmt, Statement::Insert(_)));
        assert!(input.is_eof());
    }
    /// gram.y:13757 `relation_expr` and gram.y:13771 `extended_relation_expr`:
    /// `name`, `name *`, `ONLY name` and `ONLY ( name )`, and never `ONLY name
    /// *`. Every statement gram.y gives a `relation_expr` takes all four.
    /// Before the shared node each statement had its own pair of flags, with
    /// no `ONLY ( name )` and with `ONLY name *`. PostgreSQL 17.9 agrees with
    /// every line here.
    #[test]
    fn parse_relation_expr_forms_in_every_statement() {
        use crate::ast::shared::names::{OnlyRelation, RelationExpr};

        for (src, only, parens, star) in [
            ("t", false, false, false),
            ("s.t", false, false, false),
            ("t *", false, false, true),
            ("ONLY t", true, false, false),
            ("ONLY (t)", true, true, false),
            ("only ( s.t )", true, true, false),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed =
                RelationExpr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            let relation = parsed.ast();
            assert_eq!(relation.is_only(), only, "{src:?}");
            assert_eq!(
                matches!(relation, RelationExpr::Only(OnlyRelation::Parens(_))),
                parens,
                "{src:?}"
            );
            assert_eq!(
                matches!(relation, RelationExpr::Named(named) if named.star),
                star,
                "{src:?}"
            );
            assert_eq!(relation.name().object(), "t", "{src:?}");
        }
        for src in ["ONLY t *", "ONLY ((t))", "(t)", "ONLY", "ONLY ()", "t * *"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = RelationExpr::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }

        let statements = [
            "LOCK TABLE {}",
            "LOCK TABLE {}, {} IN SHARE MODE",
            "TRUNCATE {}",
            "TRUNCATE TABLE {}, u",
            "ALTER TABLE {} ADD COLUMN a int",
            "ALTER TABLE IF EXISTS {} RENAME TO v",
            "ALTER TABLE {} ATTACH PARTITION q DEFAULT",
            "ALTER FOREIGN TABLE {} ADD COLUMN a int",
            "CREATE INDEX ON {} (a)",
            "UPDATE {} SET a = 1",
            "UPDATE {} AS x SET a = 1",
            "DELETE FROM {}",
            "DELETE FROM {} x WHERE true",
            "MERGE INTO {} USING u ON true WHEN MATCHED THEN DO NOTHING",
            "CREATE PUBLICATION p FOR TABLE {}",
            "ALTER PUBLICATION p ADD TABLE {} (a) WHERE (a > 1)",
            "TABLE {}",
            "CREATE VIEW v AS TABLE {}",
            "SELECT * FROM {}",
        ];
        for statement in statements {
            for (form, accepted) in [
                ("t", true),
                ("s.t", true),
                ("t *", true),
                ("ONLY t", true),
                ("ONLY (t)", true),
                ("ONLY (s.t)", true),
                ("ONLY t *", false),
                ("ONLY ((t))", false),
            ] {
                let src: &'static str = statement.replace("{}", form).leak();
                let lexed = crate::lex(src);
                assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
                let mut input = lexed.input();
                let parsed = Statement::parse(&mut input);
                let complete = parsed.is_ok() && input.is_eof();
                assert_eq!(complete, accepted, "{src:?}");
            }
        }
    }

    /// `COLLATE any_name` and the operator class name after it
    /// (`opt_qualified_name`) are `any_name` wherever gram.y has them:
    /// gram.y:8231 `opt_collate`, `opt_collate_clause` in a column definition
    /// and in `TableFuncElement`, and `part_elem`. pg-sql had four different
    /// single-identifier types there. PostgreSQL 17.9 accepts every statement
    /// of the first list and rejects every one of the second.
    #[test]
    fn parse_collation_and_opclass_names_as_any_name() {
        let accepted = [
            "SELECT * FROM t ORDER BY x COLLATE pg_catalog.\"C\" DESC",
            "CREATE TABLE t (a text COLLATE pg_catalog.\"C\" NOT NULL DEFAULT '')",
            "ALTER TABLE t ADD COLUMN a text COLLATE a.b.\"C\"",
            "ALTER TABLE t ALTER COLUMN a TYPE text COLLATE pg_catalog.default",
            "CREATE INDEX ON t (a COLLATE pg_catalog.\"C\" s.text_pattern_ops DESC NULLS LAST)",
            "CREATE INDEX ON t ((a || b) COLLATE pg_catalog.\"C\")",
            "CREATE INDEX ON t (a s.ops (k = 1))",
            "CREATE TABLE p (a text) PARTITION BY RANGE (a COLLATE pg_catalog.\"C\" s.text_ops)",
            "CREATE TABLE t (a text, EXCLUDE USING gist (a COLLATE pg_catalog.\"C\" WITH =))",
            "CREATE STATISTICS s ON (a COLLATE pg_catalog.\"C\"), b FROM t",
            "CREATE DOMAIN d AS text COLLATE pg_catalog.\"C\"",
            "CREATE TYPE c AS (a text COLLATE pg_catalog.\"C\")",
            // gram.y `TableFuncElement: ColId Typename opt_collate_clause`.
            "SELECT * FROM f() AS (a text COLLATE pg_catalog.\"C\")",
            "SELECT * FROM f() AS x (a int, b text COLLATE \"C\")",
            "SELECT * FROM ROWS FROM (f() AS (a text COLLATE s.\"C\")) x",
            // gram.y:13694 `func_alias_clause`.
            "SELECT * FROM f() AS x (a, b)",
            "SELECT * FROM f() x (a int, b text)",
            "SELECT * FROM f() WITH ORDINALITY AS x (a, n)",
        ];
        for src in accepted {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
        let rejected = [
            "SELECT CAST(x AS text COLLATE \"C\")",
            // A name list has no collation, and a list is names or definitions.
            "SELECT * FROM f() AS (a COLLATE \"C\")",
            "SELECT * FROM f() AS x (a int, b)",
            "SELECT * FROM f() AS x (a, b int)",
            // A list needs `AS` or a name, and with no name it holds definitions.
            "SELECT * FROM f() AS (a, b)",
            "SELECT * FROM f() (a int)",
            "SELECT * FROM f() (a)",
            "SELECT * FROM ROWS FROM (f()) (a int)",
            "SELECT * FROM f() AS x ()",
        ];
        for src in rejected {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Statement::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// gram.y:15637 `func_expr_windowless` is what a `func_table`, a
    /// `rowsfrom_item`, an `index_elem` (so an `ON CONFLICT` arbiter) and a
    /// `part_elem` take, and its `func_expr_common_subexpr` half is every
    /// "special expression considered to be a function": `CAST`, `TREAT`,
    /// `TRIM`, `EXTRACT`, the XML and SQL/JSON functions, `COALESCE` and the
    /// rest. pg-sql took four of them in `FROM` and eleven in an index
    /// element. Each position now holds the `Expr` that the expression grammar
    /// builds for the same text. PostgreSQL 17.9 accepts every statement here.
    #[test]
    fn parse_special_function_forms_in_windowless_positions() {
        let forms = [
            ("coalesce(a, b)", "Coalesce(CoalesceExpr"),
            ("greatest(a, b)", "Greatest(GreatestExpr"),
            ("nullif(a, b)", "NullIf(NullIfExpr"),
            ("normalize(a, nfc)", "Normalize(NormalizeExpr"),
            ("xmlconcat(a, b)", "XmlConcat(XmlConcatExpr"),
            ("cast(a AS int)", "CastCall(CastCall"),
            ("treat(a AS int)", "Treat(TreatCall"),
            ("collation for (a)", "CollationFor("),
            ("extract(year FROM a)", "Extract("),
            ("overlay(a PLACING b FROM 1)", "Overlay("),
            ("position(a IN b)", "Position("),
            ("substring(a FROM 1 FOR 2)", "Substring("),
            ("substring(a, 1, 2)", "Substring("),
            ("trim(BOTH a FROM b)", "Trim("),
            ("xmlelement(NAME x, a)", "XmlElement("),
            ("xmlexists(a PASSING BY REF b)", "XmlExists("),
            ("xmlforest(a, b)", "XmlForest("),
            ("xmlparse(DOCUMENT a)", "XmlParse("),
            ("xmlpi(NAME x)", "XmlPi("),
            ("xmlroot(a, VERSION '1.0')", "XmlRoot("),
            ("xmlserialize(DOCUMENT a AS text)", "XmlSerialize("),
            ("json_object('a': 1)", "JsonObject("),
            ("json_array(1, 2)", "JsonArray("),
            ("json(a)", "JsonCtor("),
            ("json_scalar(a)", "JsonScalar("),
            ("json_serialize(a)", "JsonSerialize("),
            ("json_query(a, '$')", "JsonQuery("),
            ("json_exists(a, '$')", "JsonExists("),
            ("json_value(a, '$')", "JsonValue("),
        ];
        let positions = [
            "SELECT * FROM {}",
            "SELECT * FROM {} AS x (d int, e int)",
            "SELECT * FROM {} WITH ORDINALITY x",
            "SELECT * FROM ROWS FROM ({}, f(1)) x",
            "CREATE INDEX ON t ({} DESC, b)",
            "INSERT INTO t VALUES (1) ON CONFLICT ({}) DO NOTHING",
            "CREATE TABLE p (a int) PARTITION BY RANGE ({})",
        ];
        for (form, node) in forms {
            // The expression grammar builds the same variant for the same text.
            let src: &'static str = format!("SELECT {form}").leak();
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Statement::parse(&mut input).unwrap_or_else(|e| panic!("{src:?}: {e}"));
            assert!(format!("{:?}", parsed.ast()).contains(node), "{src:?} has no {node}");

            for position in positions {
                let src: &'static str = position.replace("{}", form).leak();
                let lexed = crate::lex(src);
                assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
                let mut input = lexed.input();
                let parsed =
                    Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
                assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
                let tree = format!("{:?}", parsed.ast());
                assert!(tree.contains(node), "{src:?} did not build {node}");
            }
        }

        // `USER` is the one form with no parentheses.
        for src in ["SELECT * FROM USER", "CREATE INDEX ON t (user)", "SELECT * FROM ROWS FROM (user) x"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "{src:?}");
        }
        // A bare word is still a relation or a column, and an ordinary call
        // keeps its own node.
        for (src, absent) in [
            ("SELECT * FROM trim", "Trim("),
            ("CREATE INDEX ON t (extract)", "Extract("),
            ("CREATE INDEX ON t (lower(a))", "Special("),
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Statement::parse(&mut input).unwrap_or_else(|e| panic!("{src:?}: {e}"));
            assert!(input.is_eof(), "{src:?}");
            assert!(!format!("{:?}", parsed.ast()).contains(absent), "{src:?}");
        }
        // Only an expression in parentheses is an operator expression here,
        // a windowless call has no `OVER`, and an opclass option list
        // (gram.y:3026 `reloptions`) is never empty.
        for src in [
            "CREATE INDEX ON t (a + b)",
            "SELECT * FROM a + b",
            "CREATE INDEX ON t (f(a) OVER ())",
            "CREATE INDEX ON t (a ops ())",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Statement::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// Whether `Statement` parses `src` to the end.
    fn statement_parses(src: &'static str) -> bool {
        let lexed = crate::lex(src);
        if lexed.errors().count() != 0 {
            return false;
        }
        let mut input = lexed.input();
        Statement::parse(&mut input).is_ok() && input.is_eof()
    }

    /// Asserts that each accepted form parses and formats to a fixed point,
    /// and that each rejected form does not parse to the end.
    fn check_statement_forms(accepted: &[&'static str], rejected: &[&'static str]) {
        for &src in accepted {
            assert!(statement_parses(src), "{src:?} did not parse");
            crate::ast::test_support::reparse_stable::<Statement>(src);
        }
        for &src in rejected {
            assert!(!statement_parses(src), "{src:?} parsed completely");
        }
    }

    /// Added in 19: `CHECKPOINT opt_utility_option_list` (gram.y b73d13c:2100,
    /// `utility_option_list` 1164; research PostgreSQL 19, "Changes to
    /// existing statements").
    #[cfg(feature = "since-pg19")]
    #[test]
    fn checkpoint_takes_utility_options_from_19() {
        check_statement_forms(
            &[
                "CHECKPOINT",
                "CHECKPOINT (MODE SPREAD, FLUSH_UNLOGGED)",
                "CHECKPOINT (flush_unlogged false, mode 'fast')",
                "CHECKPOINT (verbose verbose, analyze, format json)",
                "CHECKPOINT (x -1, y +2.5, z on, w E'a')",
            ],
            &[
                "CHECKPOINT ()",
                "CHECKPOINT MODE SPREAD",
                "CHECKPOINT (x null)",
                "CHECKPOINT (x default)",
                "CHECKPOINT (select)",
            ],
        );
    }

    /// Added in 19: gram.y `RepackStmt` (b73d13c:12080; research PostgreSQL
    /// 19, "New statements"). The table is a `qualified_name`, so `ONLY` and
    /// `*` are rejected (f23de46e15b).
    #[cfg(feature = "since-pg19")]
    #[test]
    fn repack_statement_forms_from_19() {
        check_statement_forms(
            &[
                "REPACK",
                "REPACK s.t",
                "REPACK t (a, b)",
                "REPACK t USING INDEX",
                "REPACK t (a) USING INDEX i",
                "REPACK (CONCURRENTLY, VERBOSE) t USING INDEX t_pkey",
                "REPACK (VERBOSE)",
                "REPACK USING INDEX",
                "REPACK (analyze) USING INDEX",
            ],
            &[
                "REPACK ONLY t",
                "REPACK t *",
                "REPACK t USING i",
                "REPACK VERBOSE t",
                "REPACK () t",
                "REPACK USING INDEX i",
                "REPACK t ()",
            ],
        );
    }

    /// Added in 19: gram.y `WaitStmt` (b73d13c:16635, `opt_wait_with_clause`
    /// 16646; research PostgreSQL 19, "New statements"). The LSN is an
    /// `Sconst`.
    #[cfg(feature = "since-pg19")]
    #[test]
    fn wait_for_lsn_forms_from_19() {
        check_statement_forms(
            &[
                "WAIT FOR LSN '0/3000060'",
                "WAIT FOR LSN '0/3000060' WITH (MODE 'replay', TIMEOUT '1s')",
                "WAIT FOR LSN E'0/1' WITH (no_throw)",
                "WAIT FOR LSN $$0/1$$",
            ],
            &[
                "WAIT FOR LSN 12",
                "WAIT FOR LSN B'01'",
                "WAIT FOR LSN '0/1' WITH ()",
                "WAIT FOR LSN '0/1' (timeout 1)",
                "WAIT LSN '0/1'",
            ],
        );
    }

    /// Before 19 there is no `CHECKPOINT` option list, no `REPACK` and no
    /// `WAIT FOR LSN` (the research entries above).
    #[cfg(not(feature = "since-pg19"))]
    #[test]
    fn reject_19_utility_statements_before_19() {
        check_statement_forms(
            &["CHECKPOINT"],
            &[
                "CHECKPOINT (MODE SPREAD)",
                "REPACK",
                "REPACK t USING INDEX i",
                "WAIT FOR LSN '0/1'",
            ],
        );
    }

}
