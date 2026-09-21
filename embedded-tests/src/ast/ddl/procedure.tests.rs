#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_procedure_basic() {
        let lexed = crate::lex(
            "CREATE PROCEDURE ptest1(x text) LANGUAGE SQL AS $$ INSERT INTO cp_test VALUES (1, x); $$",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CreateProcedureStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_basic() {
        let lexed = crate::lex("CALL ptest1('a')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CallStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_concat_arg() {
        let lexed = crate::lex("CALL ptest1('xy' || 'zzy')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CallStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_no_args() {
        let lexed = crate::lex("CALL nonexistent()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = CallStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_procedure() {
        let lexed = crate::lex("DROP PROCEDURE ptest1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = DropProcedureStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    /// CREATE PROCEDURE with a schema-qualified name (gram.y
    /// `CreateFunctionStmt: … PROCEDURE func_name`, where `func_name` is
    /// `type_function_name` accepting `schema.name`). privileges.sql
    /// corpus uses `CREATE PROCEDURE testns.bar()`.
    #[test]
    fn parse_create_procedure_qualified_name() {
        let lexed = crate::lex("CREATE PROCEDURE testns.bar() AS 'select 1' LANGUAGE sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt_parsed = CreateProcedureStmt::parse(&mut input).unwrap();
        let stmt = stmt_parsed.ast();
        assert_eq!(stmt.name.object(), "bar");
        assert!(input.is_eof());
    }
    #[test]
    fn alter_procedure_strict() {
        let stmt = parse_stmt::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) STRICT");
        let stmt = stmt.ast();
        assert!(matches!(stmt.action, AlterFuncAction::Options(_)));
        reparse_stable::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) STRICT");
    }

    #[test]
    fn alter_procedure_rename() {
        reparse_stable::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) RENAME TO ptest1a");
    }
    /// gram.y:1159 `CallStmt: CALL func_application`: the name is a
    /// `func_name`, so it may be schema-qualified, and the arguments are a
    /// function call's. PostgreSQL 17.9 agrees with every line here.
    #[test]
    fn parse_call_as_a_func_application() {
        use crate::ast::shared::expr::FuncCallName;

        for (src, qualified) in [
            ("CALL p()", false),
            ("CALL p(1, 2)", false),
            ("CALL pg_catalog.nosuch()", true),
            ("CALL a.b.c(1)", true),
            ("CALL p(x => 1, y := 2)", false),
            ("CALL p(1, VARIADIC arr)", false),
            ("CALL p(DISTINCT 1)", false),
            ("CALL p(*)", false),
            ("CALL p(1 ORDER BY 1)", false),
            ("CALL \"P\"(1)", false),
            // `type_func_name` and unreserved keywords are function names.
            ("CALL left(1)", false),
            ("CALL value(1)", false),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed =
                CallStmt::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            assert_eq!(
                matches!(parsed.ast().call.name, FuncCallName::Qualified(_)),
                qualified,
                "{src:?}"
            );
        }
        for src in [
            "CALL p",
            "CALL p() OVER ()",
            "CALL p() FILTER (WHERE true)",
            "CALL coalesce(1, 2)",
            "CALL select(1)",
            "CALL p(), q()",
            "CALL (p())",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = CallStmt::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

}
