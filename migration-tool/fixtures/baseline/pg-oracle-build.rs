use std::path::{Path, PathBuf};

const PG_SOURCE_DIR: &str = "../pg-sql/vendor/postgres";

fn pg_source_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PG_SOURCE_DIR)
}

fn pg_build_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR")).join("postgres")
}

const REQUIRED_GENERATED: &[&str] = &[
    "src/include/pg_config.h",
    "src/backend/parser/gram.c",
    "src/backend/parser/scan.c",
    "src/backend/nodes/equalfuncs.funcs.c",
    "src/backend/nodes/outfuncs.funcs.c",
    "src/include/nodes/nodetags.h",
    "src/common/libpgcommon.a",
    "src/port/libpgport.a",
];

fn first_missing_generated(pg: &Path) -> Option<&'static str> {
    REQUIRED_GENERATED
        .iter()
        .copied()
        .find(|rel| !pg.join(rel).exists())
}

fn verify_pg_built(source: &Path, build: &Path) {
    if !source.join("configure").exists() {
        panic!(
            "PostgreSQL source not found at {}.\n\
             Initialize the submodule: git submodule update --init pg-sql/vendor/postgres",
            source.display()
        );
    }

    if first_missing_generated(build).is_none() {
        return;
    }

    println!(
        "cargo:warning=PostgreSQL is not built yet — running \
         pg-oracle/scripts/build-pg.sh out of tree (slow on the first build)"
    );
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/build-pg.sh");
    let status = std::process::Command::new("bash")
        .arg(&script)
        .arg(build)
        .status()
        .unwrap_or_else(|e| panic!("failed to launch {}: {e}", script.display()));
    if !status.success() {
        panic!(
            "PostgreSQL build ({}) failed with {status}.\n\
             It needs a C toolchain plus `make`, `bison`, `flex` and `perl`.\n\
             See the output above, or run it directly with an output path: \
             ./pg-oracle/scripts/build-pg.sh /tmp/postgres-build",
            script.display()
        );
    }

    if let Some(missing) = first_missing_generated(build) {
        panic!(
            "PostgreSQL build at {} is still missing {} after running build-pg.sh.",
            build.display(),
            missing
        );
    }
}

fn pg_source_file(source: &Path, build: &Path, relative: &str) -> PathBuf {
    let generated = build.join(relative);
    if generated.exists() {
        generated
    } else {
        source.join(relative)
    }
}

const PG_SOURCES: &[&str] = &[
    "src/backend/parser/parser.c",
    "src/backend/parser/gram.c",
    "src/backend/parser/scan.c",
    "src/backend/parser/scansup.c",
    "src/backend/nodes/makefuncs.c",
    "src/backend/nodes/list.c",
    "src/backend/nodes/value.c",
    "src/backend/nodes/bitmapset.c",
    "src/backend/nodes/nodeFuncs.c",
    "src/backend/nodes/copyfuncs.c",
    "src/backend/nodes/equalfuncs.c",
    "src/backend/nodes/outfuncs.c",
    "src/backend/nodes/extensible.c",
    "src/backend/utils/mmgr/mcxt.c",
    "src/backend/utils/mmgr/aset.c",
    "src/backend/utils/mmgr/alignedalloc.c",
    "src/backend/utils/mmgr/generation.c",
    "src/backend/utils/mmgr/slab.c",
    "src/backend/utils/mmgr/bump.c",
    "src/common/keywords.c",
    "src/common/kwlookup.c",
    "src/common/stringinfo.c",
    "src/common/psprintf.c",
    "src/common/encnames.c",
    "src/common/wchar.c",
    "src/backend/utils/adt/numutils.c",
    "src/backend/utils/adt/datum.c",
    "src/backend/utils/adt/expandeddatum.c",
    "src/port/snprintf.c",
    "src/port/pgstrcasecmp.c",
    "src/port/pg_bitutils.c",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc/oracle.c");
    println!("cargo:rerun-if-changed=csrc/pgo_elog_stub.c");
    let pg_source = pg_source_dir();
    let pg_build = pg_build_dir();
    verify_pg_built(&pg_source, &pg_build);

    let mut build = cc::Build::new();
    build
        .include(pg_build.join("src/include"))
        .include(pg_source.join("src/include"))
        .include(pg_build.join("src/backend"))
        .include(pg_build.join("src/backend/parser"))
        .include(pg_build.join("src/backend/nodes"))
        .include(pg_build.join("src/common"))
        .include(pg_source.join("src/backend"))
        .include(pg_source.join("src/backend/parser"))
        .include(pg_source.join("src/backend/nodes"))
        .include(pg_source.join("src/common"))
        .include("csrc")
        .flag_if_supported("-w")
        .flag_if_supported("-fno-strict-aliasing")
        .flag_if_supported("-fwrapv");

    build.file("csrc/oracle.c");
    build.file("csrc/pgo_elog_stub.c");
    for rel in PG_SOURCES {
        build.file(pg_source_file(&pg_source, &pg_build, rel));
    }
    build.compile("pgoracle");

    println!(
        "cargo:rustc-link-search=native={}",
        pg_build.join("src/common").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        pg_build.join("src/port").display()
    );
    println!("cargo:rustc-link-lib=static=pgcommon");
    println!("cargo:rustc-link-lib=static=pgport");
}
