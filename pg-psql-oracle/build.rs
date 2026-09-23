use std::path::{Path, PathBuf};

#[path = "../build-support/target_version.rs"]
mod target_version;

/// The one pin table of the repository (`docs/differential-baseline.md`).
/// This crate reads pg-oracle's table rather than keeping a second one.
const PINS: &str = "../pg-oracle/pins.tsv";

/// The build script that extracts and builds a pinned PostgreSQL release.
/// It is pg-oracle's, and it takes the tree as an argument, so both oracles
/// use the same extraction and the same configure options.
const BUILD_PG: &str = "../pg-oracle/scripts/build-pg.sh";

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

/// The PostgreSQL tree of this build.
///
/// This is `build-pg.sh`'s own default location, `target/pg-trees/<ref>`, not
/// `OUT_DIR`. The tree is keyed by the pinned release, so one debug build,
/// one release build and a manual `pg-oracle/scripts/build-pg.sh <feature>`
/// all share it, and a psql oracle for each of the six target versions costs
/// six trees rather than six for each Cargo profile. pg-oracle keeps its own
/// tree under its `OUT_DIR`, so the two build scripts never write to the same
/// directory and can run at the same time.
fn pg_tree_dir(reference: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/pg-trees")
        .join(reference)
}

/// Files that only exist after `configure && make`. If any is missing, the
/// tree is not built far enough for this crate to link.
const REQUIRED_GENERATED: &[&str] = &[
    "src/include/pg_config.h",
    // The two flex lexers this oracle is built from.
    "src/fe_utils/psqlscan.c",
    "src/bin/psql/psqlscanslash.c",
    // The frontend libraries they need.
    "src/fe_utils/conditional.c",
    "src/interfaces/libpq/libpq.a",
    "src/common/libpgcommon.a",
    "src/port/libpgport.a",
];

fn first_missing_generated(tree: &Path) -> Option<&'static str> {
    REQUIRED_GENERATED
        .iter()
        .copied()
        .find(|rel| !tree.join(rel).exists())
}

/// File in the tree that records the PostgreSQL commit it was extracted from.
const SOURCE_COMMIT_STAMP: &str = "pg-oracle-source-commit";

fn verify_pg_built(pin: &Pin, tree: &Path) {
    let stamp = std::fs::read_to_string(tree.join(SOURCE_COMMIT_STAMP)).ok();
    let same_commit = stamp.as_deref().map(str::trim) == Some(pin.commit.as_str());
    if same_commit && first_missing_generated(tree).is_none() {
        return;
    }

    // The PostgreSQL build is slow the first time; surface that to the user
    // since build-script output is otherwise buffered until completion.
    eprintln!(
        "PostgreSQL {} is not built — running {BUILD_PG} \
         (slow on the first build)",
        pin.reference
    );
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BUILD_PG);
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

    if let Some(missing) = first_missing_generated(tree) {
        panic!(
            "PostgreSQL build at {} is still missing {} after running \
             build-pg.sh.",
            tree.display(),
            missing
        );
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc/psql_oracle.c");
    println!("cargo:rerun-if-changed={PINS}");
    println!("cargo:rerun-if-changed={BUILD_PG}");
    println!("cargo:rerun-if-changed=../build-support/target_version.rs");
    let feature = target_version::require_one_target_version("pg-psql-oracle");
    let pin = pin_for(feature);
    // The tests compare these with the pin of their baseline.
    println!("cargo:rustc-env=PG_PSQL_ORACLE_POSTGRES_REF={}", pin.reference);
    println!(
        "cargo:rustc-env=PG_PSQL_ORACLE_POSTGRES_COMMIT={}",
        pin.commit
    );
    let tree = pg_tree_dir(&pin.reference);
    verify_pg_built(&pin, &tree);

    let mut build = cc::Build::new();
    build
        .include(tree.join("src/include"))
        .include(tree.join("src/interfaces/libpq"))
        .include(tree.join("src/bin/psql"))
        .include("csrc")
        // psql reaches `pg_valid_server_encoding_id` through the libpq
        // shared library. This oracle links the static libraries, where the
        // symbol has the `_private` spelling that `src/include/mb/pg_wchar.h`
        // gives it under `USE_PRIVATE_ENCODING_FUNCS`; psqlscan.c takes its
        // declaration from `libpq-fe.h` and never sees that header, so the
        // rename is made here instead.
        .define(
            "pg_valid_server_encoding_id",
            Some("pg_valid_server_encoding_id_private"),
        )
        .flag_if_supported("-w") // PG sources warn a lot
        .flag_if_supported("-fno-strict-aliasing")
        .flag_if_supported("-fwrapv");

    build.file("csrc/psql_oracle.c");
    // psqlscan.l is the psql SQL lexer itself.
    build.file(tree.join("src/fe_utils/psqlscan.c"));
    // psqlscanslash.l reads the name of a backslash command. It is the only
    // authority on where a command name ends, which is what decides whether
    // `\gsetfoo` is a send command or one unknown command.
    build.file(tree.join("src/bin/psql/psqlscanslash.c"));
    // psqlscanslash.l suppresses substitution in a skipped `\if` branch, so
    // it needs the conditional stack. `libpgfeutils.a` holds both this and
    // psqlscan.o, so the source is compiled rather than the library linked.
    build.file(tree.join("src/fe_utils/conditional.c"));

    build.compile("pgpsqloracle");

    // PostgreSQL's own frontend libraries, in psql's own link order. They are
    // copied into `OUT_DIR` first: `src/interfaces/libpq` holds a shared
    // libpq beside the static one, and a link search of that directory finds
    // the shared library, which the test binary then cannot load.
    let libraries = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"))
        .join("pg-libraries");
    std::fs::create_dir_all(&libraries).expect("create the link directory");
    for (from, name) in [
        ("src/interfaces/libpq/libpq.a", "libpq.a"),
        ("src/common/libpgcommon.a", "libpgcommon.a"),
        ("src/port/libpgport.a", "libpgport.a"),
    ] {
        std::fs::copy(tree.join(from), libraries.join(name))
            .unwrap_or_else(|e| panic!("cannot copy {from}: {e}"));
    }
    println!("cargo:rustc-link-search=native={}", libraries.display());
    println!("cargo:rustc-link-lib=static=pq");
    println!("cargo:rustc-link-lib=static=pgcommon");
    println!("cargo:rustc-link-lib=static=pgport");
}
