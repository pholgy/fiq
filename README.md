# fiq

Fast file intelligence CLI and MCP server, written in Rust.

Scan directories, find duplicates, search files, and organize them into folders. Works standalone or as a tool server for AI assistants via the [Model Context Protocol](https://modelcontextprotocol.io/).

## Install

**macOS / Linux:**

```bash
curl -fsSL https://raw.githubusercontent.com/pholgy/fiq/main/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/pholgy/fiq/main/install.ps1 | iex
```

**From source** (requires Rust 1.85+):

```bash
cargo install --git https://github.com/pholgy/fiq.git
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

fiq runs as a JSON-RPC 2.0 server over stdio, exposing five tools: `scan_stats`, `find_duplicates`, `search_files`, `organize_files`, and `build_index`.

### Claude Code

```bash
claude mcp add fiq -- fiq --mcp
```

### Claude Desktop

Add to your config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

```json
{
  "mcpServers": {
    "fiq": {
      "command": "fiq",
      "args": ["--mcp"]
    }
  }
}
```

### Other MCP clients

Any client that supports stdio transport works:

```bash
fiq --mcp
```

## Configuration

### FIQ_THREADS

Controls the number of threads for directory walking. Defaults to 4, which balances performance and CPU usage for I/O-bound workloads.

```bash
FIQ_THREADS=8 fiq stats ~/projects   # more threads for faster scans
FIQ_THREADS=2 fiq stats ~/projects   # fewer threads to reduce CPU usage
```

## Performance

- Persistent trigram index for name searches — first query builds the index, every query after is instant
- jemalloc allocator for better multi-threaded performance
- Parallel directory walking via `ignore` crate's work-stealing thread pool
- Two-phase duplicate detection: size grouping then parallel blake3 hashing
- Memory-mapped I/O for files >128KB (hashing and content search)
- Parallel content search via rayon

### Benchmarks

Tested on macOS (Apple Silicon), ~1.9 million files in `$HOME`. All tools configured to skip gitignore/hidden filtering for a fair comparison.

**Name search: `*.rs` (19,191 matches)**

| Tool | Time | Notes |
|------|------|-------|
| fd | 11.0s | full directory walk every time |
| ripgrep `--files` | 12.8s | full directory walk every time |
| fiq (1st run) | 15.6s | full walk + builds trigram index |
| fiq (2nd run) | **0.29s** | reads cached index, skips walk entirely |

**Name search: `*.test.js` (720 matches, same cached index)**

| Tool | Time |
|------|------|
| fd | ~10s |
| ripgrep | ~10s |
| fiq | **0.27s** |

**Full walk, no index possible: `*.c` (pattern too short for trigrams)**

| Tool | Time |
|------|------|
| fd | 9.9s |
| ripgrep | 10.8s |
| fiq | 14.7s |

**Bottom line:** fiq's raw walk speed is ~50% slower than fd. But the trigram index makes repeated name searches 30-40x faster than anything that walks the filesystem every time. The index is cached on disk (`~/Library/Caches/fiq/` on macOS) and kept in memory during MCP sessions, so the cost is paid once.

### Trigram Index

fiq builds a trigram index over file names the first time you search a directory with a name pattern that has 3+ literal characters (e.g. `*.rs`, `*.test.js`, `foo*bar`). Patterns that are too short for trigrams (e.g. `*.c`, `*`) fall back to a full directory walk.

The index is:
- **Cached on disk** — persists across CLI invocations (1-hour TTL)
- **Cached in memory** — stays alive during an MCP session for sub-second queries
- **Rebuildable** — use the `build_index` MCP tool or just delete `~/Library/Caches/fiq/`

## License

MIT
