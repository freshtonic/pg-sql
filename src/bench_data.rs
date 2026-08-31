//! Raw-data serialization for the `parse` benchmark harness.
//!
//! Exposed as a library module so the `data.json` serialization is
//! unit-testable without going through the `harness = false` bench binary
//! (whose custom `main` is not exercised by `cargo test`).
//!
//! The format is deliberately flat: every string the harness emits — benchmark
//! names (`[a-z0-9_/]`), the ISO-8601 timestamp, the short commit SHA — is
//! JSON-safe with no characters that require escaping, so the harness writes
//! the JSON by hand rather than pulling in a serializer. One benchmark object
//! per line keeps run-to-run diffs readable.

/// One benchmark's timing within a run: the benchmark name, every parser's
/// median parse time in integer nanoseconds, and the workload byte volume.
///
/// `postgres_ns` is the PostgreSQL 17.9 raw parser exercised via `pg-oracle`.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchRecord {
    pub name: String,
    pub pg_sql_ns: u128,
    pub sqlparser_ns: u128,
    pub postgres_ns: u128,
    pub bytes: u64,
}

/// Serialize one run's raw benchmark data as a JSON object. The `benchmarks`
/// array has one object per line so diffs between successive runs are minimal.
///
/// All inputs are assumed JSON-safe (see the module docs); no escaping is done.
pub fn serialize_data_json(timestamp: &str, commit: &str, records: &[BenchRecord]) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"timestamp\": \"{timestamp}\",\n"));
    s.push_str(&format!("  \"commit\": \"{commit}\",\n"));
    s.push_str("  \"benchmarks\": [\n");
    for (i, r) in records.iter().enumerate() {
        let comma = if i + 1 < records.len() { "," } else { "" };
        s.push_str(&format!(
            "    {{ \"name\": \"{}\", \"pg_sql_ns\": {}, \"sqlparser_ns\": {}, \"postgres_ns\": {}, \"bytes\": {} }}{}\n",
            r.name, r.pg_sql_ns, r.sqlparser_ns, r.postgres_ns, r.bytes, comma,
        ));
    }
    s.push_str("  ]\n");
    s.push_str("}\n");
    s
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/bench_data.tests.rs"
));
