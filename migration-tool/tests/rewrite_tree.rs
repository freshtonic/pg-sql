use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use pg_sql_migrate::rewrite::{
    FileDisposition, RewriteError, RewriteTreeRequest, SourceRewritePass, SpanEdit,
    apply_span_edits, rewrite_source, rewrite_tree,
};
use tempfile::tempdir;

#[derive(Debug, Eq, PartialEq)]
struct TreeEntry {
    directory: bool,
    mode: u32,
    bytes: Vec<u8>,
}

fn tree_snapshot(root: &Path) -> BTreeMap<String, TreeEntry> {
    fn visit(root: &Path, path: &Path, snapshot: &mut BTreeMap<String, TreeEntry>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };
        #[cfg(not(unix))]
        let mode = u32::from(metadata.permissions().readonly());
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        snapshot.insert(
            relative,
            TreeEntry {
                directory: metadata.is_dir(),
                mode,
                bytes: if metadata.is_file() {
                    fs::read(path).unwrap()
                } else {
                    Vec::new()
                },
            },
        );
        if metadata.is_dir() {
            let mut children: Vec<_> = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            children.sort();
            for child in children {
                visit(root, &child, snapshot);
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

struct ReplaceSelect;

impl SourceRewritePass for ReplaceSelect {
    fn edits(&self, _path: &Path, source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        Ok(source
            .match_indices("legacy")
            .map(|(start, value)| SpanEdit {
                start,
                end: start + value.len(),
                replacement: "recursa".into(),
            })
            .collect())
    }
}

struct RejectBad;

impl SourceRewritePass for RejectBad {
    fn edits(&self, path: &Path, source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        if source.contains("unsupported") {
            return Err(RewriteError::Pass {
                path: path.to_path_buf(),
                message: "unsupported representative shape".into(),
            });
        }
        Ok(Vec::new())
    }
}

struct OmitGenerated;

impl SourceRewritePass for OmitGenerated {
    fn file_disposition(&self, path: &Path) -> Result<FileDisposition, RewriteError> {
        Ok(if path == Path::new("src/generated/first_set.rs") {
            FileDisposition::Omit
        } else {
            FileDisposition::Keep
        })
    }

    fn edits(&self, _path: &Path, _source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        Ok(Vec::new())
    }
}

struct ProduceInvalidRust;

impl SourceRewritePass for ProduceInvalidRust {
    fn edits(&self, _path: &Path, source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        Ok(vec![SpanEdit {
            start: 0,
            end: source.len(),
            replacement: "pub struct MissingBrace {".into(),
        }])
    }
}

struct CreateLateDestination<'a>(&'a Path);

impl SourceRewritePass for CreateLateDestination<'_> {
    fn edits(&self, _path: &Path, _source: &str) -> Result<Vec<SpanEdit>, RewriteError> {
        fs::create_dir(self.0).unwrap();
        Ok(Vec::new())
    }
}

#[test]
fn span_edits_are_applied_right_to_left() {
    let edits = vec![
        SpanEdit {
            start: 0,
            end: 1,
            replacement: "left".into(),
        },
        SpanEdit {
            start: 2,
            end: 4,
            replacement: "right".into(),
        },
    ];

    assert_eq!(apply_span_edits("a é", &edits).unwrap(), "left right");
}

#[test]
fn span_edits_reject_overlap_and_non_utf8_boundaries() {
    let overlap = vec![
        SpanEdit {
            start: 0,
            end: 2,
            replacement: String::new(),
        },
        SpanEdit {
            start: 1,
            end: 3,
            replacement: String::new(),
        },
    ];
    assert!(matches!(
        apply_span_edits("abc", &overlap),
        Err(RewriteError::OverlappingEdits { .. })
    ));

    let split_codepoint = [SpanEdit {
        start: 1,
        end: 2,
        replacement: String::new(),
    }];
    assert!(matches!(
        apply_span_edits("é", &split_codepoint),
        Err(RewriteError::InvalidUtf8Boundary { .. })
    ));
}

#[test]
fn rewrite_source_validates_edits_returned_by_a_pass() {
    assert_eq!(
        rewrite_source(&ReplaceSelect, Path::new("src/example.rs"), "legacy").unwrap(),
        "recursa"
    );
}

#[test]
fn tree_rewrite_publishes_complete_deterministic_copy() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("legacy");
    let declared_root = temp.path().join("new-repository");
    fs::create_dir_all(source.join("src")).unwrap();
    fs::create_dir_all(&declared_root).unwrap();
    fs::write(source.join("src/z.rs"), "// keep\nlegacy!();\n").unwrap();
    fs::write(source.join("src/a.bin"), [0, 159, 146, 150]).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o750)).unwrap();
        fs::set_permissions(source.join("src/z.rs"), fs::Permissions::from_mode(0o751)).unwrap();
    }

    let passes: [&dyn SourceRewritePass; 1] = [&ReplaceSelect];
    let first = declared_root.join("first");
    rewrite_tree(RewriteTreeRequest {
        source_root: &source,
        destination_root: &first,
        new_repository_root: &declared_root,
        passes: &passes,
    })
    .unwrap();

    assert_eq!(
        fs::read_to_string(first.join("src/z.rs")).unwrap(),
        "// keep\nrecursa!();\n"
    );
    assert_eq!(
        fs::read(first.join("src/a.bin")).unwrap(),
        [0, 159, 146, 150]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&first).unwrap().permissions().mode() & 0o777,
            0o750
        );
        assert_eq!(
            fs::metadata(first.join("src/z.rs"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o751
        );
    }

    let second = declared_root.join("second");
    rewrite_tree(RewriteTreeRequest {
        source_root: &source,
        destination_root: &second,
        new_repository_root: &declared_root,
        passes: &passes,
    })
    .unwrap();
    assert_eq!(tree_snapshot(&first), tree_snapshot(&second));
}

#[test]
fn failed_plan_leaves_source_unchanged_and_destination_absent() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("legacy");
    let declared_root = temp.path().join("new-repository");
    let destination = declared_root.join("output");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&declared_root).unwrap();
    let input = "unsupported legacy\n";
    fs::write(source.join("bad.rs"), input).unwrap();
    let passes: [&dyn SourceRewritePass; 2] = [&ReplaceSelect, &RejectBad];

    assert!(
        rewrite_tree(RewriteTreeRequest {
            source_root: &source,
            destination_root: &destination,
            new_repository_root: &declared_root,
            passes: &passes,
        })
        .is_err()
    );
    assert!(!destination.exists());
    assert_eq!(fs::read_to_string(source.join("bad.rs")).unwrap(), input);
}

#[test]
fn invalid_rust_from_a_pass_fails_before_staging_or_publication() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("legacy");
    let declared_root = temp.path().join("new-repository");
    let destination = declared_root.join("output");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&declared_root).unwrap();
    fs::write(source.join("valid.rs"), "pub struct Valid;\n").unwrap();
    let passes: [&dyn SourceRewritePass; 1] = [&ProduceInvalidRust];

    let error = rewrite_tree(RewriteTreeRequest {
        source_root: &source,
        destination_root: &destination,
        new_repository_root: &declared_root,
        passes: &passes,
    })
    .unwrap_err();

    assert!(
        matches!(error, RewriteError::InvalidRewrittenRust { ref path, .. } if path == Path::new("valid.rs"))
    );
    assert!(!destination.exists());
    assert_eq!(
        fs::read_to_string(source.join("valid.rs")).unwrap(),
        "pub struct Valid;\n"
    );
    assert!(
        fs::read_dir(&declared_root).unwrap().next().is_none(),
        "validation failure must not leave a staging tree"
    );
}

#[test]
fn atomic_publish_does_not_replace_a_destination_created_during_planning() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("legacy");
    let declared_root = temp.path().join("new-repository");
    let destination = declared_root.join("output");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&declared_root).unwrap();
    fs::write(source.join("valid.rs"), "pub struct Valid;\n").unwrap();
    let late_destination = CreateLateDestination(&destination);
    let passes: [&dyn SourceRewritePass; 1] = [&late_destination];

    assert!(matches!(
        rewrite_tree(RewriteTreeRequest {
            source_root: &source,
            destination_root: &destination,
            new_repository_root: &declared_root,
            passes: &passes,
        }),
        Err(RewriteError::DestinationExists { .. })
    ));
    assert!(!destination.join("valid.rs").exists());
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&declared_root).unwrap().count(), 1);
}

#[test]
fn a_pass_can_explicitly_omit_an_obsolete_file() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("legacy");
    let declared_root = temp.path().join("new-repository");
    let destination = declared_root.join("output");
    fs::create_dir_all(source.join("src/generated")).unwrap();
    fs::create_dir_all(&declared_root).unwrap();
    fs::write(source.join("src/generated/first_set.rs"), "obsolete").unwrap();
    fs::write(source.join("src/kept.rs"), "pub struct Kept;\n").unwrap();
    let passes: [&dyn SourceRewritePass; 1] = [&OmitGenerated];

    rewrite_tree(RewriteTreeRequest {
        source_root: &source,
        destination_root: &destination,
        new_repository_root: &declared_root,
        passes: &passes,
    })
    .unwrap();

    assert!(!destination.join("src/generated/first_set.rs").exists());
    assert_eq!(
        fs::read_to_string(destination.join("src/kept.rs")).unwrap(),
        "pub struct Kept;\n"
    );
}

#[test]
fn tree_rewrite_rejects_unsafe_roots_and_symlinks() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("legacy");
    let declared_root = temp.path().join("new-repository");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&declared_root).unwrap();
    fs::write(source.join("file.rs"), "legacy").unwrap();
    let no_passes: [&dyn SourceRewritePass; 0] = [];

    let existing = declared_root.join("existing");
    fs::create_dir(&existing).unwrap();
    assert!(matches!(
        rewrite_tree(RewriteTreeRequest {
            source_root: &source,
            destination_root: &existing,
            new_repository_root: &declared_root,
            passes: &no_passes,
        }),
        Err(RewriteError::DestinationExists { .. })
    ));

    assert!(matches!(
        rewrite_tree(RewriteTreeRequest {
            source_root: &source,
            destination_root: &temp.path().join("outside"),
            new_repository_root: &declared_root,
            passes: &no_passes,
        }),
        Err(RewriteError::DestinationOutsideRepository { .. })
    ));

    assert!(matches!(
        rewrite_tree(RewriteTreeRequest {
            source_root: &source,
            destination_root: &source.join("nested-output"),
            new_repository_root: temp.path(),
            passes: &no_passes,
        }),
        Err(RewriteError::SourceDestinationOverlap { .. })
    ));

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source.join("file.rs"), source.join("link.rs")).unwrap();
        assert!(matches!(
            rewrite_tree(RewriteTreeRequest {
                source_root: &source,
                destination_root: &declared_root.join("with-link"),
                new_repository_root: &declared_root,
                passes: &no_passes,
            }),
            Err(RewriteError::Symlink { .. })
        ));
    }
}
