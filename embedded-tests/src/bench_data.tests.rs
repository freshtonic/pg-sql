#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, pg: u128, sp: u128, pgo: u128, bytes: u64) -> BenchRecord {
        BenchRecord {
            name: name.to_string(),
            pg_sql_ns: pg,
            sqlparser_ns: sp,
            postgres_ns: pgo,
            bytes,
        }
    }

    #[test]
    fn serializes_run_metadata_and_benchmark_array() {
        let json = serialize_data_json(
            "2026-05-22T06-14-17Z",
            "099f339",
            &[rec("corpus/boolean", 123456, 98765, 54321, 2048)],
        );
        assert_eq!(
            json,
            "{\n  \"timestamp\": \"2026-05-22T06-14-17Z\",\n  \"commit\": \"099f339\",\n  \
             \"benchmarks\": [\n    \
             { \"name\": \"corpus/boolean\", \"pg_sql_ns\": 123456, \"sqlparser_ns\": 98765, \"postgres_ns\": 54321, \"bytes\": 2048 }\n  \
             ]\n}\n"
        );
    }

    #[test]
    fn writes_one_benchmark_object_per_line() {
        let json = serialize_data_json(
            "2026-05-22T06-14-17Z",
            "099f339",
            &[
                rec("corpus/boolean", 100, 200, 50, 10),
                rec("stress/in_list_100", 300, 400, 250, 20),
            ],
        );
        let bench_lines: Vec<&str> = json
            .lines()
            .filter(|l| l.trim_start().starts_with("{ \"name\""))
            .collect();
        assert_eq!(bench_lines.len(), 2);
        assert!(bench_lines[0].ends_with(','), "all but last get a comma");
        assert!(!bench_lines[1].ends_with(','), "last has no trailing comma");
    }

    #[test]
    fn empty_benchmark_list_yields_empty_array() {
        let json = serialize_data_json("2026-05-22T06-14-17Z", "099f339", &[]);
        assert!(json.contains("\"benchmarks\": [\n  ]"));
    }

    #[test]
    fn round_trips_through_a_json_parser() {
        // The xtask consumer parses these files with serde_json; the harness
        // output must be valid JSON it can read back.
        let json = serialize_data_json(
            "2026-05-22T06-14-17Z",
            "099f339",
            &[
                rec("corpus/boolean", 123456, 98765, 54321, 2048),
                rec("stress/bool_chain_10", 4242, 9001, 3030, 64),
            ],
        );
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("harness JSON must parse");
        assert_eq!(value["timestamp"], "2026-05-22T06-14-17Z");
        assert_eq!(value["commit"], "099f339");
        let benches = value["benchmarks"].as_array().expect("benchmarks array");
        assert_eq!(benches.len(), 2);
        assert_eq!(benches[0]["name"], "corpus/boolean");
        assert_eq!(benches[0]["pg_sql_ns"], 123456);
        assert_eq!(benches[0]["postgres_ns"], 54321);
        assert_eq!(benches[1]["sqlparser_ns"], 9001);
        assert_eq!(benches[1]["postgres_ns"], 3030);
        assert_eq!(benches[1]["bytes"], 64);
    }
}
