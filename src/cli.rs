use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "fiq",
    version,
    about = "Fast file intelligence CLI + MCP server"
)]
pub struct Cli {
    /// Run as MCP (Model Context Protocol) JSON-RPC server over stdio
    #[arg(long)]
    pub mcp: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show file statistics for a directory
    Stats {
        /// Directory to scan
        #[arg(default_value = ".")]
        directory: String,

        /// Number of largest files to show
        #[arg(long, default_value = "10")]
        top: usize,

        /// Scan recursively
        #[arg(long, short, default_value = "true")]
        recursive: bool,
    },

    /// Find duplicate files by content hash
    Duplicates {
        /// Directory to scan
        #[arg(default_value = ".")]
        directory: String,

        /// Minimum file size to consider (bytes)
        #[arg(long, default_value = "1")]
        min_size: u64,

        /// Scan recursively
        #[arg(long, short, default_value = "true")]
        recursive: bool,
    },

    /// Search for files by name, content, size, or date
    Search {
        /// Directory to search
        #[arg(default_value = ".")]
        directory: String,

        /// Glob pattern for file names (e.g. "*.rs")
        #[arg(long)]
        name: Option<String>,

        /// Search file contents for this string
        #[arg(long)]
        content: Option<String>,

        /// Minimum file size (e.g. "1KB", "10MB")
        #[arg(long)]
        min_size: Option<String>,

        /// Maximum file size (e.g. "100MB", "1GB")
        #[arg(long)]
        max_size: Option<String>,

        /// Files newer than this (e.g. "2024-01-01", "7d", "24h")
        #[arg(long)]
        newer: Option<String>,

        /// Files older than this (e.g. "2024-01-01", "7d", "24h")
        #[arg(long)]
        older: Option<String>,

        /// Scan recursively
        #[arg(long, short, default_value = "true")]
        recursive: bool,
    },

    /// Organize files into folders by type, date, or size
    Organize {
        /// Directory to organize
        directory: String,

        /// Organization strategy
        #[arg(long, default_value = "type")]
        by: String,

        /// Preview changes without moving files
        #[arg(long)]
        dry_run: bool,

        /// How to handle conflicts: skip, rename, overwrite
        #[arg(long, default_value = "rename")]
        mode: String,

        /// Process subdirectories
        #[arg(long, short, default_value = "true")]
        recursive: bool,

        /// Output directory (default: organize in-place)
        #[arg(long)]
        output: Option<String>,
    },
}
