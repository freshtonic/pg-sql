//! The target-version feature check, shared by the build scripts of `pg-sql`,
//! `pg-psql` and `pg-oracle` (ADR 0009).
//!
//! A build selects exactly one target version with a version feature. The
//! build scripts run this check before they generate a grammar or build
//! PostgreSQL, so an incorrect feature set stops with this message and not
//! with a codegen or link error.

// Each build script uses a different part of this module.
#![allow(dead_code)]

/// The version features and the PostgreSQL major version of each.
pub const VERSION_FEATURES: &[(&str, u32)] = &[
    ("pg14", 14),
    ("pg15", 15),
    ("pg16", 16),
    ("pg17", 17),
    ("pg18", 18),
    ("pg19-beta", 19),
];

/// The cumulative helper features. `pgNN` enables `since-pg15` to
/// `since-pgNN`. There is no `since-pg14`, because 14 is the oldest target
/// version.
pub const SINCE_FEATURES: &[(&str, u32)] = &[
    ("since-pg15", 15),
    ("since-pg16", 16),
    ("since-pg17", 17),
    ("since-pg18", 18),
    ("since-pg19", 19),
];

/// The version feature of this build, from Cargo's `CARGO_FEATURE_*`
/// variables. The error is the message for the user.
pub fn selected_version_feature(crate_name: &str) -> Result<&'static str, String> {
    check(crate_name, |feature| {
        std::env::var_os(feature_env_var(feature)).is_some()
    })
}

/// Stop the build script with a Cargo error if the feature set is not valid.
/// Returns the version feature of this build. A different feature set is a
/// different Cargo build of the package, so the check needs no rerun rule.
pub fn require_one_target_version(crate_name: &str) -> &'static str {
    match selected_version_feature(crate_name) {
        Ok(feature) => feature,
        Err(message) => {
            for line in message.lines() {
                println!("cargo::error={line}");
            }
            std::process::exit(0);
        }
    }
}

/// The environment variable that Cargo sets for an enabled feature.
pub fn feature_env_var(feature: &str) -> String {
    format!(
        "CARGO_FEATURE_{}",
        feature.to_ascii_uppercase().replace('-', "_")
    )
}

/// The check itself. `enabled` tells if a feature is enabled.
pub fn check(crate_name: &str, enabled: impl Fn(&str) -> bool) -> Result<&'static str, String> {
    let selected = VERSION_FEATURES
        .iter()
        .filter(|(feature, _)| enabled(feature))
        .collect::<Vec<_>>();
    let all_versions = VERSION_FEATURES
        .iter()
        .map(|(feature, _)| format!("`{feature}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let unification = format!(
        "Cargo feature unification can cause this. A crate that depends on {crate_name} \
         without `default-features = false` also enables the default `pg17` feature. \
         Every crate between your crate and {crate_name} must use \
         `default-features = false` on its {crate_name} dependency and forward the \
         version features."
    );
    let (feature, major) = match selected.as_slice() {
        [] => {
            let helpers = SINCE_FEATURES
                .iter()
                .filter(|(since, _)| enabled(since))
                .map(|(since, _)| format!("`{since}`"))
                .collect::<Vec<_>>();
            let helpers = if helpers.is_empty() {
                String::new()
            } else {
                format!(
                    "\nThe helper feature {} is enabled without a version feature. \
                     The version features enable the `since-pgN` helpers; \
                     do not enable them directly.",
                    helpers.join(", ")
                )
            };
            return Err(format!(
                "{crate_name}: no target version is selected.\n\
                 Enable exactly one version feature: {all_versions}. The default is `pg17`.\n\
                 If you use `default-features = false`, also enable one version feature.\
                 {helpers}"
            ));
        }
        [(feature, major)] => (*feature, *major),
        several => {
            let names = several
                .iter()
                .map(|(feature, _)| format!("`{feature}`"))
                .collect::<Vec<_>>()
                .join(" and ");
            return Err(format!(
                "{crate_name}: more than one target version is selected: {names}.\n\
                 A build must select exactly one version feature: {all_versions}.\n\
                 {unification}"
            ));
        }
    };
    let above = SINCE_FEATURES
        .iter()
        .filter(|(since, since_major)| enabled(since) && *since_major > major)
        .map(|(since, _)| format!("`{since}`"))
        .collect::<Vec<_>>();
    if !above.is_empty() {
        return Err(format!(
            "{crate_name}: the feature {} does not agree with the target version `{feature}`.\n\
             The `since-pgN` features are helpers that the version features enable. \
             Do not enable them directly.\n\
             {unification}",
            above.join(", ")
        ));
    }
    Ok(feature)
}
