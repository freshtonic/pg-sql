//! Deterministic, span-based source and tree rewriting.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use walkdir::WalkDir;

static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpanEdit {
    /// Inclusive UTF-8 byte offset in the source.
    pub start: usize,
    /// Exclusive UTF-8 byte offset in the source.
    pub end: usize,
    pub replacement: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileDisposition {
    Keep,
    /// Deliberately exclude an obsolete source file from the published tree.
    Omit,
}

#[derive(Debug)]
pub enum RewriteError {
    InvalidSpan {
        index: usize,
        start: usize,
        end: usize,
        source_len: usize,
    },
    InvalidUtf8Boundary {
        index: usize,
        offset: usize,
    },
    OverlappingEdits {
        first: usize,
        second: usize,
    },
    Pass {
        path: PathBuf,
        message: String,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    SourceNotDirectory {
        path: PathBuf,
    },
    DestinationExists {
        path: PathBuf,
    },
    DestinationOutsideRepository {
        destination: PathBuf,
        repository: PathBuf,
    },
    SourceDestinationOverlap {
        source: PathBuf,
        destination: PathBuf,
    },
    Symlink {
        path: PathBuf,
    },
    UnsupportedFileType {
        path: PathBuf,
    },
    NonUtf8RustSource {
        path: PathBuf,
    },
    InvalidRewrittenRust {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for RewriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpan {
                index,
                start,
                end,
                source_len,
            } => write!(
                formatter,
                "edit {index} has invalid span {start}..{end} for a {source_len}-byte source"
            ),
            Self::InvalidUtf8Boundary { index, offset } => write!(
                formatter,
                "edit {index} offset {offset} is not a UTF-8 character boundary"
            ),
            Self::OverlappingEdits { first, second } => write!(
                formatter,
                "edits {first} and {second} overlap or have ambiguous insertion order"
            ),
            Self::Pass { path, message } => write!(
                formatter,
                "rewrite pass rejected {}: {message}",
                path.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::SourceNotDirectory { path } => write!(
                formatter,
                "rewrite source is not a directory: {}",
                path.display()
            ),
            Self::DestinationExists { path } => write!(
                formatter,
                "rewrite destination already exists: {}",
                path.display()
            ),
            Self::DestinationOutsideRepository {
                destination,
                repository,
            } => write!(
                formatter,
                "rewrite destination {} is outside declared new-repository root {}",
                destination.display(),
                repository.display()
            ),
            Self::SourceDestinationOverlap {
                source,
                destination,
            } => write!(
                formatter,
                "rewrite source {} overlaps destination {}",
                source.display(),
                destination.display()
            ),
            Self::Symlink { path } => write!(
                formatter,
                "rewrite trees may not contain symlinks: {}",
                path.display()
            ),
            Self::UnsupportedFileType { path } => write!(
                formatter,
                "rewrite trees may contain only directories and regular files: {}",
                path.display()
            ),
            Self::NonUtf8RustSource { path } => write!(
                formatter,
                "Rust source is not valid UTF-8: {}",
                path.display()
            ),
            Self::InvalidRewrittenRust { path, message } => write!(
                formatter,
                "rewrite pass produced invalid Rust for {}: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RewriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub trait SourceRewritePass {
    /// Select an obsolete file for exclusion from the output. The default keeps
    /// every file, so edit-only passes need not implement this method.
    fn file_disposition(&self, _path: &Path) -> Result<FileDisposition, RewriteError> {
        Ok(FileDisposition::Keep)
    }

    fn edits(&self, path: &Path, source: &str) -> Result<Vec<SpanEdit>, RewriteError>;
}

/// Validate all edits before changing the in-memory source, then apply them from
/// the greatest byte offset to the least.
pub fn apply_span_edits(source: &str, edits: &[SpanEdit]) -> Result<String, RewriteError> {
    let mut ordered: Vec<_> = edits.iter().enumerate().collect();
    ordered.sort_by_key(|(_, edit)| (edit.start, edit.end));

    for (position, (original_index, edit)) in ordered.iter().enumerate() {
        if edit.start > edit.end || edit.end > source.len() {
            return Err(RewriteError::InvalidSpan {
                index: *original_index,
                start: edit.start,
                end: edit.end,
                source_len: source.len(),
            });
        }
        for offset in [edit.start, edit.end] {
            if !source.is_char_boundary(offset) {
                return Err(RewriteError::InvalidUtf8Boundary {
                    index: *original_index,
                    offset,
                });
            }
        }
        if position > 0 {
            let (previous_index, previous) = ordered[position - 1];
            if edit.start < previous.end || edit.start == previous.start {
                return Err(RewriteError::OverlappingEdits {
                    first: previous_index,
                    second: *original_index,
                });
            }
        }
    }

    let mut rewritten = source.to_owned();
    for (_, edit) in ordered.into_iter().rev() {
        rewritten.replace_range(edit.start..edit.end, &edit.replacement);
    }
    Ok(rewritten)
}

pub fn rewrite_source(
    pass: &dyn SourceRewritePass,
    path: &Path,
    source: &str,
) -> Result<String, RewriteError> {
    let edits = pass.edits(path, source)?;
    apply_span_edits(source, &edits)
}

pub struct RewriteTreeRequest<'a> {
    pub source_root: &'a Path,
    pub destination_root: &'a Path,
    pub new_repository_root: &'a Path,
    /// Passes run in this declared order. Each pass sees the complete output of
    /// its predecessor and is independently validated before application.
    pub passes: &'a [&'a dyn SourceRewritePass],
}

struct PlannedEntry {
    relative: PathBuf,
    kind: PlannedKind,
    permissions: fs::Permissions,
}

enum PlannedKind {
    Directory,
    File(Vec<u8>),
}

/// Fully plan and validate a copy before staging it beside the destination and
/// publishing the completed tree with one rename.
pub fn rewrite_tree(request: RewriteTreeRequest<'_>) -> Result<(), RewriteError> {
    let source_link_metadata = metadata("inspect", request.source_root, true)?;
    if source_link_metadata.file_type().is_symlink() {
        return Err(RewriteError::Symlink {
            path: request.source_root.to_path_buf(),
        });
    }
    if !source_link_metadata.is_dir() {
        return Err(RewriteError::SourceNotDirectory {
            path: request.source_root.to_path_buf(),
        });
    }

    let source = canonicalize("canonicalize", request.source_root)?;
    let destination = resolve_existing_ancestors(request.destination_root)?;
    let repository = resolve_existing_ancestors(request.new_repository_root)?;
    if !destination.starts_with(&repository) {
        return Err(RewriteError::DestinationOutsideRepository {
            destination,
            repository,
        });
    }
    if destination.starts_with(&source) || source.starts_with(&destination) {
        return Err(RewriteError::SourceDestinationOverlap {
            source,
            destination,
        });
    }
    if path_present(request.destination_root)? {
        return Err(RewriteError::DestinationExists {
            path: request.destination_root.to_path_buf(),
        });
    }

    let (plan, root_permissions) = plan_tree(&source, request.passes)?;
    let parent = request
        .destination_root
        .parent()
        .ok_or_else(|| RewriteError::Io {
            operation: "find parent of",
            path: request.destination_root.to_path_buf(),
            source: io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent"),
        })?;
    let parent_metadata = metadata("inspect", parent, true)?;
    if parent_metadata.file_type().is_symlink() {
        return Err(RewriteError::Symlink {
            path: parent.to_path_buf(),
        });
    }
    if !parent_metadata.is_dir() {
        return Err(RewriteError::Io {
            operation: "stage in",
            path: parent.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::NotADirectory,
                "destination parent is not a directory",
            ),
        });
    }

    let stage = create_stage(parent)?;
    let stage_guard = StageGuard(Some(stage.clone()));
    stage_plan(&stage, &plan, root_permissions)?;
    match atomic_rename_no_replace(&stage, request.destination_root) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(RewriteError::DestinationExists {
                path: request.destination_root.to_path_buf(),
            });
        }
        Err(source) => {
            return Err(RewriteError::Io {
                operation: "publish",
                path: request.destination_root.to_path_buf(),
                source,
            });
        }
    }
    stage_guard.disarm();
    Ok(())
}

fn plan_tree(
    source: &Path,
    passes: &[&dyn SourceRewritePass],
) -> Result<(Vec<PlannedEntry>, fs::Permissions), RewriteError> {
    let root_permissions = metadata("inspect", source, true)?.permissions();
    let mut paths = Vec::new();
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry.map_err(|source_error| {
            let path = source_error.path().unwrap_or(source).to_path_buf();
            RewriteError::Io {
                operation: "walk",
                path,
                source: io::Error::other(source_error),
            }
        })?;
        if entry.path() != source {
            paths.push(entry.path().to_path_buf());
        }
    }
    paths.sort_by(|left, right| {
        left.strip_prefix(source)
            .unwrap()
            .cmp(right.strip_prefix(source).unwrap())
    });

    let mut plan = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = path
            .strip_prefix(source)
            .expect("walked path is beneath root")
            .to_path_buf();
        let metadata = metadata("inspect", &path, true)?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            return Err(RewriteError::Symlink { path });
        }
        let kind = if file_type.is_dir() {
            PlannedKind::Directory
        } else if file_type.is_file() {
            let mut disposition = FileDisposition::Keep;
            for pass in passes {
                if pass.file_disposition(&relative)? == FileDisposition::Omit {
                    disposition = FileDisposition::Omit;
                }
            }
            if disposition == FileDisposition::Omit {
                continue;
            }
            let mut bytes = fs::read(&path).map_err(|source| RewriteError::Io {
                operation: "read",
                path: path.clone(),
                source,
            })?;
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                let mut source_text = String::from_utf8(bytes)
                    .map_err(|_| RewriteError::NonUtf8RustSource { path: path.clone() })?;
                for pass in passes {
                    source_text = rewrite_source(*pass, &relative, &source_text)?;
                    syn::parse_file(&source_text).map_err(|error| {
                        RewriteError::InvalidRewrittenRust {
                            path: relative.clone(),
                            message: error.to_string(),
                        }
                    })?;
                }
                bytes = source_text.into_bytes();
            }
            PlannedKind::File(bytes)
        } else {
            return Err(RewriteError::UnsupportedFileType { path });
        };
        plan.push(PlannedEntry {
            relative,
            kind,
            permissions: metadata.permissions(),
        });
    }
    Ok((plan, root_permissions))
}

fn stage_plan(
    stage: &Path,
    plan: &[PlannedEntry],
    root_permissions: fs::Permissions,
) -> Result<(), RewriteError> {
    for entry in plan {
        let target = stage.join(&entry.relative);
        match &entry.kind {
            PlannedKind::Directory => {
                fs::create_dir(&target).map_err(|source| RewriteError::Io {
                    operation: "create directory",
                    path: target,
                    source,
                })?
            }
            PlannedKind::File(bytes) => {
                fs::write(&target, bytes).map_err(|source| RewriteError::Io {
                    operation: "write",
                    path: target.clone(),
                    source,
                })?;
                fs::set_permissions(&target, entry.permissions.clone()).map_err(|source| {
                    RewriteError::Io {
                        operation: "set permissions on",
                        path: target,
                        source,
                    }
                })?;
            }
        }
    }
    for entry in plan
        .iter()
        .rev()
        .filter(|entry| matches!(entry.kind, PlannedKind::Directory))
    {
        let target = stage.join(&entry.relative);
        fs::set_permissions(&target, entry.permissions.clone()).map_err(|source| {
            RewriteError::Io {
                operation: "set permissions on",
                path: target,
                source,
            }
        })?;
    }
    fs::set_permissions(stage, root_permissions).map_err(|source| RewriteError::Io {
        operation: "set permissions on",
        path: stage.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn create_stage(parent: &Path) -> Result<PathBuf, RewriteError> {
    for _ in 0..100 {
        let sequence = STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let stage = parent.join(format!(
            ".pg-sql-migrate-stage-{}-{sequence}",
            std::process::id()
        ));
        match fs::create_dir(&stage) {
            Ok(()) => return Ok(stage),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(RewriteError::Io {
                    operation: "create staging directory",
                    path: stage,
                    source,
                });
            }
        }
    }
    Err(RewriteError::Io {
        operation: "create staging directory in",
        path: parent.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique staging directory",
        ),
    })
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn atomic_rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::{CString, c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    const AT_FDCWD: c_int = -100;
    const RENAME_NOREPLACE: c_uint = 1;
    unsafe extern "C" {
        fn renameat2(
            olddirfd: c_int,
            oldpath: *const c_char,
            newdirfd: c_int,
            newpath: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source path contains NUL"))?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "destination path contains NUL")
    })?;
    // SAFETY: both path pointers refer to live NUL-terminated byte strings for
    // the duration of the call; both directory descriptors are the documented
    // AT_FDCWD value, and the only flag is RENAME_NOREPLACE.
    let result = unsafe {
        renameat2(
            AT_FDCWD,
            source.as_ptr(),
            AT_FDCWD,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn atomic_rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::ffi::{CString, c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    const RENAME_EXCL: c_uint = 0x0000_0004;
    unsafe extern "C" {
        fn renamex_np(old: *const c_char, new: *const c_char, flags: c_uint) -> c_int;
    }

    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source path contains NUL"))?;
    let destination = CString::new(destination.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, "destination path contains NUL")
    })?;
    // SAFETY: both pointers refer to live NUL-terminated path byte strings and
    // RENAME_EXCL is the platform's documented no-replace flag.
    let result = unsafe { renamex_np(source.as_ptr(), destination.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn atomic_rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let source: Vec<_> = source.as_os_str().encode_wide().chain([0]).collect();
    let destination: Vec<_> = destination.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: both pointers refer to live NUL-terminated UTF-16 strings. Zero
    // flags deliberately omits MOVEFILE_REPLACE_EXISTING.
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) };
    if result != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    windows
)))]
fn atomic_rename_no_replace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace publication is unsupported on this platform",
    ))
}

struct StageGuard(Option<PathBuf>);

impl StageGuard {
    fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for StageGuard {
    fn drop(&mut self) {
        if let Some(stage) = &self.0 {
            make_removable(stage);
            let _ = fs::remove_dir_all(stage);
        }
    }
}

fn make_removable(path: &Path) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o700);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    let _ = fs::set_permissions(path, permissions);
    if metadata.is_dir()
        && let Ok(children) = fs::read_dir(path)
    {
        for child in children.flatten() {
            make_removable(&child.path());
        }
    }
}

fn metadata(
    operation: &'static str,
    path: &Path,
    symlink: bool,
) -> Result<fs::Metadata, RewriteError> {
    let result = if symlink {
        fs::symlink_metadata(path)
    } else {
        fs::metadata(path)
    };
    result.map_err(|source| RewriteError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn canonicalize(operation: &'static str, path: &Path) -> Result<PathBuf, RewriteError> {
    path.canonicalize().map_err(|source| RewriteError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn path_present(path: &Path) -> Result<bool, RewriteError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(RewriteError::Io {
            operation: "inspect",
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn resolve_existing_ancestors(path: &Path) -> Result<PathBuf, RewriteError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| RewriteError::Io {
                operation: "read current directory for",
                path: path.to_path_buf(),
                source,
            })?
            .join(path)
    };
    let normalized = normalize(&absolute);
    let mut existing = normalized.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| RewriteError::Io {
            operation: "resolve",
            path: normalized.clone(),
            source: io::Error::new(io::ErrorKind::NotFound, "path has no existing ancestor"),
        })?;
        suffix.push(name.to_os_string());
        existing = existing.parent().ok_or_else(|| RewriteError::Io {
            operation: "resolve",
            path: normalized.clone(),
            source: io::Error::new(io::ErrorKind::NotFound, "path has no existing ancestor"),
        })?;
    }
    let mut resolved = canonicalize("canonicalize ancestor of", existing)?;
    for part in suffix.into_iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
