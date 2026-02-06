# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is fiq

fiq is a fast file intelligence CLI tool and MCP (Model Context Protocol) server written in Rust. It provides directory scanning, duplicate detection (via blake3 hashing), file search, and file organization capabilities. It runs either as a CLI with subcommands or as a JSON-RPC 2.0 MCP server over stdio (`--mcp` flag).

## Build & Dev Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build (optimized: opt-level=3, thin LTO, stripped)
cargo test                     # Run all tests (CLI integration + MCP protocol tests)
cargo test test_mcp_initialize # Run a single test by name
cargo clippy                   # Lint
cargo fmt                      # Format
```

Requires Rust edition 2024 (Rust 1.85+). Uses `let` chains and `floor_char_boundary` (both stable in edition 2024).

## Architecture

The binary has two modes controlled at the top of `main.rs`:
- **CLI mode** (default): `Cli` parsed via clap derive, dispatches to `commands::{stats,duplicates,search,organize}`, output formatted by `output.rs` using `termcolor`
- **MCP server mode** (`--mcp`): newline-delimited JSON-RPC 2.0 over stdin/stdout, handled by `mcp::server` → `mcp::handler` → same command functions

### Key modules

- `scanner.rs` — Core parallel directory walker using the `ignore` crate's `build_parallel()` (respects .gitignore). Returns `Vec<FileInfo>`. Thread count defaults to 4, configurable via `FIQ_THREADS` env var. All commands build on this.
- `commands/` — Each subcommand (`stats`, `duplicates`, `search`, `organize`) has a `run_*` function returning a serializable result struct. These are pure logic, no I/O formatting.
- `output.rs` — CLI-only colored terminal output. Each command has a `print_*` function.
- `mcp/protocol.rs` — JSON-RPC 2.0 types (`JsonRpcRequest`, `JsonRpcResponse`, `ToolCallParams`, `ToolResult`)
- `mcp/tools.rs` — MCP tool definitions (JSON Schema) for the 4 tools: `scan_stats`, `find_duplicates`, `search_files`, `organize_files`
- `mcp/handler.rs` — Routes `tools/call` to command functions, extracts args from `serde_json::Value`. Returns `Result<ToolResult, String>` — `Err` for unknown tools (protocol-level `-32602`), `Ok` for execution results.
- `mcp/server.rs` — Stdin/stdout loop, handles `initialize`, `ping`, `tools/list`, `tools/call`. Notifications (requests without `id`) are silently dropped per JSON-RPC 2.0.

### Design patterns

- Command functions return `Serialize` result structs — CLI mode pretty-prints them, MCP mode serializes to JSON
- Directory walking uses `ignore::WalkBuilder::build_parallel()` with a `Mutex<Vec<FileInfo>>` for thread-safe collection. Default 4 threads (I/O-bound sweet spot), overridable via `FIQ_THREADS` env var
- Duplicate detection uses a two-phase approach: group by file size first, then hash only size-matched candidates in parallel (rayon)
- Large files (>128KB) use memory-mapped I/O via `memmap2` for hashing and content search
- Search applies filters cheapest-first: name → size → date → content (content search is parallelized with rayon via `par_bridge()`)
- The `organize` command supports dry-run mode and three collision strategies: skip, rename, overwrite. Cross-device moves fall back to copy+delete.
- `search::run_search` takes a `SearchParams` struct to keep the API clean

## Tests

All tests are integration tests in `tests/`:
- `tests/cli_tests.rs` — Spawns the binary, uses `tempfile` for test directories with known file structures
- `tests/mcp_tests.rs` — Sends JSON-RPC messages to the `--mcp` process over stdin, validates JSON responses

Tests reference the binary via `env!("CARGO_BIN_EXE_fiq")`.
