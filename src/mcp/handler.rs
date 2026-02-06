use std::path::Path;

use serde_json::Value;

use crate::commands::{duplicates, organize, search, stats};
use crate::mcp::protocol::ToolResult;

/// Route a tools/call request to the appropriate command function.
/// Returns Err for unknown tools (protocol-level error), Ok for valid tools.
pub fn handle_tool_call(name: &str, arguments: &Value) -> Result<ToolResult, String> {
    match name {
        "scan_stats" => Ok(handle_scan_stats(arguments)),
        "find_duplicates" => Ok(handle_find_duplicates(arguments)),
        "search_files" => Ok(handle_search_files(arguments)),
        "organize_files" => Ok(handle_organize_files(arguments)),
        "build_index" => Ok(handle_build_index(arguments)),
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn handle_scan_stats(args: &Value) -> ToolResult {
    let directory = match args.get("directory").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return ToolResult::error("Missing required parameter: directory".to_string()),
    };
    let top_n = args.get("top_n").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let result = stats::run_stats(directory, top_n, recursive);
    match serde_json::to_string_pretty(&result) {
        Ok(json) => ToolResult::text(json),
        Err(e) => ToolResult::error(format!("Serialization error: {}", e)),
    }
}

fn handle_find_duplicates(args: &Value) -> ToolResult {
    let directory = match args.get("directory").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return ToolResult::error("Missing required parameter: directory".to_string()),
    };
    let min_size = args.get("min_size").and_then(|v| v.as_u64()).unwrap_or(1);
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let result = duplicates::run_duplicates(directory, min_size, recursive);
    match serde_json::to_string_pretty(&result) {
        Ok(json) => ToolResult::text(json),
        Err(e) => ToolResult::error(format!("Serialization error: {}", e)),
    }
}

fn handle_search_files(args: &Value) -> ToolResult {
    let directory = match args.get("directory").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return ToolResult::error("Missing required parameter: directory".to_string()),
    };
    let name = args.get("name").and_then(|v| v.as_str());
    let content = args.get("content").and_then(|v| v.as_str());
    let min_size = args.get("min_size").and_then(|v| v.as_str());
    let max_size = args.get("max_size").and_then(|v| v.as_str());
    let newer = args.get("newer").and_then(|v| v.as_str());
    let older = args.get("older").and_then(|v| v.as_str());
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // MCP mode: use in-memory index cache for instant repeated searches
    let result = search::run_search_with_cache(
        &search::SearchParams {
            directory,
            name_pattern: name,
            content_query: content,
            min_size,
            max_size,
            newer,
            older,
            recursive,
        },
        true,
    );
    match serde_json::to_string_pretty(&result) {
        Ok(json) => ToolResult::text(json),
        Err(e) => ToolResult::error(format!("Serialization error: {}", e)),
    }
}

fn handle_build_index(args: &Value) -> ToolResult {
    let directory = match args.get("directory").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return ToolResult::error("Missing required parameter: directory".to_string()),
    };

    let dir = Path::new(directory);
    if !dir.is_dir() {
        return ToolResult::error(format!("Not a directory: {}", directory));
    }

    let index = crate::index_cache::build_index(dir, true);
    let response = serde_json::json!({
        "status": "ok",
        "total_files": index.total_files,
        "directory": directory,
    });
    ToolResult::text(response.to_string())
}

fn handle_organize_files(args: &Value) -> ToolResult {
    let directory = match args.get("directory").and_then(|v| v.as_str()) {
        Some(d) => d,
        None => return ToolResult::error("Missing required parameter: directory".to_string()),
    };
    let by = args.get("by").and_then(|v| v.as_str()).unwrap_or("type");
    let dry_run = args
        .get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mode = args
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("rename");
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let output = args.get("output").and_then(|v| v.as_str());

    let result = organize::run_organize(directory, by, dry_run, mode, recursive, output);
    match serde_json::to_string_pretty(&result) {
        Ok(json) => ToolResult::text(json),
        Err(e) => ToolResult::error(format!("Serialization error: {}", e)),
    }
}
