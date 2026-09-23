//! PostgreSQL grammar backed by Recursa's sole LR parser.
//!
//! The `spans` Cargo feature retains occurrence provenance during ordinary
//! parsing. Generated `parse_without_spans` entry points always provide
//! semantic values without that provenance cost. Without the feature,
//! ordinary parsing also skips provenance and the exact-source document
//! interface is unavailable.
//!
//! Each build targets one PostgreSQL major version, selected with exactly one
//! version feature (`pg14` ... `pg19-beta`, default `pg17`). [`TARGET_VERSION`]
//! reports it.

// Keep the optional sampling recipe aligned with the facade capability.
#[cfg(feature = "arbitrary")]
recursa::grammar! {
    module = crate,
    arena_ast,
    flat,
    // One name for each target version, in release order, mapped to the
    // cumulative `since-*` helper that already removes the element (ADR 0009,
    // ADR 0010). `pg14` is the baseline, which every build admits, so it takes
    // no feature. `pg19` stands for the `pg19-beta` target version; the name
    // loses the suffix so the gates need no edit when PostgreSQL 19.0 ships.
    configurations(
        pg14,
        pg15 = feature("since-pg15"),
        pg16 = feature("since-pg16"),
        pg17 = feature("since-pg17"),
        pg18 = feature("since-pg18"),
        pg19 = feature("since-pg19"),
    ),
    derives(Arbitrary, Pretty, Requires, Visit, VisitMut),
    keyword_matching = ascii_insensitive,
    diagrams,
    framing(island = ast::file::SqlDocumentItem, boundary = SEMI),
}

#[cfg(not(feature = "arbitrary"))]
recursa::grammar! {
    module = crate,
    arena_ast,
    flat,
    // One name for each target version, in release order, mapped to the
    // cumulative `since-*` helper that already removes the element (ADR 0009,
    // ADR 0010). `pg14` is the baseline, which every build admits, so it takes
    // no feature. `pg19` stands for the `pg19-beta` target version; the name
    // loses the suffix so the gates need no edit when PostgreSQL 19.0 ships.
    configurations(
        pg14,
        pg15 = feature("since-pg15"),
        pg16 = feature("since-pg16"),
        pg17 = feature("since-pg17"),
        pg18 = feature("since-pg18"),
        pg19 = feature("since-pg19"),
    ),
    derives(Pretty, Requires, Visit, VisitMut),
    keyword_matching = ascii_insensitive,
    diagrams,
    framing(island = ast::file::SqlDocumentItem, boundary = SEMI),
}

pub mod ast;
pub mod bench_data;
#[cfg(feature = "spans")]
pub mod document;
pub mod formatter;
pub mod ident;
mod target_version;
pub mod tokens;

#[cfg(feature = "spans")]
pub use document::{SqlDocument, SqlParseError, SqlRejection};
pub use target_version::TargetVersion;

// The constants are defined here and not in `target_version`, because the
// grammar generator cannot resolve a `pub use` of a constant (RCA2012).
/// The target version of this build, selected by the Cargo version feature.
///
/// ```
/// // With the default features, pg-sql targets PostgreSQL 17.
/// assert_eq!(pg_sql::TARGET_VERSION.major(), 17);
/// ```
#[cfg(feature = "pg14")]
pub const TARGET_VERSION: TargetVersion = TargetVersion::Pg14;
/// The target version of this build, selected by the Cargo version feature.
#[cfg(feature = "pg15")]
pub const TARGET_VERSION: TargetVersion = TargetVersion::Pg15;
/// The target version of this build, selected by the Cargo version feature.
#[cfg(feature = "pg16")]
pub const TARGET_VERSION: TargetVersion = TargetVersion::Pg16;
/// The target version of this build, selected by the Cargo version feature.
#[cfg(feature = "pg17")]
pub const TARGET_VERSION: TargetVersion = TargetVersion::Pg17;
/// The target version of this build, selected by the Cargo version feature.
#[cfg(feature = "pg18")]
pub const TARGET_VERSION: TargetVersion = TargetVersion::Pg18;
/// The target version of this build, selected by the Cargo version feature.
#[cfg(feature = "pg19-beta")]
pub const TARGET_VERSION: TargetVersion = TargetVersion::Pg19Beta;

// A backstop for the build-script check (`build-support/target_version.rs`),
// which stops an incorrect feature set before the grammar is generated.
const _: () = assert!(
    cfg!(feature = "pg14") as u8
        + cfg!(feature = "pg15") as u8
        + cfg!(feature = "pg16") as u8
        + cfg!(feature = "pg17") as u8
        + cfg!(feature = "pg18") as u8
        + cfg!(feature = "pg19-beta") as u8
        == 1,
    "pg-sql: enable exactly one version feature (pg14, pg15, pg16, pg17, pg18, pg19-beta); \
     with Cargo feature unification, use `default-features = false` on pg-sql"
);
