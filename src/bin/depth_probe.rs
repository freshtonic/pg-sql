//! Microbenchmark for nested_subquery scaling. Prints per-depth parse
//! time so we can see whether the cost is linear, quadratic, or
//! exponential in nesting depth.
use pg_sql::ast::parse_sql_file;
use std::time::Instant;

fn main() {
    println!(
        "{:>5} {:>8} {:>12} {:>8}",
        "depth", "len", "per_parse", "ratio"
    );
    let mut prev_ns: Option<u128> = None;
    for depth in 1..=15 {
        let mut sql = String::from("SELECT * FROM ");
        for _ in 0..depth {
            sql.push_str("(SELECT * FROM ");
        }
        sql.push('t');
        for i in 0..depth {
            sql.push_str(&format!(") s{i}"));
        }
        sql.push_str(";\n");

        // Warm up
        for _ in 0..10 {
            let lexed = pg_sql::tokens::pg_lex(&sql);
            let mut input = recursa::Input::new(&sql, &lexed);
            let _ = parse_sql_file(&mut input);
        }

        let n: u32 = if depth <= 8 { 1000 } else { 100 };
        let start = Instant::now();
        for _ in 0..n {
            let lexed = pg_sql::tokens::pg_lex(&sql);
            let mut input = recursa::Input::new(&sql, &lexed);
            let _ = parse_sql_file(&mut input).unwrap();
        }
        let elapsed = start.elapsed();
        let per_iter_ns = elapsed.as_nanos() / n as u128;
        let ratio = match prev_ns {
            Some(p) => format!("{:.2}x", per_iter_ns as f64 / p as f64),
            None => "—".to_string(),
        };
        println!(
            "{:>5} {:>8} {:>10}ns {:>8}",
            depth,
            sql.len(),
            per_iter_ns,
            ratio
        );
        prev_ns = Some(per_iter_ns);
    }
}
