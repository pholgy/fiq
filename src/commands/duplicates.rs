use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use memmap2::Mmap;
use rayon::prelude::*;
use serde::Serialize;

use crate::scanner::scan_directory;

/// Threshold for memory-mapping files vs reading them directly.
const MMAP_THRESHOLD: u64 = 128 * 1024; // 128 KB

#[derive(Debug, Serialize)]
pub struct DuplicatesResult {
    pub total_files_scanned: usize,
    pub duplicate_groups: Vec<DuplicateGroup>,
    pub total_wasted_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub files: Vec<String>,
}

/// Hash a file using blake3. Uses mmap for large files.
fn hash_file(path: &Path, size: u64) -> Option<String> {
    if size == 0 {
        return Some(blake3::hash(b"").to_hex().to_string());
    }

    if size >= MMAP_THRESHOLD {
        // Memory-map large files
        let file = File::open(path).ok()?;
        let mmap = unsafe { Mmap::map(&file).ok()? };
        let hash = blake3::hash(&mmap);
        Some(hash.to_hex().to_string())
    } else {
        // Read small files directly
        let data = std::fs::read(path).ok()?;
        let hash = blake3::hash(&data);
        Some(hash.to_hex().to_string())
    }
}

pub fn run_duplicates(directory: &str, min_size: u64, recursive: bool) -> DuplicatesResult {
    let dir = Path::new(directory);
    let files = scan_directory(dir, recursive);

    let total_files_scanned = files.len();

    // Step 1: Group by size (files with unique sizes can't be duplicates)
    let mut size_groups: HashMap<u64, Vec<&crate::scanner::FileInfo>> = HashMap::new();
    for file in &files {
        if file.size >= min_size {
            size_groups.entry(file.size).or_default().push(file);
        }
    }

    // Step 2: Hash candidates in parallel (only files sharing a size with others)
    let hashed: Vec<(String, String, u64)> = size_groups
        .into_values()
        .filter(|group| group.len() > 1)
        .flatten()
        .collect::<Vec<_>>()
        .par_iter()
        .filter_map(|file| {
            let hash = hash_file(&file.path, file.size)?;
            Some((hash, file.path.display().to_string(), file.size))
        })
        .collect();

    // Step 3: Group by hash
    let mut hash_groups: HashMap<String, (u64, Vec<String>)> = HashMap::new();
    for (hash, path, size) in hashed {
        let entry = hash_groups.entry(hash).or_insert((size, Vec::new()));
        entry.1.push(path);
    }

    // Only keep actual duplicates (2+ files with same hash)
    let mut duplicate_groups: Vec<DuplicateGroup> = hash_groups
        .into_iter()
        .filter(|(_, (_, files))| files.len() > 1)
        .map(|(hash, (size, files))| DuplicateGroup { hash, size, files })
        .collect();

    duplicate_groups.sort_by(|a, b| {
        let a_waste = a.size * (a.files.len() as u64 - 1);
        let b_waste = b.size * (b.files.len() as u64 - 1);
        b_waste.cmp(&a_waste)
    });

    let total_wasted_bytes: u64 = duplicate_groups
        .iter()
        .map(|g| g.size * (g.files.len() as u64 - 1))
        .sum();

    DuplicatesResult {
        total_files_scanned,
        duplicate_groups,
        total_wasted_bytes,
    }
}
