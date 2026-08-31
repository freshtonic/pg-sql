use std::fs;

use pg_sql::ast::{FileItem, PsqlCommand, parse_sql_file_with_spans};
use recursa::Input;

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let corpus_root = arguments.next().expect("corpus root argument");
    let membership_path = arguments.next().expect("membership path argument");
    assert!(arguments.next().is_none(), "exactly two arguments");

    let membership = fs::read_to_string(&membership_path).expect("read corpus membership");
    for file in membership.lines() {
        let path = std::path::Path::new(&corpus_root).join(file);
        let source = fs::read_to_string(&path).expect("read corpus fixture");
        let lexed = pg_sql::tokens::pg_lex(&source);
        let mut input = Input::new(&source, &lexed);
        let items = match parse_sql_file_with_spans(&mut input) {
            Ok(items) => items,
            Err(error) => {
                eprintln!("{file}: legacy whole-file extraction failed: {error}");
                continue;
            }
        };

        for (item, span) in items {
            let legacy_item_kind = match item {
                FileItem::Command(PsqlCommand::Statement(_)) => 'S',
                FileItem::ParseError { .. } => 'E',
                _ => continue,
            };
            let statement = &source[span.clone()];
            if has_psql_interpolation(statement) {
                continue;
            }
            println!("{file}\t{}\t{}\t{legacy_item_kind}", span.start, span.end);
        }
    }
}

fn has_psql_interpolation(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b':' {
            match bytes.get(index + 1) {
                Some(b':') => {
                    index += 2;
                    continue;
                }
                Some(b'\'') | Some(b'"') => return true,
                Some(&byte) if byte.is_ascii_alphabetic() || byte == b'_' => return true,
                _ => {}
            }
        }
        index += 1;
    }
    false
}
