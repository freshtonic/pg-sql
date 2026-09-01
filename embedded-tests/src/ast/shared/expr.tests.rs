#[cfg(test)]
mod tests {
    use crate::ast::shared::expr::{
        CastType, CastTypeHead, DirectSubquery, Expr, FunctionCallBody, FunctionCallTail,
        ParenContent, ParenthesizedDotStar, ParenthesizedExpr, ParenthesizedIndirection, TypeName,
    };

    /// Parse `src` as an `Expr` through the logos lex pass.
    ///
    /// Takes `&'static str` because the returned `Expr` borrows lexical text
    /// from the source for that `'static` lifetime.
    fn parse_expr_classified(src: &'static str) -> Expr<'static> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse {src:?}: {error:?}"))
            .into_ast();
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        expr
    }

    fn parse_type_name_classified(src: &'static str) -> TypeName<'static> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
        let mut input = lexed.input();
        let ty = TypeName::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse type name {src:?}: {error}"))
            .into_ast();
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        ty
    }

    fn parse_cast_type_classified(src: &'static str) -> CastType<'static> {
        let lexed = crate::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in {src:?}");
        let mut input = lexed.input();
        let ty = CastType::parse(&mut input)
            .unwrap_or_else(|error| panic!("parse cast type {src:?}: {error}"))
            .into_ast();
        assert!(
            input.is_eof(),
            "parser cursor after {src:?}: {}",
            input.cursor()
        );
        ty
    }

    #[test]
    fn parse_json_timestamp_cast_before_unique_keys() {
        assert!(matches!(
            parse_expr_classified("JSON('2000-01-01'::timestamp WITH UNIQUE KEYS)"),
            Expr::JsonCtor(_)
        ));
    }

    #[test]
    fn parse_json_constructors() {
        // JSON()
        assert!(matches!(
            parse_expr_classified("JSON('{}' FORMAT JSON)"),
            Expr::JsonCtor(_)
        ));
        assert!(matches!(
            parse_expr_classified("JSON('1'::json WITH UNIQUE KEYS)"),
            Expr::JsonCtor(_)
        ));
        // JSON_SCALAR()
        assert!(matches!(
            parse_expr_classified("JSON_SCALAR('123')"),
            Expr::JsonScalar(_)
        ));
        // JSON_SERIALIZE()
        assert!(matches!(
            parse_expr_classified("JSON_SERIALIZE('{}' RETURNING bytea)"),
            Expr::JsonSerialize(_)
        ));
        // JSON_OBJECT() — entries, KEY/VALUE, all clauses, empty, returning-only
        for src in [
            "JSON_OBJECT('a': 1, 'b': 2)",
            "JSON_OBJECT(KEY 'a' VALUE 2 + 3)",
            "JSON_OBJECT('a': 1 ABSENT ON NULL WITH UNIQUE RETURNING jsonb)",
            "JSON_OBJECT()",
            "JSON_OBJECT(RETURNING jsonb)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonObject(_)),
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
                matches!(parse_expr_classified(src), Expr::JsonArray(_)),
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
            parse_expr_classified("json_build_array(1, 2)"),
            Expr::Func(_)
        ));
    }

    #[test]
    fn parse_json_query_functions() {
        // JSON_EXISTS — path, PASSING, ON ERROR.
        for src in [
            "JSON_EXISTS(jsonb '1', '$.a')",
            "JSON_EXISTS(js, '$.a' ERROR ON ERROR)",
            "JSON_EXISTS(js, '$ ? (@ > $x)' PASSING 1 AS x, 2 AS y)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonExists(_)),
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
                matches!(parse_expr_classified(src), Expr::JsonValue(_)),
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
                matches!(parse_expr_classified(src), Expr::JsonQuery(_)),
                "{src}"
            );
        }
        // The result is an ordinary expression operand.
        assert!(matches!(
            parse_expr_classified("JSON_VALUE(js, '$' RETURNING int) + 234"),
            Expr::Add(..)
        ));
    }

    #[test]
    fn parse_json_aggregates() {
        for src in [
            "JSON_OBJECTAGG('b': 1 RETURNING text)",
            "JSON_OBJECTAGG(k VALUE v ABSENT ON NULL WITH UNIQUE)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonObjectAgg(_)),
                "{src}"
            );
        }
        for src in [
            "JSON_ARRAYAGG(i)",
            "JSON_ARRAYAGG(i ORDER BY i DESC RETURNING jsonb)",
            "JSON_ARRAYAGG(bar) FILTER (WHERE bar > 2) OVER (PARTITION BY x)",
        ] {
            assert!(
                matches!(parse_expr_classified(src), Expr::JsonArrayAgg(_)),
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
                matches!(parse_expr_classified(src), Expr::Array(_)),
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
            ),
            Expr::Overlaps(..)
        ));
    }

    #[test]
    fn parse_xml_functions() {
        assert!(matches!(
            parse_expr_classified("xmlserialize(CONTENT x AS text NO INDENT)"),
            Expr::XmlSerialize(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlparse(DOCUMENT '<foo/>')"),
            Expr::XmlParse(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlroot(x, VERSION NO VALUE, STANDALONE YES)"),
            Expr::XmlRoot(_)
        ));
        assert!(matches!(
            parse_expr_classified("xmlexists('/a' PASSING BY REF doc BY REF)"),
            Expr::XmlExists(_)
        ));
        assert!(matches!(
            parse_expr_classified("x IS DOCUMENT"),
            Expr::IsDocument(..)
        ));
        assert!(matches!(
            parse_expr_classified("x IS NOT DOCUMENT"),
            Expr::IsDocument(..)
        ));
    }

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
                matches!(parse_expr_classified(src), Expr::IsJson(..)),
                "{src}"
            );
        }
        // `IS NULL` still resolves to the boolean test, not `IS JSON`.
        assert!(matches!(
            parse_expr_classified("js IS NULL"),
            Expr::BoolTest(..)
        ));
    }

    // --- Atom tests ---

    #[test]
    fn parse_integer_literal() {
        let lexed = crate::lex("42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
            parse_expr_classified("a ^@ b"),
            Expr::StartsWith(..)
        ));
    }

    /// `#-` (jsonb delete-path) is a single PostgreSQL operator token. With the
    /// classifier active it must NOT split into `Pound` + `Minus`.
    #[test]
    fn parse_json_delete_path_operator_classified() {
        assert!(matches!(
            parse_expr_classified("a #- b"),
            Expr::JsonDeletePath(..)
        ));
    }

    #[test]
    fn parse_dollar_string_literal_expr() {
        // Regression: json.sql uses `$$'foo'$$::json` and similar.
        let lexed = crate::lex("$$''$$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::DollarStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_string_literal() {
        let lexed = crate::lex("'hello'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_adjacent_string_literals() {
        let lexed = crate::lex("'a' 'b'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2);
        } else {
            panic!("expected Expr::StringLit, got {:?}", expr);
        }
        assert!(input.is_eof());
    }

    /// A block comment does not itself qualify as string-continuation
    /// whitespace, but the later newline in this gap does (ADR 0004).
    #[test]
    fn parse_string_continuation_after_comment_and_later_newline() {
        let lexed = crate::lex("'first line'\n/* comment */\n' - next line'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2, "later newline must continue the string");
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
        assert!(input.is_eof());
    }

    /// A legitimate newline-separated 3-part string continuation (no comment)
    /// must still concatenate under the classifier — the regression guard for
    /// `strings.sql`'s "Three lines to one" fixture.
    #[test]
    fn parse_three_part_string_continuation_classified() {
        let expr = parse_expr_classified("'first line'\n' - next line'\n\t' - third line'");
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 3, "all three parts must concatenate");
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 2);
        } else {
            panic!("expected Expr::StringLit, got {expr:?}");
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_three_part_string_concat() {
        // 3-part adjacent string literal concatenation. Postgres concatenates
        // these into a single value at parse time.
        let lexed = crate::lex("'first' 'second' 'third'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 3);
        } else {
            panic!("expected StringLit, got {:?}", expr);
        }
        assert!(input.is_eof());
    }

    #[test]
    fn parse_four_part_string_concat() {
        let lexed = crate::lex("'a' 'b' 'c' 'd'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        if let Expr::StringLit(seq) = &expr {
            assert_eq!(seq.parts.len(), 4);
        } else {
            panic!("expected StringLit");
        }
    }

    #[test]
    fn parse_three_adjacent_strings_with_quoted_alias() {
        use crate::ast::dml::select::SelectStmt;
        let lexed = crate::lex(
            "SELECT 'first line' ' - next line' ' - third line' AS \"Three lines to one\"",
        );
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_three_adjacent_strings_with_alias() {
        // SELECT 'first line' ' - next line' AS foo
        use crate::ast::dml::select::SelectStmt;
        let lexed = crate::lex("SELECT 'first line' ' - next line' AS foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlelement_simple() {
        let lexed = crate::lex("xmlelement(name foo, 'content')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlelement_with_attributes() {
        let lexed = crate::lex("xmlelement(name foo, xmlattributes(1 as a, 2 as b), 'content')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlElement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlpi_basic() {
        let lexed = crate::lex("xmlpi(name foo)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_xmlpi_with_content() {
        let lexed = crate::lex("xmlpi(name foo, 'bar')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::XmlPi(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unicode_string_lit_basic() {
        let lexed = crate::lex(r"U&'d\0061t\+000061'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unicode_string_lit_uescape() {
        let lexed = crate::lex(r"U&'d!0061t\+000061' UESCAPE '!'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
            let expr = Expr::parse(&mut input)
                .unwrap_or_else(|error| panic!("parse {src:?}: {error}"))
                .into_ast();
            match expr {
                Expr::Func(call) if identifier_led => assert!(
                    matches!(call.tail, FunctionCallTail::TypedLiteral(_)),
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
        assert!(matches!(parse_type_name_classified("bool"), TypeName::Bool));
        assert!(matches!(parse_type_name_classified("text"), TypeName::Text));
        assert!(matches!(
            parse_type_name_classified("serial"),
            TypeName::Serial
        ));
        assert!(matches!(
            parse_type_name_classified("double precision"),
            TypeName::DoublePrecision
        ));
        assert!(matches!(
            parse_type_name_classified("unknown"),
            TypeName::Unknown
        ));

        let TypeName::Ident(double) = parse_type_name_classified("double") else {
            panic!("bare double must remain an identifier type name")
        };
        assert_eq!(double.object(), "double");

        let TypeName::Ident(json) = parse_type_name_classified("json") else {
            panic!("json must preserve the legacy identifier variant")
        };
        assert_eq!(json.object(), "json");

        let TypeName::Ident(qualified) = parse_type_name_classified("pg_catalog.json") else {
            panic!("qualified json must remain an identifier type name")
        };
        assert_eq!(qualified.parts.len(), 2);
        assert_eq!(qualified.object(), "json");

        assert_cast_type_families_track_type_name_spellings();
        assert_cast_type_family_modifiers_and_json_unique_boundary_round_trip();
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
            let name = parse_type_name_classified(src);
            let ty = parse_cast_type_classified(src);
            assert_eq!(
                matches!(&ty.head, CastTypeHead::General(_)),
                expects_general_head(&name),
                "cast family for {src:?}",
            );
            assert_eq!(
                format_tokens_sql(&ty, PrettyConfig::default()).trim(),
                canonical,
                "cast type did not render canonically for {src:?}",
            );
        }
    }

    fn assert_cast_type_family_modifiers_and_json_unique_boundary_round_trip() {
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
            let ty = parse_cast_type_classified(src);
            let actual_family = match &ty.head {
                CastTypeHead::General(_) => "general",
                CastTypeHead::DateTime(_) => "datetime",
                CastTypeHead::Interval(_) => "interval",
            };
            assert_eq!(actual_family, expected_family, "cast family for {src:?}");
            assert_eq!(
                format_tokens_sql(&ty, PrettyConfig::default()).trim(),
                canonical,
                "cast type did not render canonically for {src:?}",
            );
        }

        // The boundary itself is asserted structurally: the cast must not
        // consume `WITH`, and the JSON constructor must own `UNIQUE KEYS`.
        // Rendering compares against the canonical mechanical form; exact
        // source-preserving output (`::timestamp`) is deferred to the
        // provenance-aware formatting milestone.
        let src = "JSON('2000-01-01'::timestamp WITH UNIQUE KEYS)";
        let expr = parse_expr_classified(src);
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
            format_tokens_sql(&expr, PrettyConfig::default()).trim(),
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
            let expr = parse_expr_classified(src);
            let Expr::Func(call) = &expr else {
                panic!("expected an ordinary function call for {src:?}");
            };
            let (actual_body, actual_tail) = match &call.tail {
                FunctionCallTail::TypedLiteral(_) => ("typed", "typed-literal"),
                FunctionCallTail::WithinGroup(tail) => {
                    let body = match tail.body.as_ref() {
                        None => "empty",
                        Some(crate::ast::shared::expr::FunctionWithinGroupBody::Star(_)) => "star",
                        Some(crate::ast::shared::expr::FunctionWithinGroupBody::All(_)) => "all",
                        Some(crate::ast::shared::expr::FunctionWithinGroupBody::Args(_)) => "args",
                    };
                    (body, "within-group")
                }
                FunctionCallTail::Plain(tail) => {
                    let body = match tail.body.as_ref() {
                        None => "empty",
                        Some(FunctionCallBody::Star(_)) => "star",
                        Some(FunctionCallBody::All(_)) => "all",
                        Some(FunctionCallBody::Distinct(_)) => "distinct",
                        Some(FunctionCallBody::LeadingVariadic(_)) => "leading-variadic",
                        Some(FunctionCallBody::Args(args)) if args.args.has_trailing_variadic() => {
                            "trailing-variadic"
                        }
                        Some(FunctionCallBody::Args(args)) if args.order_by.is_some() => "ordered",
                        Some(FunctionCallBody::Args(_)) => "args",
                    };
                    (body, "plain")
                }
            };
            assert_eq!(actual_body, expected_body, "application body for {src:?}");
            assert_eq!(actual_tail, expected_tail, "call tail for {src:?}");
            let formatted = format_tokens_sql(&expr, PrettyConfig::default());
            assert_eq!(
                formatted.trim(),
                src,
                "function call did not round-trip exactly for {src:?}",
            );
        }

        // With no standalone ALL expression atom, the token-identical compact
        // spelling has exactly the PostgreSQL interpretation: ALL qualifies
        // the parenthesized first argument.
        let Expr::Func(call) = parse_expr_classified("f(ALL(1))") else {
            panic!("expected compact ALL-qualified function call");
        };
        assert!(matches!(
            call.tail,
            FunctionCallTail::Plain(crate::ast::shared::expr::FunctionPlainTail {
                body: Some(FunctionCallBody::All(_)),
                ..
            })
        ));
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
            "f(1 ORDER BY 1) WITHIN GROUP (ORDER BY 1)",
            "f(DISTINCT 1) WITHIN GROUP (ORDER BY 1)",
            "f(VARIADIC xs) WITHIN GROUP (ORDER BY 1)",
            "char() 'x'",
            "char(*) 'x'",
            "char(DISTINCT 1) 'x'",
            "char(VARIADIC xs) 'x'",
            "char(n => 1) 'x'",
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::UnicodeStringLit(_)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_xmlforest() {
        let lexed = crate::lex("xmlforest(a, b AS bee, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
            let _stmt = SelectStmt::parse(&mut input).unwrap().into_ast();
            assert!(input.is_eof(), "leftover for {sql}");
        }
    }

    #[test]
    fn parse_escape_string_literal() {
        let lexed = crate::lex(r"E'r_\_view%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_order_by() {
        let lexed = crate::lex("jsonb_agg(q ORDER BY x, y)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_psql_var() {
        let lexed = crate::lex(":foo_oid");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_psql_var_in_func_call() {
        let lexed = crate::lex("pg_stat_get_function_calls(:func_oid)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_both_from() {
        let lexed = crate::lex("TRIM(BOTH FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_leading_from() {
        let lexed = crate::lex("TRIM(LEADING FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_trailing_from() {
        let lexed = crate::lex("TRIM(TRAILING FROM '  hi  ')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_trim_both_chars_from() {
        let lexed = crate::lex("TRIM(BOTH 'x' FROM 'xxhixx')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
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
            let _expr = Expr::parse(&mut input)
                .unwrap_or_else(|e| panic!("parse {src:?}: {e}"))
                .into_ast();
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
    fn parse_substring_from() {
        let lexed = crate::lex("SUBSTRING('1234567890' FROM 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_from_for() {
        let lexed = crate::lex("SUBSTRING('1234567890' FROM 4 FOR 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_notnull_isnull() {
        let lexed = crate::lex("x.c NOTNULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Notnull(..)));
        assert!(input.is_eof());
        let lexed = crate::lex("x.c ISNULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Isnull(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_collation_for() {
        let lexed = crate::lex("collation for ('foo')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let lexed = crate::lex("collation for ((SELECT a FROM t LIMIT 1))");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_call() {
        let lexed = crate::lex("CAST('42' AS text COLLATE \"C\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
        let lexed = crate::lex("CAST(b AS varchar)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_for_only() {
        let lexed = crate::lex("substring(d FOR 30)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_substring_similar_escape() {
        let lexed = crate::lex("SUBSTRING('abcdefg' SIMILAR 'a#\"%#\"g' ESCAPE '#')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_position_in() {
        let lexed = crate::lex("POSITION('4' IN '1234567890')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
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
            parse_expr_classified(src);
        }

        // PostgreSQL deliberately admits a full `a_expr` inside parentheses
        // as a `b_expr` atom.
        parse_expr_classified("POSITION((1 IN (1)) IN 2)");
        parse_expr_classified("POSITION((1 AND 2) IN 3)");
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
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlay_placing_from_for() {
        let lexed = crate::lex("OVERLAY('abcdef' PLACING '45' FROM 4 FOR 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_epoch_from_date() {
        let lexed = crate::lex("EXTRACT(EPOCH FROM DATE '1970-01-01')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Extract(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_century_from_ident() {
        let lexed = crate::lex("EXTRACT(CENTURY FROM d)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_string_field() {
        let lexed = crate::lex("EXTRACT('year' FROM t)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_named_arg_mixed() {
        let lexed = crate::lex("f(a, b => 1, c)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_path_query_silent() {
        let lexed = crate::lex("jsonb_path_query('[1]', 'strict $[1]', silent => true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_all_named_args() {
        let lexed = crate::lex("f(silent => false, verbose => true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_extract_year_from_now() {
        let lexed = crate::lex("EXTRACT(year FROM now())");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_is_distinct_from() {
        let lexed = crate::lex("a IS DISTINCT FROM b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_is_not_distinct_from() {
        let lexed = crate::lex("a IS NOT DISTINCT FROM b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_power_operator() {
        let lexed = crate::lex("2^1000");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_double_precision_type_cast() {
        let lexed = crate::lex("3.14::double precision");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_searched() {
        let lexed = crate::lex("CASE WHEN 1 < 2 THEN 3 END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Case(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_searched_with_else() {
        let lexed = crate::lex("CASE WHEN 1 < 2 THEN 3 WHEN 4 < 5 THEN 6 ELSE 7 END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_simple() {
        let lexed = crate::lex("CASE x WHEN 1 THEN 'a' WHEN 2 THEN 'b' ELSE 'c' END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_case_nested() {
        let lexed = crate::lex("CASE WHEN (CASE WHEN 1=1 THEN 1 END) > 0 THEN 'y' END");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_within_group() {
        let lexed = crate::lex("percentile_disc(0.5) WITHIN GROUP (ORDER BY v)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_within_group_multi() {
        let lexed = crate::lex("rank(1, 2) WITHIN GROUP (ORDER BY a, b)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_filter() {
        let lexed = crate::lex("sum(x) FILTER (WHERE y > 0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_filter_over() {
        let lexed = crate::lex("sum(x) FILTER (WHERE y > 0) OVER (PARTITION BY z)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_order_by_nulls_first() {
        let lexed = crate::lex("jsonb_agg(q ORDER BY x NULLS FIRST, y)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_func_call_variadic() {
        let lexed = crate::lex("jsonb_build_array(VARIADIC a)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_timestamp_with_tz_literal() {
        let lexed = crate::lex("timestamp with time zone '2001-12-27 04:05:06+08'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_timestamp_precision_without_tz_literal() {
        // Regression: timestamp.sql uses `timestamp(2) without time zone 'now'`.
        let lexed = crate::lex("timestamp(2) without time zone 'now'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TimestampLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_time_zone() {
        let lexed = crate::lex("f1 AT TIME ZONE 'UTC+10'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_time_zone_interval() {
        let lexed = crate::lex("f1 AT TIME ZONE INTERVAL '-10:00'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtTimeZone(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_at_local() {
        let lexed = crate::lex("f1 AT LOCAL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtLocal(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_time_literal() {
        let lexed = crate::lex("time '12:34'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TimeLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_date_literal_as_castfunc() {
        let lexed = crate::lex("date '2024-01-01'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        // `date` is an Ident-based TypeName, so this parses as CastFunc.
        assert!(matches!(expr, Expr::CastFunc(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_bare() {
        let lexed = crate::lex("interval '1 hour'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_year() {
        let lexed = crate::lex("INTERVAL '1' YEAR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_year_to_month() {
        let lexed = crate::lex("INTERVAL '1-2' YEAR TO MONTH");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_named_arg_colon_equals() {
        let lexed = crate::lex("make_interval(years := 1, months := 2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unary_plus() {
        let lexed = crate::lex("+42");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Pos(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_positional_param() {
        let lexed = crate::lex("$1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::PositionalParam(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_positional_param_in_expr() {
        let lexed = crate::lex("$1 + $2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        let formatted = crate::formatter::format_tokens_sql(&expr, PrettyConfig::default());
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
            let expr = Expr::parse(&mut input).unwrap().into_ast();
            assert!(matches!(expr, Expr::IntervalLit(_)), "failed for {src:?}");
            assert!(input.is_eof(), "leftover for {src:?}");
        }
    }

    #[test]
    fn parse_interval_second_precision() {
        let lexed = crate::lex("INTERVAL '1.234' second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_day_to_second_precision() {
        let lexed = crate::lex("INTERVAL '1 2:03:04.5678' day to second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_interval_day_to_minute() {
        let lexed = crate::lex("f1::INTERVAL DAY TO MINUTE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_minute_to_second_precision() {
        let lexed = crate::lex("INTERVAL '12:34.5678' minute to second(2)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_day_to_hour() {
        let lexed = crate::lex("INTERVAL '1 2:03' DAY TO HOUR");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_interval_literal_hour_to_second() {
        let lexed = crate::lex("INTERVAL '1' HOUR TO SECOND");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::IntervalLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_escape_string_literal_lowercase_e() {
        let lexed = crate::lex("e'foo'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::EscapeStringLit(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bool_true() {
        let lexed = crate::lex("true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTrue));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bool_false() {
        let lexed = crate::lex("false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolFalse));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_null() {
        let lexed = crate::lex("null");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Null));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_column_ref() {
        let lexed = crate::lex("f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::ColumnRef(_)));
    }

    #[test]
    fn parse_qualified_column_ref() {
        let lexed = crate::lex("BOOLTBL1.f1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QualRef(_)));
    }

    #[test]
    fn parse_qualified_wildcard() {
        let lexed = crate::lex("BOOLTBL1.*");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QualWild(_)));
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
        let item = SelectItem::parse(&mut input).unwrap().into_ast();
        assert!(matches!(item, SelectItem::Star(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_function_call_no_args() {
        let lexed = crate::lex("foo()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_with_args() {
        let lexed = crate::lex("pg_input_is_valid('true', 'bool')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_function_call_booleq() {
        let lexed = crate::lex("booleq(bool 'false', f1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Func(_)));
    }

    #[test]
    fn parse_parenthesized_expr() {
        let lexed = crate::lex("(1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Exprs(ref expressions),
                ref indirection,
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Exprs(ref expressions),
                ref indirection,
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_cast_parenthesized_field_access() {
        let lexed = crate::lex("((row_value)::record_type).field");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Subquery(ref subquery),
                ref indirection,
                ..
            }) if matches!(subquery.as_ref(), DirectSubquery::ParenthesizedSet(_))
                && indirection.is_empty()
        ));
        assert!(input.is_eof());
    }

    // --- Type cast function-style: bool 'foo' ---

    #[test]
    fn parse_type_cast_bool_string() {
        let lexed = crate::lex("bool 't'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    #[test]
    fn parse_type_cast_boolean_string() {
        let lexed = crate::lex("boolean 'false'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CastFunc(_)));
    }

    // --- Prefix operators ---

    #[test]
    fn parse_not_expr() {
        let lexed = crate::lex("not false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Not(_)));
    }

    // --- Infix operators ---

    #[test]
    fn parse_and_expr() {
        let lexed = crate::lex("true AND false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::And(..)));
    }

    #[test]
    fn parse_or_expr() {
        let lexed = crate::lex("true OR false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn parse_eq_expr() {
        let lexed = crate::lex("f1 = true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Eq(..)));
    }

    #[test]
    fn parse_neq_expr() {
        let lexed = crate::lex("f1 <> false");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Neq(..)));
    }

    // --- Postfix: :: type cast ---

    #[test]
    fn parse_cast_colon_colon() {
        let lexed = crate::lex("0::boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    #[test]
    fn parse_chained_cast() {
        let lexed = crate::lex("'TrUe'::text::boolean");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        // Outer should be Cast
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Postfix: IS [NOT] TRUE/FALSE/UNKNOWN/NULL ---

    #[test]
    fn parse_is_true() {
        let lexed = crate::lex("f1 IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_false() {
        let lexed = crate::lex("f1 IS NOT FALSE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_unknown() {
        let lexed = crate::lex("b IS UNKNOWN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    #[test]
    fn parse_is_not_unknown() {
        let lexed = crate::lex("b IS NOT UNKNOWN");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
    }

    // --- Postfix: BETWEEN / NOT BETWEEN ---

    #[test]
    fn parse_between_expr() {
        let lexed = crate::lex("a BETWEEN 12 AND 17");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn parse_not_between_expr() {
        let lexed = crate::lex("a NOT BETWEEN 1 AND 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotBetweenExpr(..)));
    }

    #[test]
    fn parse_between_as_value() {
        // BETWEEN yields a boolean value that can appear in a SELECT list.
        let lexed = crate::lex("x BETWEEN a AND b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BetweenExpr(..)));
    }

    #[test]
    fn between_does_not_break_and_parse() {
        // A plain AND expression must still parse as And, not be confused
        // with the BETWEEN postfix.
        let lexed = crate::lex("a AND b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::And(..)));
    }

    // --- Precedence ---

    #[test]
    fn and_binds_tighter_than_or() {
        // a OR b AND c should parse as a OR (b AND c)
        let lexed = crate::lex("true OR false AND true");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Or(..)));
    }

    #[test]
    fn is_true_in_select_item() {
        // b IS TRUE should parse without consuming AS that follows
        let lexed = crate::lex("b IS TRUE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BoolTest(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn cast_chain_in_expression() {
        // true::boolean::text should chain
        let lexed = crate::lex("true::boolean::text");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Cast(..)));
    }

    // --- Arithmetic operators ---

    #[test]
    fn parse_addition() {
        let lexed = crate::lex("4+4");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Add(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn concat_binds_less_tightly_than_addition() {
        let lexed = crate::lex("a || b + c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());

        let Expr::Concat(_, right) = expr else {
            panic!("expected concatenation at the root")
        };
        assert!(
            matches!(*right, Expr::Add(..)),
            "addition must bind inside the concatenation right operand"
        );
    }

    #[test]
    fn parse_subtraction() {
        let lexed = crate::lex("10-3");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Sub(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_unary_minus() {
        let lexed = crate::lex("-1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Neg(..)));
        assert!(input.is_eof());
    }

    // --- Numeric literal ---

    #[test]
    fn parse_numeric_literal() {
        let lexed = crate::lex("77.7");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NumericLit(_)));
        assert!(input.is_eof());
    }

    // --- IN expression ---

    #[test]
    fn parse_in_expr() {
        let lexed = crate::lex("f1 IN (1, 2, 3)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::InExpr(..)));
        assert!(input.is_eof());
    }

    // --- JSON / JSONB operators ---

    #[test]
    fn parse_json_field() {
        let lexed = crate::lex("data -> 'key'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonField(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_field_text() {
        let lexed = crate::lex("data ->> 'key'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonFieldText(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path() {
        let lexed = crate::lex("data #> '{a,b}'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonPath(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path_text() {
        let lexed = crate::lex("data #>> '{a,b}'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonPathText(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_contains() {
        let lexed = crate::lex("a @> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonContains(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_contained_by() {
        let lexed = crate::lex("a <@ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonContainedBy(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_key_exists() {
        let lexed = crate::lex("a ? 'k'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonKey(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_any_key() {
        let lexed = crate::lex("a ?| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonAnyKey(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_jsonb_all_keys() {
        let lexed = crate::lex("a ?& b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonAllKeys(..)));
        assert!(input.is_eof());
    }

    // --- Postgres text-search / range / geometric operators ---

    #[test]
    fn parse_ts_match() {
        let lexed = crate::lex("a @@ 'foo|bar'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TsMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ts_match3() {
        let lexed = crate::lex("a @@@ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TsMatch3(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_json_path_exists() {
        let lexed = crate::lex("j @? '$.a'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::JsonPathExists(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_overlap() {
        let lexed = crate::lex("r && s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Overlap(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_left() {
        let lexed = crate::lex("a << b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyLeft(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_right() {
        let lexed = crate::lex("a >> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyRight(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_subset_eq() {
        let lexed = crate::lex("a <<= b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SubsetEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_superset_eq() {
        let lexed = crate::lex("a >>= b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SupersetEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_adjacent() {
        let lexed = crate::lex("a -|- b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Adjacent(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_distance() {
        let lexed = crate::lex("p1 <-> p2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Distance(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_right() {
        let lexed = crate::lex("a &< b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendRight(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_left() {
        let lexed = crate::lex("a &> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendLeft(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_above() {
        let lexed = crate::lex("a |>> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyAbove(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_strictly_below() {
        let lexed = crate::lex("a <<| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::StrictlyBelow(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_above() {
        let lexed = crate::lex("a &<| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendAbove(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_no_extend_below() {
        let lexed = crate::lex("a |&> b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NoExtendBelow(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_intersect() {
        let lexed = crate::lex("a ?# b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Intersect(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_horizontal() {
        let lexed = crate::lex("a ?- b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Horizontal(..)));
        assert!(input.is_eof());
    }

    // --- LIKE / ILIKE ---

    #[test]
    fn parse_like_expr() {
        let lexed = crate::lex("table_name LIKE 'foo%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_like_escape_string() {
        let lexed = crate::lex(r"table_name LIKE E'r_\_view%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_like_expr() {
        let lexed = crate::lex("table_name NOT LIKE 'bar%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_expr() {
        let lexed = crate::lex("x SIMILAR TO 'a%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_similar_to_expr() {
        let lexed = crate::lex("x NOT SIMILAR TO 'a%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ilike_expr() {
        let lexed = crate::lex("name ILIKE '%FOO%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_ilike_expr() {
        let lexed = crate::lex("name NOT ILIKE '%bar%'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_like_escape() {
        let lexed = crate::lex("'hawkeye' LIKE 'h%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Like(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_like_escape() {
        let lexed = crate::lex("'hawkeye' NOT LIKE 'h%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotLike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_escape() {
        let lexed = crate::lex("'abcdefg' SIMILAR TO '_bcd#%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_similar_to_escape() {
        let lexed = crate::lex("'abc' NOT SIMILAR TO 'a%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotSimilarTo(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_ilike_escape() {
        let lexed = crate::lex("name ILIKE '%FOO%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Ilike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_not_ilike_escape() {
        let lexed = crate::lex("name NOT ILIKE '%bar%' ESCAPE '#'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::NotIlike(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_similar_to_escape_null() {
        let lexed = crate::lex("'abcdefg' SIMILAR TO '_bcd%' ESCAPE NULL");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::SimilarTo(..)));
        assert!(input.is_eof());
    }

    // --- Regex match operators ---

    #[test]
    fn parse_regex_match() {
        let lexed = crate::lex("relname ~ '^foo'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_not_match() {
        let lexed = crate::lex("name !~ 'bar'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexNotMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_imatch() {
        let lexed = crate::lex("name ~* 'FOO'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexIMatch(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_regex_not_imatch() {
        let lexed = crate::lex("name !~* '.*'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::RegexNotIMatch(..)));
        assert!(input.is_eof());
    }

    // --- COLLATE postfix ---

    #[test]
    fn parse_collate_postfix() {
        let lexed = crate::lex("a COLLATE \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Collate(..)));
        assert!(input.is_eof());
    }

    // --- DEFAULT atom ---

    #[test]
    fn parse_default_atom() {
        let lexed = crate::lex("DEFAULT");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Default));
        assert!(input.is_eof());
    }

    // --- Subquery expression ---

    #[test]
    fn parse_subquery_expr() {
        let lexed = crate::lex("(SELECT 1)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(
            expr,
            Expr::Parenthesized(ParenthesizedExpr {
                content: ParenContent::Subquery(_),
                ref indirection,
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeLtTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_leq_tilde_infix() {
        let lexed = crate::lex("t ~<=~ 'Aztec'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeLeqTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_geq_tilde_infix() {
        let lexed = crate::lex("t ~>=~ 'Worth'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeGeqTilde(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_tilde_gt_tilde_infix() {
        let lexed = crate::lex("t ~>~ 'Worth'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TildeGtTilde(..)));
        assert!(input.is_eof());
    }

    // --- User-defined equality/inequality ---

    #[test]
    fn parse_triple_eq_infix() {
        let lexed = crate::lex("a === 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TripleEq(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_bang_eq_eq_infix() {
        let lexed = crate::lex("a !== 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BangEqEq(..)));
        assert!(input.is_eof());
    }

    // --- Geometric closest-point / intersection ---

    #[test]
    fn parse_hash_hash_infix() {
        let lexed = crate::lex("p.f1 ## l.s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::GeomClosest(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: geometric path length `@-@` ---

    #[test]
    fn parse_at_minus_at_prefix() {
        let lexed = crate::lex("@-@ s");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::PathLength(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: user-defined `@#@` ---

    #[test]
    fn parse_at_hash_at_prefix() {
        let lexed = crate::lex("@#@ 24");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::AtHashAtPrefix(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: user-defined `!=-` ---

    #[test]
    fn parse_bang_eq_minus_prefix() {
        let lexed = crate::lex("!=- 10");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::BangEqMinusPrefix(..)));
        assert!(input.is_eof());
    }

    // --- Prefix: geometric `#` (number of points in path) ---

    #[test]
    fn parse_pound_prefix() {
        let lexed = crate::lex("#thepath");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::PointCount(..)));
        assert!(input.is_eof());
    }

    // --- Infix: geometric `?||` (parallel) and `?-|` (perpendicular) ---

    #[test]
    fn parse_question_pipe_pipe_infix() {
        let lexed = crate::lex("a ?|| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Parallel(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_question_dash_pipe_infix() {
        let lexed = crate::lex("a ?-| b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Perpendicular(..)));
        assert!(input.is_eof());
    }

    // --- Infix: geometric `<^` (below) and `>^` (above) ---

    #[test]
    fn parse_lt_caret_infix() {
        let lexed = crate::lex("a <^ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Below(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_gt_caret_infix() {
        let lexed = crate::lex("a >^ b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Above(..)));
        assert!(input.is_eof());
    }

    // --- Infix: user-defined `<<<` and `>>>` ---

    #[test]
    fn parse_triple_lt_infix() {
        let lexed = crate::lex("a <<< 5");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TripleLt(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_triple_gt_infix() {
        let lexed = crate::lex("a >>> 0");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::TripleGt(..)));
        assert!(input.is_eof());
    }

    // --- Infix: user-defined `<%` ---

    #[test]
    fn parse_lt_percent_infix() {
        let lexed = crate::lex("a <% b");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::CustomInfix(..)));
        assert!(input.is_eof());
    }

    #[test]
    fn custom_infix_chain_is_left_associative() {
        let lexed = crate::lex("a <% b <% c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());

        let Expr::CustomInfix(left, _, _) = expr else {
            panic!("expected custom operator at the root")
        };
        assert!(
            matches!(*left, Expr::CustomInfix(..)),
            "equal-precedence custom operators must associate to the left"
        );
    }

    #[test]
    fn custom_infix_does_not_consume_lower_precedence_comparison() {
        let lexed = crate::lex("a <% b = c");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());

        let Expr::Eq(left, _) = expr else {
            panic!("expected comparison at the root")
        };
        assert!(
            matches!(*left, Expr::CustomInfix(..)),
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QuantifiedComparison(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
        assert_eq!(
            format_tokens_sql(&expr, PrettyConfig::default()).trim(),
            src,
        );
    }

    #[test]
    fn parse_eq_all_array() {
        // `a = ALL('{ab}')` — comparison with quantified array.
        let lexed = crate::lex("a = ALL('{ab}')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QuantifiedComparison(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_not_tilde_all() {
        // `a !~ ALL('{ab}')` — regex not-match with ALL quantifier.
        let lexed = crate::lex("a !~ ALL('{ab}')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QuantifiedComparison(..)));
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
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::QuantifiedComparison(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    // --- Array slice subscripts ---

    #[test]
    fn parse_array_slice_full() {
        // `a[1:2]` — full slice with lower and upper bounds.
        let lexed = crate::lex("a[1:2]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Subscript(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    /// Slice on a parenthesised cast: `(arr::int[])[1:2]` — PG accepts the
    /// postfix subscript on any a_expr including a parenthesised cast.
    /// Slices with a reserved keyword (NULL/TRUE/FALSE) as a bound rely on
    /// the `pg_lex` post-processor splitting `:NULL` PsqlVars; the
    /// jsonb-string-range form `[ 'a':'b' ]` is a separate limitation —
    /// PsqlVar's `:'…'` quoted form is preserved to keep psql-style
    /// `COPY ... :'filename'` round-tripping.
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
            let _expr = Expr::parse(&mut input)
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
    fn parse_array_slice_lower_only() {
        // `a[1:]` — slice with only lower bound.
        let lexed = crate::lex("a[1:]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Subscript(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_array_slice_upper_only() {
        // `a[:2]` — slice with only upper bound.
        let lexed = crate::lex("a[:2]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Subscript(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_array_slice_unbounded() {
        // `a[:]` — unbounded slice (all elements).
        let lexed = crate::lex("a[:]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Subscript(..)));
        assert!(input.is_eof(), "parser cursor: {}", input.cursor());
    }

    #[test]
    fn parse_subscript_unchanged() {
        // `a[1]` — regular subscript still works.
        let lexed = crate::lex("a[1]");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let expr = Expr::parse(&mut input).unwrap().into_ast();
        assert!(matches!(expr, Expr::Subscript(..)));
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
    /// prefix; generated balanced dispatch selects them after the matching
    /// close while bare subqueries remain bounded decisions.
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

    /// `(SubSelect)::Typename` is gram.y `c_expr → '(' SubSelect ')' typecast`.
    /// The unified parenthesized atom returns the inner `(SubSelect)` to the
    /// Pratt loop, which consumes each trailing cast before the next enclosing
    /// `ParenContent` expects its close parenthesis.
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
            let expr = parse_expr_classified(src);
            // Confirm the atom is the dedicated bit/hex variant, not a
            // StringLit / ColumnRef pair.
            assert!(
                matches!(expr, Expr::BitStringLit(_) | Expr::HexStringLit(_)),
                "expected BitStringLit/HexStringLit atom for {src:?}, got {:?}",
                std::mem::discriminant(&expr),
            );
            let formatted = format_tokens_sql(&expr, PrettyConfig::default());
            assert_eq!(
                formatted.trim(),
                src,
                "non-exact round-trip for {src:?}: {formatted:?}",
            );
        }
    }
}
