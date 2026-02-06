use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::scanner::scan_directory;

#[derive(Debug, Serialize)]
pub struct StatsResult {
    pub total_files: usize,
    pub total_size: u64,
    pub by_extension: Vec<ExtensionStats>,
    pub largest_files: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
pub struct ExtensionStats {
    pub extension: String,
    pub count: usize,
    pub total_size: u64,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
}

pub fn run_stats(directory: &str, top_n: usize, recursive: bool) -> StatsResult {
    let dir = Path::new(directory);
    let mut files = scan_directory(dir, recursive);

    let total_files = files.len();
    let total_size: u64 = files.iter().map(|f| f.size).sum();

    // Group by extension
    let mut ext_map: HashMap<String, (usize, u64)> = HashMap::new();
    for file in &files {
        let ext = file
            .extension
            .clone()
            .unwrap_or_else(|| "(no ext)".to_string());
        let entry = ext_map.entry(ext).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += file.size;
    }

    let mut by_extension: Vec<ExtensionStats> = ext_map
        .into_iter()
        .map(|(extension, (count, total_size))| ExtensionStats {
            extension,
            count,
            total_size,
        })
        .collect();
    by_extension.sort_by(|a, b| b.total_size.cmp(&a.total_size));

    // Largest files — sort in-place, no intermediate Vec<&FileInfo>
    files.sort_unstable_by(|a, b| b.size.cmp(&a.size));

    let largest_files: Vec<FileEntry> = files
        .iter()
        .take(top_n)
        .map(|f| FileEntry {
            path: f.path.display().to_string(),
            size: f.size,
        })
        .collect();

    StatsResult {
        total_files,
        total_size,
        by_extension,
        largest_files,
    }
}
