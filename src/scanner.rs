use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use ignore::WalkBuilder;
use ignore::WalkState;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub is_dir: bool,
    pub extension: Option<String>,
}

const DEFAULT_WALKER_THREADS: usize = 4;

/// Thread count for the I/O-bound directory walker.
/// Override with `FIQ_THREADS` env var. Defaults to 4.
fn walker_threads() -> usize {
    std::env::var("FIQ_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WALKER_THREADS)
}

/// Walk a directory using the `ignore` crate's parallel walker (respects .gitignore).
/// Returns a Vec<FileInfo> of all files (not directories) found.
pub fn scan_directory(dir: &Path, recursive: bool) -> Vec<FileInfo> {
    let files = Mutex::new(Vec::with_capacity(4096));

    WalkBuilder::new(dir)
        .max_depth(if recursive { None } else { Some(1) })
        .hidden(false)
        .threads(walker_threads())
        .build_parallel()
        .run(|| {
            Box::new(|entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return WalkState::Continue,
                };

                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => return WalkState::Continue,
                };

                if metadata.is_dir() {
                    return WalkState::Continue;
                }

                let path = entry.into_path();

                let extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase());

                let modified = metadata.modified().ok();

                let info = FileInfo {
                    path,
                    size: metadata.len(),
                    modified,
                    is_dir: false,
                    extension,
                };

                files.lock().unwrap().push(info);

                WalkState::Continue
            })
        });

    files.into_inner().unwrap()
}
