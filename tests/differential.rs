//! Differential parser test: pg-sql vs PostgreSQL 17.9's raw_parser.
//! See docs/plans/2026-05-21-differential-parser-testing-design.md.

mod support;

use std::collections::BTreeSet;

use pg_oracle::parse_ok;
use support::baseline::{Baseline, BaselineOutcome, FrozenStatements, OutcomeCounts};
use support::diff_check::{Outcome, check_statement};

fn fixture_name(name: &str) -> &str {
    name.strip_prefix("r#").unwrap_or(name)
}

fn baseline_name(name: &str) -> String {
    format!("{}.sql", fixture_name(name))
}

fn run_corpus_file(name: &str) {
    // Fixture names that collide with Rust keywords (`async`, `box`, `enum`)
    // are written as raw identifiers in `corpus_tests!`. `stringify!` keeps
    // the `r#` prefix, but the fixture file on disk has none — strip it so
    // the path resolves.
    let name = fixture_name(name);
    let baseline = Baseline::pinned();
    let baseline_name = baseline_name(name);
    let expected = baseline.file(&baseline_name);
    let frozen = FrozenStatements::pinned().file(&baseline_name);
    let path = format!(
        "{}/vendor/postgres/src/test/regress/sql/{name}.sql",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    let source_git_blob = git_blob(&path);
    assert_eq!(
        source_git_blob, frozen.source_git_blob,
        "{name}: source fixture differs from the frozen PostgreSQL blob"
    );

    let statements = frozen
        .statements(&text)
        .unwrap_or_else(|error| panic!("{name}: cannot load frozen statements: {error}"));
    assert_eq!(
        statements.len(),
        expected.statements,
        "{name}: extracted statement count changed from the frozen baseline"
    );

    let mut actual = OutcomeCounts {
        pass: 0,
        skip: 0,
        fail: 0,
    };
    let mut skips = Vec::new();
    let mut failures = Vec::new();
    let mut identity_mismatches = Vec::new();

    for (i, (source, legacy_item_kind)) in statements
        .into_iter()
        .zip(frozen.legacy_item_kinds())
        .enumerate()
    {
        let stmt = support::Stmt {
            source: source.to_owned(),
        };
        let expected_outcome = legacy_item_kind.expected_outcome(parse_ok(source));
        let outcome = check_statement(&stmt);
        let matches_frozen_identity = matches!(
            (expected_outcome, &outcome),
            (BaselineOutcome::Pass, Outcome::Pass)
                | (BaselineOutcome::Skip, Outcome::Skip(_) | Outcome::Pass)
        );
        if !matches_frozen_identity {
            identity_mismatches.push(format!(
                "  stmt {i}: expected {expected_outcome:?}, got {outcome:?}"
            ));
        }
        match outcome {
            Outcome::Pass => actual.pass += 1,
            Outcome::Skip(reason) => {
                actual.skip += 1;
                skips.push(format!("  stmt {i}: {reason}"));
            }
            Outcome::Fail(reason) => {
                actual.fail += 1;
                failures.push(format!("  stmt {i}: {reason}"));
            }
        }
    }

    eprintln!(
        "[{name}] pass={} skip={} fail={}",
        actual.pass, actual.skip, actual.fail
    );
    assert!(
        identity_mismatches.is_empty(),
        "{name}: statement outcomes changed identity while aggregate counts may still match:\n{}",
        identity_mismatches.join("\n")
    );
    assert_eq!(
        actual.fail,
        0,
        "{name}: differential failures were introduced:\n{}",
        failures.join("\n")
    );
    assert!(
        actual.skip <= expected.outcomes.skip,
        "{name}: skip count increased from {} to {}:\n{}",
        expected.outcomes.skip,
        actual.skip,
        skips.join("\n")
    );
    assert!(
        actual.pass >= expected.outcomes.pass,
        "{name}: pass count fell from {} to {}",
        expected.outcomes.pass,
        actual.pass
    );
}

fn git_blob(path: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["hash-object", "--no-filters", path])
        .output()
        .unwrap_or_else(|error| panic!("cannot hash {path}: {error}"));
    assert!(
        output.status.success(),
        "git hash-object failed for {path}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git object ID is UTF-8")
        .trim()
        .to_owned()
}

macro_rules! corpus_tests {
    ($($name:ident),* $(,)?) => {
        const CORPUS_FILES: &[&str] = &[$(stringify!($name)),*];

        $(
            #[test]
            fn $name() { run_corpus_file(stringify!($name)); }
        )*

        #[test]
        fn corpus_membership_matches_frozen_baseline() {
            let declared = CORPUS_FILES
                .iter()
                .map(|name| baseline_name(name))
                .collect::<BTreeSet<_>>();
            let expected = Baseline::pinned()
                .file_names()
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();

            assert_eq!(declared, expected);
            assert_eq!(
                FrozenStatements::pinned().file_names(),
                Baseline::pinned().file_names()
            );
            assert_eq!(FrozenStatements::pinned().total_statements(), 43_474);
        }
    };
}

// One entry per file in the PostgreSQL submodule's regression SQL
// corpus (pg-sql/vendor/postgres/src/test/regress/sql/). Generated with:
//   ls pg-sql/vendor/postgres/src/test/regress/sql/*.sql \
//     | xargs -n1 basename | sed 's/\.sql$//' \
//     | grep -E '^[A-Za-z_][A-Za-z0-9_]*$' | tr '\n' ','
//
// The four `collate.*` fixtures (collate.icu.utf8, collate.linux.utf8,
// collate.utf8, collate.windows.win1252) carry dots and are not valid Rust
// identifiers, so they cannot be macro-generated test functions and are
// omitted. The dotless `collate` fixture is included.
corpus_tests! {
    advisory_lock, aggregates, alter_generic, alter_operator, alter_table,
    amutils, arrays, r#async, bit, bitmapops, boolean, r#box, brin_bloom,
    brin_multi, brin, btree_index, case, char, circle, cluster, collate,
    combocid, comments, compression, constraints, conversion, copy, copy2,
    copydml, copyselect, create_aggregate, create_am, create_cast,
    create_function_c, create_function_sql, create_index_spgist,
    create_index, create_misc, create_operator, create_procedure,
    create_role, create_schema, create_table_like, create_table,
    create_type, create_view, database, date, dbsize, delete, dependency,
    domain, drop_if_exists, drop_operator, encoding, r#enum, equivclass,
    errors, euc_kr, event_trigger_login, event_trigger, explain,
    expressions, fast_default, float4, float8, foreign_data, foreign_key,
    functional_deps, generated, geometry, gin, gist, groupingsets, guc,
    hash_func, hash_index, hash_part, horology, identity, incremental_sort,
    index_including_gist, index_including, indexing, indirect_toast, inet,
    infinite_recurse, inherit, init_privs, insert_conflict, insert, int2,
    int4, int8, interval, join_hash, join, json_encoding, json,
    jsonb_jsonpath, jsonb, jsonpath_encoding, jsonpath, largeobject, limit,
    line, lock, lseg, macaddr, macaddr8, maintain_every, matview, md5,
    memoize, merge, misc_functions, misc_sanity, misc, money,
    multirangetypes, mvcc, name, namespace, numeric_big, numeric,
    numerology, object_address, oid, oidjoins, opr_sanity,
    partition_aggregate, partition_info, partition_join, partition_prune,
    password, path, pg_lsn, plancache, plpgsql, point, polygon,
    polymorphism, portals_p2, portals, predicate, prepare, prepared_xacts,
    privileges, psql_crosstab, psql, publication, random, rangefuncs,
    rangetypes, regex, regproc, reindex_catalog, reloptions,
    replica_identity, returning, roleattributes, rowsecurity, rowtypes,
    rules, sanity_check, security_label, select_distinct_on,
    select_distinct, select_having, select_implicit, select_into,
    select_parallel, select_views, select, sequence, spgist,
    sqljson_jsontable, sqljson_queryfuncs, sqljson, stats_ext, stats,
    strings, subscription, subselect, sysviews, tablesample, tablespace,
    temp, test_setup, text, tid, tidrangescan, tidscan, time, timestamp,
    timestamptz, timetz, transactions, triggers, truncate, tsdicts,
    tsearch, tsrf, tstypes, tuplesort, txid, type_sanity, typed_table,
    unicode, union, updatable_views, update, uuid, vacuum_parallel, vacuum,
    varchar, window, with, write_parallel, xid, xml, xmlmap,
}
