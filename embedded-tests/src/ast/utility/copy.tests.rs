#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn copy_table_from_stdin_bare() {
        let stmt: CopyStmt = parse_stmt("COPY t FROM STDIN");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(matches!(table.direction, CopyDirection::From));
        assert!(matches!(table.target, CopyTarget::Stdin));
        assert!(table.columns.is_none());
        assert!(!table.program);
        assert!(table.delimiter.is_none());
        assert!(table.options.is_none());
        assert!(table.where_clause.is_none());
        reparse_stable::<CopyStmt>("COPY t FROM STDIN");
    }

    #[test]
    fn copy_table_to_stdout() {
        let stmt: CopyStmt = parse_stmt("COPY t TO STDOUT");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(matches!(table.direction, CopyDirection::To));
        assert!(matches!(table.target, CopyTarget::Stdout));
        reparse_stable::<CopyStmt>("COPY t TO STDOUT");
    }

    #[test]
    fn copy_table_with_columns_from_stdin() {
        let stmt: CopyStmt = parse_stmt("COPY t (a, b, c) FROM STDIN");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(table.columns.is_some());
        reparse_stable::<CopyStmt>("COPY t (a, b, c) FROM STDIN");
    }

    #[test]
    fn copy_table_to_file() {
        reparse_stable::<CopyStmt>("COPY t TO 'foo.csv'");
    }

    #[test]
    fn copy_table_from_file() {
        let stmt: CopyStmt = parse_stmt("COPY t FROM 'foo.csv'");
        let CopyBody::Table(table) = &stmt.body else {
            panic!("expected table body");
        };
        assert!(matches!(table.target, CopyTarget::File(_)));
    }

    #[test]
    fn copy_table_to_stdout_csv_legacy() {
        // Legacy `csv` option without WITH or parens.
        reparse_stable::<CopyStmt>("COPY t TO STDOUT CSV");
    }

    #[test]
    fn copy_table_from_stdin_csv_header_legacy() {
        // Two consecutive legacy options.
        reparse_stable::<CopyStmt>("COPY t FROM STDIN CSV HEADER");
    }

    #[test]
    fn copy_table_to_file_csv_quote_escape_legacy() {
        // Legacy options that carry string arguments.
        reparse_stable::<CopyStmt>("COPY t TO 'f.csv' CSV QUOTE '|' ESCAPE '\\'");
    }

    #[test]
    fn copy_table_with_legacy_delimiter_null_as() {
        // `WITH` followed by legacy form.
        reparse_stable::<CopyStmt>("COPY x FROM STDIN WITH DELIMITER AS ';' NULL AS ''");
    }

    #[test]
    fn copy_table_using_delimiters_legacy() {
        // `USING DELIMITERS 'c'` precedes the option list.
        reparse_stable::<CopyStmt>("COPY t FROM 'f' USING DELIMITERS '|'");
    }

    #[test]
    fn copy_table_binary_legacy() {
        // `COPY BINARY t TO file` legacy binary option — `CopyBody::BinaryTable`.
        let stmt: CopyStmt = parse_stmt("COPY BINARY t TO 'f'");
        let CopyBody::BinaryTable(body) = &stmt.body else {
            panic!("expected binary-table body");
        };
        assert!(matches!(body.inner.direction, CopyDirection::To));
        reparse_stable::<CopyStmt>("COPY BINARY t TO 'f'");
    }

    #[test]
    fn copy_table_from_program() {
        reparse_stable::<CopyStmt>("COPY t FROM PROGRAM 'cat foo.csv'");
    }

    #[test]
    fn copy_table_generic_options_single() {
        // `(freeze)` — a single generic option with no value.
        reparse_stable::<CopyStmt>("COPY t FROM 'f' (FREEZE)");
    }

    #[test]
    fn copy_table_with_generic_options() {
        // `WITH (header match, format csv)` — generic options after WITH.
        reparse_stable::<CopyStmt>("COPY t FROM STDIN WITH (HEADER MATCH, FORMAT CSV)");
    }

    #[test]
    fn copy_table_generic_option_star() {
        reparse_stable::<CopyStmt>("COPY t FROM STDIN (FORCE_QUOTE *)");
    }

    #[test]
    fn copy_table_generic_option_paren_list() {
        reparse_stable::<CopyStmt>("COPY t FROM STDIN (FORCE_QUOTE (a, b))");
    }

    #[test]
    fn copy_table_from_where_expr() {
        // `WHERE expr` after the option list (FROM-only).
        reparse_stable::<CopyStmt>("COPY x FROM STDIN WHERE a = 1");
    }

    #[test]
    fn copy_query_to_stdout() {
        let stmt: CopyStmt = parse_stmt("COPY (SELECT * FROM t) TO STDOUT");
        assert!(matches!(stmt.body, CopyBody::Query(_)));
        reparse_stable::<CopyStmt>("COPY (SELECT * FROM t) TO STDOUT");
    }

    #[test]
    fn copy_query_to_file() {
        reparse_stable::<CopyStmt>("COPY (SELECT 1) TO 'f'");
    }

    #[test]
    fn copy_query_with_generic_options() {
        reparse_stable::<CopyStmt>("COPY (SELECT 1) TO STDOUT WITH (DEFAULT '\\D')");
    }

    #[test]
    fn copy_query_insert_returning() {
        // Query body must accept INSERT ... RETURNING (PreparableStmt).
        reparse_stable::<CopyStmt>("COPY (INSERT INTO t (a) VALUES (1) RETURNING id) TO STDOUT");
    }

    #[test]
    fn copy_table_with_encoding_legacy() {
        reparse_stable::<CopyStmt>("COPY t FROM STDIN WITH ENCODING 'sql_ascii'");
    }

    #[test]
    fn copy_table_legacy_force_quote_star() {
        // Three-keyword legacy item.
        reparse_stable::<CopyStmt>("COPY t TO 'f' CSV FORCE QUOTE *");
    }

    #[test]
    fn copy_table_legacy_force_not_null_list() {
        reparse_stable::<CopyStmt>("COPY t FROM 'f' CSV FORCE NOT NULL a, b");
    }

    #[test]
    fn copy_table_legacy_with_null_as() {
        // Corpus regression case: `WITH ... NULL AS '...'` chained options.
        reparse_stable::<CopyStmt>("COPY t TO STDOUT WITH NULL AS E'\\0'");
    }

    #[test]
    fn copy_table_legacy_chained_delimiter_null_encoding() {
        // Three chained legacy options after WITH.
        reparse_stable::<CopyStmt>(
            "COPY x FROM STDIN WITH DELIMITER AS ':' NULL AS E'\\X' ENCODING 'sql_ascii'",
        );
    }

    #[test]
    fn copy_table_psql_var_target() {
        reparse_stable::<CopyStmt>("COPY t TO :'filename' CSV");
    }
}
