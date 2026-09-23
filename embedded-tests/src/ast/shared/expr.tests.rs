#[cfg(test)]
mod tests {
    use crate::ast::shared::expr::{
        CastType, CastTypeHead, ColumnRef, Expr,
        FunctionCallBody,
        FunctionCallTail, ParenContent, ParenthesizedDotStar, ParenthesizedExpr,
        ParenthesizedIndirection, StringLitSeq0, TypeName,
    };
    // Added in 16: research, PostgreSQL 16, "Queries and expressions".
    #[cfg(feature = "since-pg16")]
    use crate::ast::shared::expr::JsonObject;
    use crate::ast::dml::values::{SelectClause, Subquery};

    /// Parse `src` as an `Expr` through the logos lex pass.
    ///
    /// Takes `&'static str` because the returned `Expr` borrows lexical text
    /// from the source for that `'static` lifetime.
    type ExprFamily = <Expr<'static> as recursa::ArenaParse<crate::Input<'static>>>::Family;
    type TypeNameFamily =
        <TypeName<'static> as recursa::ArenaParse<crate::Input<'static>>>::Family;
    type CastTypeFamily =
        <CastType<'static> as recursa::ArenaParse<crate::Input<'static>>>::Family;

    fn parse_expr_classified(src: &'static str) -> recursa::ArenaParsed<'static, ExprFamily> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse {src:?}: {error:?}"))
            ;
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        expr_parsed
    }

    fn parse_type_name_classified(
        src: &'static str,
    ) -> recursa::ArenaParsed<'static, TypeNameFamily> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
        let mut input = lexed.input();
        let ty_parsed = TypeName::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse type name {src:?}: {error}"))
            ;
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        ty_parsed
    }

    fn parse_cast_type_classified(
        src: &'static str,
    ) -> recursa::ArenaParsed<'static, CastTypeFamily> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
        let mut input = lexed.input();
        let ty_parsed = CastType::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse cast type {src:?}: {error}"))
            ;
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        ty_parsed
    }

    // Added in 17: research, PostgreSQL 17, "Queries and expressions".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_json_timestamp_cast_before_unique_keys() {
        assert!(matches!(
            parse_expr_classified("JSON('2000-01-01'::timestamp WITH UNIQUE KEYS)").ast(),
            Expr::JsonCtor(_)
        ));
    }

    // `JSON()`, `JSON_SCALAR()` and `JSON_SERIALIZE()` are added in 17:
    // research, PostgreSQL 17, "Queries and expressions".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_json_value_constructors() {
        // JSON()
        assert!(matches!(
            parse_expr_classified("JSON('{}' FORMAT JSON)").ast(),
            Expr::JsonCtor(_)
        ));
        assert!(matches!(
            parse_expr_classified("JSON('1'::json WITH UNIQUE KEYS)").ast(),
            Expr::JsonCtor(_)
        ));
        // JSON_SCALAR()
        assert!(matches!(
            parse_expr_classified("JSON_SCALAR('123')").ast(),
            Expr::JsonScalar(_)
        ));
        // JSON_SERIALIZE()
        assert!(matches!(
            parse_expr_classified("JSON_SERIALIZE('{}' RETURNING bytea)").ast(),
            Expr::JsonSerialize(_)
        ));
    }

    // Added in 16: research, PostgreSQL 16, "Queries and expressions".
    #[cfg(feature = "since-pg16")]
    #[test]
    fn parse_json_constructors() {
        // JSON_OBJECT() — entries, KEY/VALUE, all clauses, empty, returning-only
        for src in [
            "JSON_OBJECT('a': 1, 'b': 2)",
            "JSON_OBJECT(KEY 'a' VALUE 2 + 3)",
            "JSON_OBJECT('a': 1 ABSENT ON NULL WITH UNIQUE RETURNING jsonb)",
            "JSON_OBJECT()",
            "JSON_OBJECT(RETURNING jsonb)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonObject(_)),
                "{src}"
            );
        }
        // JSON_ARRAY() — element list, query form, empty, returning-only
        for src in [
            "JSON_ARRAY(1, 2, 3)",
            "JSON_ARRAY('a', NULL ABSENT ON NULL RETURNING jsonb)",
            "JSON_ARRAY(SELECT i FROM t)",
            // These share an arbitrarily nested `(` prefix and are selected
            // by the token after the matching inner close.
            "JSON_ARRAY((SELECT 1))",
            "JSON_ARRAY((SELECT 1) UNION SELECT 2)",
            "JSON_ARRAY()",
            "JSON_ARRAY(RETURNING jsonb)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonArray(_)),
                "{src}"
            );
        }
    }

    /// A legacy lowercase `json_object(...)`-style call with plain
    /// comma-separated arguments is NOT the SQL/JSON construct — it must
    /// fall through to an ordinary function call via soft-keyword
    /// identifier reclamation.
    #[test]
    fn legacy_json_object_call_is_ordinary_func() {
        assert!(matches!(
            parse_expr_classified("json_build_array(1, 2)").ast(),
            Expr::Func(_)
        ));
    }

    // Added in 17: research, PostgreSQL 17, "Queries and expressions".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_json_query_functions() {
        // JSON_EXISTS — path, PASSING, ON ERROR.
        for src in [
            "JSON_EXISTS(jsonb '1', '$.a')",
            "JSON_EXISTS(js, '$.a' ERROR ON ERROR)",
            "JSON_EXISTS(js, '$ ? (@ > $x)' PASSING 1 AS x, 2 AS y)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonExists(_)),
                "{src}"
            );
        }
        // JSON_VALUE — RETURNING, DEFAULT behavior, ON EMPTY/ERROR.
        for src in [
            "JSON_VALUE(js, '$')",
            "JSON_VALUE(jsonb '123', '$' RETURNING int)",
            "JSON_VALUE(js, '$' RETURNING char(5) DEFAULT '0' ON ERROR)",
            "JSON_VALUE(js, '$' ERROR ON EMPTY NULL ON ERROR)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonValue(_)),
                "{src}"
            );
        }
        // JSON_QUERY — wrapper, quotes, behaviors.
        for src in [
            "JSON_QUERY(js, '$')",
            "JSON_QUERY(js, '$' WITH UNCONDITIONAL ARRAY WRAPPER)",
            "JSON_QUERY(js, '$' WITHOUT WRAPPER)",
            "JSON_QUERY(js, '$' OMIT QUOTES EMPTY ARRAY ON EMPTY)",
            "JSON_QUERY(js, '$' KEEP QUOTES ON SCALAR STRING ERROR ON ERROR)",
            "JSON_QUERY(js, '$' RETURNING bytea FORMAT JSON EMPTY OBJECT ON ERROR)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonQuery(_)),
                "{src}"
            );
        }
        // The result is an ordinary expression operand.
        assert!(matches!(
            parse_expr_classified("JSON_VALUE(js, '$' RETURNING int) + 234").ast(),
            Expr::Add(..)
        ));
    }

    // Added in 16: research, PostgreSQL 16, "Queries and expressions".
    #[cfg(feature = "since-pg16")]
    #[test]
    fn parse_json_aggregates() {
        for src in [
            "JSON_OBJECTAGG('b': 1 RETURNING text)",
            "JSON_OBJECTAGG(k VALUE v ABSENT ON NULL WITH UNIQUE)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonObjectAgg(_)),
                "{src}"
            );
        }
        for src in [
            "JSON_ARRAYAGG(i)",
            "JSON_ARRAYAGG(i ORDER BY i DESC RETURNING jsonb)",
            "JSON_ARRAYAGG(bar) FILTER (WHERE bar > 2) OVER (PARTITION BY x)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonArrayAgg(_)),
                "{src}"
            );
        }
    }

    #[test]
    fn parse_multidim_array_literal() {
        for src in [
            "ARRAY[1, 2, 3]",
            "ARRAY[[1,2],[3,4]]",
            "ARRAY[[[1],[2]],[[3],[4]]]",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::Array(_)),
                "{src}"
            );
        }
    }

    #[test]
    fn parse_overlaps() {
        assert!(matches!(
            parse_expr_classified(
                "(timestamp '2000-11-27', interval '12 hours') \
                 OVERLAPS (timestamp '2000-11-27', interval '12 hours')"
            ).ast(),
            Expr::Overlaps(..)
        ));
    }

    #[test]
    fn parse_xml_functions() {
        assert!(matches!(
            parse_expr_classified("xmlserialize(CONTENT x AS text)").ast(),
            Expr::XmlSerialize(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlparse(DOCUMENT '<foo/>')").ast(),
            Expr::XmlParse(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlroot(x, VERSION NO VALUE, STANDALONE YES)").ast(),
            Expr::XmlRoot(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlexists('/a' PASSING BY REF doc BY REF)").ast(),
            Expr::XmlExists(_)
        ));
        assert!(matches!(
            parse_expr_classified("x IS DOCUMENT").ast(),
            Expr::IsDocument(..)
        ));
        assert!(matches!(
            parse_expr_classified("x IS NOT DOCUMENT").ast(),
            Expr::IsDocument(..)
        ));
    }

    // Added in 16: research, PostgreSQL 16, "Queries and expressions".
    #[cfg(feature = "since-pg16")]
    #[test]
    fn parse_is_json_predicate() {
        for src in [
            "js IS JSON",
            "js IS NOT JSON",
            "js IS JSON ARRAY",
            "js IS JSON OBJECT WITH UNIQUE KEYS",
            "js IS JSON SCALAR",
            "js IS JSON VALUE WITHOUT UNIQUE",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::IsJson(..)),
                "{src}"
            );
        }
        // `IS NULL` still resolves to the boolean test, not `IS JSON`.
        assert!(matches!(
            parse_expr_classified("js IS NULL").ast(),
            Expr::BoolTest(..)
        ));
    }

    // --- Atom tests ---

    #[test]
    fn parse_integer_literal() {
        let lexed = crate::lex("42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntegerLit(_)));
        assert!(input.is_eof());
    }

    /// Regression: the Pratt-enum kind-match `peek` (emitted when a
    /// classifier is installed) must not answer `false` for atoms that are
    /// not covered by cached token kinds — identifier column-refs and
    /// `FuncCall` reach the parser only through the sequential fallback.
    /// A wrongly-`false` peek made `Seq1<SelectItem, Comma>` skip every
    /// identifier-led SELECT list, dropping fixture coverage to ~52%.
    #[test]
    fn pratt_peek_classified_covers_identifier_atoms() {
        for src in ["a", "abc", "foo(1)", "count(*)"] {
            let plain_lexed = crate::lex(src);
            assert_eq!(plain_lexed.errors().count(), 0, "lex errors in plain");
            let mut plain = plain_lexed.input();
            assert!(
                Expr::parse(&mut plain).is_ok(),
                "Expr::parse (no classifier) should accept {src:?}"
            );
            let classified_lexed = crate::lex(src);
            assert_eq!(
                classified_lexed.errors().count(),
                0,
                "lex errors in classified"
            );
            let mut classified = classified_lexed.input();
            assert!(
                Expr::parse(&mut classified).is_ok(),
                "Expr::parse (classified) should accept {src:?}"
            );
        }
    }

    /// `^@` (text starts-with) is a single PostgreSQL operator token. With the
    /// classifier active it must NOT split into `Caret` + `At`.
    #[test]
    fn parse_starts_with_operator_classified() {
        assert!(matches!(
            parse_expr_classified("a ^@ b").ast(),
            Expr::StartsWith(..)
        ));
    }

    /// `#-` (jsonb delete-path) is a single PostgreSQL operator token. With the
    /// classifier active it must NOT split into `Pound` + `Minus`.
    #[test]
    fn parse_json_delete_path_operator_classified() {
        assert!(matches!(
            parse_expr_classified("a #- b").ast(),
            Expr::JsonDeletePath(..)
        ));
    }

    #[test]
    fn parse_dollar_string_literal_expr() {
        // Regression: json.sql uses `$$'foo'$$::json` and similar.
        let lexed = crate::lex("$$''$$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::DollarStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_string_literal() {
        let lexed = crate::lex("'hello'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::StringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn reject_same_line_adjacent_string_literals() {
        let lexed = crate::lex("'a' 'b'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Expr::parse(&mut input).is_err() || !input.is_eof());
    }

    /// PostgreSQL's scanner permits only spaces and line comments in a
    /// string-continuation gap. A block comment therefore prevents it even
    /// when another newline follows (ADR 0004).
    #[test]
    fn reject_string_continuation_across_block_comment() {
        let lexed = crate::lex("'first line'\n/* comment */\n' - next line'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Expr::parse(&mut input).is_err() || !input.is_eof());
    }

    /// A legitimate newline-separated 3-part string continuation (no comment)
    /// must still concatenate under the classifier — the regression guard for
    /// `strings.sql`'s "Three lines to one" fixture.
    #[test]
    fn parse_three_part_string_continuation_classified() {
        let expr_parsed = parse_expr_classified("'first line'\n' - next line'\n\t' - third line'");
        let expr = expr_parsed.ast();
        if let Expr::StringLit(seq) = &expr {
            assert!(matches!(seq, StringLitSeq0::Sequence(_)));
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
    }

    /// A legitimate newline-separated string continuation (no comment) must
    /// still concatenate — the regression guard for `reject_…_across_comment`.
    #[test]
    fn parse_string_continuation_across_newline() {
        let lexed = crate::lex("'first line'\n' - next line'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        if let Expr::StringLit(seq) = &expr {
            assert!(matches!(seq, StringLitSeq0::Sequence(_)));
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_three_part_string_concat() {
        // 3-part adjacent string literal concatenation. Postgres concatenates
        // these into a single value at parse time.
        let lexed = crate::lex("'first'\n'second'\n'third'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        if let Expr::StringLit(seq) = &expr {
            assert!(matches!(seq, StringLitSeq0::Sequence(_)));
        } else {
            panic!("expected StringLit, got {:?}", expr);
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_four_part_string_concat() {
        let lexed = crate::lex("'a'\n'b'\n'c'\n'd'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        if let Expr::StringLit(seq) = &expr {
            assert!(matches!(seq, StringLitSeq0::Sequence(_)));
        } else {
            panic!("expected StringLit");
        }
    }

    #[test]
    fn parse_three_adjacent_strings_with_quoted_alias() {
        use crate::ast::dml::select::SelectStmt;
        let lexed = crate::lex(
            "SELECT 'first line'\n' - next line'\n' - third line' AS \"Three lines to one\"",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_three_adjacent_strings_with_alias() {
        // SELECT 'first line' newline ' - next line' AS foo
        use crate::ast::dml::select::SelectStmt;
        let lexed = crate::lex("SELECT 'first line'\n' - next line' AS foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
        let _stmt = _stmt_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlelement_simple() {
        let lexed = crate::lex("xmlelement(name foo, 'content')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlelement_with_attributes() {
        let lexed = crate::lex("xmlelement(name foo, xmlattributes(1 as a, 2 as b), 'content')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlpi_basic() {
        let lexed = crate::lex("xmlpi(name foo)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlpi_with_content() {
        let lexed = crate::lex("xmlpi(name foo, 'bar')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unicode_string_lit_basic() {
        let lexed = crate::lex(r"U&'d\0061t\+000061'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unicode_string_lit_uescape() {
        let lexed = crate::lex(r"U&'d!0061t\+000061' UESCAPE '!'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_func_with_precision() {
        for (src, identifier_led) in [
            ("char(20) 'characters'", true),
            ("numeric(10, 2) '1234.50'", false),
            ("varchar(8) 'postgres'", false),
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|error| panic!("parse {src:?}: {error}"))
                ;
            let expr = expr_parsed.ast();
            match expr {
                Expr::Func(call) if identifier_led => assert!(
                    matches!(&call.tail, FunctionCallTail::TypedLiteral(_)),
                    "missing typed-literal state for {src:?}",
                ),
                Expr::CastFunc(_) if !identifier_led => {}
                _ => panic!("expected a typed-literal node for {src:?}"),
            }
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn type_name_preserves_legacy_fixed_variants_and_json_ident() {
        assert!(matches!(parse_type_name_classified("bool").ast(), TypeName::Bool));
        assert!(matches!(parse_type_name_classified("text").ast(), TypeName::Text));
        assert!(matches!(
            parse_type_name_classified("serial").ast(),
            TypeName::Serial
        ));
        assert!(matches!(
            parse_type_name_classified("double precision").ast(),
            TypeName::DoublePrecision
        ));
        assert!(matches!(
            parse_type_name_classified("unknown").ast(),
            TypeName::Unknown
        ));

        let double_parsed = parse_type_name_classified("double");
        let TypeName::Ident(double) = double_parsed.ast() else {
            panic!("bare double must remain an identifier type name")
        };
        assert_eq!(double.object(), "double");

        let json_parsed = parse_type_name_classified("json");
        let TypeName::Ident(json) = json_parsed.ast() else {
            panic!("json must preserve the legacy identifier variant")
        };
        assert_eq!(json.object(), "json");

        let qualified_parsed = parse_type_name_classified("pg_catalog.json");
        let TypeName::Ident(qualified) = qualified_parsed.ast() else {
            panic!("qualified json must remain an identifier type name")
        };
        assert_eq!(qualified.parts.len(), 2);
        assert_eq!(qualified.object(), "json");

        assert_cast_type_families_track_type_name_spellings();
        assert_cast_type_family_modifiers_round_trip();
    }

    fn assert_cast_type_families_track_type_name_spellings() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        fn expects_general_head(name: &TypeName<'_>) -> bool {
            match name {
                TypeName::Timestamp | TypeName::Time | TypeName::Interval => false,
                TypeName::Bool
                | TypeName::Boolean
                | TypeName::Text
                | TypeName::Integer
                | TypeName::Int
                | TypeName::Serial
                | TypeName::Numeric
                | TypeName::Varchar
                | TypeName::DoublePrecision
                | TypeName::Bit
                | TypeName::Character
                | TypeName::Unknown
                | TypeName::Ident(_) => true,
            }
        }

        for (src, canonical) in [
            ("bool", "BOOL"),
            ("boolean", "BOOLEAN"),
            ("text", "TEXT"),
            ("integer", "INTEGER"),
            ("int", "INT"),
            ("serial", "SERIAL"),
            ("numeric", "NUMERIC"),
            ("varchar", "VARCHAR"),
            ("double precision", "DOUBLE PRECISION"),
            ("timestamp", "TIMESTAMP"),
            ("time", "TIME"),
            ("interval", "INTERVAL"),
            ("bit", "BIT"),
            ("character", "CHARACTER"),
            ("unknown", "UNKNOWN"),
            ("json", "json"),
            ("pg_catalog.custom_type", "pg_catalog.custom_type"),
        ] {
            let name_parsed = parse_type_name_classified(src);
            let name = name_parsed.ast();
            let ty_parsed = parse_cast_type_classified(src);
            let ty = ty_parsed.ast();
            assert_eq!(
                matches!(&ty.head, CastTypeHead::General(_)),
                expects_general_head(name),
                "cast family for {src:?}",
            );
            assert_eq!(
                format_tokens_sql(ty, PrettyConfig::default()).trim(),
                canonical,
                "cast type did not render canonically for {src:?}",
            );
        }
    }

    fn assert_cast_type_family_modifiers_round_trip() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        for (src, expected_family, canonical) in [
            ("json", "general", "json"),
            (
                "timestamp(2) without time zone",
                "datetime",
                "TIMESTAMP(2) WITHOUT TIME ZONE",
            ),
            (
                "interval day to minute",
                "interval",
                "INTERVAL DAY TO MINUTE",
            ),
        ] {
            let ty_parsed = parse_cast_type_classified(src);
            let ty = ty_parsed.ast();
            let actual_family = match &ty.head {
                CastTypeHead::General(_) => "general",
                CastTypeHead::DateTime(_) => "datetime",
                CastTypeHead::Interval(_) => "interval",
            };
            assert_eq!(actual_family, expected_family, "cast family for {src:?}");
            assert_eq!(
                format_tokens_sql(ty, PrettyConfig::default()).trim(),
                canonical,
                "cast type did not render canonically for {src:?}",
            );
        }
    }

    // `JSON()` is added in 17: research, PostgreSQL 17, "Queries and
    // expressions".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn json_constructor_owns_unique_keys_after_a_cast() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        // The boundary itself is asserted structurally: the cast must not
        // consume `WITH`, and the JSON constructor must own `UNIQUE KEYS`.
        // Rendering compares against the canonical mechanical form; exact
        // source-preserving output (`::timestamp`) is deferred to the
        // provenance-aware formatting milestone.
        let src = "JSON('2000-01-01'::timestamp WITH UNIQUE KEYS)";
        let expr_parsed = parse_expr_classified(src);
        let expr = expr_parsed.ast();
        let Expr::JsonCtor(ctor) = &expr else {
            panic!("JSON constructor must own the WITH UNIQUE KEYS clause")
        };
        let unique = ctor
            .inner
            .unique
            .as_ref()
            .expect("WITH UNIQUE KEYS belongs to the JSON constructor");
        assert!(unique.keys, "the optional KEYS noise word is preserved");
        let Expr::Cast(_, cast_type) = ctor.inner.value.as_ref() else {
            panic!("the constructor argument remains a cast")
        };
        let CastTypeHead::DateTime(datetime) = &cast_type.head else {
            panic!("timestamp keeps the date/time cast family")
        };
        assert!(
            datetime.tz.is_none(),
            "the cast must not consume WITH toward a time-zone qualifier",
        );
        assert_eq!(
            format_tokens_sql(expr, PrettyConfig::default()).trim(),
            "JSON('2000-01-01':: TIMESTAMP WITH UNIQUE KEYS)",
        );
    }

    /// Function-call parentheses remain required independently of the
    /// optional typed-literal and aggregate suffixes that follow them.
    #[test]
    fn parse_function_calls_with_required_delimiters() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        for (src, expected_body, expected_tail) in [
            ("f()", "empty", "plain"),
            ("f(*)", "star", "plain"),
            ("f(ALL 1)", "all", "plain"),
            ("f(ALL 1 ORDER BY 1)", "all", "plain"),
            ("f(DISTINCT 1)", "distinct", "plain"),
            ("f(DISTINCT 1 ORDER BY 1)", "distinct", "plain"),
            ("f(1 ORDER BY 1)", "ordered", "plain"),
            ("f(1, 2, 3, 4, 5, 6)", "args", "plain"),
            ("f(VARIADIC xs)", "leading-variadic", "plain"),
            ("f(1, VARIADIC xs ORDER BY 1)", "trailing-variadic", "plain"),
            (
                "f(1) WITHIN GROUP (ORDER BY 1) FILTER (WHERE TRUE) OVER ()",
                "args",
                "within-group",
            ),
            ("f() WITHIN GROUP (ORDER BY 1)", "empty", "within-group"),
            ("f(*) WITHIN GROUP (ORDER BY 1)", "star", "within-group"),
            ("f(ALL 1) WITHIN GROUP (ORDER BY 1)", "all", "within-group"),
            (
                "f(1, 2, 3, 4, 5, 6) WITHIN GROUP (ORDER BY 1)",
                "args",
                "within-group",
            ),
            ("char(20) 'x'", "typed", "typed-literal"),
            ("f(1, 2, 3, 4, 5, 6) 'x'", "typed", "typed-literal"),
            (r#""normalize"()"#, "empty", "plain"),
        ] {
            let expr_parsed = parse_expr_classified(src);
            let expr = expr_parsed.ast();
            let Expr::Func(call) = &expr else {
                panic!("expected an ordinary function call for {src:?}");
            };
            let (actual_body, actual_tail) = match &call.tail {
                FunctionCallTail::TypedLiteral(_) => ("typed", "typed-literal"),
                FunctionCallTail::Call(tail) => {
                    let body = match tail.body.as_ref() {
                        None => "empty",
                        Some(FunctionCallBody::Star(_)) => "star",
                        Some(FunctionCallBody::All(_)) => "all",
                        Some(FunctionCallBody::Distinct(_)) => "distinct",
                        Some(FunctionCallBody::LeadingVariadic(_)) => "leading-variadic",
                        Some(FunctionCallBody::Args(args))
                            if args.trailing_variadic.is_some() =>
                        {
                            "trailing-variadic"
                        }
                        Some(FunctionCallBody::Args(args)) if args.order_by.is_some() => "ordered",
                        Some(FunctionCallBody::Args(_)) => "args",
                    };
                    let kind = if tail.within_group.is_some() { "within-group" } else { "plain" };
                    (body, kind)
                }
            };
            assert_eq!(actual_body, expected_body, "application body for {src:?}");
            assert_eq!(actual_tail, expected_tail, "call tail for {src:?}");
            let formatted = format_tokens_sql(expr, PrettyConfig::default());
            assert_eq!(
                formatted.trim(),
                src,
                "function call did not round-trip exactly for {src:?}",
            );
        }

        // With no standalone ALL expression atom, the token-identical compact
        // spelling has exactly the PostgreSQL interpretation: ALL qualifies
        // the parenthesized first argument.
        let parsed = parse_expr_classified("f(ALL(1))");
        let Expr::Func(call) = parsed.ast() else {
            panic!("expected compact ALL-qualified function call");
        };
        assert!(matches!(
            &call.tail,
            FunctionCallTail::Call(crate::ast::shared::expr::FunctionCallSuffix {
                body: Some(FunctionCallBody::All(_)),
                ..
            })
        ));
    }

    /// Applications gram.y's `func_expr` admits: `func_application` is one
    /// production for every argument shape, so an ordered, `DISTINCT` or
    /// `VARIADIC` argument list before `WITHIN GROUP` is grammatical and only
    /// parse analysis rejects it (`cannot use multiple ORDER BY clauses with
    /// WITHIN GROUP`). A typed literal still admits a named argument, which
    /// gram.y rejects in a rule action. Its semantically rejected inner
    /// `ORDER BY` is excluded here because pg-sql has no rule-action phase.
    #[test]
    fn accept_function_applications_gram_y_admits() {
        for src in [
            "f(1 ORDER BY 1) WITHIN GROUP (ORDER BY 1)",
            "f(DISTINCT 1) WITHIN GROUP (ORDER BY 1)",
            "f(VARIADIC xs) WITHIN GROUP (ORDER BY 1)",
            "char(n => 1) 'x'",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            Expr::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    /// PostgreSQL's aggregate wildcard is an exclusive function-application
    /// production, never an ordinary expression argument.
    #[test]
    fn reject_wildcard_as_an_ordinary_function_argument() {
        for src in [
            "f(*, 1)",
            "f(1, *)",
            "f(VARIADIC *)",
            "f(name => *)",
            "f(DISTINCT *)",
            "f(DISTINCT)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "invalid function application parsed completely: {src:?}",
            );
        }
    }

    #[test]
    fn reject_invalid_function_application_states() {
        for src in [
            "f(ALL)",
            "f(DISTINCT)",
            "f(VARIADIC xs, 1)",
            "f(1, VARIADIC xs, 2)",
            "f(VARIADIC xs, VARIADIC ys)",
            "f(ALL VARIADIC xs)",
            "f(DISTINCT VARIADIC xs)",
            "char() 'x'",
            "char(*) 'x'",
            "char(DISTINCT 1) 'x'",
            "char(VARIADIC xs) 'x'",
            "char(1 ORDER BY 1) 'x'",
            "char(1) 'x' FILTER (WHERE true)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "invalid function application parsed completely: {src:?}",
            );
        }
    }

    #[test]
    fn parse_unicode_string_with_backslash() {
        // `U&' \'` — backslash is literal content, not an escape. The string
        // ends at the second quote. UESCAPE '!' follows.
        let lexed = crate::lex(r"U&' \' UESCAPE '!'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_xmlforest() {
        let lexed = crate::lex("xmlforest(a, b AS bee, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::XmlForest(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_select_exponent_numeric() {
        use crate::ast::dml::select::SelectStmt;
        for sql in [
            "SELECT 4.5e10",
            "SELECT 4.4e131071",
            "SELECT 1.5e-5",
            "SELECT round(4.5e10, -5)",
            "SELECT .5",
            "SELECT 2e3",
        ] {
            let lexed = crate::lex(sql);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = SelectStmt::parse(&mut input).unwrap();
            let _stmt = _stmt_parsed.ast();
            assert!(input.is_eof(), "leftover for {sql}");
        }
    }

    #[test]
    fn parse_escape_string_literal() {
        let lexed = crate::lex(r"E'r_\_view%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_order_by() {
        let lexed = crate::lex("jsonb_agg(q ORDER BY x, y)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_both_from() {
        let lexed = crate::lex("TRIM(BOTH FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_leading_from() {
        let lexed = crate::lex("TRIM(LEADING FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_trailing_from() {
        let lexed = crate::lex("TRIM(TRAILING FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_both_chars_from() {
        let lexed = crate::lex("TRIM(BOTH 'x' FROM 'xxhixx')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    /// `TRIM([LEADING|TRAILING|BOTH] expr_list)` — gram.y `trim_list`
    /// includes the bare `expr_list` form (no FROM separator), so
    /// `TRIM(TRAILING ' foo ')` is valid: trim trailing whitespace from
    /// `' foo '`. Exercised by create_view.tt201v.
    #[test]
    fn parse_trim_direction_no_from() {
        for src in [
            "TRIM(TRAILING ' foo ')",
            "TRIM(LEADING ' foo ')",
            "TRIM(BOTH ' foo ')",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _expr = _expr_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    /// `USER` is the SQL-standard zero-arg synonym for `CURRENT_USER`.
    /// pg-sql keeps `USER` reserved at the token level (for the
    /// `CREATE USER ...` statement), so it cannot lex as an
    /// `UnquotedIdent` and needs a dedicated `Expr::User` atom.
    #[test]
    fn parse_user_zero_arg_atom() {
        for src in ["SELECT USER", "SELECT USER AS us", "SELECT * FROM USER"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = crate::ast::Statement::parse(&mut input)
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

    #[test]
    fn parse_substring_from() {
        let lexed = crate::lex("SUBSTRING('1234567890' FROM 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_from_for() {
        let lexed = crate::lex("SUBSTRING('1234567890' FROM 4 FOR 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_notnull_isnull() {
        let lexed = crate::lex("x.c NOTNULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Notnull(..)));
        assert!(input.is_eof());
        let lexed = crate::lex("x.c ISNULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Isnull(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_collation_for() {
        let lexed = crate::lex("collation for ('foo')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
        let lexed = crate::lex("collation for ((SELECT a FROM t LIMIT 1))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_call() {
        // gram.y `CAST '(' a_expr AS Typename ')'` has no collation inside the
        // parentheses; PostgreSQL rejects `CAST('42' AS text COLLATE "C")`
        // (collate.sql). The collation follows the cast.
        let lexed = crate::lex("CAST('42' AS text COLLATE \"C\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Expr::parse(&mut input).is_err() || !input.is_eof());
        let lexed = crate::lex("CAST('42' AS text) COLLATE \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
        let lexed = crate::lex("CAST(b AS varchar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_for_only() {
        let lexed = crate::lex("substring(d FOR 30)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_similar_escape() {
        let lexed = crate::lex("SUBSTRING('abcdefg' SIMILAR 'a#\"%#\"g' ESCAPE '#')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_position_in() {
        let lexed = crate::lex("POSITION('4' IN '1234567890')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn position_needle_uses_postgres_restricted_expression_extensions() {
        // `b_expr` retains comparisons, symbolic operators, casts, and the
        // selected IS forms that PostgreSQL declares in both expression
        // grammars. The restriction must reach recursive Pratt operands so
        // the delimiter IN is not swallowed by a comparison's right side.
        for src in [
            "POSITION(1 + 2 = 3 IN 4)",
            "POSITION(1::int = 1 IN 2)",
            "POSITION(1 IS DISTINCT FROM 2 IN 3)",
            "POSITION(1 IS DOCUMENT IN 2)",
            "POSITION(1 || 2 IN 3)",
        ] {
            parse_expr_classified(src).ast();
        }

        // PostgreSQL deliberately admits a full `a_expr` inside parentheses
        // as a `b_expr` atom.
        parse_expr_classified("POSITION((1 IN (1)) IN 2)").ast();
        parse_expr_classified("POSITION((1 AND 2) IN 3)").ast();
    }

    #[test]
    fn position_needle_rejects_unparenthesized_a_expr_only_extensions() {
        for src in [
            "POSITION(1 COLLATE c IN 2)",
            "POSITION(1 > ANY (0) IN 2)",
            "POSITION(1 IS TRUE IN 2)",
            "POSITION(1 AT LOCAL IN 2)",
            "POSITION(1 NOT IN (0) IN 2)",
            "POSITION(1 LIKE 1 IN 2)",
            "POSITION(1 BETWEEN 0 AND 2 IN 3)",
            "POSITION((1, 2) OVERLAPS (3, 4) IN 5)",
            "POSITION(1 OR 2 IN 3)",
            "POSITION(1 AND 2 IN 3)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "unparenthesized a_expr-only needle extension parsed: {src:?}",
            );
        }
    }

    #[test]
    fn parse_overlay_placing_from() {
        let lexed = crate::lex("OVERLAY('abcdef' PLACING '45' FROM 4)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlay_placing_from_for() {
        let lexed = crate::lex("OVERLAY('abcdef' PLACING '45' FROM 4 FOR 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_epoch_from_date() {
        let lexed = crate::lex("EXTRACT(EPOCH FROM DATE '1970-01-01')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Extract(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_century_from_ident() {
        let lexed = crate::lex("EXTRACT(CENTURY FROM d)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_string_field() {
        let lexed = crate::lex("EXTRACT('year' FROM t)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_named_arg_mixed() {
        let lexed = crate::lex("f(a, b => 1, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_path_query_silent() {
        let lexed = crate::lex("jsonb_path_query('[1]', 'strict $[1]', silent => true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_all_named_args() {
        let lexed = crate::lex("f(silent => false, verbose => true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_year_from_now() {
        let lexed = crate::lex("EXTRACT(year FROM now())");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_is_distinct_from() {
        let lexed = crate::lex("a IS DISTINCT FROM b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_is_not_distinct_from() {
        let lexed = crate::lex("a IS NOT DISTINCT FROM b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_power_operator() {
        let lexed = crate::lex("2^1000");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_double_precision_type_cast() {
        let lexed = crate::lex("3.14::double precision");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_searched() {
        let lexed = crate::lex("CASE WHEN 1 < 2 THEN 3 END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Case(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_searched_with_else() {
        let lexed = crate::lex("CASE WHEN 1 < 2 THEN 3 WHEN 4 < 5 THEN 6 ELSE 7 END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_simple() {
        let lexed = crate::lex("CASE x WHEN 1 THEN 'a' WHEN 2 THEN 'b' ELSE 'c' END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_nested() {
        let lexed = crate::lex("CASE WHEN (CASE WHEN 1=1 THEN 1 END) > 0 THEN 'y' END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_within_group() {
        let lexed = crate::lex("percentile_disc(0.5) WITHIN GROUP (ORDER BY v)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_within_group_multi() {
        let lexed = crate::lex("rank(1, 2) WITHIN GROUP (ORDER BY a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_filter() {
        let lexed = crate::lex("sum(x) FILTER (WHERE y > 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_filter_over() {
        let lexed = crate::lex("sum(x) FILTER (WHERE y > 0) OVER (PARTITION BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_order_by_nulls_first() {
        let lexed = crate::lex("jsonb_agg(q ORDER BY x NULLS FIRST, y)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_variadic() {
        let lexed = crate::lex("jsonb_build_array(VARIADIC a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr_parsed = Expr::parse(&mut input).unwrap();
        let _expr = _expr_parsed.ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_timestamp_with_tz_literal() {
        let lexed = crate::lex("timestamp with time zone '2001-12-27 04:05:06+08'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_timestamp_precision_without_tz_literal() {
        // Regression: timestamp.sql uses `timestamp(2) without time zone 'now'`.
        let lexed = crate::lex("timestamp(2) without time zone 'now'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_time_zone() {
        let lexed = crate::lex("f1 AT TIME ZONE 'UTC+10'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_time_zone_interval() {
        let lexed = crate::lex("f1 AT TIME ZONE INTERVAL '-10:00'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_eof());
    }

    // Added in 17: research, PostgreSQL 17, "Queries and expressions".
    #[cfg(feature = "since-pg17")]
    #[test]
    fn parse_at_local() {
        let lexed = crate::lex("f1 AT LOCAL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::AtLocal(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_time_literal() {
        let lexed = crate::lex("time '12:34'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TimeLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_date_literal_as_castfunc() {
        let lexed = crate::lex("date '2024-01-01'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        // `date` is an Ident-based TypeName, so this parses as CastFunc.
        assert!(matches!(expr, Expr::CastFunc(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_bare() {
        let lexed = crate::lex("interval '1 hour'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_year() {
        let lexed = crate::lex("INTERVAL '1' YEAR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_year_to_month() {
        let lexed = crate::lex("INTERVAL '1-2' YEAR TO MONTH");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_named_arg_colon_equals() {
        let lexed = crate::lex("make_interval(years := 1, months := 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unary_plus() {
        let lexed = crate::lex("+42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Pos(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_positional_param() {
        let lexed = crate::lex("$1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::PositionalParam(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_positional_param_in_expr() {
        let lexed = crate::lex("$1 + $2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_eof());
    }

    /// `$1` must preserve its digits when reformatted — a positional parameter
    /// is not interchangeable with `$2`. The token must capture the number.
    #[test]
    fn positional_param_preserves_digits() {
        use recursa::PrettyConfig;
        let lexed = crate::lex("$2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        let formatted = crate::formatter::format_tokens_sql(expr, PrettyConfig::default());
        assert_eq!(formatted.trim(), "$2");
    }

    #[test]
    fn parse_interval_with_precision() {
        for src in [
            "INTERVAL(0) '1 day 01:23:45.6789'",
            "interval(2) '1 day 01:23:45.6789'",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input).unwrap();
            let expr = expr_parsed.ast();
            assert!(matches!(expr, Expr::IntervalLit(_)), "failed for {src:?}");
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_interval_second_precision() {
        let lexed = crate::lex("INTERVAL '1.234' second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_day_to_second_precision() {
        let lexed = crate::lex("INTERVAL '1 2:03:04.5678' day to second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_interval_day_to_minute() {
        let lexed = crate::lex("f1::INTERVAL DAY TO MINUTE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Cast(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_minute_to_second_precision() {
        let lexed = crate::lex("INTERVAL '12:34.5678' minute to second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_day_to_hour() {
        let lexed = crate::lex("INTERVAL '1 2:03' DAY TO HOUR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_hour_to_second() {
        let lexed = crate::lex("INTERVAL '1' HOUR TO SECOND");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_escape_string_literal_lowercase_e() {
        let lexed = crate::lex("e'foo'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bool_true() {
        let lexed = crate::lex("true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolTrue));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bool_false() {
        let lexed = crate::lex("false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolFalse));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_null() {
        let lexed = crate::lex("null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Null));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_ref() {
        let lexed = crate::lex("f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(_)));
    }

    #[test]
    fn parse_qualified_column_ref() {
        let lexed = crate::lex("BOOLTBL1.f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::QualRef(_)));
    }

    #[test]
    fn parse_qualified_wildcard() {
        let lexed = crate::lex("BOOLTBL1.*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::QualRef(_)));
    }

    #[test]
    fn parse_multi_part_qualified_references() {
        for source in ["s.t.column", "s.t.*", "s.func(1)"] {
            let lexed = crate::lex(source);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {source:?}");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|error| panic!("parse {source:?}: {error}"))
                ;
            let expr = expr_parsed.ast();
            assert!(matches!(expr, Expr::QualRef(_)), "{source:?}");
            assert!(input.is_eof(), "trailing input in {source:?}");
        }
    }

    /// gram.y qualifies a `columnref` with a `ColId`, so unreserved and
    /// column-name keywords qualify and reserved keywords do not.
    ///
    /// The negative half is load-bearing: `Expr` heads every SELECT target
    /// list, so a qualifier admitting every keyword would put every reserved
    /// word into FIRST(`SelectHead`) and break the targetless-`SELECT` set
    /// operations of issue #55.
    #[test]
    fn qualified_ref_qualifier_is_a_col_id() {
        for src in ["excluded.a", "new.a", "old.a", "value.a", "excluded.*"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let expr = expr_parsed.ast();
            assert!(
                matches!(expr, Expr::QualRef(_)),
                "{src:?} must qualify",
            );
        }
        for src in ["union.a", "select.a", "grant.a", "create.*"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            assert!(
                Expr::parse(&mut input).is_err(),
                "a reserved keyword must not qualify a column reference: {src:?}",
            );
        }
    }

    #[test]
    fn parse_star() {
        use crate::ast::dml::select::SelectItem;

        let lexed = crate::lex("*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        assert!(Expr::parse(&mut input).is_err());

        let lexed = crate::lex("*");
        let mut input = lexed.input();
        let item_parsed = SelectItem::parse(&mut input).unwrap();
        let item = item_parsed.ast();
        assert!(matches!(item, SelectItem::Star(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_function_call_no_args() {
        let lexed = crate::lex("foo()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_with_args() {
        let lexed = crate::lex("pg_input_is_valid('true', 'bool')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_booleq() {
        let lexed = crate::lex("booleq(bool 'false', f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_parenthesized_expr() {
        let lexed = crate::lex("(1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Exprs(expressions),
                indirection,
                ..
            }) if expressions.len() == 1 && indirection.is_empty()
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_row_expr() {
        let lexed = crate::lex("(a,b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Exprs(expressions),
                indirection,
                ..
            }) if expressions.len() == 2 && indirection.is_empty()
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_field_star() {
        let lexed = crate::lex("(row_value).*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                indirection,
                ..
            }) if matches!(
                indirection.as_slice(),
                [ParenthesizedIndirection::Star(ParenthesizedDotStar::Value)]
            )
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_field_access() {
        let lexed = crate::lex("(row_value).field");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                indirection,
                ..
            }) if matches!(
                indirection.as_slice(),
                [ParenthesizedIndirection::Field(_)]
            )
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_chained_fields_and_star() {
        let lexed = crate::lex("(row_value).field.nested.*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                indirection,
                ..
            })
                if matches!(
                    indirection.as_slice(),
                    [
                        ParenthesizedIndirection::Field(_),
                        ParenthesizedIndirection::Field(_),
                        ParenthesizedIndirection::Star(ParenthesizedDotStar::Value),
                    ]
                )
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_mixed_indirection() {
        let lexed = crate::lex("(row_value).a[1].b[2:3].*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                indirection,
                ..
            }) if matches!(
                indirection.as_slice(),
                [
                    ParenthesizedIndirection::Field(_),
                    ParenthesizedIndirection::Subscript(_),
                    ParenthesizedIndirection::Field(_),
                    ParenthesizedIndirection::Subscript(_),
                    ParenthesizedIndirection::Star(ParenthesizedDotStar::Value),
                ]
            )
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_star_then_cast() {
        let lexed = crate::lex("(row_value).*::text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Cast(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_parenthesized_field_access() {
        let lexed = crate::lex("((row_value)::record_type).field");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                indirection,
                ..
            }) if matches!(
                indirection.as_slice(),
                [ParenthesizedIndirection::Field(_)]
            )
        ));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_parenthesized_grouped_set_query() {
        let lexed = crate::lex("((SELECT 1) UNION SELECT 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Subquery(subquery),
                indirection,
                ..
            }) if matches!(
                &subquery.body.clause,
                SelectClause::Union(left, _, _) if matches!(**left, SelectClause::Parens(_))
            )
                && indirection.is_empty()
        ));
        assert!(input.is_eof());
    }

    /// The parenthesized set-operation query admitted inside expressions
    /// accepts at most one limiting clause (`LIMIT` or `FETCH FIRST`) and at
    /// most one `OFFSET` clause. The right-hand side is a `VALUES` body so no
    /// nested query can absorb the first clause: the duplicates land on this
    /// form's own limit/offset tail.
    #[test]
    fn reject_direct_parenthesized_set_duplicate_limit_offset_clauses() {
        for src in [
            "(SELECT 1) UNION VALUES(2) LIMIT 1 LIMIT 1",
            "(SELECT 1) UNION VALUES(2) OFFSET 1 OFFSET 1",
            "(SELECT 1) UNION VALUES(2) FETCH FIRST 1 ROWS ONLY FETCH FIRST 1 ROWS ONLY",
            "(SELECT 1) UNION VALUES(2) LIMIT 1 FETCH FIRST 1 ROWS ONLY",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Subquery::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "invalid duplicate clause parsed to EOF: {src:?}"
            );
        }
    }

    /// Both PostgreSQL clause orders and each bare clause round-trip on the
    /// parenthesized set-operation query with the written order preserved.
    /// The right-hand side is a `VALUES` body so the clauses land on this
    /// form's own limit/offset tail rather than a nested query.
    #[test]
    fn parse_direct_parenthesized_set_limit_offset_orders_roundtrip() {
        use crate::ast::test_support::roundtrip;
        for src in [
            "(SELECT 1) UNION VALUES(2) LIMIT 2 OFFSET 3",
            "(SELECT 1) UNION VALUES(2) OFFSET 3 LIMIT 2",
            "(SELECT 1) UNION VALUES(2) OFFSET 3 FETCH FIRST 2 ROWS ONLY",
            "(SELECT 1) UNION VALUES(2) LIMIT 2",
            "(SELECT 1) UNION VALUES(2) OFFSET 3",
            "(SELECT 1) UNION VALUES(2) FETCH FIRST 2 ROWS ONLY",
        ] {
            assert_eq!(roundtrip::<Subquery>(src), src);
        }
    }

    // --- Type cast function-style: bool 'foo' ---

    #[test]
    fn parse_type_cast_bool_string() {
        let lexed = crate::lex("bool 't'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    #[test]
    fn parse_type_cast_boolean_string() {
        let lexed = crate::lex("boolean 'false'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    // --- Prefix operators ---

    #[test]
    fn parse_not_expr() {
        let lexed = crate::lex("not false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Not(_)));
    }

    // --- Infix operators ---

    #[test]
    fn parse_and_expr() {
        let lexed = crate::lex("true AND false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::And(..)));
    }

    #[test]
    fn parse_or_expr() {
        let lexed = crate::lex("true OR false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn parse_eq_expr() {
        let lexed = crate::lex("f1 = true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Eq(..)));
    }

    #[test]
    fn parse_neq_expr() {
        let lexed = crate::lex("f1 <> false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Neq(..)));
    }

    // --- Postfix: :: type cast ---

    #[test]
    fn parse_cast_colon_colon() {
        let lexed = crate::lex("0::boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    #[test]
    fn parse_chained_cast() {
        let lexed = crate::lex("'TrUe'::text::boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        // Outer should be Cast
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Postfix: IS [NOT] TRUE/FALSE/UNKNOWN/NULL ---

    #[test]
    fn parse_is_true() {
        let lexed = crate::lex("f1 IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_false() {
        let lexed = crate::lex("f1 IS NOT FALSE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_unknown() {
        let lexed = crate::lex("b IS UNKNOWN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_unknown() {
        let lexed = crate::lex("b IS NOT UNKNOWN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    // --- Postfix: BETWEEN / NOT BETWEEN ---

    #[test]
    fn parse_between_expr() {
        let lexed = crate::lex("a BETWEEN 12 AND 17");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn parse_not_between_expr() {
        let lexed = crate::lex("a NOT BETWEEN 1 AND 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotBetweenExpr(..)));
    }

    #[test]
    fn parse_between_as_value() {
        // BETWEEN yields a boolean value that can appear in a SELECT list.
        let lexed = crate::lex("x BETWEEN a AND b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn between_does_not_break_and_parse() {
        // A plain AND expression must still parse as And, not be confused
        // with the BETWEEN postfix.
        let lexed = crate::lex("a AND b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::And(..)));
    }

    // --- Precedence ---

    #[test]
    fn and_binds_tighter_than_or() {
        // a OR b AND c should parse as a OR (b AND c)
        let lexed = crate::lex("true OR false AND true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        // Top-level should be OR
        match &expr {
            Expr::Or(..) => {}
            other => panic!("expected OR at top level, got {other:?}"),
        }
    }

    #[test]
    fn comparison_binds_tighter_than_and() {
        // a AND b = c should parse as a AND (b = c)
        let lexed = crate::lex("true AND f1 = false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        match &expr {
            Expr::And(..) => {}
            other => panic!("expected AND at top level, got {other:?}"),
        }
    }

    #[test]
    fn bool_cast_or_expr() {
        // bool 't' or bool 'f' should parse as (bool 't') OR (bool 'f')
        let lexed = crate::lex("bool 't' or bool 'f'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn is_true_in_select_item() {
        // b IS TRUE should parse without consuming AS that follows
        let lexed = crate::lex("b IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn cast_chain_in_expression() {
        // true::boolean::text should chain
        let lexed = crate::lex("true::boolean::text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Arithmetic operators ---

    #[test]
    fn parse_addition() {
        let lexed = crate::lex("4+4");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn concat_binds_less_tightly_than_addition() {
        let lexed = crate::lex("a || b + c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(input.is_eof());

        let Expr::Concat(_, right) = expr else {
            panic!("expected concatenation at the root")
        };
        assert!(
            matches!(**right, Expr::Add(..)),
            "addition must bind inside the concatenation right operand"
        );
    }

    #[test]
    fn parse_subtraction() {
        let lexed = crate::lex("10-3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Sub(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unary_minus() {
        let lexed = crate::lex("-1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Neg(..)));
        assert!(input.is_eof());
    }

    // --- Numeric literal ---

    #[test]
    fn parse_numeric_literal() {
        let lexed = crate::lex("77.7");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NumericLit(_)));
        assert!(input.is_eof());
    }

    // --- IN expression ---

    #[test]
    fn parse_in_expr() {
        let lexed = crate::lex("f1 IN (1, 2, 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::InExpr(..)));
        assert!(input.is_eof());
    }

    // --- JSON / JSONB operators ---

    #[test]
    fn parse_json_field() {
        let lexed = crate::lex("data -> 'key'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonField(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_field_text() {
        let lexed = crate::lex("data ->> 'key'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonFieldText(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path() {
        let lexed = crate::lex("data #> '{a,b}'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonPath(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path_text() {
        let lexed = crate::lex("data #>> '{a,b}'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonPathText(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_contains() {
        let lexed = crate::lex("a @> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonContains(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_contained_by() {
        let lexed = crate::lex("a <@ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonContainedBy(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_key_exists() {
        let lexed = crate::lex("a ? 'k'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonKey(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_any_key() {
        let lexed = crate::lex("a ?| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonAnyKey(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_all_keys() {
        let lexed = crate::lex("a ?& b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonAllKeys(..)));
        assert!(input.is_eof());
    }

    // --- Postgres text-search / range / geometric operators ---

    #[test]
    fn parse_ts_match() {
        let lexed = crate::lex("a @@ 'foo|bar'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TsMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ts_match3() {
        let lexed = crate::lex("a @@@ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TsMatch3(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path_exists() {
        let lexed = crate::lex("j @? '$.a'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::JsonPathExists(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlap() {
        let lexed = crate::lex("r && s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Overlap(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_left() {
        let lexed = crate::lex("a << b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::StrictlyLeft(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_right() {
        let lexed = crate::lex("a >> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::StrictlyRight(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_subset_eq() {
        let lexed = crate::lex("a <<= b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::SubsetEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_superset_eq() {
        let lexed = crate::lex("a >>= b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::SupersetEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_adjacent() {
        let lexed = crate::lex("a -|- b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Adjacent(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_distance() {
        let lexed = crate::lex("p1 <-> p2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Distance(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_right() {
        let lexed = crate::lex("a &< b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NoExtendRight(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_left() {
        let lexed = crate::lex("a &> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NoExtendLeft(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_above() {
        let lexed = crate::lex("a |>> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::StrictlyAbove(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_below() {
        let lexed = crate::lex("a <<| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::StrictlyBelow(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_above() {
        let lexed = crate::lex("a &<| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NoExtendAbove(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_below() {
        let lexed = crate::lex("a |&> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NoExtendBelow(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_intersect() {
        let lexed = crate::lex("a ?# b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Intersect(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_horizontal() {
        let lexed = crate::lex("a ?- b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Horizontal(..)));
        assert!(input.is_eof());
    }

    // --- LIKE / ILIKE ---

    #[test]
    fn parse_like_expr() {
        let lexed = crate::lex("table_name LIKE 'foo%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_like_escape_string() {
        let lexed = crate::lex(r"table_name LIKE E'r_\_view%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_like_expr() {
        let lexed = crate::lex("table_name NOT LIKE 'bar%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_expr() {
        let lexed = crate::lex("x SIMILAR TO 'a%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_similar_to_expr() {
        let lexed = crate::lex("x NOT SIMILAR TO 'a%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ilike_expr() {
        let lexed = crate::lex("name ILIKE '%FOO%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_ilike_expr() {
        let lexed = crate::lex("name NOT ILIKE '%bar%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_like_escape() {
        let lexed = crate::lex("'hawkeye' LIKE 'h%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_like_escape() {
        let lexed = crate::lex("'hawkeye' NOT LIKE 'h%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_escape() {
        let lexed = crate::lex("'abcdefg' SIMILAR TO '_bcd#%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_similar_to_escape() {
        let lexed = crate::lex("'abc' NOT SIMILAR TO 'a%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ilike_escape() {
        let lexed = crate::lex("name ILIKE '%FOO%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_ilike_escape() {
        let lexed = crate::lex("name NOT ILIKE '%bar%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_escape_null() {
        let lexed = crate::lex("'abcdefg' SIMILAR TO '_bcd%' ESCAPE NULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    // --- Regex match operators ---

    #[test]
    fn parse_regex_match() {
        let lexed = crate::lex("relname ~ '^foo'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::RegexMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_not_match() {
        let lexed = crate::lex("name !~ 'bar'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::RegexNotMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_imatch() {
        let lexed = crate::lex("name ~* 'FOO'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::RegexIMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_not_imatch() {
        let lexed = crate::lex("name !~* '.*'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::RegexNotIMatch(..)));
        assert!(input.is_eof());
    }

    // --- COLLATE postfix ---

    #[test]
    fn parse_collate_postfix() {
        let lexed = crate::lex("a COLLATE \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Collate(..)));
        assert!(input.is_eof());
    }

    // --- DEFAULT atom ---

    #[test]
    fn parse_default_atom() {
        let lexed = crate::lex("DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Default));
        assert!(input.is_eof());
    }

    // --- Subquery expression ---

    #[test]
    fn parse_subquery_expr() {
        let lexed = crate::lex("(SELECT 1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Subquery(_),
                indirection,
                ..
            }) if indirection.is_empty()
        ));
        assert!(input.is_eof());
    }

    // --- Locale-aware text comparison operators ---

    #[test]
    fn parse_tilde_lt_tilde_infix() {
        let lexed = crate::lex("f1 ~<~ 'YX'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TildeLtTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_leq_tilde_infix() {
        let lexed = crate::lex("t ~<=~ 'Aztec'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TildeLeqTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_geq_tilde_infix() {
        let lexed = crate::lex("t ~>=~ 'Worth'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TildeGeqTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_gt_tilde_infix() {
        let lexed = crate::lex("t ~>~ 'Worth'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TildeGtTilde(..)));
        assert!(input.is_eof());
    }

    // --- User-defined equality/inequality ---

    #[test]
    fn parse_triple_eq_infix() {
        let lexed = crate::lex("a === 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TripleEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bang_eq_eq_infix() {
        let lexed = crate::lex("a !== 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BangEqEq(..)));
        assert!(input.is_eof());
    }

    // --- Geometric closest-point / intersection ---

    #[test]
    fn parse_hash_hash_infix() {
        let lexed = crate::lex("p.f1 ## l.s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::GeomClosest(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: geometric path length `@-@` ---

    #[test]
    fn parse_at_minus_at_prefix() {
        let lexed = crate::lex("@-@ s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::PathLength(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: user-defined `@#@` ---

    #[test]
    fn parse_at_hash_at_prefix() {
        let lexed = crate::lex("@#@ 24");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::AtHashAtPrefix(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: user-defined `!=-` ---

    #[test]
    fn parse_bang_eq_minus_prefix() {
        let lexed = crate::lex("!=- 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::BangEqMinusPrefix(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: geometric `#` (number of points in path) ---

    #[test]
    fn parse_pound_prefix() {
        let lexed = crate::lex("#thepath");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::PointCount(..)));
        assert!(input.is_eof());
    }

    // --- Infix: geometric `?||` (parallel) and `?-|` (perpendicular) ---

    #[test]
    fn parse_question_pipe_pipe_infix() {
        let lexed = crate::lex("a ?|| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Parallel(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_question_dash_pipe_infix() {
        let lexed = crate::lex("a ?-| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Perpendicular(..)));
        assert!(input.is_eof());
    }

    // --- Infix: geometric `<^` (below) and `>^` (above) ---

    #[test]
    fn parse_lt_caret_infix() {
        let lexed = crate::lex("a <^ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Below(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_gt_caret_infix() {
        let lexed = crate::lex("a >^ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::Above(..)));
        assert!(input.is_eof());
    }

    // --- Infix: user-defined `<<<` and `>>>` ---

    #[test]
    fn parse_triple_lt_infix() {
        let lexed = crate::lex("a <<< 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TripleLt(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_triple_gt_infix() {
        let lexed = crate::lex("a >>> 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::TripleGt(..)));
        assert!(input.is_eof());
    }

    // --- Infix: user-defined `<%` ---

    #[test]
    fn parse_lt_percent_infix() {
        let lexed = crate::lex("a <% b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::CustomInfix(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn custom_infix_chain_is_left_associative() {
        let lexed = crate::lex("a <% b <% c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(input.is_eof());

        let Expr::CustomInfix(left, _, _) = expr else {
            panic!("expected custom operator at the root")
        };
        assert!(
            matches!(**left, Expr::CustomInfix(..)),
            "equal-precedence custom operators must associate to the left"
        );
    }

    #[test]
    fn custom_infix_does_not_consume_lower_precedence_comparison() {
        let lexed = crate::lex("a <% b = c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(input.is_eof());

        let Expr::Eq(left, _) = expr else {
            panic!("expected comparison at the root")
        };
        assert!(
            matches!(**left, Expr::CustomInfix(..)),
            "the custom operator must finish before the lower-precedence comparison"
        );
    }

    // --- Subquery quantifier: ANY / ALL / SOME ---

    #[test]
    fn parse_eq_any_subquery() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        // `a = ANY(SELECT 1)` — comparison with quantified subquery.
        let src = "a = ANY(SELECT 1)";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::QuantifiedComparisonCmp(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
        assert_eq!(
            format_tokens_sql(expr, PrettyConfig::default()).trim(),
            src,
        );
    }

    #[test]
    fn parse_eq_all_array() {
        // `a = ALL('{ab}')` — comparison with quantified array.
        let lexed = crate::lex("a = ALL('{ab}')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::QuantifiedComparisonCmp(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_not_tilde_all() {
        // `a !~ ALL('{ab}')` — regex not-match with ALL quantifier.
        let lexed = crate::lex("a !~ ALL('{ab}')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::QuantifiedComparisonCmp(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_eq_some_subquery() {
        // `a = SOME(SELECT 1)` — SOME is synonym for ANY.
        // The grouped set form must dispatch on UNION after the matching
        // inner close rather than a fixed token horizon.
        let lexed = crate::lex("a = SOME((SELECT 1) UNION SELECT 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::QuantifiedComparisonCmp(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    // --- Array slice subscripts ---

    #[test]
    fn parse_array_slice_full() {
        // `a[1:2]` — full slice with lower and upper bounds.
        let lexed = crate::lex("a[1:2]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(ColumnRef { subscripts, .. }) if !subscripts.is_empty()));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// A typed literal no longer takes a psql variable as its payload.
    ///
    /// gram.y's `AexprConst: ConstTypename Sconst` puts a string after the
    /// type name and nothing else. pg-sql used to admit psql's `:'var'`
    /// there, which is what made `SELECT int :'x'` a typed literal while
    /// `int :` also begins a SQL/JSON `JSON_OBJECT` entry -- 8 of the
    /// grammar's LALR conflicts. psql substitutes before the server lexes,
    /// so what reaches this grammar is an ordinary string constant; the
    /// `pg-psql` crate is what renders it.
    #[test]
    fn reject_typed_literal_with_a_psql_variable_payload() {
        for src in ["numeric :'txid'", "int :'x'", "bigint :'n'"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let consumed = Expr::parse(&mut input).is_ok() && input.is_eof();
            assert!(!consumed, "{src:?} must not parse as an expression");
        }
        // The spelling gram.y does have is untouched.
        assert!(matches!(
            parse_expr_classified("numeric '1'").ast(),
            Expr::CastFunc(_)
        ));
    }

    /// `j['a':'b']` — a slice whose bounds are string literals.
    ///
    /// This did not parse while psql's `:'name'` was one token of the SQL
    /// lexer: `:'b'` was taken for an interpolation, so the slice colon was
    /// never seen, and the form was recorded as a limitation. psql now has
    /// its own grammar, and both bounds are ordinary `opt_slice_bound`
    /// expressions (gram.y:16812).
    #[test]
    fn parse_array_slice_with_string_bounds() {
        let lexed = crate::lex("j['a':'b']");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(ColumnRef { subscripts, .. }) if !subscripts.is_empty()));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// Slice on a parenthesised cast: `(arr::int[])[1:2]` — PG accepts the
    /// postfix subscript on any a_expr including a parenthesised cast.
    ///
    /// Both bounds are plainly optional expressions now, as gram.y's
    /// `opt_slice_bound` has them. While the colon could also start a psql
    /// variable, a bound spelled with a reserved keyword (`[1:NULL]`) or a
    /// string (`['a':'b']`) had to be recovered from that reading; with psql
    /// in its own grammar there is nothing to recover from.
    #[test]
    fn parse_array_slice_on_paren_cast() {
        for src in [
            "('{1,2,3}'::int[])[1:2]",
            "a[1:3]",
            "a[NULL:3]",
            "a[1:NULL]",
            "('{1,2,3}'::int[])[1:NULL]",
            "('{{{1},{2},{3}},{{4},{5},{6}}}'::int[])[1][1:NULL][1]",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _expr = _expr_parsed.ast();
            assert!(
                input.is_eof(),
                "parser cursor for {src:?}: {}",
                input.cursor()
            );
        }
    }

    #[test]
    fn parse_array_slice_lower_only() {
        // `a[1:]` — slice with only lower bound.
        let lexed = crate::lex("a[1:]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(ColumnRef { subscripts, .. }) if !subscripts.is_empty()));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_array_slice_upper_only() {
        // `a[:2]` — slice with only upper bound.
        let lexed = crate::lex("a[:2]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(ColumnRef { subscripts, .. }) if !subscripts.is_empty()));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_array_slice_unbounded() {
        // `a[:]` — unbounded slice (all elements).
        let lexed = crate::lex("a[:]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(ColumnRef { subscripts, .. }) if !subscripts.is_empty()));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_subscript_unchanged() {
        // `a[1]` — regular subscript still works.
        let lexed = crate::lex("a[1]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr_parsed = Expr::parse(&mut input).unwrap();
        let expr = expr_parsed.ast();
        assert!(matches!(expr, Expr::ColumnRef(ColumnRef { subscripts, .. }) if !subscripts.is_empty()));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_any_array_literal() {
        for src in ["ANY('{red,green}'::rainbow[])", "SOME(1)"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "standalone quantified RHS parsed completely: {src:?}",
            );
        }
    }

    #[test]
    fn parse_all_array_literal() {
        let src = "ALL('{red,red}'::rainbow[])";
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
        let mut input = lexed.input();
        let parsed = Expr::parse(&mut input);
        assert!(
            parsed.is_err() || !input.is_eof(),
            "standalone quantified RHS parsed completely: {src:?}",
        );
    }

    /// `IN ((SELECT 1), (SELECT 2))` — gram.y `in_expr → '(' expr_list ')'`
    /// where each `expr_list` element is a parenthesized subquery expression.
    /// The expression-list and grouped-query alternatives share the `(`
    /// prefix; the LR state after the inner close distinguishes a comma from
    /// a set operator, while bare subqueries have their own leading tokens.
    #[test]
    fn parse_in_list_of_parenthesised_subqueries() {
        for src in [
            "SELECT * FROM t WHERE b IN ((select 1), (select 2))",
            // Mixed parenthesised subquery + bare expr.
            "SELECT * FROM t WHERE b IN (1, (select 2))",
            // Single bare subquery (no surrounding paren) — still a Subquery.
            "SELECT * FROM t WHERE b IN (select 1)",
            // A grouped set query is selected after the matching inner close.
            "SELECT * FROM t WHERE b IN ((select 1) UNION select 2)",
            // Single value list.
            "SELECT * FROM t WHERE b IN (1, 2, 3)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = crate::ast::Statement::parse(&mut input)
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

    /// `(SubSelect)::Typename` is gram.y `c_expr → '(' SubSelect ')' typecast`.
    /// The unified parenthesized atom returns the inner `(SubSelect)` to the
    /// Pratt loop, which consumes each trailing cast before the next enclosing
    /// the parenthesized expression expects its close parenthesis.
    #[test]
    fn parse_paren_subquery_cast_in_nested_contexts() {
        for src in [
            "SELECT ((select 1)::int)",
            "SELECT ((select 1)::int[])",
            "SELECT 1 = ANY((select array['abc']::text[])::text[])",
            "SELECT 1 = ANY((select array_agg(i) from generate_series(1, 100, 15) i)::int[])",
            // Chained casts inside the nested paren context.
            "SELECT ((select 1)::int::text)",
            // Bare Subquery must still match when no trailing cast follows.
            "SELECT ((select 1))",
            "SELECT ((select 1) UNION select 2)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt_parsed = crate::ast::Statement::parse(&mut input)
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

    /// `B'…'` and `X'…'` must parse as a single literal atom and round-trip
    /// through the formatter byte-for-byte. The previous behaviour lexed the
    /// prefix as an identifier followed by an ordinary `StringLit`, which the
    /// formatter then separated with a space (`B '10'`). Exact-equality
    /// assertion subsumes the narrower "no inserted space" check and also
    /// catches related unfaithfulness modes (prefix dropped, case-folded,
    /// doubled, etc.).
    #[test]
    fn bit_and_hex_string_literals_round_trip_without_space() {
        use crate::formatter::format_tokens_sql;
        use recursa::PrettyConfig;

        for src in ["B'10'", "X'1FF'", "b'001'", "x'42f'", "B''"] {
            let expr_parsed = parse_expr_classified(src);
            let expr = expr_parsed.ast();
            // Confirm the atom is the dedicated bit/hex variant, not a
            // StringLit / ColumnRef pair.
            assert!(
                matches!(expr, Expr::BitStringLit(_) | Expr::HexStringLit(_)),
                "expected BitStringLit/HexStringLit atom for {src:?}, got {:?}",
                std::mem::discriminant(expr),
            );
            let formatted = format_tokens_sql(expr, PrettyConfig::default());
            assert_eq!(
                formatted.trim(),
                src,
                "non-exact round-trip for {src:?}: {formatted:?}",
            );
        }
    }

    // Added in 17: research, PostgreSQL 17, "Queries and expressions".
    #[cfg(feature = "since-pg17")]
    /// `json '…'` — PostgreSQL's `JsonType` as a function-style typed
    /// literal. `JSON` is a COL_NAME keyword, so it reaches neither the
    /// identifier-named nor the typmod typed-literal form, and it must stay
    /// disjoint from the `JSON ( … )` SQL/JSON value constructor.
    #[test]
    fn parse_json_typed_literal() {
        for src in [
            r#"json '{"a": 1}'"#,
            r#"json '"foo"'"#,
            r#"jsonb 'null'"#,
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::CastFunc(_)),
                "expected a typed literal for {src:?}",
            );
        }
        for src in ["JSON('{}' FORMAT JSON)", "JSON('1'::json WITH UNIQUE KEYS)"] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonCtor(_)),
                "expected the JSON value constructor for {src:?}",
            );
        }
        // The typed literal composes as a JSON_OBJECT key and as an element.
        for src in [
            r#"JSON_OBJECT(json '[1]': 123)"#,
            r#"JSON_ARRAY('aaa', json '{"a": [1]}', jsonb '["a",3]')"#,
            r#"json '{ "a": 1 }' -> 'a'"#,
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let _expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _expr = _expr_parsed.ast();
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

    // Added in 16: research, PostgreSQL 16, "Queries and expressions".
    #[cfg(feature = "since-pg16")]
    /// `JSON_OBJECTAGG(k: v)` and `JSON_OBJECT(k: v)` with an identifier key.
    ///
    /// pg-sql used to model psql's `:name` interpolation in the expression
    /// grammar, where PostgreSQL has none. An identifier followed by a colon
    /// then also read as the typed literal `type_function_name :'var'`, and
    /// that reading swallowed the SQL/JSON key/value separator. psql now has
    /// its own grammar, so a colon here is only ever the separator.
    #[test]
    fn parse_json_object_entry_with_identifier_key() {
        for src in [
            "JSON_OBJECTAGG(k: v)",
            "JSON_OBJECTAGG(i: i RETURNING jsonb)",
            "JSON_OBJECTAGG(k: v ABSENT ON NULL)",
            "JSON_OBJECTAGG(k: v WITH UNIQUE KEYS)",
            "JSON_OBJECTAGG(i: ('111' || i)::bytea FORMAT JSON WITH UNIQUE RETURNING text)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonObjectAgg(_)),
                "expected JsonObjectAgg for {src:?}",
            );
        }
        // The constructor form must read `k: v` as an entry, not as one
        // legacy `json_object(text[])` argument.
        let parsed = parse_expr_classified("JSON_OBJECT(k: v)");
        let Expr::JsonObject(object) = parsed.ast() else {
            panic!("expected the SQL/JSON constructor");
        };
        let JsonObject::Entries(args) = object.as_ref() else {
            panic!("expected the entry form");
        };
        assert!(args.entries.first().value.is_some(), "expected a key/value entry");
        // The typed literal keeps its keyword-named spelling, which is
        // gram.y's `ConstTypename Sconst` and takes a string, never a colon.
        assert!(matches!(
            parse_expr_classified("bigint 'txid'").ast(),
            Expr::CastFunc(_)
        ));
    }

    /// `GROUPING(a, b)` — gram.y's `func_expr_common_subexpr:
    /// GROUPING '(' expr_list ')'`. `GROUPING` is a COL_NAME keyword, so it
    /// can never be an ordinary function name, and it stays usable as a
    /// bare column reference.
    #[test]
    fn parse_grouping_function() {
        for src in ["grouping(a)", "grouping(a, b)", "grouping(v || 'a')"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let expr = expr_parsed.ast();
            assert!(matches!(expr, Expr::Grouping(_)), "expected Grouping for {src:?}");
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
        assert!(matches!(parse_expr_classified("grouping").ast(), Expr::ColumnRef(_)));
    }

    /// `substring(x, 3, 1)` — the ordinary function-call spelling that
    /// gram.y keeps as `SUBSTRING '(' func_arg_list_opt ')'`, alongside the
    /// SQL-standard FROM/FOR and SIMILAR forms.
    #[test]
    fn parse_substring_comma_argument_form() {
        for src in [
            "substring(good, 3, 1)",
            "substring(indtoasttest::text, 1, 200)",
            "substring(VALUE, 1, 1)",
            "SUBSTRING(x FROM 3 FOR 1)",
            "SUBSTRING(x FROM 3)",
            "SUBSTRING(x SIMILAR 'p' ESCAPE '#')",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let expr = expr_parsed.ast();
            assert!(matches!(expr, Expr::Substring(_)), "expected Substring for {src:?}");
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

    /// The legacy ordinary-function spelling of `json_object`
    /// (`JSON_OBJECT '(' func_arg_list ')'` in gram.y) alongside the SQL/JSON
    /// constructor forms, which must keep winning where they apply.
    #[test]
    fn parse_json_object_ordinary_function_form() {
        for src in [
            "json_object('{}')",
            "json_object('{a,b}', '{1,2}')",
            "json_object(array_agg(g))",
            "json_object_keys(json_object(array_agg(g)))",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let _expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _expr = _expr_parsed.ast();
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
        // Added in 16: research, PostgreSQL 16, "Queries and expressions".
        #[cfg(feature = "since-pg16")]
        for src in [
            "JSON_OBJECT('a': 1, 'b': 2)",
            "JSON_OBJECT(KEY 'a' VALUE 2 + 3)",
            "JSON_OBJECT()",
            "JSON_OBJECT(RETURNING jsonb)",
        ] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::JsonObject(_)),
                "expected the SQL/JSON constructor for {src:?}",
            );
        }
    }

    /// `arr[i].field` — PostgreSQL's `opt_indirection` chains subscripts and
    /// attribute names freely, so a `.field` selector must be able to follow
    /// a subscript.
    #[test]
    fn parse_field_selection_after_subscript() {
        for src in [
            "c2[2].f2",
            "d1[1].r + 1",
            "value[1].r",
            "a.b[1].c",
            "d1[1].r.s",
            "d1[1].r[2]",
            "a[1]",
            "a[1][2]",
            "a[1:2]",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let _expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _expr = _expr_parsed.ast();
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

    /// `ARRAY[]` — the empty array constructor (`array_expr: '[' ']'` in
    /// gram.y). It appears bare, cast, as a VARIADIC argument and as a
    /// function-parameter default.
    #[test]
    fn parse_empty_array_constructor() {
        for src in [
            "ARRAY[]",
            "ARRAY[]::int[]",
            "array[]::oidvector",
            "ARRAY[1]",
            "ARRAY[[1, 2], [3, 4]]",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let _expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let _expr = _expr_parsed.ast();
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }

    /// `ROW()` — the empty row constructor (`row: ROW '(' ')'` in gram.y).
    /// It must work bare, as an `IS NULL` operand and on both sides of `=`.
    #[test]
    fn parse_empty_row_constructor() {
        for src in ["ROW()", "ROW(1)", "ROW(1, 2)"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let expr_parsed = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                ;
            let expr = expr_parsed.ast();
            assert!(matches!(expr, Expr::RowExpr(_)), "expected RowExpr for {src:?}");
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
        }
    }
    /// `COALESCE`, `GREATEST`, `LEAST` and `NULLIF` are gram.y
    /// `func_expr_common_subexpr` productions with nodes of their own
    /// (gram.y:15844-15865), not function calls. The differential oracle
    /// compares PostgreSQL's parse of two texts that both say `COALESCE(...)`,
    /// so it cannot see which variant pg-sql built; these tests can.
    #[test]
    fn parse_coalesce_as_its_own_variant() {
        for src in ["COALESCE(a, b)", "coalesce(a)", "Coalesce(a, b + 1, NULL)"] {
            let parsed = parse_expr_classified(src);
            assert!(
                matches!(parsed.ast(), Expr::Coalesce(_)),
                "expected Coalesce for {src:?}, got {:?}",
                parsed.ast(),
            );
        }
        let parsed = parse_expr_classified("COALESCE(a, b, c)");
        let Expr::Coalesce(coalesce) = parsed.ast() else {
            panic!("expected Coalesce, got {:?}", parsed.ast());
        };
        assert_eq!(coalesce.args.len(), 3);
    }

    #[test]
    fn parse_greatest_as_its_own_variant() {
        for src in ["GREATEST(a, b)", "greatest(a)", "greatest(1, 2, 3)"] {
            let parsed = parse_expr_classified(src);
            assert!(
                matches!(parsed.ast(), Expr::Greatest(_)),
                "expected Greatest for {src:?}, got {:?}",
                parsed.ast(),
            );
        }
    }

    #[test]
    fn parse_least_as_its_own_variant() {
        for src in ["LEAST(a, b)", "least(a)", "least(1, 2, 3)"] {
            let parsed = parse_expr_classified(src);
            assert!(
                matches!(parsed.ast(), Expr::Least(_)),
                "expected Least for {src:?}, got {:?}",
                parsed.ast(),
            );
        }
    }

    /// gram.y:15844 `NULLIF '(' a_expr ',' a_expr ')'`: exactly two arguments.
    #[test]
    fn parse_nullif_as_its_own_variant() {
        let parsed = parse_expr_classified("NULLIF(a, b + 1)");
        let Expr::NullIf(nullif) = parsed.ast() else {
            panic!("expected NullIf, got {:?}", parsed.ast());
        };
        assert!(matches!(&*nullif.left, Expr::ColumnRef(_)));
        assert!(matches!(&*nullif.right, Expr::Add(..)));
        for src in ["nullif(a)", "nullif(a, b, c)", "nullif()"] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "NULLIF without exactly two arguments parsed completely: {src:?}",
            );
        }
    }

    /// `expr_list` is not `func_arg_list`: no `*`, no `DISTINCT`, no
    /// `VARIADIC`, no named argument, and never empty. PostgreSQL 17.9's
    /// `raw_parser` rejects every one of these.
    #[test]
    fn reject_function_call_syntax_in_coalesce_greatest_least() {
        for src in [
            "coalesce(*)",
            "coalesce(DISTINCT a)",
            "coalesce(ALL a)",
            "coalesce(VARIADIC a)",
            "coalesce(x => a)",
            "coalesce()",
            "coalesce(a ORDER BY a)",
            "greatest(*)",
            "greatest(DISTINCT a)",
            "greatest()",
            "least(*)",
            "least(DISTINCT a)",
            "least()",
            "coalesce(a) OVER ()",
            "coalesce(a) FILTER (WHERE true)",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "function-call syntax parsed completely in a special form: {src:?}",
            );
        }
    }

    /// The four words are `col_name_keyword` and `bare_label_keyword`
    /// (gram.y:17875-17904, 18120-18323): a column name, a label with or
    /// without `AS`, and an `attr_name` after a dot, but never an unqualified
    /// function name.
    #[test]
    fn parse_coalesce_family_words_as_names() {
        for word in ["coalesce", "greatest", "least", "nullif"] {
            assert!(
                matches!(parse_expr_classified(word).ast(), Expr::ColumnRef(_)),
                "expected ColumnRef for {word:?}",
            );
        }
        for src in [
            "SELECT coalesce FROM t",
            "SELECT nullif, greatest, least FROM t",
            "SELECT 1 AS coalesce",
            "SELECT 1 coalesce",
            "SELECT t.coalesce FROM t",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = crate::ast::Statement::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert!(input.is_eof(), "parser cursor for {src:?}: {}", input.cursor());
            let tree = format!("{:?}", parsed.ast());
            assert!(!tree.contains("CoalesceExpr"), "{src:?} built the special form");
        }
    }

    /// `pg_catalog.coalesce(a, b)` is not the special form. gram.y's
    /// `func_name: ColId indirection` takes `coalesce` as an `attr_name`
    /// (`ColLabel`), so PostgreSQL's raw parser accepts it as an ordinary
    /// function call and fails later, in parse analysis, because no such
    /// function exists.
    #[test]
    fn parse_qualified_coalesce_as_an_ordinary_call() {
        let parsed = parse_expr_classified("pg_catalog.coalesce(a, b)");
        let Expr::QualRef(qualified) = parsed.ast() else {
            panic!("expected QualRef, got {:?}", parsed.ast());
        };
        assert!(qualified.call.is_some());
    }

    /// gram.y:14844 `a_expr qual_Op a_expr %prec Op` with `qual_Op:
    /// OPERATOR '(' any_operator ')'` (gram.y:16494). The level is `Op`
    /// whatever operator the parentheses name.
    #[test]
    fn parse_decorated_infix_operator_at_op_precedence() {
        use crate::ast::shared::names::QualifiedOperatorName;

        // `(1 OPERATOR(pg_catalog.=) 2) = 3`: a decorated `=` is above `=`.
        let parsed = parse_expr_classified("1 OPERATOR(pg_catalog.=) 2 = 3");
        let Expr::Eq(left, right) = parsed.ast() else {
            panic!("expected Eq at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**right, Expr::IntegerLit(_)));
        let Expr::DecoratedInfix(one, operator, two) = &**left else {
            panic!("expected DecoratedInfix on the left, got {left:?}");
        };
        assert!(matches!(&**one, Expr::IntegerLit(_)));
        assert!(matches!(&**two, Expr::IntegerLit(_)));
        assert!(matches!(operator.name, QualifiedOperatorName::Qualified(_)));

        // `a OPERATOR(pg_catalog.+) (b * c)`: a decorated `+` is below `*`.
        let parsed = parse_expr_classified("a OPERATOR(pg_catalog.+) b * c");
        let Expr::DecoratedInfix(left, _, right) = parsed.ast() else {
            panic!("expected DecoratedInfix at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**left, Expr::ColumnRef(_)));
        assert!(matches!(&**right, Expr::Mul(..)));

        // gram.y:889 `%left Op OPERATOR`.
        let parsed = parse_expr_classified("a OPERATOR(+) b OPERATOR(-) c");
        let Expr::DecoratedInfix(left, operator, right) = parsed.ast() else {
            panic!("expected DecoratedInfix at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**left, Expr::DecoratedInfix(..)));
        assert!(matches!(&**right, Expr::ColumnRef(_)));
        assert!(matches!(operator.name, QualifiedOperatorName::Plain(_)));
    }

    /// `any_operator` is `all_Op` behind any number of `ColId '.'` parts
    /// (gram.y:9004), and `all_Op` is `Op | MathOp`: every `MathOp` spelling
    /// (gram.y:16478) must be a decorated operator, as must an `Op`.
    #[test]
    fn parse_decorated_operator_with_every_math_op() {
        for op in [
            "+", "-", "*", "/", "%", "^", "<", ">", "=", "<=", ">=", "<>", "||", "@>", "~~", "<->",
            "!=", "&&&",
        ] {
            for path in ["", "pg_catalog.", "a.b."] {
                let src: &'static str = format!("x OPERATOR({path}{op}) y").leak();
                assert!(
                    matches!(parse_expr_classified(src).ast(), Expr::DecoratedInfix(..)),
                    "expected DecoratedInfix for {src:?}",
                );
            }
        }
    }

    /// gram.y:14846 `qual_Op a_expr %prec Op`.
    #[test]
    fn parse_decorated_prefix_operator_at_op_precedence() {
        let parsed = parse_expr_classified("OPERATOR(pg_catalog.-) a * b");
        let Expr::DecoratedPrefix(_, operand) = parsed.ast() else {
            panic!("expected DecoratedPrefix at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**operand, Expr::Mul(..)));

        let parsed = parse_expr_classified("OPERATOR(pg_catalog.-) a = b");
        let Expr::Eq(left, _) = parsed.ast() else {
            panic!("expected Eq at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**left, Expr::DecoratedPrefix(..)));

        let parsed = parse_expr_classified("a OPERATOR(pg_catalog.+) OPERATOR(pg_catalog.-) b");
        let Expr::DecoratedInfix(_, _, right) = parsed.ast() else {
            panic!("expected DecoratedInfix at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**right, Expr::DecoratedPrefix(..)));
    }

    /// gram.y:15322-15324 `b_expr qual_Op b_expr` and `qual_Op b_expr`: both
    /// forms are in `b_expr`, so they stand as the low bound of `BETWEEN`.
    /// The quantified form keeps its own production (gram.y `subquery_Op`).
    #[test]
    fn parse_decorated_operator_in_b_expr_and_beside_quantified_form() {
        let parsed = parse_expr_classified("x BETWEEN 1 OPERATOR(pg_catalog.+) 2 AND 5");
        let Expr::BetweenExpr(_, low, _) = parsed.ast() else {
            panic!("expected BetweenExpr, got {:?}", parsed.ast());
        };
        assert!(matches!(&*low.low, Expr::DecoratedInfix(..)));

        let parsed = parse_expr_classified("x BETWEEN OPERATOR(pg_catalog.-) 2 AND 5");
        let Expr::BetweenExpr(_, low, _) = parsed.ast() else {
            panic!("expected BetweenExpr, got {:?}", parsed.ast());
        };
        assert!(matches!(&*low.low, Expr::DecoratedPrefix(..)));

        assert!(matches!(
            parse_expr_classified("a OPERATOR(pg_catalog.=) ANY (ARRAY[1])").ast(),
            Expr::QuantifiedComparisonOp(..)
                | Expr::QuantifiedComparisonCmp(..)
                | Expr::QuantifiedComparisonAdd(..)
        ));

        // `=>` is not an operator name, and the parentheses are not optional.
        for src in ["a OPERATOR(pg_catalog.=>) b", "a OPERATOR pg_catalog.= b", "a OPERATOR() b"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                lexed.errors().count() > 0 || parsed.is_err() || !input.is_eof(),
                "invalid decorated operator parsed completely: {src:?}",
            );
        }
    }

    /// gram.y:15874 `XMLCONCAT '(' expr_list ')'` is PostgreSQL's `XmlExpr`
    /// with `IS_XMLCONCAT`, not a function call, and `XMLCONCAT` is a
    /// `col_name_keyword` (gram.y:17922). The differential oracle cannot see
    /// which variant pg-sql built.
    #[test]
    fn parse_xmlconcat_as_its_own_variant() {
        for src in ["XMLCONCAT(a, b)", "xmlconcat(a)", "xmlconcat('<a/>', NULL, x || y)"] {
            let parsed = parse_expr_classified(src);
            assert!(
                matches!(parsed.ast(), Expr::XmlConcat(_)),
                "expected XmlConcat for {src:?}, got {:?}",
                parsed.ast(),
            );
        }
        let parsed = parse_expr_classified("xmlconcat(a, b, c)");
        let Expr::XmlConcat(concat) = parsed.ast() else {
            panic!("expected XmlConcat, got {:?}", parsed.ast());
        };
        assert_eq!(concat.args.len(), 3);
        assert!(matches!(parse_expr_classified("xmlconcat").ast(), Expr::ColumnRef(_)));
        for src in [
            "xmlconcat()",
            "xmlconcat(*)",
            "xmlconcat(DISTINCT a)",
            "xmlconcat(VARIADIC a)",
            "xmlconcat(x => a)",
            "xmlconcat(a) OVER ()",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "function-call syntax parsed completely in XMLCONCAT: {src:?}",
            );
        }
    }

    /// gram.y:15730 `NORMALIZE '(' a_expr ')'` and gram.y:15737 `NORMALIZE '('
    /// a_expr ',' unicode_normal_form ')'`. PostgreSQL turns the form into a
    /// string constant, so it must never be a column reference here.
    #[test]
    fn parse_normalize_as_its_own_variant() {
        use crate::ast::shared::expr::UnicodeNormalForm;

        let parsed = parse_expr_classified("NORMALIZE(a || b)");
        let Expr::Normalize(normalize) = parsed.ast() else {
            panic!("expected Normalize, got {:?}", parsed.ast());
        };
        assert!(matches!(&*normalize.arg, Expr::Concat(..)));
        assert!(normalize.form.is_none());

        fn form_name(form: &UnicodeNormalForm) -> &'static str {
            match form {
                UnicodeNormalForm::Nfc => "NFC",
                UnicodeNormalForm::Nfd => "NFD",
                UnicodeNormalForm::Nfkc => "NFKC",
                UnicodeNormalForm::Nfkd => "NFKD",
            }
        }
        for (src, expected) in [
            ("normalize(a, NFC)", "NFC"),
            ("normalize(a, nfd)", "NFD"),
            ("normalize(a, NFKC)", "NFKC"),
            ("normalize(a, Nfkd)", "NFKD"),
        ] {
            let parsed = parse_expr_classified(src);
            let Expr::Normalize(normalize) = parsed.ast() else {
                panic!("expected Normalize for {src:?}, got {:?}", parsed.ast());
            };
            assert!(matches!(&*normalize.arg, Expr::ColumnRef(_)), "{src:?}");
            assert_eq!(normalize.form.as_ref().map(form_name), Some(expected), "{src:?}");
        }
        assert!(matches!(parse_expr_classified("normalize").ast(), Expr::ColumnRef(_)));
        // `nfc` alone is an unreserved keyword: a column like any other.
        assert!(matches!(parse_expr_classified("nfc").ast(), Expr::ColumnRef(_)));
        for src in [
            "normalize()",
            "normalize(a, b)",
            "normalize(a, 'NFC')",
            "normalize(a, NFC, NFD)",
            "normalize(NFC, a)",
            "normalize(*)",
            "normalize(DISTINCT a)",
            "normalize(a) OVER ()",
        ] {
            let lexed = crate::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                parsed.is_err() || !input.is_eof(),
                "invalid NORMALIZE parsed completely: {src:?}",
            );
        }
    }

    /// As for `coalesce`: after a dot the word is an `attr_name`
    /// (`ColLabel`), so PostgreSQL's raw parser takes `pg_catalog.xmlconcat(a,
    /// b)` and `pg_catalog.normalize(a)` as ordinary calls. Neither is the
    /// special form, and in the second one `NFC` is an ordinary argument.
    #[test]
    fn parse_qualified_xmlconcat_and_normalize_as_ordinary_calls() {
        for src in [
            "pg_catalog.xmlconcat(a, b)",
            "pg_catalog.normalize(a)",
            "pg_catalog.normalize(a, nfc)",
        ] {
            let parsed = parse_expr_classified(src);
            let Expr::QualRef(qualified) = parsed.ast() else {
                panic!("expected QualRef for {src:?}, got {:?}", parsed.ast());
            };
            assert!(qualified.call.is_some(), "{src:?}");
        }
    }

    /// gram.y has `BETWEEN opt_asymmetric`, `NOT_LA BETWEEN opt_asymmetric`,
    /// `BETWEEN SYMMETRIC` and `NOT_LA BETWEEN SYMMETRIC` (gram.y:15076-15100).
    /// `SYMMETRIC` and `ASYMMETRIC` are reserved words; pg-sql used to read
    /// `SYMMETRIC '1997-01-01'` as a typed literal of a type named `symmetric`.
    #[test]
    fn parse_between_symmetric_and_asymmetric() {
        use crate::ast::shared::expr::BetweenModifier;

        for (src, negated, modifier) in [
            ("f1 BETWEEN '1997-01-01' AND '1998-01-01'", false, None),
            ("f1 BETWEEN SYMMETRIC '1997-01-01' AND '1998-01-01'", false, Some(true)),
            ("f1 BETWEEN ASYMMETRIC '1997-01-01' AND '1998-01-01'", false, Some(false)),
            ("f1 NOT BETWEEN '1997-01-01' AND '1998-01-01'", true, None),
            ("f1 NOT BETWEEN SYMMETRIC '1997-01-01' AND '1998-01-01'", true, Some(true)),
            ("f1 not between asymmetric 1 and 2", true, Some(false)),
        ] {
            let parsed = parse_expr_classified(src);
            let (low, is_negated) = match parsed.ast() {
                Expr::BetweenExpr(_, low, _) => (low, false),
                Expr::NotBetweenExpr(_, low, _) => (low, true),
                other => panic!("expected a BETWEEN for {src:?}, got {other:?}"),
            };
            assert_eq!(is_negated, negated, "{src:?}");
            assert_eq!(
                low.modifier
                    .as_ref()
                    .map(|modifier| matches!(modifier, BetweenModifier::Symmetric)),
                modifier,
                "{src:?}"
            );
            // The low bound is the literal, not a typed literal that ate the word.
            assert!(
                matches!(&*low.low, Expr::StringLit(_) | Expr::IntegerLit(_)),
                "{src:?}: {:?}",
                low.low
            );
        }

        // The low bound stays a `b_expr`, and the level stays `BETWEEN`'s.
        let parsed = parse_expr_classified("a BETWEEN SYMMETRIC b + 1 AND c AND d");
        let Expr::And(left, _) = parsed.ast() else {
            panic!("expected And at the root, got {:?}", parsed.ast());
        };
        assert!(matches!(&**left, Expr::BetweenExpr(..)));

        // Reserved: neither word is a column, a type or a function name.
        for src in [
            "symmetric",
            "asymmetric",
            "symmetric '1'",
            "a BETWEEN SYMMETRIC ASYMMETRIC 1 AND 2",
            "a BETWEEN SYMMETRIC SYMMETRIC 1 AND 2",
            "a BETWEEN SYMMETRIC AND 2",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
        // Both are bare labels.
        for src in ["SELECT 1 symmetric", "SELECT 1 AS asymmetric"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            crate::ast::Statement::parse(&mut input).unwrap_or_else(|e| panic!("{src:?}: {e}"));
            assert!(input.is_eof(), "{src:?}");
        }
    }

    /// gram.y:14783 `a_expr COLLATE any_name`: the collation is an `any_name`,
    /// so it may be schema-qualified. `CAST '(' a_expr AS Typename ')'` has no
    /// collation at all; PostgreSQL 17.9 rejects `CAST(x AS text COLLATE "C")`
    /// (collate.sql expects that error).
    #[test]
    fn parse_collate_with_a_qualified_name() {
        for (src, parts) in [
            ("x COLLATE \"C\"", 1),
            ("x COLLATE pg_catalog.\"C\"", 2),
            ("x COLLATE a.b.\"C\"", 3),
            ("x COLLATE pg_catalog.default", 2),
            ("x COLLATE value", 1),
        ] {
            let parsed = parse_expr_classified(src);
            let Expr::Collate(operand, name) = parsed.ast() else {
                panic!("expected Collate for {src:?}, got {:?}", parsed.ast());
            };
            assert!(matches!(&**operand, Expr::ColumnRef(_)), "{src:?}");
            assert_eq!(1 + name.rest.len(), parts, "{src:?}");
        }
        // `COLLATE` binds tighter than `||` and looser than `::`.
        let parsed = parse_expr_classified("x::text COLLATE pg_catalog.\"C\" || y");
        let Expr::Concat(left, _) = parsed.ast() else {
            panic!("expected Concat at the root, got {:?}", parsed.ast());
        };
        let Expr::Collate(operand, _) = &**left else {
            panic!("expected Collate on the left, got {left:?}");
        };
        assert!(matches!(&**operand, Expr::Cast(..)));

        for src in [
            "CAST(x AS text COLLATE \"C\")",
            "CAST(x AS text COLLATE pg_catalog.\"C\")",
            "x COLLATE",
            "x COLLATE pg_catalog.",
            "x COLLATE 'C'",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(
                lexed.errors().count() > 0 || parsed.is_err() || !input.is_eof(),
                "{src:?} parsed completely"
            );
        }
        assert!(matches!(
            parse_expr_classified("CAST(x AS text) COLLATE \"C\"").ast(),
            Expr::Collate(..)
        ));
    }

    /// gram.y:15797 `TREAT '(' a_expr AS Typename ')'`. `TREAT` is a
    /// `col_name_keyword`, so a bare `treat` is a column and `treat(...)` is
    /// only ever this form.
    #[test]
    fn parse_treat_as_its_own_variant() {
        for src in ["TREAT(a AS int)", "treat(a + 1 AS s.t)", "treat(a AS numeric(10, 2))"] {
            assert!(
                matches!(parse_expr_classified(src).ast(), Expr::Treat(_)),
                "expected Treat for {src:?}",
            );
        }
        assert!(matches!(parse_expr_classified("treat").ast(), Expr::ColumnRef(_)));
        for src in ["treat(a, b)", "treat(a)", "treat(a AS int COLLATE \"C\")", "treat()"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    // Before 17, `json` is an unreserved keyword and has no `JsonType`: research, PostgreSQL 17,
    // "Keywords" and "Queries and expressions". So `json(...)` is an ordinary
    // call, and the type `json` takes type modifiers like any `GenericType`.
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn json_is_an_unreserved_keyword_before_17() {
        for src in [
            "json('{}')",
            "json(x, y)",
            "json(*)",
            "json(DISTINCT a)",
            "json(a ORDER BY b)",
            "json()",
            "json(a) FILTER (WHERE a > 1) OVER ()",
        ] {
            assert!(matches!(parse_expr_classified(src).ast(), Expr::Func(_)), "{src}");
        }
        for src in ["json '{}'", "json(2) '{}'"] {
            let tree = format!("{:?}", parse_expr_classified(src).ast());
            assert!(!tree.contains("JsonCtor"), "{src}: {tree}");
        }
        for src in ["'1'::json(2)", "CAST('1' AS json(2))", "'1'::json.x"] {
            assert!(matches!(parse_expr_classified(src).ast(), Expr::Cast(..) | Expr::CastCall(_)), "{src}");
        }
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT public.json(1)",
            "CREATE FUNCTION json(int) RETURNS int LANGUAGE sql AS 'select 1'",
            "DROP FUNCTION json(int)",
            "CREATE TABLE t (a json(10))",
        ]);
        // `JSON_OBJECT(...)` is added in 16: research, PostgreSQL 16, "Queries
        // and expressions".
        #[cfg(feature = "since-pg16")]
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT JSON_OBJECT('a': 1 RETURNING json(3))",
        ]);
    }

    // Before 17, the SQL/JSON function names of 17 are identifiers: research, PostgreSQL 17,
    // "Keywords". A call with any argument list is an ordinary call.
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn sql_json_17_names_are_ordinary_calls_before_17() {
        for src in [
            "json_scalar(1, 2)",
            "json_serialize(a, b)",
            "json_query(j)",
            "json_value(j, '$.n', 3)",
            "json_exists(j)",
            "json_table(1, 2)",
            "merge_action(1)",
        ] {
            assert!(matches!(parse_expr_classified(src).ast(), Expr::Func(_)), "{src}");
        }
        crate::ast::test_support::assert_statements_parse(&[
            "CREATE FUNCTION json_exists(int) RETURNS int LANGUAGE sql AS 'select 1'",
            "CREATE FUNCTION json_table() RETURNS int LANGUAGE sql AS 'select 1'",
            "CREATE TABLE t (a json_value)",
            "SELECT '1'::json_query",
        ]);
    }

    // Added in 17, so rejected before 17: research, PostgreSQL 17, "Queries and expressions".
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn sql_json_17_syntax_is_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "SELECT JSON('{}' WITH UNIQUE KEYS)",
            "SELECT JSON('{}' FORMAT JSON)",
            "SELECT JSON_SERIALIZE('{}' RETURNING bytea)",
            "SELECT JSON_SERIALIZE('{}' FORMAT JSON)",
            "SELECT JSON_QUERY(j, '$.a' WITH CONDITIONAL WRAPPER OMIT QUOTES) FROM t",
            "SELECT JSON_QUERY(j, '$.a' EMPTY ARRAY ON EMPTY) FROM t",
            "SELECT JSON_VALUE(j, '$.n' RETURNING int DEFAULT -1 ON ERROR) FROM t",
            "SELECT JSON_EXISTS(j, '$.a ? (@ > $x)' PASSING 1 AS x) FROM t",
            "SELECT JSON_EXISTS(j, '$.a' FALSE ON ERROR) FROM t",
        ]);
    }

    // `AT LOCAL` is added in 17, so rejected before 17: research, PostgreSQL 17, "Queries and
    // expressions" (REL_17_11 gram.y 14798).
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn at_local_is_rejected_before_17() {
        crate::ast::test_support::assert_statements_rejected(&[
            "SELECT now() AT LOCAL",
            "SELECT a AT LOCAL + 1 FROM t",
        ]);
        crate::ast::test_support::assert_statements_parse(&["SELECT now() AT TIME ZONE 'UTC'", "SELECT 1 local"]);
    }

    // The words that 17 makes keywords are identifiers before 17: research, PostgreSQL 17,
    // "Keywords". gram.y has positions that admit an `IDENT` and no keyword.
    #[cfg(not(feature = "since-pg17"))]
    #[test]
    fn words_that_17_reserves_are_identifiers_before_17() {
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT 1 AS error, 2 empty, 3 keep, 4 omit, 5 quotes, 6 string, 7 conditional, \
             8 unconditional, 9 source, 10 target",
            "CREATE TABLE source (target int, error text, string text)",
            "CREATE AGGREGATE a (basetype = int, sfunc = f, stype = int, error = 1)",
            "SELECT EXTRACT(error FROM ts) FROM t",
        ]);
    }
    /// Added in 19: gram.y `func_expr: func_application within_group_clause
    /// filter_clause null_treatment over_clause` (b73d13c:16017,
    /// `null_treatment` 16668; research PostgreSQL 19, "Queries and
    /// expressions", commit 25a30bbd4). The grammar takes it after any
    /// function application, also with no `OVER`.
    #[cfg(feature = "since-pg19")]
    #[test]
    fn parse_null_treatment_between_filter_and_over() {
        use crate::ast::shared::expr::{FunctionCallSuffix, NullTreatment};
        for (src, ignore) in [
            ("lag(x) IGNORE NULLS OVER w", true),
            ("lag(x) RESPECT NULLS OVER (ORDER BY y)", false),
            ("count(*) FILTER (WHERE x > 1) IGNORE NULLS OVER ()", true),
            ("f(x) respect nulls", false),
            ("percentile_cont(0.5) WITHIN GROUP (ORDER BY x) IGNORE NULLS", true),
        ] {
            let parsed = parse_expr_classified(src);
            let Expr::Func(call) = parsed.ast() else {
                panic!("expected a function call for {src:?}");
            };
            let FunctionCallTail::Call(FunctionCallSuffix {
                null_treatment: Some(treatment),
                ..
            }) = &call.tail
            else {
                panic!("expected a null treatment for {src:?}");
            };
            assert_eq!(matches!(treatment, NullTreatment::Ignore), ignore, "{src:?}");
        }
        for src in [
            "lag(x) IGNORE OVER ()",
            "lag(x) NULLS OVER ()",
            "lag(x) OVER () IGNORE NULLS",
            "lag(x) IGNORE NULLS RESPECT NULLS",
            "lag(x) IGNORE NULLS FILTER (WHERE true)",
        ] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }

    /// Before 19 there is no `null_treatment` (the same research entry).
    #[cfg(not(feature = "since-pg19"))]
    #[test]
    fn reject_null_treatment_before_19() {
        for src in ["lag(x) IGNORE NULLS OVER w", "lag(x) RESPECT NULLS"] {
            let lexed = crate::lex(src);
            let mut input = lexed.input();
            let parsed = Expr::parse(&mut input);
            assert!(parsed.is_err() || !input.is_eof(), "{src:?} parsed completely");
        }
    }


    // Added in 16: research, PostgreSQL 16, "Queries and expressions"
    // (REL_16_15 gram.y `xml_indent_option`).
    #[cfg(feature = "since-pg16")]
    #[test]
    fn parse_xmlserialize_indent() {
        for src in [
            "xmlserialize(CONTENT x AS text NO INDENT)",
            "XMLSERIALIZE(DOCUMENT x AS text INDENT)",
        ] {
            assert!(matches!(parse_expr_classified(src).ast(), Expr::XmlSerialize(_)), "{src}");
        }
    }

    // Added in 16, so rejected before 16: research, PostgreSQL 16, "Queries and
    // expressions". REL_15_19 gram.y has `XMLSERIALIZE '(' document_or_content
    // a_expr AS SimpleTypename ')'`.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn xmlserialize_indent_is_rejected_before_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "SELECT XMLSERIALIZE(DOCUMENT x AS text INDENT)",
            "SELECT XMLSERIALIZE(CONTENT x AS text NO INDENT)",
        ]);
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT XMLSERIALIZE(DOCUMENT x AS text)",
            "SELECT xmlserialize(content x AS text) indent",
        ]);
    }

    // Before 16, `json_object`, `json_array`, `json_objectagg` and
    // `json_arrayagg` are not keywords (research, PostgreSQL 16, "Keywords";
    // REL_15_19 kwlist.h). So each call is an ordinary function call, and each
    // word is an ordinary name.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn sql_json_16_names_are_ordinary_calls_before_16() {
        for src in [
            "json_object('{a,b}')",
            "json_object(VARIADIC ARRAY['a'])",
            "json_object(a => 1)",
            "JSON_OBJECT()",
            "json_array(1, 2)",
            "JSON_ARRAY()",
            "json_arrayagg(a ORDER BY a)",
            "json_arrayagg(a) FILTER (WHERE a > 0)",
            "json_objectagg(a, b) OVER ()",
        ] {
            assert!(matches!(parse_expr_classified(src).ast(), Expr::Func(_)), "{src}");
        }
        crate::ast::test_support::assert_statements_parse(&[
            "CREATE TABLE json_object (json_array int, json_arrayagg int, json_objectagg int)",
            "SELECT 1::json_object",
            "SELECT * FROM json_arrayagg",
            "CREATE FUNCTION json_array() RETURNS int LANGUAGE sql AS 'select 1'",
        ]);
    }

    // Added in 16, so rejected before 16: research, PostgreSQL 16, "Queries and
    // expressions". REL_15_19 gram.y has no SQL/JSON production.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn sql_json_16_syntax_is_rejected_before_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "SELECT JSON_OBJECT('a' : 1)",
            "SELECT JSON_OBJECT('a' VALUE 1)",
            "SELECT JSON_OBJECT(KEY 'a' VALUE 1)",
            "SELECT JSON_OBJECT('a' : 1 ABSENT ON NULL WITH UNIQUE KEYS)",
            "SELECT JSON_OBJECT(RETURNING jsonb)",
            "SELECT JSON_ARRAY(1 ABSENT ON NULL)",
            "SELECT JSON_ARRAY(SELECT 1)",
            "SELECT JSON_ARRAY('[1]' FORMAT JSON)",
            "SELECT JSON_ARRAY(RETURNING jsonb)",
            "SELECT JSON_OBJECTAGG(a : b) FROM t",
            "SELECT JSON_OBJECTAGG(a VALUE b) FROM t",
            "SELECT JSON_ARRAYAGG(a NULL ON NULL) FROM t",
            "SELECT JSON_ARRAYAGG(a RETURNING jsonb) FROM t",
            "SELECT '{}' IS JSON",
            "SELECT '{}' IS NOT JSON",
            "SELECT '{}' IS JSON OBJECT",
            "SELECT '{}' IS JSON SCALAR",
            "SELECT '{}' IS JSON WITH UNIQUE KEYS",
        ]);
    }

    // The keywords that 16 added are identifiers before 16: research,
    // PostgreSQL 16, "Keywords" (REL_15_19 kwlist.h). So each is a name where
    // gram.y takes only `IDENT`: `old_aggr_elem` and `createdb_opt_name`.
    // `system_user` is an identifier in every build.
    #[cfg(not(feature = "since-pg16"))]
    #[test]
    fn words_that_16_adds_are_identifiers_before_16() {
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT absent, indent, format, json, keys, scalar FROM t",
            "SELECT 1 absent, 2 indent, 3 format, 4 json, 5 keys, 6 scalar",
            "CREATE TABLE absent (indent int, format int, json int, keys int, scalar int)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, absent = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, indent = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, format = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, json = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, keys = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, scalar = 1)",
            "CREATE DATABASE d absent = 1",
            "CREATE DATABASE d indent = 1",
            "CREATE DATABASE d format = 1",
            "CREATE DATABASE d json = 1",
            "CREATE DATABASE d keys = 1",
            "CREATE DATABASE d scalar = 1",
            "SELECT format json FROM t",
            "SELECT system_user FROM t",
            "CREATE TABLE system_user (a int)",
            "SELECT system_user()",
        ]);
    }

    // From 16, the 16 keywords are no `IDENT`, so they are not names in the
    // positions of `words_that_16_adds_are_identifiers_before_16` (REL_16_15
    // gram.y `old_aggr_elem`, `createdb_opt_name`).
    #[cfg(feature = "since-pg16")]
    #[test]
    fn words_that_16_adds_are_no_ident_from_16() {
        crate::ast::test_support::assert_statements_rejected(&[
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, format = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, json = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, keys = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, scalar = 1)",
            "CREATE DATABASE d format = 1",
            "CREATE DATABASE d json = 1",
            "CREATE DATABASE d keys = 1",
            "CREATE DATABASE d scalar = 1",
        ]);
    }

    // The keywords that 15 added are identifiers before 15: research,
    // PostgreSQL 15, "Keywords" (REL_14_24 kwlist.h has no `merge`, `matched`
    // or `parameter`). So each is a name where gram.y takes only `IDENT`.
    #[cfg(not(feature = "since-pg15"))]
    #[test]
    fn words_that_15_adds_are_identifiers_before_15() {
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT merge, matched, parameter FROM t",
            "SELECT 1 merge, 2 matched, 3 parameter",
            "CREATE TABLE merge (matched int, parameter int)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, merge = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, matched = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, parameter = 1)",
            "CREATE DATABASE d merge = 1",
            "CREATE DATABASE d matched = 1",
            "CREATE DATABASE d parameter = 1",
            "SELECT EXTRACT(merge FROM x)",
        ]);
    }

    // From 15, the 15 keywords are unreserved keywords: still column names
    // and labels, but no `IDENT` (REL_15_19 gram.y `old_aggr_elem`,
    // `createdb_opt_name`).
    #[cfg(feature = "since-pg15")]
    #[test]
    fn words_that_15_adds_are_no_ident_from_15() {
        crate::ast::test_support::assert_statements_parse(&[
            "SELECT merge, matched, parameter FROM t",
            "SELECT 1 merge, 2 matched, 3 parameter",
            "CREATE TABLE merge (matched int, parameter int)",
        ]);
        crate::ast::test_support::assert_statements_rejected(&[
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, merge = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, matched = 1)",
            "CREATE AGGREGATE agg (basetype = int, sfunc = f, stype = int, parameter = 1)",
            "CREATE DATABASE d merge = 1",
            "CREATE DATABASE d matched = 1",
            "CREATE DATABASE d parameter = 1",
        ]);
    }
}

