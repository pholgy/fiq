use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use globset::{Glob, GlobMatcher};
use ignore::WalkBuilder;
use ignore::WalkState;
use ignore::overrides::OverrideBuilder;
use serde::Serialize;

const DEFAULT_WALKER_THREADS: usize = 4;
const BATCH_SIZE: usize = 512;

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
    pub extension: Option<String>,
}

/// Thread count for the I/O-bound directory walker.
/// Override with `FIQ_THREADS` env var. Defaults to 4.
fn walker_threads() -> usize {
    std::env::var("FIQ_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WALKER_THREADS)
}

/// Batch collector that flushes to a shared Vec on drop,
/// reducing Mutex acquisitions from once-per-file to once-per-batch.
struct Collector {
    batch: Vec<FileInfo>,
    target: Arc<Mutex<Vec<FileInfo>>>,
}

impl Collector {
    fn push(&mut self, info: FileInfo) {
        self.batch.push(info);
        if self.batch.len() >= BATCH_SIZE {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if !self.batch.is_empty() {
            self.target.lock().unwrap().append(&mut self.batch);
        }
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Walk a directory, collecting all files. Used by stats, duplicates, organize.
pub fn scan_directory(dir: &Path, recursive: bool) -> Vec<FileInfo> {
    scan_directory_filtered(dir, recursive, None)
}

/// Walk a directory with an optional name glob filter.
///
/// Three levels of optimization depending on what's needed:
///   - No filter: full metadata collection (stats, duplicates, organize)
///   - Name filter: skip non-matching files at walker level, skip extension computation
///   - skip_metadata: skip stat() entirely (set size=0, modified=None) for name-only search
pub fn scan_directory_filtered(
    dir: &Path,
    recursive: bool,
    name_glob: Option<&str>,
) -> Vec<FileInfo> {
    scan_directory_impl(dir, recursive, name_glob, false)
}

/// Walk a directory, skipping metadata collection for maximum speed.
/// Files will have size=0 and modified=None.
pub fn scan_directory_names_only(
    dir: &Path,
    recursive: bool,
    name_glob: Option<&str>,
) -> Vec<FileInfo> {
    scan_directory_impl(dir, recursive, name_glob, true)
}

fn scan_directory_impl(
    dir: &Path,
    recursive: bool,
    name_glob: Option<&str>,
    skip_metadata: bool,
) -> Vec<FileInfo> {
    let files = Arc::new(Mutex::new(Vec::with_capacity(if name_glob.is_some() {
        256
    } else {
        4096
    })));

    let mut builder = WalkBuilder::new(dir);
    builder
        .max_depth(if recursive { None } else { Some(1) })
        .hidden(false)
        // Disable all ignore/gitignore features to eliminate per-directory
        // .git stat + gitignore parsing overhead (thousands of saved syscalls)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .ignore(false);

    // Push name glob into the walker as an override when possible.
    // The walker skips non-matching files internally — they never
    // reach our callback (no file_type check, no path extraction).
    let mut has_override = false;
    let manual_matcher: Arc<Option<GlobMatcher>>;

    if let Some(pattern) = name_glob {
        let mut ob = OverrideBuilder::new(dir);
        if ob.add(pattern).is_ok()
            && let Ok(overrides) = ob.build()
        {
            builder.overrides(overrides);
            has_override = true;
        }

        if has_override {
            manual_matcher = Arc::new(None);
            // Half the CPUs for filtered scans — enough parallelism on the I/O
            // bus without wasting CPU on context switching
            builder.threads(
                std::thread::available_parallelism()
                    .map(|n| (n.get() / 2).max(2))
                    .unwrap_or(4),
            );
        } else {
            manual_matcher =
                Arc::new(Some(Glob::new(pattern).ok().map(|g| g.compile_matcher())).flatten());
            builder.threads(walker_threads());
        }
    } else {
        manual_matcher = Arc::new(None);
        builder.threads(walker_threads());
    }

    let is_filtered = name_glob.is_some();

    builder.build_parallel().run(|| {
        let matcher = Arc::clone(&manual_matcher);
        let mut collector = Collector {
            batch: Vec::with_capacity(if is_filtered { 64 } else { BATCH_SIZE }),
            target: Arc::clone(&files),
        };

        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return WalkState::Continue,
            };

            // file_type() comes from readdir — no stat() syscall
            if entry.file_type().is_none_or(|ft| ft.is_dir()) {
                return WalkState::Continue;
            }

            // Manual name filter only when override wasn't set
            if let Some(ref m) = *matcher {
                let name = entry
                    .path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !m.is_match(name) {
                    return WalkState::Continue;
                }
            }

            let path = entry.into_path();

            if skip_metadata {
                collector.push(FileInfo {
                    path,
                    size: 0,
                    modified: None,
                    is_dir: false,
                    extension: None,
                });
            } else {
                // metadata() only for files that passed all cheap filters
                let metadata = match std::fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => return WalkState::Continue,
                };

                // Skip extension computation for filtered scans — search
                // never uses it, saves a String allocation per match
                let extension = if is_filtered {
                    None
                } else {
                    path.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                };

                collector.push(FileInfo {
                    path,
                    size: metadata.len(),
                    modified: metadata.modified().ok(),
                    is_dir: false,
                    extension,
                });
            }

            WalkState::Continue
        })
    });

    Arc::try_unwrap(files).unwrap().into_inner().unwrap()
}
