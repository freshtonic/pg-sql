#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    /// `CREATE AGGREGATE name(args) ( ... SFUNC = balkifnull(int8, int4) ... )`
    /// — the aggregates.sql corpus exercises a `def_arg` whose value is a
    /// function-style name with type-name arguments. PG's `def_arg` accepts
    /// this via `func_type → Typename → GenericType → name opt_type_modifiers`
    /// (`opt_type_modifiers` is `'(' expr_list ')'`), but pg-sql's
    /// `TypePrecision` is restricted to signed-integer modifiers, so the
    /// function-style form needs a dedicated `DefArg::FuncWithArgs` variant.
    #[test]
    fn parse_create_aggregate_funcname_def_arg() {
        for src in [
            "CREATE AGGREGATE balk(int4) ( SFUNC = balkifnull(int8, int4), STYPE = int8 )",
            "CREATE AGGREGATE balk(int4) ( SFUNC = balkifnull(int8, int4), STYPE = int8, PARALLEL = SAFE, INITCOND = '0' )",
            "CREATE AGGREGATE balk(int4) ( SFUNC = int4_sum(int8, int4), STYPE = int8, COMBINEFUNC = balkifnull(int8, int8), PARALLEL = SAFE, INITCOND = '0' )",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn parse_drop_aggregate_typed() {
        let lexed = crate::lex("DROP AGGREGATE myavg(numeric)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.targets.len(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_aggregate_star() {
        let lexed = crate::lex("DROP AGGREGATE IF EXISTS test_agg(*)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(matches!(
            stmt.targets.iter().next().unwrap().args,
            AggregateArgs::Star
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_modern() {
        let lexed = crate::lex(
            "CREATE AGGREGATE sumdouble (float8) (sfunc = float8pl, stype = float8)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.signature.definition.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_old_style() {
        let lexed = crate::lex(
            "CREATE AGGREGATE newavg (sfunc = int4_avg_accum, basetype = int4, stype = _int8)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.signature.definition.is_none());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_zero_args() {
        let lexed =
            crate::lex("CREATE AGGREGATE newcnt (*) (sfunc = int8inc, stype = int8)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.signature.definition.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_ordered_set() {
        let lexed = crate::lex(
            "CREATE AGGREGATE my_percentile_disc(float8 ORDER BY anyelement) \
             (stype = internal, sfunc = ordered_set_transition)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.signature.definition.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_aggregate_or_replace() {
        let lexed = crate::lex(
            "CREATE OR REPLACE AGGREGATE myavg (numeric) (stype = numeric, sfunc = numeric_add)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateAggregateStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.or_replace);
        assert!(input.is_eof());
    }

    #[test]
    fn alter_aggregate_rename() {
        let stmt: AlterAggregateStmt =
            parse_stmt("ALTER AGGREGATE alt_agg1(int) RENAME TO alt_agg2");
        assert_eq!(stmt.name.object(), "alt_agg1");
        assert!(matches!(stmt.action, AlterAggregateAction::Rename(_)));
        reparse_stable::<AlterAggregateStmt>("ALTER AGGREGATE alt_agg1(int) RENAME TO alt_agg2");
    }

    #[test]
    fn alter_aggregate_owner() {
        reparse_stable::<AlterAggregateStmt>(
            "ALTER AGGREGATE alt_agg2(int) OWNER TO regress_alter_generic_user3",
        );
    }

    #[test]
    fn alter_aggregate_set_schema() {
        reparse_stable::<AlterAggregateStmt>("ALTER AGGREGATE alt_agg2(int) SET SCHEMA alt_nsp2");
    }

    #[test]
    fn alter_aggregate_star_args() {
        reparse_stable::<AlterAggregateStmt>("ALTER AGGREGATE my_count(*) RENAME TO new_count");
    }

    #[test]
    fn alter_aggregate_order_by_args() {
        // Ordered-set aggregate signature with `ORDER BY`.
        reparse_stable::<AlterAggregateStmt>(
            "ALTER AGGREGATE my_percentile_disc(float8 ORDER BY anyelement) RENAME TO test_percentile_disc",
        );
    }

    #[test]
    fn alter_aggregate_variadic_order_by() {
        reparse_stable::<AlterAggregateStmt>(
            "ALTER AGGREGATE my_rank(VARIADIC \"any\" ORDER BY VARIADIC \"any\") RENAME TO test_rank",
        );
    }
}
