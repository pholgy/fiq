use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use globset::Glob;
use serde::{Deserialize, Serialize};

use crate::scanner::scan_directory_names_only;

/// A persistent trigram index over file names in a directory tree.
///
/// Trigrams are 3-byte substrings of lowercased file names. Given a glob
/// pattern like `*.rs`, we extract the literal `.rs`, decompose it into
/// the trigram `['.','r','s']`, look up its posting list, and verify
/// candidates against the full glob — turning O(total_files) into
/// O(posting_list_size).
#[derive(Serialize, Deserialize)]
pub struct TrigramIndex {
    /// Root directory this index covers
    pub root: PathBuf,
    /// When the index was built
    pub built_at: SystemTime,
    /// (start_offset, length) into path_data for each file's relative path
    path_offsets: Vec<(u32, u16)>,
    /// Packed relative paths (stored as-is, lowercased names used only for trigrams)
    path_data: Vec<u8>,
    /// Trigram → sorted list of path indices
    trigrams: HashMap<[u8; 3], Vec<u32>>,
    /// Total file count
    pub total_files: u32,
}

impl TrigramIndex {
    /// Build a new trigram index by walking the directory tree.
    pub fn build(root: &Path) -> Self {
        let files = scan_directory_names_only(root, true, None);

        let mut path_offsets = Vec::with_capacity(files.len());
        let mut path_data = Vec::with_capacity(files.len() * 30); // ~30 bytes avg relative path
        let mut trigrams: HashMap<[u8; 3], Vec<u32>> = HashMap::new();

        for (idx, file) in files.iter().enumerate() {
            let rel = file
                .path
                .strip_prefix(root)
                .unwrap_or(&file.path)
                .to_string_lossy();
            let rel_bytes = rel.as_bytes();

            let start = path_data.len() as u32;
            let len = rel_bytes.len().min(u16::MAX as usize) as u16;
            path_data.extend_from_slice(&rel_bytes[..len as usize]);
            path_offsets.push((start, len));

            // Extract trigrams from the lowercased file name only (not full path)
            if let Some(name) = file.path.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_lowercase();
                let name_bytes = lower.as_bytes();
                if name_bytes.len() >= 3 {
                    for window in name_bytes.windows(3) {
                        let tri = [window[0], window[1], window[2]];
                        trigrams.entry(tri).or_default().push(idx as u32);
                    }
                }
            }
        }

        // Sort and deduplicate posting lists
        for list in trigrams.values_mut() {
            list.sort_unstable();
            list.dedup();
        }

        TrigramIndex {
            root: root.to_path_buf(),
            built_at: SystemTime::now(),
            path_offsets,
            path_data,
            trigrams,
            total_files: files.len() as u32,
        }
    }

    /// Get the relative path for a given index.
    fn get_path(&self, idx: u32) -> Option<&str> {
        let (start, len) = self.path_offsets.get(idx as usize)?;
        let end = *start as usize + *len as usize;
        std::str::from_utf8(&self.path_data[*start as usize..end]).ok()
    }

    /// Query the index with a glob pattern. Returns matching relative paths.
    /// Returns None if the pattern has no usable trigrams (falls back to full scan).
    pub fn query(&self, pattern: &str) -> Option<Vec<PathBuf>> {
        let tri_sets = extract_trigrams_from_glob(pattern);
        if tri_sets.is_empty() {
            return None; // No useful trigrams — caller should fall back
        }

        // Look up posting lists and intersect
        let mut candidate_indices: Option<Vec<u32>> = None;

        for tri in &tri_sets {
            let posting = match self.trigrams.get(tri) {
                Some(list) => list.as_slice(),
                None => return Some(Vec::new()), // Trigram not in index → no matches
            };

            candidate_indices = Some(match candidate_indices {
                None => posting.to_vec(),
                Some(current) => intersect_sorted(&current, posting),
            });
        }

        let candidates = candidate_indices.unwrap_or_default();

        // Verify candidates against the full glob
        let matcher = Glob::new(pattern)
            .ok()
            .map(|g| g.compile_matcher())
            .unwrap_or_else(|| Glob::new("*").unwrap().compile_matcher());

        let results: Vec<PathBuf> = candidates
            .iter()
            .filter_map(|&idx| {
                let rel = self.get_path(idx)?;
                let path = Path::new(rel);
                let name = path.file_name()?.to_str()?;
                if matcher.is_match(name) {
                    Some(self.root.join(rel))
                } else {
                    None
                }
            })
            .collect();

        Some(results)
    }

    /// Check if the index is still fresh (root dir hasn't been modified since build).
    pub fn is_fresh(&self) -> bool {
        match std::fs::metadata(&self.root).and_then(|m| m.modified()) {
            Ok(mtime) => mtime <= self.built_at,
            Err(_) => false,
        }
    }

    /// Cache directory: ~/.cache/fiq/
    fn cache_dir() -> Option<PathBuf> {
        dirs::cache_dir().map(|d| d.join("fiq"))
    }

    /// Deterministic cache key from root path.
    fn cache_key(root: &Path) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        root.hash(&mut hasher);
        format!("{:016x}.idx", hasher.finish())
    }

    /// Save the index to disk cache.
    pub fn save_to_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dir = Self::cache_dir().ok_or("no cache dir")?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(Self::cache_key(&self.root));
        let bytes = bincode::serialize(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Load a cached index from disk. Returns None if not found or stale.
    pub fn load_cached(root: &Path) -> Option<Self> {
        let dir = Self::cache_dir()?;
        let path = dir.join(Self::cache_key(root));
        let bytes = std::fs::read(path).ok()?;
        let index: Self = bincode::deserialize(&bytes).ok()?;
        if index.root == root && index.is_fresh() {
            Some(index)
        } else {
            None
        }
    }
}

/// Extract trigrams from the literal portions of a glob pattern.
///
/// Examples:
///   "*.rs"       → literals [".rs"]  → trigrams [['.','r','s']]
///   "*.test.js"  → literals [".test.js"] → trigrams [['.','t','e'], ['t','e','s'], ...]
///   "foo*bar"    → literals ["foo", "bar"] → trigrams [['f','o','o'], ['b','a','r']]
///   "*.c"        → literals [".c"] → (too short) → []
///   "*"          → literals [] → []
pub fn extract_trigrams_from_glob(pattern: &str) -> Vec<[u8; 3]> {
    let lower = pattern.to_lowercase();
    let mut trigrams = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Split on glob metacharacters to find literal runs
    let mut literal = String::new();
    for ch in lower.chars() {
        match ch {
            '*' | '?' | '[' | ']' | '{' | '}' => {
                extract_trigrams_from_literal(&literal, &mut trigrams, &mut seen);
                literal.clear();
            }
            _ => literal.push(ch),
        }
    }
    extract_trigrams_from_literal(&literal, &mut trigrams, &mut seen);

    trigrams
}

fn extract_trigrams_from_literal(
    literal: &str,
    trigrams: &mut Vec<[u8; 3]>,
    seen: &mut std::collections::HashSet<[u8; 3]>,
) {
    let bytes = literal.as_bytes();
    if bytes.len() >= 3 {
        for window in bytes.windows(3) {
            let tri = [window[0], window[1], window[2]];
            if seen.insert(tri) {
                trigrams.push(tri);
            }
        }
    }
}

/// Merge-join two sorted u32 slices, returning their intersection.
fn intersect_sorted(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut result = Vec::with_capacity(a.len().min(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                result.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_extract_trigrams_star_rs() {
        let tris = extract_trigrams_from_glob("*.rs");
        assert_eq!(tris, vec![[b'.', b'r', b's']]);
    }

    #[test]
    fn test_extract_trigrams_star_test_js() {
        let tris = extract_trigrams_from_glob("*.test.js");
        assert!(tris.contains(&[b'.', b't', b'e']));
        assert!(tris.contains(&[b'.', b'j', b's']));
        assert!(tris.len() >= 5);
    }

    #[test]
    fn test_extract_trigrams_foo_star_bar() {
        let tris = extract_trigrams_from_glob("foo*bar");
        assert!(tris.contains(&[b'f', b'o', b'o']));
        assert!(tris.contains(&[b'b', b'a', b'r']));
    }

    #[test]
    fn test_extract_trigrams_too_short() {
        assert!(extract_trigrams_from_glob("*.c").is_empty());
        assert!(extract_trigrams_from_glob("*").is_empty());
    }

    #[test]
    fn test_intersect_sorted() {
        assert_eq!(intersect_sorted(&[1, 3, 5, 7], &[2, 3, 5, 8]), vec![3, 5]);
        assert_eq!(intersect_sorted(&[1, 2, 3], &[4, 5, 6]), Vec::<u32>::new());
        assert_eq!(intersect_sorted(&[1, 2, 3], &[1, 2, 3]), vec![1, 2, 3]);
    }

    #[test]
    fn test_build_and_query() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello.rs"), "").unwrap();
        fs::write(dir.path().join("world.rs"), "").unwrap();
        fs::write(dir.path().join("readme.md"), "").unwrap();
        fs::write(dir.path().join("test.txt"), "").unwrap();

        let index = TrigramIndex::build(dir.path());
        assert_eq!(index.total_files, 4);

        // Query for *.rs — should find 2 files
        let results = index.query("*.rs").expect("should use index");
        assert_eq!(results.len(), 2);
        let names: Vec<String> = results
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"hello.rs".to_string()));
        assert!(names.contains(&"world.rs".to_string()));

        // Query for *.txt
        let results = index.query("*.txt").expect("should use index");
        assert_eq!(results.len(), 1);

        // Query for *.c — too short, should return None
        assert!(index.query("*.c").is_none());
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.rs"), "").unwrap();

        let index = TrigramIndex::build(dir.path());
        index.save_to_cache().expect("save failed");

        let loaded = TrigramIndex::load_cached(dir.path()).expect("load failed");
        assert_eq!(loaded.total_files, 1);
        assert_eq!(loaded.root, dir.path());

        let results = loaded.query("*.rs").expect("should use index");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_no_matches() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hello.rs"), "").unwrap();

        let index = TrigramIndex::build(dir.path());
        let results = index.query("*.xyz").expect("should use index");
        assert!(results.is_empty());
    }

    #[test]
    fn test_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("main.rs"), "").unwrap();
        fs::write(sub.join("lib.rs"), "").unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();

        let index = TrigramIndex::build(dir.path());
        assert_eq!(index.total_files, 3);

        let results = index.query("*.rs").expect("should use index");
        assert_eq!(results.len(), 2);
    }
}
