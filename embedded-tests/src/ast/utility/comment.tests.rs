#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_comment_on_operator_custom_op() {
        // Regression: `COMMENT ON OPERATOR === (a, b)` shares the
        // `operator_with_argtypes` grammar with DROP/ALTER OPERATOR and must
        // accept non-standard operator names.
        let stmt: CommentStmt =
            parse_stmt("COMMENT ON OPERATOR === (int4, int4) IS 'custom equality'");
        assert!(matches!(stmt.object, CommentObject::Operator(_)));
    }

    #[test]
    fn comment_on_table_is_modelled() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON TABLE attmp IS 'table comment'");
        assert!(matches!(stmt.text, CommentText::Text(_)));
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TABLE attmp IS 'table comment'"),
            "COMMENT ON TABLE attmp IS 'table comment'"
        );
    }

    #[test]
    fn comment_on_table_null_is_modelled() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON TABLE attmp IS NULL");
        assert!(matches!(stmt.text, CommentText::Null));
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TABLE attmp IS NULL"),
            "COMMENT ON TABLE attmp IS NULL"
        );
    }

    #[test]
    fn comment_on_column_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON COLUMN ctlt1.a IS 'A'");
        assert!(matches!(stmt.object, CommentObject::Column(_)));
        reparse_stable::<CommentStmt>("COMMENT ON COLUMN ctlt1.a IS 'A'");
    }

    #[test]
    fn comment_on_materialized_view_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON MATERIALIZED VIEW mv IS 'm'"),
            "COMMENT ON MATERIALIZED VIEW mv IS 'm'"
        );
    }

    #[test]
    fn comment_on_text_search_parser_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TEXT SEARCH PARSER p IS 'x'"),
            "COMMENT ON TEXT SEARCH PARSER p IS 'x'"
        );
    }

    #[test]
    fn comment_on_database_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON DATABASE db IS 'x'"),
            "COMMENT ON DATABASE db IS 'x'"
        );
    }

    #[test]
    fn comment_on_access_method_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON ACCESS METHOD am IS 'x'"),
            "COMMENT ON ACCESS METHOD am IS 'x'"
        );
    }

    #[test]
    fn comment_on_constraint_on_table_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON CONSTRAINT c ON t IS 'x'"),
            "COMMENT ON CONSTRAINT c ON t IS 'x'"
        );
    }

    #[test]
    fn comment_on_constraint_on_domain_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON CONSTRAINT c ON DOMAIN d IS 'x'"),
            "COMMENT ON CONSTRAINT c ON DOMAIN d IS 'x'"
        );
    }

    #[test]
    fn comment_on_trigger_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TRIGGER tg ON t IS 'x'"),
            "COMMENT ON TRIGGER tg ON t IS 'x'"
        );
    }

    #[test]
    fn comment_on_type_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TYPE default_test_row IS 'x'"),
            "COMMENT ON TYPE default_test_row IS 'x'"
        );
    }

    #[test]
    fn comment_on_function_with_args_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON FUNCTION f(int, text) IS 'x'");
        assert!(matches!(stmt.object, CommentObject::Function(_)));
        reparse_stable::<CommentStmt>("COMMENT ON FUNCTION f(int, text) IS 'x'");
    }

    #[test]
    fn comment_on_aggregate_star_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON AGGREGATE newcnt(*) IS 'x'");
        assert!(matches!(stmt.object, CommentObject::Aggregate(_)));
        reparse_stable::<CommentStmt>("COMMENT ON AGGREGATE newcnt(*) IS 'x'");
    }

    #[test]
    fn comment_on_aggregate_types_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON AGGREGATE newavg(int4) IS 'x'");
        assert!(matches!(stmt.object, CommentObject::Aggregate(_)));
        reparse_stable::<CommentStmt>("COMMENT ON AGGREGATE newavg(int4) IS 'x'");
    }

    #[test]
    fn security_label_on_table_is_modelled() {
        let stmt: SecurityLabelStmt = parse_stmt("SECURITY LABEL ON TABLE t IS 'classified'");
        assert!(stmt.provider.is_none());
        assert_eq!(
            roundtrip::<SecurityLabelStmt>("SECURITY LABEL ON TABLE t IS 'classified'"),
            "SECURITY LABEL ON TABLE t IS 'classified'"
        );
    }

    #[test]
    fn security_label_with_provider_keeps_provider() {
        let stmt: SecurityLabelStmt =
            parse_stmt("SECURITY LABEL FOR 'dummy' ON TABLE t IS 'classified'");
        assert!(stmt.provider.is_some());
        assert_eq!(
            roundtrip::<SecurityLabelStmt>("SECURITY LABEL FOR 'dummy' ON TABLE t IS 'classified'"),
            "SECURITY LABEL FOR 'dummy' ON TABLE t IS 'classified'"
        );
    }

    #[test]
    fn security_label_on_role_null_roundtrips() {
        assert_eq!(
            roundtrip::<SecurityLabelStmt>("SECURITY LABEL ON ROLE r IS NULL"),
            "SECURITY LABEL ON ROLE r IS NULL"
        );
    }
}
