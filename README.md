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

All benchmarks on macOS (Apple Silicon), ~1.9 million files in `$HOME`, warm filesystem cache. fd 10.3, ripgrep 14.1, fiq 0.1.0. All tools configured with `--no-ignore --hidden` / equivalent for a fair comparison.

### Name search

fiq builds a trigram index on first run, then uses it for all subsequent queries. fd and ripgrep walk the entire directory tree every time.

| Pattern | Matches | fd | ripgrep | fiq (indexed) | fiq speedup |
|---------|--------:|---:|--------:|--------------:|------------:|
| `*.rs` | 19,191 | 14.07s | 10.90s | **0.49s** | 22-29x |
| `*.test.js` | 713 | 11.79s | 12.02s | **0.40s** | 29-30x |
| `*.json` | 70,737 | 10.20s | 12.64s | **0.45s** | 23-28x |
| `*.py` | 105,891 | 11.14s | 11.09s | **0.65s** | 17x |
| `*.tsx` | 603 | 11.22s | 16.93s | **0.44s** | 26-38x |
| `*.c` | 1,480 | 10.52s | 12.57s | 11.71s | no index* |

*`*.c` has only 2 literal characters — not enough for a trigram. Falls back to full directory walk, where fiq is ~15% slower than fd.

### Where fiq is slower

Being straight: fiq's raw directory walk (no index) is slower than both fd and ripgrep.

| Operation | fd | ripgrep | fiq |
|-----------|---:|--------:|----:|
| Full walk, name filter `*.c` | **10.5s** | 12.6s | 11.7s |
| Count all files in `$HOME` | **10.7s** | — | 20.5s |
| Content search `TODO` (project dir) | — | **0.30s** | 1.45s |

fiq's full walk is ~15% slower than fd. For stats (`fiq stats`) it's ~2x slower because it collects metadata (size, dates, extensions) for every file. ripgrep is significantly faster than fiq for content search — it has years of SIMD-optimized string matching that fiq doesn't try to replicate.

### Where fiq wins

The trigram index changes the game for repeated name searches. This is the typical MCP server scenario — an AI assistant searching the same codebase many times in one session.

**5 sequential name searches across 1.9M files:**

| Tool | `*.rs` | `*.test.js` | `*.json` | `*.tsx` | `*.toml` | Total |
|------|-------:|------------:|---------:|--------:|---------:|------:|
| fd | 15.76s | 14.54s | 12.59s | 17.30s | 8.52s | **68.95s** |
| fiq | 0.32s | 0.30s | 0.43s | 0.28s | 0.29s | **1.91s** |

**36x faster total.** Each fd/ripgrep query re-walks the entire filesystem. fiq looks up a cached index and returns in under half a second.

### First run cost

The first search for a directory builds the index. This is slower than a plain walk because it also constructs and saves the trigram data:

| | Time |
|--|-----:|
| fiq first search (walk + build index) | ~16s |
| fiq second search (cached index) | ~0.3s |
| fd (every search) | ~11s |

You pay once, then every subsequent query is instant.

### How the trigram index works

fiq decomposes file names into 3-byte substrings (trigrams). Given `*.test.js`, it extracts the literal `.test.js` and generates trigrams like `.te`, `tes`, `est`, `st.`, `t.j`, `.js`. At query time, it intersects the posting lists for each trigram and verifies candidates against the full glob.

Patterns need at least 3 consecutive literal characters to use the index. These work: `*.rs`, `*.test.js`, `foo*bar`. These fall back to a full walk: `*.c` (2 chars), `*` (no literals).

The index is:
- **Cached on disk** — persists across CLI invocations (1-hour TTL, stored in `~/Library/Caches/fiq/` on macOS)
- **Cached in memory** — stays alive during an MCP server session for sub-second queries
- **Rebuildable** — use the `build_index` MCP tool or delete the cache directory

### What makes fiq fast (and not fast)

**Fast:**
- jemalloc allocator (better multi-threaded allocation than system malloc)
- Trigram index eliminates filesystem walks for repeated name searches
- Skip metadata syscalls when only file names are needed
- Parallel blake3 hashing with memory-mapped I/O for duplicate detection

**Not fast (compared to fd/ripgrep):**
- Raw directory walking — fd has more walker optimizations
- Content search — ripgrep has SIMD-accelerated string matching
- Stats collection — fiq stats every file for size/date/extension breakdown

## License

MIT
