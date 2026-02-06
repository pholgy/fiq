use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::commands::duplicates::DuplicatesResult;
use crate::commands::organize::OrganizeResult;
use crate::commands::search::SearchResult;
use crate::commands::stats::StatsResult;

/// Format a byte count into a human-readable string.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_000;
    const MB: u64 = 1_000_000;
    const GB: u64 = 1_000_000_000;
    const TB: u64 = 1_000_000_000_000;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn write_colored(stream: &mut StandardStream, text: &str, color: Color) {
    let _ = stream.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(true));
    let _ = write!(stream, "{}", text);
    let _ = stream.reset();
}

fn write_bold(stream: &mut StandardStream, text: &str) {
    let _ = stream.set_color(ColorSpec::new().set_bold(true));
    let _ = write!(stream, "{}", text);
    let _ = stream.reset();
}

pub fn print_stats(result: &StatsResult) {
    let mut out = StandardStream::stdout(ColorChoice::Auto);

    write_colored(&mut out, "\n  Directory Stats\n", Color::Cyan);
    let _ = writeln!(out);

    write_bold(&mut out, "  Total files: ");
    let _ = writeln!(out, "{}", result.total_files);

    write_bold(&mut out, "  Total size:  ");
    let _ = writeln!(out, "{}", format_size(result.total_size));
    let _ = writeln!(out);

    if !result.by_extension.is_empty() {
        write_colored(&mut out, "  By Extension\n", Color::Yellow);
        let _ = writeln!(out, "  {:<15} {:>8} {:>12}", "Extension", "Count", "Size");
        let _ = writeln!(out, "  {}", "-".repeat(37));

        for ext in &result.by_extension {
            let _ = writeln!(
                out,
                "  {:<15} {:>8} {:>12}",
                format!(".{}", ext.extension),
                ext.count,
                format_size(ext.total_size)
            );
        }
        let _ = writeln!(out);
    }

    if !result.largest_files.is_empty() {
        write_colored(&mut out, "  Largest Files\n", Color::Yellow);
        for (i, file) in result.largest_files.iter().enumerate() {
            let _ = writeln!(
                out,
                "  {}. {} ({})",
                i + 1,
                file.path,
                format_size(file.size)
            );
        }
        let _ = writeln!(out);
    }
}

pub fn print_duplicates(result: &DuplicatesResult) {
    let mut out = StandardStream::stdout(ColorChoice::Auto);

    write_colored(&mut out, "\n  Duplicate Files\n", Color::Cyan);
    let _ = writeln!(out);

    write_bold(&mut out, "  Files scanned: ");
    let _ = writeln!(out, "{}", result.total_files_scanned);

    write_bold(&mut out, "  Duplicate groups: ");
    let _ = writeln!(out, "{}", result.duplicate_groups.len());

    write_bold(&mut out, "  Wasted space: ");
    let _ = writeln!(out, "{}", format_size(result.total_wasted_bytes));
    let _ = writeln!(out);

    for (i, group) in result.duplicate_groups.iter().enumerate() {
        write_colored(
            &mut out,
            &format!(
                "  Group {} ({}, {} copies)\n",
                i + 1,
                format_size(group.size),
                group.files.len()
            ),
            Color::Yellow,
        );
        for file in &group.files {
            let _ = writeln!(out, "    {}", file);
        }
        let _ = writeln!(out);
    }
}

pub fn print_search(result: &SearchResult) {
    let mut out = StandardStream::stdout(ColorChoice::Auto);

    write_colored(&mut out, "\n  Search Results\n", Color::Cyan);
    let _ = writeln!(out);

    write_bold(&mut out, "  Files scanned: ");
    let _ = writeln!(out, "{}", result.files_scanned);

    write_bold(&mut out, "  Matches: ");
    let _ = writeln!(out, "{}", result.total_matches);
    let _ = writeln!(out);

    for m in &result.matches {
        write_colored(&mut out, &format!("  {}", m.path), Color::Green);
        let _ = writeln!(out, "  ({})", format_size(m.size));

        if let Some(ref content_matches) = m.content_matches {
            for cm in content_matches {
                let _ = write!(out, "    ");
                write_colored(&mut out, &format!("{}:", cm.line_number), Color::Yellow);
                let _ = writeln!(out, " {}", cm.line.trim());
            }
        }
    }

    let _ = writeln!(out);
}

pub fn print_organize(result: &OrganizeResult) {
    let mut out = StandardStream::stdout(ColorChoice::Auto);

    if result.dry_run {
        write_colored(&mut out, "\n  Organize Preview (dry run)\n", Color::Cyan);
    } else {
        write_colored(&mut out, "\n  Organize Complete\n", Color::Cyan);
    }
    let _ = writeln!(out);

    write_bold(&mut out, "  Total files: ");
    let _ = writeln!(out, "{}", result.total_files);

    write_bold(&mut out, "  Files to move: ");
    let _ = writeln!(out, "{}", result.moves.len());
    let _ = writeln!(out);

    for m in &result.moves {
        let _ = write!(out, "  ");
        write_colored(&mut out, &m.from, Color::Red);
        let _ = write!(out, " → ");
        write_colored(&mut out, &m.to, Color::Green);
        let _ = writeln!(out, "  ({})", format_size(m.size));
    }

    if !result.errors.is_empty() {
        let _ = writeln!(out);
        write_colored(&mut out, "  Errors:\n", Color::Red);
        for err in &result.errors {
            let _ = writeln!(out, "    {}", err);
        }
    }

    let _ = writeln!(out);
}
