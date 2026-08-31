#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_operator_custom_op() {
        let lexed = crate::lex(
            "CREATE OPERATOR @-@ ( leftarg = int4, rightarg = int4, procedure = int4mi )",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_operator_custom_op() {
        let lexed = crate::lex("DROP OPERATOR ===(bigint, bigint)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_custom_op() {
        let lexed = crate::lex(
            "ALTER OPERATOR @+@(int4, int4) OWNER TO regress_alter_generic_user2",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_operator_commutator_custom_op() {
        // From `create_operator.sql`: `COMMUTATOR = ===` on a CREATE OPERATOR
        // body. The RHS of the `=` is itself a custom operator.
        let lexed = crate::lex(
            "CREATE OPERATOR === (\
                 LEFTARG = boolean,\
                 RIGHTARG = boolean,\
                 PROCEDURE = fn_op2,\
                 COMMUTATOR = ===,\
                 NEGATOR = !==\
             )",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_set_commutator_custom_op() {
        // From `alter_operator.sql`: `SET (COMMUTATOR = ====)` — the RHS is
        // a 4-char custom op (`====`) which lexes as `CustomOp`.
        let lexed =
            crate::lex("ALTER OPERATOR === (boolean, real) SET (COMMUTATOR = ====)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_set_negator_at_eq() {
        // `SET (COMMUTATOR = @=)` — `@=` is a 2-char custom op starting with `@`.
        let lexed = crate::lex("ALTER OPERATOR === (boolean, real) SET (COMMUTATOR = @=)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_operator_single_char() {
        let lexed = crate::lex(
            "CREATE OPERATOR = (procedure = int8alias1eq, leftarg = int8alias1, rightarg = int8alias1)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_operator_single_char() {
        let lexed = crate::lex("DROP OPERATOR <|(bigint, bigint)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropOperatorStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_operator_family_basic() {
        let lexed = crate::lex("CREATE OPERATOR FAMILY my_family USING hash");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert_eq!(stmt.name.object(), "my_family");
        assert_eq!(stmt.access_method.text(), "hash");
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_operator_class_storage_only() {
        let lexed = crate::lex(
            "CREATE OPERATOR CLASS alt_opc1 FOR TYPE uuid USING hash AS STORAGE uuid",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateOperatorClassStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert_eq!(stmt.name.object(), "alt_opc1");
        assert!(!stmt.default);
        assert!(stmt.family.is_none());
        assert_eq!(stmt.items.iter().count(), 1);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_operator_class_default_with_family() {
        let lexed = crate::lex(
            "CREATE OPERATOR CLASS my_ops DEFAULT FOR TYPE int4 USING btree \
             FAMILY my_fam AS OPERATOR 1 < , OPERATOR 3 = , FUNCTION 1 my_cmp(int4, int4)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateOperatorClassStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(stmt.default);
        assert!(stmt.family.is_some());
        assert_eq!(stmt.items.iter().count(), 3);
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_family_add() {
        let lexed = crate::lex(
            "ALTER OPERATOR FAMILY alt_opf17 USING btree ADD \
             OPERATOR 1 < (int4, int4), FUNCTION 1 btint4cmp(int4, int4)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_family_drop() {
        let lexed = crate::lex(
            "ALTER OPERATOR FAMILY alt_opf11 USING gist DROP OPERATOR 1 (int4, int4)",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_family_rename() {
        let lexed =
            crate::lex("ALTER OPERATOR FAMILY alt_opf1 USING hash RENAME TO alt_opf3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_family_owner() {
        let lexed = crate::lex(
            "ALTER OPERATOR FAMILY alt_opf1 USING hash OWNER TO regress_alter_generic_user1",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_family_set_schema() {
        let lexed =
            crate::lex("ALTER OPERATOR FAMILY alt_opf2 USING hash SET SCHEMA alt_nsp2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_operator_family_add_order_by() {
        // `OPERATOR n any_op FOR ORDER BY family_name` — the ordering-operator
        // arm of opclass_purpose.
        let lexed = crate::lex(
            "ALTER OPERATOR FAMILY alt_opf10 USING btree ADD \
             OPERATOR 1 < (int4, int4) FOR ORDER BY some_family",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_operator_class_basic() {
        let lexed = crate::lex("DROP OPERATOR CLASS my_ops USING btree CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropOperatorClassStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "my_ops");
        assert_eq!(stmt.access_method.text(), "btree");
        assert!(stmt.if_exists.is_none());
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_operator_family_if_exists() {
        let lexed =
            crate::lex("DROP OPERATOR FAMILY IF EXISTS my_family USING hash RESTRICT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropOperatorFamilyStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert!(stmt.if_exists.is_some());
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_operator_class_recheck_modifier() {
        // Legacy `RECHECK` modifier on opclass operator items — PG accepts it
        // for old-dump portability (no-op since 8.4) and we round-trip it.
        let lexed = crate::lex(
            "CREATE OPERATOR CLASS legacy_ops FOR TYPE int4 USING gist AS \
             OPERATOR 1 < RECHECK, STORAGE int4",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateOperatorClassStmt::parse(&mut input)
            .unwrap()
            .into_ast();
        assert_eq!(stmt.items.iter().count(), 2);
        assert!(input.is_eof());
    }
}
