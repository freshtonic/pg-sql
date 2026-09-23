//! What psql's own lexer reports, checked against `psqlscan.l` directly.
//!
//! These tests keep the shim honest: they do not compare anything with
//! pg-psql, they only show that the oracle reports what psql does. The
//! comparison lives in `pg-psql/tests/psql_oracle.rs`.

use pg_psql_oracle::{Event, Prompt, Quote, scan};

fn interpolations(source: &str, variables: &[(&str, &str)]) -> Vec<(String, Quote, bool)> {
    scan(source, variables)
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Interpolation {
                name, quote, bound, ..
            } => Some((name.clone(), *quote, *bound)),
            _ => None,
        })
        .collect()
}

fn terminators(source: &str) -> usize {
    scan(source, &[])
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Terminator { .. }))
        .count()
}

#[test]
fn the_oracle_names_its_postgresql_release() {
    assert_eq!(pg_psql_oracle::POSTGRES_COMMIT.len(), 40);
    assert!(pg_psql_oracle::POSTGRES_REF.starts_with("REL_"));
}

#[test]
fn forwarded_text_is_the_document_when_nothing_is_rewritten() {
    // psql forwards its input unchanged where no rule rewrites it, so the
    // oracle's text is the document itself. The seeded query buffer is what
    // keeps the leading trivia; see csrc/psql_oracle.c.
    for source in [
        "SELECT 1;",
        "  SELECT 1;",
        "/* block */ SELECT 1;",
        "SELECT 'a;b';",
    ] {
        assert_eq!(scan(source, &[]).sql(), source, "forwarding {source:?}");
    }
}

#[test]
fn a_line_comment_is_forwarded_from_15() {
    // psql 14 does not echo a `{whitespace}` match that starts with `-`, so
    // it removes every `--` comment from the query it sends and keeps the
    // line ending. 15 sends the comment
    // (docs/research/psql-14-19-syntax-changes.md, class (A) item A3;
    // `REL_14_24:psqlscan.l` 388, commit 83884682f).
    let source = "\n-- a comment\nSELECT 1;\n";
    #[cfg(feature = "since-pg15")]
    assert_eq!(scan(source, &[]).sql(), source);
    #[cfg(not(feature = "since-pg15"))]
    assert_eq!(scan(source, &[]).sql(), "\n\nSELECT 1;\n");
}

#[test]
fn each_interpolation_form_is_reported_once() {
    let bindings = [("v", "42")];
    assert_eq!(
        interpolations("SELECT :v, :'v', :\"v\", :{?v}", &bindings),
        vec![
            ("v".to_owned(), Quote::Plain, true),
            ("v".to_owned(), Quote::Literal, true),
            ("v".to_owned(), Quote::Identifier, true),
            ("v".to_owned(), Quote::Plain, true),
        ]
    );
    assert_eq!(
        scan("SELECT :v, :'v', :\"v\", :{?v}", &bindings).sql(),
        "SELECT 42, '42', \"42\", TRUE"
    );
}

#[test]
fn an_unbound_variable_is_forwarded_as_it_stands() {
    let source = "SELECT :v, :'v', :\"v\", :{?v}";
    assert_eq!(scan(source, &[]).sql(), "SELECT :v, :'v', :\"v\", FALSE");
    assert!(interpolations(source, &[]).iter().all(|(_, _, bound)| !bound));
}

#[test]
fn an_interpolation_extent_names_the_whole_token() {
    let scanned = scan("SELECT :'v';", &[("v", "1")]);
    let Event::Interpolation { source, .. } = &scanned.events()[0] else {
        panic!("the first event is the interpolation");
    };
    assert_eq!(source.clone().unwrap(), 7..11);
}

#[test]
fn a_colon_inside_a_lexical_shell_is_not_an_interpolation() {
    // The states in which psqlscan.l does not recognise an interpolation.
    for source in [
        "SELECT ':v'",
        "SELECT E':v'",
        "SELECT U&':v'",
        "SELECT B'0:1'",
        "SELECT X'0:1'",
        "SELECT \":v\"",
        "SELECT $$:v$$",
        "SELECT 1 -- :v\n",
        "SELECT 1 /* :v */",
        "SELECT a::v",
        "SELECT a :=v",
        "SELECT \\:v",
    ] {
        assert_eq!(
            interpolations(source, &[("v", "1")]),
            vec![],
            "{source:?} holds no interpolation",
        );
    }
}

#[test]
fn a_semicolon_submits_the_query_buffer_but_a_nested_one_does_not() {
    assert_eq!(terminators("SELECT 1; SELECT 2;"), 2);
    // psqlscan.l suppresses the boundary inside parentheses, and `\;` is not
    // a boundary at all.
    assert_eq!(terminators("SELECT (1;"), 0);
    assert_eq!(terminators("SELECT 1 \\; SELECT 2"), 0);
    assert_eq!(scan("SELECT 1 \\; SELECT 2", &[]).sql(), "SELECT 1 ; SELECT 2");
}

#[test]
fn a_backslash_command_name_is_read_whole() {
    // psqlscanslash.l reads a command name up to whitespace or a backslash,
    // so a longer name is never a send command with a suffix.
    for (source, name) in [
        (r"SELECT 1 \gset", "gset"),
        (r"SELECT 1 \gsetfoo", "gsetfoo"),
        (r"\getenv abs_srcdir", "getenv"),
        (r"\d users", "d"),
        (r"\.", "."),
    ] {
        let scanned = scan(source, &[]);
        assert_eq!(
            scanned.backslash_commands().collect::<Vec<_>>(),
            vec![name],
            "reading {source:?}",
        );
    }
}

#[test]
fn a_backslash_command_forwards_none_of_itself() {
    let scanned = scan("SELECT 1 \\gset x\nSELECT 2;", &[]);
    assert_eq!(scanned.sql(), "SELECT 1 \nSELECT 2;");
    let Event::Backslash {
        name,
        source,
        command,
        ..
    } = &scanned.events()[0]
    else {
        panic!("the first event is the backslash command");
    };
    assert_eq!(name, "gset");
    // The command itself, then the argument psql reads to the line break.
    assert_eq!(*command, 9..14);
    assert_eq!(*source, 9..16);
}

#[test]
fn an_open_lexical_region_leaves_the_buffer_incomplete() {
    for (source, prompt) in [
        ("SELECT 'a", Prompt::SingleQuote),
        ("SELECT \"a", Prompt::DoubleQuote),
        ("SELECT $$a", Prompt::DollarQuote),
        ("SELECT /* a", Prompt::Comment),
        ("SELECT (1", Prompt::Paren),
    ] {
        let scanned = scan(source, &[]);
        assert!(!scanned.is_complete(), "{source:?} is incomplete");
        assert_eq!(scanned.final_prompt(), prompt, "{source:?}");
    }
    assert!(scan("SELECT 1", &[]).is_complete());
}

#[test]
fn copy_from_stdin_is_counted() {
    assert_eq!(scan("COPY t FROM STDIN;\n1\n\\.\n", &[]).copy_from_stdin(), 1);
    assert_eq!(scan("SELECT 1;", &[]).copy_from_stdin(), 0);
}

#[test]
fn a_substituted_value_is_rescanned_only_for_the_plain_form() {
    // psql pushes a `:name` value back into the lexer, so a colon in the
    // value substitutes again. The quoted forms are emitted in place.
    let bindings = [("a", ":b"), ("b", "1")];
    assert_eq!(scan("SELECT :a", &bindings).sql(), "SELECT 1");
    assert_eq!(scan("SELECT :'a'", &bindings).sql(), "SELECT ':b'");
}

#[test]
fn a_literal_value_is_quoted_as_pqescapeliteral_does() {
    assert_eq!(
        scan("SELECT :'v'", &[("v", "it's")]).sql(),
        "SELECT 'it''s'"
    );
    assert_eq!(
        scan("SELECT :'v'", &[("v", "a\\b")]).sql(),
        "SELECT  E'a\\\\b'"
    );
    assert_eq!(
        scan("SELECT :\"v\"", &[("v", "a\"b")]).sql(),
        "SELECT \"a\"\"b\""
    );
}

#[test]
fn the_send_commands_of_this_release_are_classified() {
    use pg_psql_oracle::{Command, classify};
    // `exec_command_*` returns `PSQL_CMD_SEND` for each of these in every
    // target version (`REL_17_11:command.c` 761, 1474, 1570, 1624, 1651).
    for name in ["g", "gx", "gset", "gdesc", "gexec", "crosstabview"] {
        assert_eq!(classify(name), Command::Send, "\\{name}");
    }
    // A name psqlscanslash.l reads whole is not a send command with a
    // suffix.
    for name in ["gsetfoo", "gexecx", "getenv", "set", "d", "", "."] {
        assert_eq!(classify(name), Command::Other, "\\{name}");
    }
    // `\watch` and `\r` end the query buffer without `PSQL_CMD_SEND`, so the
    // table does not cover them (docs/psql-oracle.md).
    assert_eq!(classify("watch"), Command::Other);
    assert_eq!(classify("r"), Command::Other);
}

// Added in 18: nine commands end the query buffer, and two of them send its
// text (docs/research/psql-14-19-syntax-changes.md, class (A) item A1).
#[cfg(feature = "since-pg18")]
#[test]
fn the_18_pipeline_commands_are_classified() {
    use pg_psql_oracle::{Command, classify};
    for name in ["parse", "sendpipeline"] {
        assert_eq!(classify(name), Command::Send, "\\{name}");
    }
    for name in [
        "close_prepared",
        "endpipeline",
        "flush",
        "flushrequest",
        "getresults",
        "startpipeline",
        "syncpipeline",
    ] {
        assert_eq!(classify(name), Command::Discard, "\\{name}");
    }
}

// Before 18 the pipeline commands do not exist, so psql reads each one as an
// unknown command.
#[cfg(not(feature = "since-pg18"))]
#[test]
fn the_18_pipeline_commands_are_unknown_before_18() {
    use pg_psql_oracle::{Command, classify};
    for name in ["parse", "sendpipeline", "startpipeline", "close_prepared"] {
        assert_eq!(classify(name), Command::Other, "\\{name}");
    }
}
