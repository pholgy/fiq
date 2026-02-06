# fiq

Fast file intelligence CLI and MCP server, written in Rust.

Scan directories, find duplicates, search files, and organize them into folders. Works standalone or as a tool server for AI assistants via the [Model Context Protocol](https://modelcontextprotocol.io/).

## Install

Requires Rust 1.85+ (edition 2024).

```bash
cargo build --release
# binary at target/release/fiq
```

## Usage

### stats

Show file counts, total size, breakdown by extension, and largest files.

```bash
fiq stats ~/projects
fiq stats ~/projects --top 20
```

### duplicates

Find duplicate files using blake3 content hashing. Groups by file size first, then hashes only candidates that share a size.

```bash
fiq duplicates ~/Downloads
fiq duplicates ~/Downloads --min-size 1048576   # only files >= 1MB
```

### search

Search by name glob, content string, size range, or date range. Filters apply cheapest-first (name, size, date, then content).

```bash
fiq search ~/projects --name "*.rs"
fiq search ~/projects --content "TODO" --newer 7d
fiq search ~/projects --min-size 1MB --max-size 100MB
fiq search ~/projects --newer 2024-01-01 --older 2024-06-01
```

Size values: `1KB`, `10MB`, `1GB`, or plain bytes.
Time values: `7d`, `24h`, `30m`, or dates like `2024-01-01`.

### organize

Sort files into folders by type, date, or size. Supports dry-run preview and three collision modes.

```bash
fiq organize ~/Downloads --by type --dry-run     # preview first
fiq organize ~/Downloads --by type                # move files
fiq organize ~/Downloads --by date --output ~/sorted
fiq organize ~/Downloads --by size --mode skip    # skip collisions
```

Strategies: `type` (by extension), `date` (by year/month), `size` (small/medium/large).
Collision modes: `rename` (default), `skip`, `overwrite`.

## MCP Server

Run as a JSON-RPC 2.0 server over stdio for AI tool use:

```bash
fiq --mcp
```

Exposes four tools: `scan_stats`, `find_duplicates`, `search_files`, `organize_files`.

Add to your MCP client config (e.g. Claude Desktop):

```json
{
  "mcpServers": {
    "fiq": {
      "command": "/path/to/fiq",
      "args": ["--mcp"]
    }
  }
}
```

## Configuration

### FIQ_THREADS

Controls the number of threads for directory walking. Defaults to 4, which balances performance and CPU usage for I/O-bound workloads.

```bash
FIQ_THREADS=8 fiq stats ~/projects   # more threads for faster scans
FIQ_THREADS=2 fiq stats ~/projects   # fewer threads to reduce CPU usage
```

## Performance

- Parallel directory walking via `ignore` crate's work-stealing thread pool
- Two-phase duplicate detection: size grouping then parallel blake3 hashing
- Memory-mapped I/O for files >128KB (hashing and content search)
- Parallel content search via rayon
- Respects `.gitignore` rules (skips ignored files automatically)

## License

MIT
