use pg_sql::ast::Statement;
use pg_sql::formatter::format_tokens_sql;
use recursa::PrettyConfig;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fmt");

fn update_mode() -> bool {
    std::env::var("RECURSA_UPDATE_FIXTURES")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn parse_fmt_header(src: &str) -> Result<PrettyConfig, &'static str> {
    let defaults = PrettyConfig::default();
    let mut max_width = defaults.max_width();
    let mut indent_width = defaults.indent();
    for line in src.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("--") else {
            break;
        };
        let rest = rest.trim_start();
        let Some(kvs) = rest.strip_prefix("fmt:") else {
            continue;
        };
        for kv in kvs.split_whitespace() {
            let Some((k, v)) = kv.split_once('=') else {
                continue;
            };
            match k {
                "max_width" => max_width = v.parse().expect("max_width parses"),
                "indent_width" => indent_width = v.parse().expect("indent_width parses"),
                "uppercase_keywords" => {
                    if !v.parse::<bool>().expect("uppercase_keywords parses") {
                        return Err("lowercase keyword rendering is not supported");
                    }
                }
                "leading_commas" => {
                    if v.parse::<bool>().expect("leading_commas parses") {
                        return Err("leading comma rendering is not supported");
                    }
                }
                other => panic!("unknown fmt key: {other}"),
            }
        }
    }
    Ok(PrettyConfig::new(max_width, indent_width))
}

fn format_sql(src: &str, style: PrettyConfig) -> String {
    let source = src.trim_end();
    let source = source.strip_suffix(';').unwrap_or(source);
    let lexed = pg_sql::lex(source);
    assert!(
        lexed.errors().next().is_none(),
        "formatter fixture has lexical errors"
    );
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).expect("strict PostgreSQL statement");
    assert!(input.is_eof(), "formatter fixture contains trailing input");
    let ast = parsed.into_ast();
    format!("{};", format_tokens_sql(&ast, style))
}

fn run_fixture(name: &str) {
    let input_path = format!("{FIXTURE_DIR}/{name}.input.sql");
    let golden_path = format!("{FIXTURE_DIR}/{name}.golden.sql");
    let input = std::fs::read_to_string(&input_path).expect("read input");
    let golden = if update_mode() {
        std::fs::read_to_string(&golden_path).unwrap_or_default()
    } else {
        std::fs::read_to_string(&golden_path).expect("read golden")
    };
    let style = match parse_fmt_header(&input) {
        Ok(style) => style,
        Err(error) => {
            assert_eq!(name, "uppercase_off", "unexpected unsupported fixture");
            assert_eq!(error, "lowercase keyword rendering is not supported");
            return;
        }
    };

    let actual = format_sql(&input, style);

    if update_mode() {
        std::fs::write(&golden_path, format!("{}\n", actual.trim_end())).expect("write golden");
        eprintln!("WRITING FIXTURES: {name}.golden.sql updated");
        return;
    }

    assert_eq!(
        actual.trim_end(),
        golden.trim_end(),
        "fixture {name}: format(input) != golden",
    );

    // Idempotence
    let reformatted = format_sql(&golden, style);
    assert_eq!(
        reformatted.trim_end(),
        golden.trim_end(),
        "fixture {name}: format(golden) != golden (non-idempotent)",
    );

    // Parse-golden guard: the golden must itself parse.
    let golden_source = golden.trim_end();
    let golden_source = golden_source.strip_suffix(';').unwrap_or(golden_source);
    let lexed = pg_sql::lex(golden_source);
    assert!(
        lexed.errors().next().is_none(),
        "fixture {name}: golden has lexical errors"
    );
    let mut parser_input = lexed.input();
    Statement::parse(&mut parser_input)
        .unwrap_or_else(|e| panic!("fixture {name}: golden does not parse: {e:?}"));
    assert!(
        parser_input.is_eof(),
        "fixture {name}: golden contains trailing input"
    );
}

#[test]
fn all_fixtures() {
    if update_mode() {
        eprintln!("RECURSA_UPDATE_FIXTURES=1 — writing goldens, not asserting");
    }
    let mut failures = Vec::new();
    let entries = std::fs::read_dir(FIXTURE_DIR).expect("read fixture dir");
    let mut names: Vec<String> = entries
        .filter_map(|e| {
            let path = e.ok()?.path();
            let fname = path.file_name()?.to_str()?.to_string();
            fname.strip_suffix(".input.sql").map(|s| s.to_string())
        })
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures in {FIXTURE_DIR}");

    for name in &names {
        let result = std::panic::catch_unwind(|| run_fixture(name));
        if let Err(payload) = result {
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".to_string());
            failures.push(format!("  {name}: {msg}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{}/{} fixtures failed:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n"),
    );
}
