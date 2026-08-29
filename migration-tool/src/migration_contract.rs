//! Closed identities and path sets for the one-shot grammar migration.

use std::path::Path;

pub const MIGRATION_SOURCE_COMMIT: &str = "b61ff1b85e566950a0675a4d26758430cebb6a92";
pub const MIGRATION_SOURCE_TREE: &str = "0ae4e5c2abbbbb33c0f1937a182d679f4f8c255a";
pub const LEGACY_COMMIT: &str = "1e71421d66baac15c8c5264e8f29b5f80122f50e";
pub const LEGACY_TREE: &str = "f3191ab707c8a957d1bb5fe142e74fc624fe6661";
pub const PG_SQL_TREE: &str = "50e1376d16796e5f05db88d99dab42252a9f78a4";
pub const PG_ORACLE_TREE: &str = "0780d057e4d54db150d0f388c45a720a825bcbcf";
pub const POSTGRES_GITLINK: &str = "6d396980fc5aed4f1a525e0bd75cb16b25ed40ca";
pub const SOURCE_CHECKPOINT: &str = "e97d3c3570c2a04ca9a233334b46d3f443800a5a";
pub const RECURSA_REVISION: &str = "8ae631142147919eeb3197cb87fe2f4aa0e9a8e3";

pub const OMITTED_PATHS: &[&str] = &[
    "src/bin/depth_probe.rs",
    "src/bin/flame_report.rs",
    "src/bin/flame_target.rs",
    "src/flame.rs",
    "src/flame_report.rs",
    "src/flame_report/git.rs",
    "src/flame_report/host.rs",
    "src/flame_report/orchestrator.rs",
    "src/flame_report/profiler.rs",
    "src/flame_report/render.rs",
    "src/generated/first_set.rs",
    "src/main.rs",
];

pub const OBSOLETE_ROOT_MODULES: &[&str] = &["flame", "flame_report"];
pub const OBSOLETE_EXPLICIT_BIN_PATHS: &[&str] =
    &["src/bin/flame_report.rs", "src/bin/flame_target.rs"];
pub const PUBLICATION_ADDITIONS: &[(&str, u32)] = &[("build.rs", 0o644)];

pub fn is_omitted_path(path: &Path) -> bool {
    OMITTED_PATHS
        .iter()
        .any(|candidate| path == Path::new(candidate))
}

pub fn is_publication_owned_import(path: &str) -> bool {
    path == "Cargo.toml" || path.starts_with("src/")
}
