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
        let _stmt = Statement::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
            .into_ast();
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
            let _stmt = Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    /// `UPDATE arrtest SET c[1:NULL] = '{…}'` — slice with a SQL keyword
    /// (NULL) as the upper bound. Relies on the `pg_lex` post-processor
    /// splitting the `:NULL` PsqlVar into a `Colon` + `NULL` pair.
    #[test]
    fn parse_subscript_assign_slice_null_bound() {
        let src = "UPDATE arrtest SET c[1:NULL] = '{16,25}' WHERE array_dims(c) is null";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = Statement::parse(&mut input)
            .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
            .into_ast();
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
            let _stmt = Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
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
            let _stmt = Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    /// Regression guard: keep the top-level statement enums small enough that the
    /// recursive descent parser fits in the default test thread stack.
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
                "QualifiedWildcard",
                size_of::<crate::ast::shared::expr::QualifiedWildcard<'_>>(),
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
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        // All query forms share the Subquery path so SELECT, VALUES, TABLE,
        // WITH, parentheses, and set operations have one predictive branch.
        assert!(matches!(stmt, Statement::Query(_)));
    }

    #[test]
    fn parse_statement_create_table() {
        let lexed = crate::lex("CREATE TABLE t (f1 bool)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::CreateTable(_)));
    }

    #[test]
    fn parse_statement_insert() {
        let lexed = crate::lex("INSERT INTO t (f1) VALUES (true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::Insert(_)));
    }

    #[test]
    fn parse_statement_delete() {
        let lexed = crate::lex("DELETE FROM t WHERE a > 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::Delete(_)));
    }

    #[test]
    fn parse_statement_drop_table() {
        let lexed = crate::lex("DROP TABLE t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::DropTable(_)));
    }

    #[test]
    fn parse_strict_statement_to_end_of_input() {
        let lexed = crate::lex("SELECT 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
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
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::Query(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_full_insert_with_type_cast() {
        let lexed = crate::lex("INSERT INTO BOOLTBL1 (f1) VALUES (bool 't')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::Insert(_)));
        assert!(input.is_eof());
    }
}
