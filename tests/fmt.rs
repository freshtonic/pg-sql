use pg_sql::ast::{FileItem, parse_sql_file};
use pg_sql::formatter::format_tokens_sql;
use recursa::Input;
use recursa::fmt::FormatStyle;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fmt");

fn update_mode() -> bool {
    std::env::var("RECURSA_UPDATE_FIXTURES")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn parse_fmt_header(src: &str) -> FormatStyle {
    let mut style = FormatStyle::default();
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
                "max_width" => style.max_width = v.parse().expect("max_width parses"),
                "indent_width" => style.indent_width = v.parse().expect("indent_width parses"),
                "uppercase_keywords" => {
                    style.uppercase_keywords = v.parse().expect("uppercase_keywords parses");
                }
                "leading_commas" => {
                    style.leading_commas = v.parse().expect("leading_commas parses");
                }
                other => panic!("unknown fmt key: {other}"),
            }
        }
    }
    style
}

fn format_sql(src: &str, style: FormatStyle) -> String {
    let lexed = pg_sql::tokens::pg_lex(src);
    let mut input = Input::new(src, &lexed);
    let items = parse_sql_file(&mut input).expect("parse");
    match items.first().expect("at least one item") {
        FileItem::Command(cmd) => format_tokens_sql(cmd, style),
        FileItem::RawLines(_) => {
            panic!("fixture parsed as RawLines, not a Command — wrong fixture content")
        }
        FileItem::ParseError { .. } => panic!("fixture parsed as ParseError"),
    }
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
    let style = parse_fmt_header(&input);

    let actual = format_sql(&input, style.clone());

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
    let lexed = pg_sql::tokens::pg_lex(&golden);
    let mut parser_input = Input::new(&golden, &lexed);
    parse_sql_file(&mut parser_input)
        .unwrap_or_else(|e| panic!("fixture {name}: golden does not parse: {e:?}"));
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
