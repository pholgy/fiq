use serde_json::{Value, json};

/// Return the list of tools with their JSON Schema definitions.
pub fn tool_definitions() -> Value {
    json!({
        "tools": [
            {
                "name": "scan_stats",
                "description": "Get file statistics for a directory: total files, total size, breakdown by extension, and largest files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory path to scan"
                        },
                        "top_n": {
                            "type": "integer",
                            "description": "Number of largest files to return",
                            "default": 10
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Scan subdirectories",
                            "default": true
                        }
                    },
                    "required": ["directory"]
                }
            },
            {
                "name": "find_duplicates",
                "description": "Find duplicate files by content hash (blake3). Groups files by size first, then hashes only potential duplicates for speed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory path to scan"
                        },
                        "min_size": {
                            "type": "integer",
                            "description": "Minimum file size in bytes to consider",
                            "default": 1
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Scan subdirectories",
                            "default": true
                        }
                    },
                    "required": ["directory"]
                }
            },
            {
                "name": "search_files",
                "description": "Search for files by name pattern, content, size range, and date range. Filters are applied cheapest-first for speed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory path to search"
                        },
                        "name": {
                            "type": "string",
                            "description": "Glob pattern for file names (e.g. '*.rs', '*.{js,ts}')"
                        },
                        "content": {
                            "type": "string",
                            "description": "Search file contents for this string (case-insensitive)"
                        },
                        "min_size": {
                            "type": "string",
                            "description": "Minimum file size (e.g. '1KB', '10MB')"
                        },
                        "max_size": {
                            "type": "string",
                            "description": "Maximum file size (e.g. '100MB', '1GB')"
                        },
                        "newer": {
                            "type": "string",
                            "description": "Files modified after this time (e.g. '2024-01-01', '7d', '24h')"
                        },
                        "older": {
                            "type": "string",
                            "description": "Files modified before this time (e.g. '2024-01-01', '7d', '24h')"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Search subdirectories",
                            "default": true
                        }
                    },
                    "required": ["directory"]
                }
            },
            {
                "name": "organize_files",
                "description": "Organize files into folders by type, date, or size. Supports dry-run mode to preview changes without moving files.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Directory to organize"
                        },
                        "by": {
                            "type": "string",
                            "description": "Organization strategy: 'type', 'date', or 'size'",
                            "enum": ["type", "date", "size"],
                            "default": "type"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "description": "Preview changes without moving files",
                            "default": true
                        },
                        "mode": {
                            "type": "string",
                            "description": "Collision handling: 'skip', 'rename', or 'overwrite'",
                            "enum": ["skip", "rename", "overwrite"],
                            "default": "rename"
                        },
                        "recursive": {
                            "type": "boolean",
                            "description": "Process subdirectories",
                            "default": true
                        },
                        "output": {
                            "type": "string",
                            "description": "Output directory (default: organize in-place)"
                        }
                    },
                    "required": ["directory"]
                }
            }
        ]
    })
}
