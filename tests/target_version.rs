//! The target version of the build, and the version feature check that the
//! build scripts run before grammar generation (ADR 0009).

#[path = "../build-support/target_version.rs"]
mod target_version;

use pg_sql::{TARGET_VERSION, TargetVersion};

fn enabled_in_this_build(feature: &str) -> bool {
    match feature {
        "pg14" => cfg!(feature = "pg14"),
        "pg15" => cfg!(feature = "pg15"),
        "pg16" => cfg!(feature = "pg16"),
        "pg17" => cfg!(feature = "pg17"),
        "pg18" => cfg!(feature = "pg18"),
        "pg19-beta" => cfg!(feature = "pg19-beta"),
        "since-pg15" => cfg!(feature = "since-pg15"),
        "since-pg16" => cfg!(feature = "since-pg16"),
        "since-pg17" => cfg!(feature = "since-pg17"),
        "since-pg18" => cfg!(feature = "since-pg18"),
        "since-pg19" => cfg!(feature = "since-pg19"),
        _ => false,
    }
}

#[test]
fn target_version_reports_the_version_feature_of_this_build() {
    let feature = target_version::check("pg-sql", enabled_in_this_build)
        .expect("this build has a valid feature set");
    assert_eq!(TARGET_VERSION.feature(), feature);
    let (_, major) = target_version::VERSION_FEATURES
        .iter()
        .find(|(name, _)| *name == feature)
        .expect("known version feature");
    assert_eq!(TARGET_VERSION.major(), *major);
}

#[test]
fn a_version_feature_enables_exactly_the_helpers_up_to_its_version() {
    for (since, since_major) in target_version::SINCE_FEATURES {
        assert_eq!(
            enabled_in_this_build(since),
            TARGET_VERSION.major() >= *since_major,
            "{since} with target version {TARGET_VERSION:?}"
        );
    }
}

#[test]
fn every_target_version_has_a_distinct_feature_and_major() {
    let versions = [
        TargetVersion::Pg14,
        TargetVersion::Pg15,
        TargetVersion::Pg16,
        TargetVersion::Pg17,
        TargetVersion::Pg18,
        TargetVersion::Pg19Beta,
    ];
    assert_eq!(
        versions
            .iter()
            .map(|version| (version.feature(), version.major()))
            .collect::<Vec<_>>(),
        target_version::VERSION_FEATURES.to_vec()
    );
}

fn check(features: &[&str]) -> Result<&'static str, String> {
    target_version::check("pg-sql", |feature| features.contains(&feature))
}

#[test]
fn the_check_accepts_one_version_feature_with_its_helpers() {
    assert_eq!(check(&["pg14"]), Ok("pg14"));
    assert_eq!(
        check(&["pg17", "since-pg15", "since-pg16", "since-pg17"]),
        Ok("pg17")
    );
    assert_eq!(check(&["pg19-beta", "since-pg19"]), Ok("pg19-beta"));
}

#[test]
fn the_check_rejects_zero_version_features() {
    let message = check(&[]).unwrap_err();
    assert!(
        message.contains("no target version is selected"),
        "{message}"
    );
    assert!(message.contains("default-features = false"), "{message}");

    let message = check(&["since-pg16"]).unwrap_err();
    assert!(
        message.contains("no target version is selected"),
        "{message}"
    );
    assert!(message.contains("`since-pg16`"), "{message}");
}

#[test]
fn the_check_rejects_two_version_features_and_names_feature_unification() {
    let message = check(&["pg16", "pg17"]).unwrap_err();
    assert!(message.contains("`pg16` and `pg17`"), "{message}");
    assert!(message.contains("feature unification"), "{message}");
    assert!(message.contains("default-features = false"), "{message}");
}

#[test]
fn the_check_rejects_a_helper_above_the_target_version() {
    let message = check(&["pg15", "since-pg15", "since-pg16"]).unwrap_err();
    assert!(message.contains("`since-pg16`"), "{message}");
    assert!(message.contains("`pg15`"), "{message}");
    assert!(message.contains("feature unification"), "{message}");
}
