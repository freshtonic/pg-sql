use std::path::{Path, PathBuf};

#[path = "../build-support/target_version.rs"]
mod target_version;

/// The pin table: one row `feature<TAB>ref<TAB>commit` for each target version.
const PINS: &str = "pins.tsv";

/// The pinned PostgreSQL release of one target version.
struct Pin {
    feature: String,
    reference: String,
    commit: String,
}

fn pin_for(feature: &str) -> Pin {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PINS);
    let table = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    table
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3, "{PINS}: a row has three fields: {line:?}");
            Pin {
                feature: fields[0].to_owned(),
                reference: fields[1].to_owned(),
                commit: fields[2].to_owned(),
            }
        })
        .find(|pin| pin.feature == feature)
        .unwrap_or_else(|| panic!("{PINS} has no pin for the version feature {feature}"))
}

/// The PostgreSQL tree of this build: the pinned commit, extracted and built
/// in place under `OUT_DIR`. `OUT_DIR` differs for each feature set, so each
/// target version has its own tree.
fn pg_tree_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR")).join("postgres")
}

/// Files that only exist after `configure && make`. If any is missing, the
/// tree is not built. The second field is the first PostgreSQL major that
/// generates the file: the node support files are generated from 16.
const REQUIRED_GENERATED: &[(&str, u32)] = &[
    ("src/include/pg_config.h", 14),
    ("src/backend/parser/gram.c", 14),
    ("src/backend/parser/scan.c", 14),
    ("src/backend/nodes/equalfuncs.funcs.c", 16),
    ("src/backend/nodes/outfuncs.funcs.c", 16),
    ("src/include/nodes/nodetags.h", 16),
    ("src/common/libpgcommon.a", 14),
    ("src/port/libpgport.a", 14),
];

/// First `REQUIRED_GENERATED` entry that does not yet exist — `None` once the
/// tree is fully built.
fn first_missing_generated(tree: &Path, major: u32) -> Option<&'static str> {
    REQUIRED_GENERATED
        .iter()
        .filter(|(_, since)| major >= *since)
        .map(|(rel, _)| *rel)
        .find(|rel| !tree.join(rel).exists())
}

/// File in the tree that records the PostgreSQL commit it was extracted from.
const SOURCE_COMMIT_STAMP: &str = "pg-oracle-source-commit";

/// Ensure the pinned PostgreSQL tree is built far enough for pg-oracle to
/// link.
///
/// The first build of each target version extracts the pinned commit from
/// the `vendor/postgres` object store and runs `configure && make` with
/// `scripts/build-pg.sh`. The script is idempotent, so the slow path runs
/// only once. When the pin moves, the commit stamp does not agree, and the
/// script builds the tree again from an empty directory, so the oracle always
/// comes from the pinned release.
fn verify_pg_built(pin: &Pin, major: u32, tree: &Path) {
    let stamp = std::fs::read_to_string(tree.join(SOURCE_COMMIT_STAMP)).ok();
    let same_commit = stamp.as_deref().map(str::trim) == Some(pin.commit.as_str());
    if same_commit && first_missing_generated(tree, major).is_none() {
        return;
    }

    // The PostgreSQL build is slow the first time; surface that to the user
    // since build-script output is otherwise buffered until completion.
    eprintln!(
        "PostgreSQL {} is not built — running pg-oracle/scripts/build-pg.sh \
         (slow on the first build)",
        pin.reference
    );
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/build-pg.sh");
    let status = std::process::Command::new("bash")
        .arg(&script)
        .arg(&pin.feature)
        .arg(tree)
        .status()
        .unwrap_or_else(|e| panic!("failed to launch {}: {e}", script.display()));
    if !status.success() {
        panic!(
            "PostgreSQL build ({}) failed with {status}.\n\
             It needs a C toolchain plus `make`, `bison`, `flex` and `perl`, and the\n\
             pinned commit {} in the vendor/postgres object store.\n\
             See the output above, or run it directly: \
             ./pg-oracle/scripts/build-pg.sh {}",
            script.display(),
            pin.commit,
            pin.feature
        );
    }

    if let Some(missing) = first_missing_generated(tree, major) {
        panic!(
            "PostgreSQL build at {} is still missing {} after running \
             build-pg.sh.",
            tree.display(),
            missing
        );
    }
}

/// PostgreSQL backend `.c` files compiled into the oracle static lib, with
/// the first PostgreSQL major that has each file.
/// SEED LIST — extended empirically by the Task 4 link loop until the
/// parser links. Reference: /tmp/libpg_query-ref Makefile + src/postgres/.
const PG_SOURCES: &[(&str, u32)] = &[
    // Parser proper.
    ("src/backend/parser/parser.c", 14),
    ("src/backend/parser/gram.c", 14),
    ("src/backend/parser/scan.c", 14),
    ("src/backend/parser/scansup.c", 14),
    // Node support.
    ("src/backend/nodes/makefuncs.c", 14),
    ("src/backend/nodes/list.c", 14),
    ("src/backend/nodes/value.c", 14),
    ("src/backend/nodes/bitmapset.c", 14),
    ("src/backend/nodes/nodeFuncs.c", 14),
    ("src/backend/nodes/copyfuncs.c", 14),
    ("src/backend/nodes/equalfuncs.c", 14),
    // outfuncs.c provides nodeToString; the generated outfuncs.funcs.c /
    // outfuncs.switch.c are #included by it, so they need no entry.
    ("src/backend/nodes/outfuncs.c", 14),
    ("src/backend/nodes/extensible.c", 14),
    // Memory management.
    ("src/backend/utils/mmgr/mcxt.c", 14),
    ("src/backend/utils/mmgr/aset.c", 14),
    ("src/backend/utils/mmgr/alignedalloc.c", 16),
    ("src/backend/utils/mmgr/generation.c", 14),
    ("src/backend/utils/mmgr/slab.c", 14),
    ("src/backend/utils/mmgr/bump.c", 17),
    // Common (frontend-shared) helpers.
    ("src/common/keywords.c", 14),
    ("src/common/kwlookup.c", 14),
    ("src/common/stringinfo.c", 14),
    ("src/common/psprintf.c", 14),
    ("src/common/encnames.c", 14),
    ("src/common/wchar.c", 14),
    // Numeric literal scanning used by the grammar.
    ("src/backend/utils/adt/numutils.c", 14),
    // Datum copy/compare used by copyfuncs/equalfuncs.
    ("src/backend/utils/adt/datum.c", 14),
    ("src/backend/utils/adt/expandeddatum.c", 14),
    // Port helpers.
    ("src/port/snprintf.c", 14),
    ("src/port/pgstrcasecmp.c", 14),
    ("src/port/pg_bitutils.c", 14),
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc/oracle.c");
    println!("cargo:rerun-if-changed=csrc/pgo_elog_stub.c");
    println!("cargo:rerun-if-changed=scripts/build-pg.sh");
    println!("cargo:rerun-if-changed={PINS}");
    println!("cargo:rerun-if-changed=../build-support/target_version.rs");
    let feature = target_version::require_one_target_version("pg-oracle");
    let major = target_version::VERSION_FEATURES
        .iter()
        .find(|(name, _)| *name == feature)
        .map(|(_, major)| *major)
        .expect("a version feature has a major version");
    let pin = pin_for(feature);
    // The tests compare these with the pin of their baseline.
    println!("cargo:rustc-env=PG_ORACLE_POSTGRES_REF={}", pin.reference);
    println!("cargo:rustc-env=PG_ORACLE_POSTGRES_COMMIT={}", pin.commit);
    let tree = pg_tree_dir();
    verify_pg_built(&pin, major, &tree);

    let mut build = cc::Build::new();
    build
        .include(tree.join("src/include"))
        .include(tree.join("src/backend"))
        .include(tree.join("src/backend/parser"))
        .include(tree.join("src/backend/nodes"))
        .include(tree.join("src/common"))
        .include("csrc")
        .flag_if_supported("-w") // PG sources warn a lot
        .flag_if_supported("-fno-strict-aliasing")
        .flag_if_supported("-fwrapv");

    // Our shim + scaffolding.
    build.file("csrc/oracle.c");
    build.file("csrc/pgo_elog_stub.c");

    // PostgreSQL backend sources of this major.
    for (rel, since) in PG_SOURCES {
        if major >= *since {
            build.file(tree.join(rel));
        }
    }

    build.compile("pgoracle");

    // PostgreSQL's own static libs cover most remaining symbols.
    println!(
        "cargo:rustc-link-search=native={}",
        tree.join("src/common").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        tree.join("src/port").display()
    );
    println!("cargo:rustc-link-lib=static=pgcommon");
    println!("cargo:rustc-link-lib=static=pgport");
}
