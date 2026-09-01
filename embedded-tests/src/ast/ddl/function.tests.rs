#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ddl::function::{CreateFunctionStmt, DropFunctionStmt};
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_function_return_body() {
        let lexed = crate::lex("CREATE FUNCTION f() RETURNS boolean RETURN false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_basic() {
        let lexed = crate::lex(
            "create function sillysrf(int) returns setof int as 'values (1),(10),(2),($1)' language sql immutable",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "sillysrf");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_basic() {
        let lexed = crate::lex("drop function sillysrf(int)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(
            stmt.targets.first().unwrap().name.object(),
            "sillysrf"
        );
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_multi() {
        let lexed = crate::lex("drop function a(), b(), c()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_named_param() {
        let lexed = crate::lex(
            "create function polyf(x anyelement) returns anyelement as $$ select x + 1 $$ language sql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_cascade() {
        let lexed = crate::lex("DROP FUNCTION int4_casttesttype(int4) CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_function_named_param() {
        let lexed = crate::lex("drop function polyf(x anyelement)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_returns_trigger() {
        let lexed = crate::lex(
            "create function f() returns trigger language plpgsql as $$ begin end $$",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_strict_immutable() {
        let lexed = crate::lex(
            "create function f() returns int immutable strict language sql as 'SELECT 1'",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_options_reordered() {
        let lexed =
            crate::lex("create function f() returns int language sql strict as 'SELECT 1'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_in_out_named() {
        let lexed = crate::lex(
            "create function f(in i int, out j int) returns int as $$ begin return i+1; end $$ language plpgsql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_in_out_no_returns() {
        let lexed = crate::lex(
            "create function f(in i int, out j int) as $$ begin end $$ language plpgsql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_setof_record() {
        let lexed = crate::lex(
            "create function gs(v integer, out a integer, out b integer) returns setof record as $f$ select 1 $f$ language plpgsql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_polymorphic_out() {
        let lexed = crate::lex(
            "create function poly(a anyelement, b anyarray, OUT x anyarray) as $$ begin end $$ language plpgsql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_param_eq_default() {
        let lexed = crate::lex(
            "create function f(a int = 1, b int = 2) returns int as $$ select 1 $$ language sql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_param_default_keyword() {
        let lexed = crate::lex(
            "create function f(a int default 1) returns int as $$ select 1 $$ language sql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_unnamed_default() {
        let lexed = crate::lex(
            "create function dfunc(a int = 1, int = 2) returns int as $$ select 1 $$ language sql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_array_arg() {
        let lexed = crate::lex(
            "CREATE FUNCTION stfnp(int[]) RETURNS int[] AS 'select $1' LANGUAGE SQL",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_array_arg_multi() {
        let lexed = crate::lex(
            "CREATE FUNCTION f(int[], text[]) RETURNS int[] AS 'select $1' LANGUAGE SQL",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_nested_array() {
        let lexed = crate::lex(
            "CREATE FUNCTION f(x int[][]) RETURNS int[][] AS 'select x' LANGUAGE SQL",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_function_multi_named_params() {
        let lexed = crate::lex(
            "create function tg_hub_adjustslots(hname bpchar, oldn integer, newn integer) returns integer as ' begin return 1; end ' language plpgsql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn func_body_dollar_quoted() {
        let lexed = crate::lex(
            "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $$ BEGIN PERFORM 1; END; $$",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "plpgsql");
        assert_eq!(body.body.trim(), "BEGIN PERFORM 1; END;");
    }

    #[test]
    fn func_body_single_quoted() {
        let lexed =
            crate::lex("CREATE FUNCTION f() RETURNS int AS 'SELECT 1' LANGUAGE sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "sql");
        assert_eq!(body.body, "SELECT 1");
    }

    #[test]
    fn func_body_tagged_dollar_quote() {
        let lexed = crate::lex(
            "CREATE FUNCTION f() RETURNS void LANGUAGE plpgsql AS $proc$ DECLARE x int; BEGIN x := 1; END; $proc$",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        let body = stmt.func_body().expect("should extract body");
        assert_eq!(body.lang, "plpgsql");
        assert_eq!(body.body.trim(), "DECLARE x int; BEGIN x := 1; END;");
    }

    #[test]
    fn func_returns_table() {
        let lexed = crate::lex(
            "CREATE FUNCTION f(int) RETURNS TABLE(a int, b int) AS $$ BEGIN RETURN QUERY SELECT 1, 2; END; $$ LANGUAGE plpgsql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn func_returns_table_varchar() {
        let lexed = crate::lex(
            "CREATE FUNCTION f() RETURNS TABLE(a varchar(5)) AS $$ SELECT 'hello'::varchar(5) $$ LANGUAGE sql",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// Postgres' `set_rest_more: ColId TO var_list | ColId '=' var_list`
    /// admits a comma-separated `var_list` after `TO` / `=`. The rules.sql
    /// regression exercises `SET datestyle to iso, mdy` as one option in
    /// a `createfunc_opt_list`.
    #[test]
    fn parse_create_function_set_var_list() {
        let lexed = crate::lex(
            "CREATE FUNCTION f() RETURNS integer AS 'select 1;' LANGUAGE SQL SET datestyle to iso, mdy",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// Multiple `SET` options on a single CREATE FUNCTION — each is its own
    /// `createfunc_opt_item`. The rules.sql regression chains five of them.
    #[test]
    fn parse_create_function_multiple_set_options() {
        let lexed = crate::lex(
            "CREATE FUNCTION f() RETURNS integer AS 'select 1;' LANGUAGE SQL \
             SET search_path TO PG_CATALOG \
             SET extra_float_digits TO 2 \
             SET work_mem TO '4MB' \
             SET datestyle to iso, mdy \
             SET local_preload_libraries TO ''",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateFunctionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }
    #[test]
    fn alter_function_rename_to() {
        let stmt: AlterFunctionStmt =
            parse_stmt("ALTER FUNCTION alt_func1(int) RENAME TO alt_func2");
        assert_eq!(stmt.target.name.object(), "alt_func1");
        assert!(matches!(stmt.action, AlterFuncAction::Rename(_)));
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION alt_func1(int) RENAME TO alt_func2");
    }

    #[test]
    fn alter_function_owner_to() {
        let stmt: AlterFunctionStmt =
            parse_stmt("ALTER FUNCTION alt_func2(int) OWNER TO regress_alter_generic_user2");
        assert!(matches!(stmt.action, AlterFuncAction::Owner(_)));
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION alt_func2(int) OWNER TO regress_alter_generic_user2",
        );
    }

    #[test]
    fn alter_function_set_schema() {
        let stmt: AlterFunctionStmt =
            parse_stmt("ALTER FUNCTION alt_func2(int) SET SCHEMA alt_nsp2");
        assert!(matches!(stmt.action, AlterFuncAction::SetSchema(_)));
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION alt_func2(int) SET SCHEMA alt_nsp2");
    }

    #[test]
    fn alter_function_depends_on_extension() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION f(int) DEPENDS ON EXTENSION my_extension",
        );
    }

    #[test]
    fn alter_function_no_depends_on_extension() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION f(int) NO DEPENDS ON EXTENSION my_extension",
        );
    }

    #[test]
    fn alter_function_immutable() {
        let stmt: AlterFunctionStmt = parse_stmt("ALTER FUNCTION functest_C_1(int) IMMUTABLE");
        assert!(matches!(stmt.action, AlterFuncAction::Options(_)));
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_C_1(int) IMMUTABLE");
    }

    #[test]
    fn alter_function_strict() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_F_2(int) STRICT");
    }

    #[test]
    fn alter_function_called_on_null_input() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION functest_F_3(int) CALLED ON NULL INPUT",
        );
    }

    #[test]
    fn alter_function_returns_null_on_null_input() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION non_strict(text) RETURNS NULL ON NULL INPUT",
        );
    }

    #[test]
    fn alter_function_security_invoker() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_C_2(int) SECURITY INVOKER");
    }

    #[test]
    fn alter_function_security_definer() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_C_3(int) SECURITY DEFINER");
    }

    #[test]
    fn alter_function_external_security_definer() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION f(int) EXTERNAL SECURITY DEFINER");
    }

    #[test]
    fn alter_function_leakproof() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_E_1(int) LEAKPROOF");
    }

    #[test]
    fn alter_function_not_leakproof() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_E_2(int) NOT LEAKPROOF");
    }

    #[test]
    fn alter_function_cost() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_B_3(int) COST 100");
    }

    #[test]
    fn alter_function_rows() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION f(int) ROWS 200");
    }

    #[test]
    fn alter_function_support() {
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION my_int_eq(int, int) SUPPORT test_support_func",
        );
    }

    #[test]
    fn alter_function_parallel_safe() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION f(int) PARALLEL SAFE");
    }

    #[test]
    fn alter_function_volatile() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION functest_B_2(int) VOLATILE");
    }

    #[test]
    fn alter_function_set_param() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION report_guc(text) SET work_mem = '2MB'");
    }

    #[test]
    fn alter_function_reset_all() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION report_guc(text) RESET ALL");
    }

    #[test]
    fn alter_function_multi_options_with_restrict() {
        // Multiple options space-separated, optional RESTRICT at end (opt_restrict
        // is the deprecated trailing modifier).
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION f(int) STRICT IMMUTABLE LEAKPROOF RESTRICT",
        );
    }

    #[test]
    fn alter_function_qualified_name() {
        reparse_stable::<AlterFunctionStmt>("ALTER FUNCTION alter1.plus1(int) SET SCHEMA alter2");
    }

    #[test]
    fn alter_function_no_argtypes() {
        // function_with_argtypes admits bare name (no parens) per gram.y.
        reparse_stable::<AlterFunctionStmt>(
            "ALTER FUNCTION terminate_nothrow OWNER TO pg_signal_backend",
        );
    }

    #[test]
    fn alter_routine_rename_no_argtypes() {
        // ALTER ROUTINE accepts bare name (no parens).
        reparse_stable::<AlterRoutineStmt>("ALTER ROUTINE cp_testfunc1a RENAME TO cp_testfunc1");
    }

    #[test]
    fn alter_routine_rename_with_argtypes() {
        reparse_stable::<AlterRoutineStmt>(
            "ALTER ROUTINE cp_testfunc1(int) RENAME TO cp_testfunc1a",
        );
    }
}
