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
        let _stmt = CreateProcedureStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_basic() {
        let lexed = crate::lex("CALL ptest1('a')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CallStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_concat_arg() {
        let lexed = crate::lex("CALL ptest1('xy' || 'zzy')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CallStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_no_args() {
        let lexed = crate::lex("CALL nonexistent()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CallStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_procedure() {
        let lexed = crate::lex("DROP PROCEDURE ptest1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropProcedureStmt::parse(&mut input).unwrap().into_ast();
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
        let stmt = CreateProcedureStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "bar");
        assert!(input.is_eof());
    }
    #[test]
    fn alter_procedure_strict() {
        let stmt: AlterProcedureStmt = parse_stmt("ALTER PROCEDURE ptest1(text) STRICT");
        assert!(matches!(stmt.action, AlterFuncAction::Options(_)));
        reparse_stable::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) STRICT");
    }

    #[test]
    fn alter_procedure_rename() {
        reparse_stable::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) RENAME TO ptest1a");
    }
}
