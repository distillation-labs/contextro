# Contextro Installation Guide

## Prerequisites

- Rust toolchain with `cargo`
- A built `contextro` binary on your `PATH`

Install Rust with `rustup` if needed:

```bash
curl https://sh.rustup.rs -sSf | sh
```

## Install the Binary

Prebuilt installer:

```bash
curl -fsSL https://install.contextro.dev | sh
```

Or build/install from source:

```bash
git clone <internal-contextro-repo-url>
cd contextro/crates
cargo install --path contextro-server
```

Contextro is a single compiled Rust binary. There is no Python runtime, virtual environment, or interpreter requirement.

## Build From Source for Development

```bash
git clone <internal-contextro-repo-url>
cd contextro/crates

# Debug build
cargo build

# Release build
cargo build --release
```

The binary is named `contextro` and lives at `crates/target/debug/contextro` or `crates/target/release/contextro`.

## Verify Installation

```bash
# Check the binary is available
command -v contextro

# Or run the workspace-built binary directly
test -x ./target/debug/contextro

# Start HTTP transport and verify the health endpoint
CTX_TRANSPORT=http ./target/debug/contextro
curl http://127.0.0.1:8000/health
```

Expected `/health` response includes `status: healthy`.

## AI Tool Integrations

### Claude Code

```bash
claude mcp add contextro -- contextro
```

With environment variables:

```bash
claude mcp add contextro -e CTX_EMBEDDING_MODEL=potion-code-16m -- contextro
```

Manual config example:

```json
{
  "mcpServers": {
    "contextro": {
      "command": "contextro"
    }
  }
}
```

### Claude Desktop

| Platform | Config file |
|---|---|
| macOS | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| Windows | `%APPDATA%\\Claude\\claude_desktop_config.json` |
| Linux | `~/.config/Claude/claude_desktop_config.json` |

```json
{
  "mcpServers": {
    "contextro": {
      "command": "contextro"
    }
  }
}
```

### Cursor / Windsurf / Generic MCP Clients

```json
{
  "contextro": {
    "command": "contextro",
    "transport": "stdio"
  }
}
```

### HTTP Deployments

Use HTTP transport when you want to run Contextro as a local or containerized service:

```bash
CTX_TRANSPORT=http CTX_HTTP_HOST=0.0.0.0 CTX_HTTP_PORT=8000 contextro
```

Endpoints:

- `GET /health`
- `POST /mcp`

## Configuration

All runtime settings use the `CTX_` prefix.

| Variable | Default | Description |
|---|---|---|
| `CTX_STORAGE_DIR` | `~/.contextro` | Base storage directory |
| `CTX_EMBEDDING_MODEL` | `potion-code-16m` | Embedding model |
| `CTX_EMBEDDING_DEVICE` | `auto` | `auto`, `cpu`, or other supported device selector |
| `CTX_EMBEDDING_BATCH_SIZE` | `512` | Embedding batch size |
| `CTX_MAX_FILE_SIZE_MB` | `10` | Skip files larger than this |
| `CTX_CHUNK_MAX_CHARS` | `4000` | Max characters per chunk |
| `CTX_SEARCH_MODE` | `hybrid` | `hybrid`, `vector`, or `bm25` |
| `CTX_MAX_MEMORY_MB` | `350` | Memory budget target |
| `CTX_LOG_LEVEL` | `INFO` | Logging level |
| `CTX_PERMISSION_LEVEL` | `full` | Default permission mode |
| `CTX_TRANSPORT` | `stdio` | `stdio` or `http` |
| `CTX_HTTP_HOST` | `0.0.0.0` | HTTP bind host |
| `CTX_HTTP_PORT` | `8000` | HTTP bind port |

Example:

```bash
CTX_LOG_LEVEL=DEBUG CTX_SEARCH_MODE=vector contextro
```

## Troubleshooting

### `contextro: command not found`

Ensure the installed binary location is on your `PATH`, or run the workspace-built binary directly from `crates/target/debug/contextro` or `crates/target/release/contextro`.

### HTTP server does not start

Set `CTX_TRANSPORT=http`. The default transport is stdio for MCP clients.

### `/health` is unreachable

Check `CTX_HTTP_HOST` and `CTX_HTTP_PORT`, then retry `curl http://127.0.0.1:8000/health`.

### High memory usage during indexing

Contextro loads models lazily and keeps the idle footprint low, but peak indexing memory can still be higher than idle memory. Tune `CTX_MAX_MEMORY_MB` if needed.

### Python source is not being indexed

Python remains a supported target language for parsing and search. It is not a runtime dependency for installing or running Contextro.
