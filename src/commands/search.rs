use std::fs::File;
use std::path::Path;
use std::time::{Duration, SystemTime};

use memmap2::Mmap;
use rayon::prelude::*;
use serde::Serialize;

use crate::scanner::{FileInfo, scan_directory_filtered, scan_directory_names_only};

const MMAP_THRESHOLD: u64 = 128 * 1024;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub total_matches: usize,
    pub files_scanned: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_matches: Option<Vec<ContentMatch>>,
}

#[derive(Debug, Serialize)]
pub struct ContentMatch {
    pub line_number: usize,
    pub line: String,
}

/// Parse a size string like "1KB", "10MB", "1GB" into bytes.
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }

    let (num_str, multiplier) = if let Some(n) = s.strip_suffix("GB") {
        (n, 1_000_000_000u64)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n, 1_000_000u64)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n, 1_000u64)
    } else if let Some(n) = s.strip_suffix('B') {
        (n, 1u64)
    } else {
        return None;
    };

    num_str
        .trim()
        .parse::<f64>()
        .ok()
        .map(|n| (n * multiplier as f64) as u64)
}

/// Parse a relative time string like "7d", "24h", "30m" into a SystemTime.
pub fn parse_time(s: &str) -> Option<SystemTime> {
    let s = s.trim();

    // Try parsing as a date: YYYY-MM-DD
    if s.len() == 10
        && s.chars().nth(4) == Some('-')
        && let Ok(dt) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
    {
        let datetime = dt.and_hms_opt(0, 0, 0)?;
        let timestamp = datetime.and_utc().timestamp();
        if timestamp < 0 {
            return None;
        }
        return Some(SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp as u64));
    }

    // Try relative times: 7d, 24h, 30m
    let (num_str, unit) = if let Some(n) = s.strip_suffix('d') {
        (n, 86400u64)
    } else if let Some(n) = s.strip_suffix('h') {
        (n, 3600u64)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60u64)
    } else {
        return None;
    };

    let num: u64 = num_str.trim().parse().ok()?;
    let duration = Duration::from_secs(num * unit);
    SystemTime::now().checked_sub(duration)
}

/// Check if file content contains the search string. Returns matching lines.
fn search_content(file: &FileInfo, query: &str) -> Option<Vec<ContentMatch>> {
    let path = &file.path;

    let content = if file.size >= MMAP_THRESHOLD {
        let f = File::open(path).ok()?;
        let mmap = unsafe { Mmap::map(&f).ok()? };
        // Check if the mmap data looks like valid UTF-8 (or at least contains the query)
        String::from_utf8_lossy(&mmap).into_owned()
    } else {
        std::fs::read_to_string(path).ok()?
    };

    let query_lower = query.to_lowercase();
    let matches: Vec<ContentMatch> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.to_lowercase().contains(&query_lower))
        .take(10) // Limit matches per file
        .map(|(i, line)| ContentMatch {
            line_number: i + 1,
            line: if line.len() > 200 {
                let end = line.floor_char_boundary(200);
                format!("{}...", &line[..end])
            } else {
                line.to_string()
            },
        })
        .collect();

    if matches.is_empty() {
        None
    } else {
        Some(matches)
    }
}

pub struct SearchParams<'a> {
    pub directory: &'a str,
    pub name_pattern: Option<&'a str>,
    pub content_query: Option<&'a str>,
    pub min_size: Option<&'a str>,
    pub max_size: Option<&'a str>,
    pub newer: Option<&'a str>,
    pub older: Option<&'a str>,
    pub recursive: bool,
}

pub fn run_search(params: &SearchParams<'_>) -> SearchResult {
    run_search_with_cache(params, false)
}

pub fn run_search_with_cache(params: &SearchParams<'_>, use_memory_cache: bool) -> SearchResult {
    let dir = Path::new(params.directory);

    // Determine if we need metadata (size/date/content filters)
    let needs_metadata = params.min_size.is_some()
        || params.max_size.is_some()
        || params.newer.is_some()
        || params.older.is_some()
        || params.content_query.is_some();

    // Try trigram index fast path for name-only searches
    if !needs_metadata
        && let Some(name_pattern) = params.name_pattern
        && let Some(result) = crate::index_cache::try_indexed_search(
            dir,
            name_pattern,
            params.recursive,
            use_memory_cache,
        )
    {
        return result;
    }

    // Full scan path
    let files = if needs_metadata {
        scan_directory_filtered(dir, params.recursive, params.name_pattern)
    } else {
        scan_directory_names_only(dir, params.recursive, params.name_pattern)
    };
    let files_scanned = files.len();

    // Build remaining filters (name already applied by scanner)
    let min_bytes = params.min_size.and_then(parse_size);
    let max_bytes = params.max_size.and_then(parse_size);
    let newer_time = params.newer.and_then(parse_time);
    let older_time = params.older.and_then(parse_time);

    // Apply filters: size → date → content (name already done)
    let filtered = files
        .iter()
        .filter(|f| {
            // Size filters
            if let Some(min) = min_bytes
                && f.size < min
            {
                return false;
            }
            if let Some(max) = max_bytes
                && f.size > max
            {
                return false;
            }
            true
        })
        .filter(|f| {
            // Date filters
            if let Some(newer_t) = newer_time {
                match f.modified {
                    Some(mod_time) if mod_time >= newer_t => {}
                    _ => return false,
                }
            }
            if let Some(older_t) = older_time {
                match f.modified {
                    Some(mod_time) if mod_time <= older_t => {}
                    _ => return false,
                }
            }
            true
        });

    // Content search (most expensive, done in parallel via par_bridge)
    let matches: Vec<SearchMatch> = if let Some(query) = params.content_query {
        filtered
            .par_bridge()
            .filter_map(|f| {
                let content_matches = search_content(f, query);
                content_matches.map(|cm| SearchMatch {
                    path: f.path.display().to_string(),
                    size: f.size,
                    content_matches: Some(cm),
                })
            })
            .collect()
    } else {
        filtered
            .map(|f| SearchMatch {
                path: f.path.display().to_string(),
                size: f.size,
                content_matches: None,
            })
            .collect()
    };

    let total_matches = matches.len();

    SearchResult {
        matches,
        total_matches,
        files_scanned,
    }
}
