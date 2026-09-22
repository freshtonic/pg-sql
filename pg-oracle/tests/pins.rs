//! The pin table (`pins.tsv`) agrees with the target versions, the
//! `vendor/postgres` submodule and the oracle of this build.

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../../build-support/target_version.rs"]
mod target_version;

struct Pin {
    feature: String,
    reference: String,
    commit: String,
}

fn pins() -> Vec<Pin> {
    include_str!("../pins.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3, "a pin row has three fields: {line:?}");
            Pin {
                feature: fields[0].to_owned(),
                reference: fields[1].to_owned(),
                commit: fields[2].to_owned(),
            }
        })
        .collect()
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("pg-oracle is inside the repository")
        .to_owned()
}

fn git(repository: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .expect("run git");
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[test]
fn there_is_one_pin_for_each_target_version() {
    let pins = pins();
    assert_eq!(
        pins.iter()
            .map(|pin| pin.feature.as_str())
            .collect::<Vec<_>>(),
        target_version::VERSION_FEATURES
            .iter()
            .map(|(feature, _)| *feature)
            .collect::<Vec<_>>()
    );
    for pin in &pins {
        assert!(
            pin.commit.len() == 40 && pin.commit.bytes().all(|b| b.is_ascii_hexdigit()),
            "{}: the pin must be a full commit ID",
            pin.feature
        );
    }
}

#[test]
fn the_pg17_pin_is_the_submodule_gitlink() {
    let pin = pins()
        .into_iter()
        .find(|pin| pin.feature == "pg17")
        .expect("pg17 pin");
    let stage = git(
        &repository_root(),
        &["ls-files", "--stage", "--", "vendor/postgres"],
    )
    .expect("read the vendor/postgres gitlink");
    let gitlink = stage
        .split_whitespace()
        .nth(1)
        .expect("gitlink commit in `git ls-files --stage`");
    assert_eq!(
        gitlink, pin.commit,
        "the pg17 pin in pins.tsv and the vendor/postgres gitlink must move together"
    );
}

#[test]
fn each_tag_pin_names_the_commit_of_its_tag() {
    let repository = repository_root().join("vendor/postgres");
    for pin in pins() {
        if pin.reference.ends_with("_STABLE") {
            // A pre-release major pins a named commit on its branch.
            continue;
        }
        let tagged = git(
            &repository,
            &["rev-parse", &format!("{}^{{commit}}", pin.reference)],
        )
        .unwrap_or_else(|| {
            panic!(
                "the tag {} is not in vendor/postgres; fetch it: \
                     git -C vendor/postgres fetch origin tag {}",
                pin.reference, pin.reference
            )
        });
        assert_eq!(
            tagged, pin.commit,
            "{} names a different commit",
            pin.reference
        );
    }
}

#[test]
fn this_oracle_is_built_from_the_pin_of_its_version_feature() {
    let feature = target_version::check("pg-oracle", |feature| match feature {
        "pg14" => cfg!(feature = "pg14"),
        "pg15" => cfg!(feature = "pg15"),
        "pg16" => cfg!(feature = "pg16"),
        "pg17" => cfg!(feature = "pg17"),
        "pg18" => cfg!(feature = "pg18"),
        "pg19-beta" => cfg!(feature = "pg19-beta"),
        _ => false,
    })
    .expect("one version feature");
    let pin = pins()
        .into_iter()
        .find(|pin| pin.feature == feature)
        .expect("pin of the version feature");
    assert_eq!(pg_oracle::POSTGRES_REF, pin.reference);
    assert_eq!(pg_oracle::POSTGRES_COMMIT, pin.commit);
}
