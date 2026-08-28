//! Differential parser test: pg-sql vs PostgreSQL 17.9's raw_parser.
//! See docs/plans/2026-05-21-differential-parser-testing-design.md.

mod support;

use support::diff_check::{Outcome, check_statement};
use support::extract_statements;

fn run_corpus_file(name: &str) {
    // Fixture names that collide with Rust keywords (`async`, `box`, `enum`)
    // are written as raw identifiers in `corpus_tests!`. `stringify!` keeps
    // the `r#` prefix, but the fixture file on disk has none — strip it so
    // the path resolves.
    let name = name.strip_prefix("r#").unwrap_or(name);
    let path = format!(
        "{}/vendor/postgres/src/test/regress/sql/{name}.sql",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));

    let mut pass = 0usize;
    let mut skip = 0usize;
    let mut failures = Vec::new();

    for (i, stmt) in extract_statements(&text).into_iter().enumerate() {
        match check_statement(&stmt) {
            Outcome::Pass => pass += 1,
            Outcome::Skip(_) => skip += 1,
            Outcome::Fail(reason) => failures.push(format!("  stmt {i}: {reason}")),
        }
    }

    eprintln!("[{name}] pass={pass} skip={skip} fail={}", failures.len());
    assert!(
        failures.is_empty(),
        "{name}: {} statement(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

macro_rules! corpus_tests {
    ($($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() { run_corpus_file(stringify!($name)); }
        )*
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
