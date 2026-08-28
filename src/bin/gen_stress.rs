//! Deterministically regenerates stress-test SQL fixtures under
//! `pg-sql/fixtures/stress/`. Run with `cargo run --bin gen-stress -p pg-sql`.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = root.join("fixtures/stress");
    fs::create_dir_all(&out).expect("create stress dir");

    for n in [100usize, 1_000, 10_000] {
        write(&out, &format!("insert_values_{n}.sql"), &insert_values(n));
    }
    for n in [10usize, 100, 1_000] {
        write(&out, &format!("bool_chain_{n}.sql"), &bool_chain(n));
    }
    for n in [100usize, 1_000, 10_000] {
        write(&out, &format!("select_list_{n}.sql"), &select_list(n));
    }
    for n in [5usize, 10, 15] {
        write(
            &out,
            &format!("nested_subquery_{n}.sql"),
            &nested_subquery(n),
        );
    }
    for n in [100usize, 1_000, 10_000] {
        write(&out, &format!("in_list_{n}.sql"), &in_list(n));
    }

    println!("wrote stress fixtures to {}", out.display());
}

fn write(dir: &Path, name: &str, contents: &str) {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn insert_values(n: usize) -> String {
    let mut s = String::from("INSERT INTO t (a, b, c, d) VALUES\n");
    for i in 0..n {
        if i > 0 {
            s.push_str(",\n");
        }
        s.push_str(&format!("  ({i}, 'x{i}', {i}.5, true)"));
    }
    s.push_str(";\n");
    s
}

fn bool_chain(n: usize) -> String {
    let mut s = String::from("SELECT 1 WHERE ");
    for i in 0..n {
        if i > 0 {
            s.push_str(" AND ");
        }
        s.push('a');
    }
    s.push_str(";\n");
    s
}

fn select_list(n: usize) -> String {
    let mut s = String::from("SELECT ");
    for i in 0..n {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&format!("c{i}"));
    }
    s.push_str(" FROM t;\n");
    s
}

fn nested_subquery(n: usize) -> String {
    let mut s = String::from("SELECT * FROM ");
    for _ in 0..n {
        s.push_str("(SELECT * FROM ");
    }
    s.push('t');
    for i in 0..n {
        s.push_str(&format!(") s{i}"));
    }
    s.push_str(";\n");
    s
}

fn in_list(n: usize) -> String {
    let mut s = String::from("SELECT * FROM t WHERE x IN (");
    for i in 0..n {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&format!("{i}"));
    }
    s.push_str(");\n");
    s
}
