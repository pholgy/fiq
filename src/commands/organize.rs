use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::scanner::scan_directory;

#[derive(Debug, Serialize)]
pub struct OrganizeResult {
    pub total_files: usize,
    pub moves: Vec<FileMove>,
    pub dry_run: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FileMove {
    pub from: String,
    pub to: String,
    pub size: u64,
}

/// Map file extensions to category folders.
fn categorize_by_type(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "tif" => {
            "Images"
        }
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" => "Videos",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" => "Audio",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
        | "txt" | "rtf" | "csv" | "md" => "Documents",
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" => "Archives",
        "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" | "hpp" | "java" | "rb" | "php"
        | "swift" | "kt" | "cs" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "toml" | "yaml"
        | "yml" | "json" | "xml" | "html" | "css" | "scss" | "less" | "sql" | "r" | "lua"
        | "vim" | "el" | "ex" | "exs" | "hs" | "ml" | "clj" => "Code",
        "exe" | "msi" | "dmg" | "app" | "deb" | "rpm" | "appimage" | "bin" => "Executables",
        "ttf" | "otf" | "woff" | "woff2" | "eot" => "Fonts",
        "iso" | "img" | "vmdk" | "vdi" | "qcow2" => "DiskImages",
        _ => "Other",
    }
}

/// Categorize by date (YYYY/MM folder structure).
fn categorize_by_date(modified: Option<std::time::SystemTime>) -> String {
    match modified {
        Some(t) => {
            let datetime: chrono::DateTime<chrono::Local> = t.into();
            datetime.format("%Y/%m").to_string()
        }
        None => "Unknown".to_string(),
    }
}

/// Categorize by size bucket.
fn categorize_by_size(size: u64) -> &'static str {
    if size == 0 {
        "Empty"
    } else if size < 1_000 {
        "Tiny (< 1KB)"
    } else if size < 1_000_000 {
        "Small (1KB-1MB)"
    } else if size < 100_000_000 {
        "Medium (1MB-100MB)"
    } else if size < 1_000_000_000 {
        "Large (100MB-1GB)"
    } else {
        "Huge (> 1GB)"
    }
}

/// Generate a non-colliding path by appending _1, _2, etc.
fn resolve_collision(dest: &Path, mode: &str) -> PathBuf {
    if !dest.exists() || mode == "overwrite" {
        return dest.to_path_buf();
    }

    if mode == "skip" {
        return dest.to_path_buf(); // Caller checks exists + skip
    }

    // mode == "rename"
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = dest.extension().and_then(|e| e.to_str()).unwrap_or("");
    let parent = dest.parent().unwrap_or(Path::new("."));

    for i in 1..1000 {
        let new_name = if ext.is_empty() {
            format!("{}_{}", stem, i)
        } else {
            format!("{}_{}.{}", stem, i, ext)
        };
        let candidate = parent.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
    }

    dest.to_path_buf()
}

pub fn run_organize(
    directory: &str,
    by: &str,
    dry_run: bool,
    mode: &str,
    recursive: bool,
    output: Option<&str>,
) -> OrganizeResult {
    let dir = Path::new(directory);
    let output_base = output
        .map(PathBuf::from)
        .unwrap_or_else(|| dir.to_path_buf());
    let files = scan_directory(dir, recursive);
    let total_files = files.len();

    let mut moves = Vec::new();
    let mut errors = Vec::new();

    // Track destination counts for dry-run collision simulation
    let mut dest_counts: HashMap<PathBuf, usize> = HashMap::new();

    for file in &files {
        let ext = file.extension.as_deref().unwrap_or("");

        let category = match by {
            "type" => categorize_by_type(ext).to_string(),
            "date" => categorize_by_date(file.modified),
            "size" => categorize_by_size(file.size).to_string(),
            _ => {
                errors.push(format!("Unknown strategy: {}", by));
                continue;
            }
        };

        let file_name = file
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let dest_dir = output_base.join(&category);
        let dest_path = dest_dir.join(file_name);

        // Skip if source and destination are the same
        if file.path == dest_path {
            continue;
        }

        let final_dest = if dry_run {
            // Simulate collision handling in dry-run
            let count = dest_counts.entry(dest_path.clone()).or_insert(0);
            *count += 1;
            if *count > 1 && mode == "rename" {
                let stem = dest_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file");
                let ext_str = dest_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext_str.is_empty() {
                    dest_dir.join(format!("{}_{}", stem, *count - 1))
                } else {
                    dest_dir.join(format!("{}_{}.{}", stem, *count - 1, ext_str))
                }
            } else {
                dest_path
            }
        } else {
            // Real move
            if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                errors.push(format!("Failed to create {}: {}", dest_dir.display(), e));
                continue;
            }

            if dest_path.exists() && mode == "skip" {
                continue;
            }

            let resolved = resolve_collision(&dest_path, mode);

            if let Err(e) = std::fs::rename(&file.path, &resolved) {
                // Fall back to copy+delete for cross-device moves
                if e.kind() == std::io::ErrorKind::CrossesDevices || e.raw_os_error() == Some(18) {
                    if let Err(e) = std::fs::copy(&file.path, &resolved)
                        .and_then(|_| std::fs::remove_file(&file.path))
                    {
                        errors.push(format!(
                            "Failed to copy {} → {}: {}",
                            file.path.display(),
                            resolved.display(),
                            e
                        ));
                        continue;
                    }
                } else {
                    errors.push(format!(
                        "Failed to move {} → {}: {}",
                        file.path.display(),
                        resolved.display(),
                        e
                    ));
                    continue;
                }
            }

            resolved
        };

        moves.push(FileMove {
            from: file.path.display().to_string(),
            to: final_dest.display().to_string(),
            size: file.size,
        });
    }

    OrganizeResult {
        total_files,
        moves,
        dry_run,
        errors,
    }
}
