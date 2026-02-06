use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::commands::search::{SearchMatch, SearchResult};
use crate::index::TrigramIndex;

/// Global in-memory index cache for MCP mode.
/// Keeps built indices alive between tool calls so repeated searches are instant.
static INDEX_CACHE: Mutex<Option<HashMap<PathBuf, Arc<TrigramIndex>>>> = Mutex::new(None);

/// Get or build a trigram index for a directory.
///
/// - `use_memory_cache=true` (MCP mode): checks in-memory cache first, then disk, then builds.
/// - `use_memory_cache=false` (CLI mode): checks disk cache only, then builds.
pub fn get_or_build_index(root: &Path, use_memory_cache: bool) -> Arc<TrigramIndex> {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    // Check in-memory cache
    if use_memory_cache {
        let cache = INDEX_CACHE.lock().unwrap();
        if let Some(map) = cache.as_ref()
            && let Some(idx) = map.get(&canonical)
            && idx.is_fresh()
        {
            return Arc::clone(idx);
        }
    }

    // Check disk cache
    if let Some(idx) = TrigramIndex::load_cached(&canonical) {
        let arc = Arc::new(idx);
        if use_memory_cache {
            store_in_cache(&canonical, Arc::clone(&arc));
        }
        return arc;
    }

    // Build from scratch
    let idx = Arc::new(TrigramIndex::build(&canonical));
    let _ = idx.save_to_cache();
    if use_memory_cache {
        store_in_cache(&canonical, Arc::clone(&idx));
    }
    idx
}

fn store_in_cache(root: &Path, idx: Arc<TrigramIndex>) {
    let mut cache = INDEX_CACHE.lock().unwrap();
    let map = cache.get_or_insert_with(HashMap::new);
    map.insert(root.to_path_buf(), idx);
}

/// Try to answer a name-only search using the trigram index.
/// Returns None if the pattern has no usable trigrams (caller should fall back to full scan).
pub fn try_indexed_search(
    dir: &Path,
    name_pattern: &str,
    recursive: bool,
    use_memory_cache: bool,
) -> Option<SearchResult> {
    // Only use index for recursive searches (index always covers full tree)
    if !recursive {
        return None;
    }

    // Check if the pattern has enough trigrams to be useful
    let trigrams = crate::index::extract_trigrams_from_glob(name_pattern);
    if trigrams.is_empty() {
        return None;
    }

    let index = get_or_build_index(dir, use_memory_cache);
    let paths = index.query(name_pattern)?;

    let matches: Vec<SearchMatch> = paths
        .into_iter()
        .map(|path| SearchMatch {
            path: path.display().to_string(),
            size: 0,
            content_matches: None,
        })
        .collect();

    let total_matches = matches.len();

    Some(SearchResult {
        matches,
        total_matches,
        files_scanned: index.total_files as usize,
    })
}

/// Build (or rebuild) the index for a directory explicitly.
/// Used by the MCP `build_index` tool.
pub fn build_index(root: &Path, use_memory_cache: bool) -> Arc<TrigramIndex> {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    // Always build fresh
    let idx = Arc::new(TrigramIndex::build(&canonical));
    let _ = idx.save_to_cache();
    if use_memory_cache {
        store_in_cache(&canonical, Arc::clone(&idx));
    }
    idx
}
