# dig2memory

Code intelligence server and CLI for AI agents. Indexes Rust codebases using tree-sitter AST parsing, builds dependency graphs via cargo_metadata, and provides fuzzy symbol search, caller tracking, and impact analysis.

Built for [Claude Code](https://claude.ai/code) agents but works with any AI coding assistant that can run shell commands.

## Features

- **Fuzzy symbol search** — find functions, structs, traits, impls by name (trigram index)
- **Caller tracking** — who calls a given function/method
- **File dependencies** — what a file imports (`use`, `mod`)
- **Impact analysis** — what breaks if a file changes (reverse dependency graph)
- **Crate dependency graph** — workspace-level crate relationships via cargo_metadata
- **Hotspots** — most depended-upon files in the codebase
- **Symbol-to-crate resolution** — which crate owns a symbol

## Architecture

```
dig2memory/
├── dig2memory-core/     # Library: AST parsing, SQLite storage, graph queries
├── dig2memory-server/   # HTTP server (axum, localhost:18200)
├── dig2memory-cli/      # CLI binary (reads SQLite directly, no server needed)
├── skills/              # Claude Code skill template
├── agents/              # Claude Code agent template
└── data/                # SQLite database (gitignored)
```

**Stack**: tree-sitter-rust, rusqlite (bundled), cargo_metadata, clap, axum + tokio (server only)

## Quick Start

### Build

```bash
cargo build --release
```

This produces two binaries:
- `target/release/dig2memory` — CLI (recommended for AI agents)
- `target/release/dig2memory-server` — HTTP server

### Index a workspace

Start the server and trigger indexing:

```bash
# Start server
DIG2MEMORY_DATA_DIR="./data" ./target/release/dig2memory-server &

# Index your workspace
curl -s -X POST http://127.0.0.1:18200/index \
  -H "Content-Type: application/json" \
  -d '{
    "workspace_id": "my-project",
    "workspace_name": "my-project",
    "root_path": "/path/to/your/workspace",
    "force": false
  }'
```

Indexing is incremental — only changed files are re-parsed on subsequent runs.

### CLI Usage

```bash
# Set up shortcuts
DB="./data/index.db"
DM="./target/release/dig2memory"

# Search for a symbol
"$DM" --db "$DB" search "MyStruct"

# Who calls this function?
"$DM" --db "$DB" callers "process_message"

# What depends on this file?
"$DM" --db "$DB" impact "src/core/engine.rs"

# Most important files in the codebase
"$DM" --db "$DB" hotspots --limit 20

# Crate dependency graph
"$DM" --db "$DB" crates

# Which crate owns this symbol?
"$DM" --db "$DB" resolve "Config"

# List symbols in a file
"$DM" --db "$DB" symbols "src/main.rs"

# File dependencies
"$DM" --db "$DB" deps "src/core/mod.rs"

# JSON output (for structured processing)
"$DM" --db "$DB" --json search "MyStruct"
```

### HTTP API

All endpoints return JSON. Default port: 18200.

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Server status + index stats |
| `/index` | POST | Trigger incremental indexing |
| `/ast/search?q=NAME` | GET | Fuzzy symbol search |
| `/ast/symbols?file=PATH&workspace=ID` | GET | Symbols in a file |
| `/ast/callers?sym=NAME&workspace=ID` | GET | Callers of a symbol |
| `/ast/deps?file=PATH&workspace=ID` | GET | File dependencies |
| `/graph/crates?workspace=ID` | GET | Crate dependency graph |
| `/graph/impact?file=PATH&workspace=ID` | GET | Reverse file dependencies |
| `/graph/hotspots?workspace=ID&limit=N` | GET | Most connected files |
| `/graph/resolve?sym=NAME&workspace=ID` | GET | Symbol-to-crate mapping |

## Claude Code Integration

### As a Skill

Copy `skills/dig2memory/SKILL.md` to your project's `.claude/skills/dig2memory/SKILL.md`. Then use `/dig2memory search MyStruct` in Claude Code.

### As an Agent

Copy `agents/refactoring-explorer.md` to your project's `.claude/agents/`. This creates a specialized agent for cross-crate dependency analysis that uses the CLI directly.

### In CLAUDE.md

Add CLI instructions to your project's `CLAUDE.md` so all agents know about dig2memory:

```markdown
## dig2memory (Code Intelligence)

CLI: `dig2memory/target/release/dig2memory --db dig2memory/data/index.db`

Commands: search, symbols, callers, deps, impact, hotspots, crates, resolve
Add --json for structured output.
```

## Performance

Tested on a 250K symbol / 13K file Rust workspace:

| Metric | Value |
|--------|-------|
| Full index | ~4.5 min |
| Incremental re-index | seconds |
| Database size | ~700 MB |
| Symbol search | <50ms |
| Callers query | <100ms |

## Support the Project

If you find this tool useful, consider supporting development:

| Currency | Network | Address |
|----------|---------|---------|
| USDT | TRC20 | `TNxMKsvVLYViQ5X5sgCYmkzH4qjhhh5U7X` |
| USDC | Arbitrum | `0xEF3B94Fe845E21371b4C4C5F2032E1f23A13Aa6e` |
| ETH | Ethereum | `0xEF3B94Fe845E21371b4C4C5F2032E1f23A13Aa6e` |
| BTC | Bitcoin | `bc1qjgzthxja8umt5tvrp5tfcf9zeepmhn0f6mnt40` |
| SOL | Solana | `DZJjmH8Cs5wEafz5Ua86wBBkurSA4xdWXa3LWnBUR94c` |

## License

MIT
