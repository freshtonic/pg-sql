//! The minimum PostgreSQL version that a parsed statement needs (issue #92).
//!
//! The answer comes from the version gates, which are declarations that
//! Recursa records (ADR 0010). These tests pin the three classes of gate that
//! the declaration covers, and the mapping between the grammar's declared
//! configurations and the target versions.

use pg_sql::minimum_version::minimum_version;
use pg_sql::{Configuration, MinimumVersion as _, TargetVersion, ast::Statement, lex};

/// Parses one complete statement, or reports that this build rejects it.
fn parse_and_report(source: &str) -> Option<TargetVersion> {
    let lexed = lex(source);
    assert_eq!(lexed.errors().count(), 0, "{source} lexes");
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).expect("statement parses");
    assert!(input.is_eof(), "{source} is one complete statement");
    Some(minimum_version(&parsed).map_or(TargetVersion::Pg14, |requirement| requirement.version()))
}

#[test]
fn the_configuration_scale_names_the_six_target_versions() {
    assert_eq!(
        Configuration::ALL.map(Configuration::name),
        ["pg14", "pg15", "pg16", "pg17", "pg18", "pg19"]
    );
    assert_eq!(Configuration::BUILD.name(), {
        // The configuration list drops the pre-release suffix of `pg19-beta`.
        let feature = pg_sql::TARGET_VERSION.feature();
        feature.strip_suffix("-beta").unwrap_or(feature)
    });
}

#[test]
fn a_statement_with_no_gated_construct_needs_only_the_oldest_version() {
    assert_eq!(parse_and_report("SELECT 1"), Some(TargetVersion::Pg14));
    assert_eq!(
        parse_and_report("CREATE TABLE t (a int PRIMARY KEY)"),
        Some(TargetVersion::Pg14)
    );
}

/// The span of one requirement, as `start..end`, or `None`.
fn span_of(source: &str) -> Option<std::ops::Range<usize>> {
    let lexed = lex(source);
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).expect("statement parses");
    minimum_version(&parsed).and_then(|found| found.span().map(|span| span.range()))
}

/// Class 1: a gate on a node, field or variant.
#[test]
fn an_item_gate_reports_the_version_that_added_the_item() {
    #[cfg(feature = "since-pg15")]
    assert_eq!(
        parse_and_report("MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DELETE"),
        Some(TargetVersion::Pg15)
    );

    #[cfg(feature = "since-pg16")]
    assert_eq!(
        parse_and_report("SELECT JSON_ARRAY(1, 2)"),
        Some(TargetVersion::Pg16)
    );

    #[cfg(feature = "since-pg17")]
    assert_eq!(
        parse_and_report("SELECT JSON_VALUE(j, '$.a')"),
        Some(TargetVersion::Pg17)
    );

    #[cfg(feature = "since-pg18")]
    assert_eq!(
        parse_and_report("CREATE TABLE t (a int, b int GENERATED ALWAYS AS (a * 2) VIRTUAL)"),
        Some(TargetVersion::Pg18)
    );

    // The answer keeps the span of the construct that sets the version
    // (recursa#136). `JSON_ARRAY(1, 2)` starts after `SELECT `.
    #[cfg(feature = "since-pg16")]
    {
        let source = "SELECT JSON_ARRAY(1, 2)";
        let span = span_of(source).expect("a span");
        assert_eq!(&source[span], "JSON_ARRAY(1, 2)");
    }
}

/// Class 3: a shape rule, where a newer grammar makes optional what an older
/// one required. The requirement belongs to the absence of the value.
#[cfg(feature = "since-pg16")]
#[test]
fn a_shape_rule_reports_the_version_that_made_the_value_optional() {
    use pg_sql::minimum_version::minimum_version_above;

    for source in [
        "SELECT * FROM (SELECT 1)",
        "SELECT * FROM LATERAL (SELECT 1)",
    ] {
        let lexed = lex(source);
        let mut input = lexed.input();
        let parsed = Statement::parse(&mut input).expect("statement parses");
        let found = minimum_version(&parsed).expect("a requirement above 14");
        assert_eq!(found.version(), TargetVersion::Pg16, "{source}");
        assert!(found.is_absence(), "{source}");
        assert_eq!(found.sqlstate(), Some("42601"), "{source}");
        assert_eq!(
            found.message(),
            Some("subquery in FROM must have an alias"),
            "{source}"
        );
        assert_eq!(
            found.hint(),
            Some("For example, FROM (SELECT ...) [AS] foo."),
            "{source}"
        );
        assert!(found.citation().is_some(), "{source}");
        // The span of a shape rule covers the element that holds no value:
        // the parenthesised subquery that needs the alias (recursa#136).
        let span = found.span().expect("a span");
        assert_eq!(
            &source[span.start() as usize..span.end() as usize],
            "(SELECT 1)",
            "{source}"
        );
        // A PostgreSQL 15 server rejects it; a PostgreSQL 16 server does not.
        assert!(minimum_version_above(&parsed, TargetVersion::Pg15).is_some());
        assert!(minimum_version_above(&parsed, TargetVersion::Pg16).is_none());
    }

    // The same rule with the alias present needs nothing above the baseline.
    assert_eq!(
        parse_and_report("SELECT * FROM (SELECT 1) AS s"),
        Some(TargetVersion::Pg14)
    );

    // Two more shape rules of 16: a statistics object and a whole-database
    // REINDEX may omit their name.
    for source in ["CREATE STATISTICS ON a, b FROM t", "REINDEX DATABASE"] {
        let lexed = lex(source);
        let mut input = lexed.input();
        let parsed = Statement::parse(&mut input).expect("statement parses");
        let found = minimum_version(&parsed).expect("a requirement above 14");
        assert_eq!(found.version(), TargetVersion::Pg16, "{source}");
        assert!(found.is_absence(), "{source}");
        // The older grammar has no alternative without the name, so it gives
        // a plain syntax error and the gate carries no message.
        assert_eq!(found.sqlstate(), None, "{source}");
        assert!(found.citation().is_some(), "{source}");
        // The absent name still has a span inside the statement.
        let span = found.span().expect("a span");
        assert!(
            span.end() as usize <= source.len() && span.start() <= span.end(),
            "{source}: {span:?}"
        );
    }
    assert_eq!(
        parse_and_report("CREATE STATISTICS s ON a, b FROM t"),
        Some(TargetVersion::Pg14)
    );
    assert_eq!(
        parse_and_report("REINDEX DATABASE d"),
        Some(TargetVersion::Pg14)
    );
}

/// The two questions differ, and both are right: the minimum is the highest
/// requirement of the value, and the first construct above a floor is the
/// earliest one in source order.
#[cfg(feature = "since-pg17")]
#[test]
fn the_minimum_and_the_first_construct_above_a_floor_can_differ() {
    use pg_sql::minimum_version::minimum_version_above;

    // The missing alias needs 16 and comes first; `JSON_VALUE` needs 17.
    let source = "SELECT * FROM (SELECT 1) WHERE JSON_VALUE(j, '$.a') IS NOT NULL";
    let lexed = lex(source);
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).expect("statement parses");

    let minimum = minimum_version(&parsed).expect("a requirement");
    assert_eq!(minimum.version(), TargetVersion::Pg17);

    let above_14 = minimum_version_above(&parsed, TargetVersion::Pg14).expect("above 14");
    assert_eq!(above_14.version(), TargetVersion::Pg16);
    assert_eq!(above_14.construct(), "ParenQueryRef.alias");

    // The trait gives the same answers as methods.
    let above_16 = parsed
        .minimum_version_above(TargetVersion::Pg16)
        .expect("above 16");
    assert_eq!(above_16.version(), TargetVersion::Pg17);

    assert!(minimum_version_above(&parsed, TargetVersion::Pg17).is_none());
}

/// A gate that carries message text cites the `gram.y` line the text comes
/// from (`CLAUDE.md` principle 11).
#[cfg(feature = "since-pg16")]
#[test]
fn every_gate_that_carries_a_message_cites_its_source() {
    for source in [
        "SELECT * FROM (SELECT 1)",
        "SELECT * FROM LATERAL (SELECT 1)",
    ] {
        let lexed = lex(source);
        let mut input = lexed.input();
        let parsed = Statement::parse(&mut input).expect("statement parses");
        let found = minimum_version(&parsed).expect("a requirement");
        assert!(found.message().is_some());
        let citation = found.citation().expect("a citation");
        assert!(citation.contains("gram.y"), "{citation}");
    }
}

/// Class 2: a gate on a `tokens!` entry, which the parsed value cannot record.
/// The scan over the tokens reports it, with the span of the token.
#[test]
fn a_lexical_gate_reports_the_version_that_widened_the_pattern() {
    use pg_sql::TokenKind;
    use pg_sql::minimum_version::{
        LEXICAL_GATES, lexical_minimum_version, lexical_minimum_version_above,
    };

    // Every gate names a kind, a version above the baseline and a citation.
    assert!(!LEXICAL_GATES.is_empty());
    for gate in LEXICAL_GATES {
        assert!(gate.version() > TargetVersion::Pg14, "{}", gate.site());
    }

    // Nothing lexical in a plain statement.
    let lexed = lex("SELECT 1071");
    assert!(lexical_minimum_version(lexed.tokens()).is_none());

    if pg_sql::TARGET_VERSION >= TargetVersion::Pg16 {
        for (source, start) in [
            ("SELECT 0x42F", 7),
            ("SELECT 0o273", 7),
            ("SELECT 0b100101", 7),
            ("SELECT 1_000", 7),
            ("SELECT 1_000.5", 7),
        ] {
            let lexed = lex(source);
            assert_eq!(lexed.errors().count(), 0, "{source} lexes");
            let found = lexical_minimum_version(lexed.tokens()).expect(source);
            assert_eq!(found.version(), TargetVersion::Pg16, "{source}");
            assert_eq!(
                found.span().map(|span| span.start()),
                Some(start),
                "{source}"
            );
            assert!(found.citation().is_some(), "{source}");
            let tokens = lexed.tokens().collect::<Vec<_>>();
            assert!(
                lexical_minimum_version_above(tokens.iter().copied(), TargetVersion::Pg15)
                    .is_some(),
                "{source}"
            );
            assert!(
                lexical_minimum_version_above(tokens.iter().copied(), TargetVersion::Pg16)
                    .is_none(),
                "{source}"
            );
        }

        // A plain decimal literal of the same kind needs nothing.
        let lexed = lex("SELECT 42, 4.25");
        assert!(lexical_minimum_version(lexed.tokens()).is_none());
        assert!(
            lexed
                .tokens()
                .any(|token| token.kind() == TokenKind::IntegerLit)
        );
    }

    if pg_sql::TARGET_VERSION >= TargetVersion::Pg17 {
        // A vertical tab joins the parts of a quote-continued string only
        // from 17.
        let source = "SELECT 'a'\u{b}\n'b'";
        let lexed = lex(source);
        let found = lexical_minimum_version(lexed.tokens()).expect("a continued string");
        assert_eq!(found.version(), TargetVersion::Pg17);
    }
}
