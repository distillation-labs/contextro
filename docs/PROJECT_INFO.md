# Contextro Project Information

## Overview

Contextro is a local code-intelligence MCP server shipped as a single compiled Rust binary named `contextro`.

It provides:

- semantic and keyword code search
- call-graph navigation and impact analysis
- semantic memory and session recovery
- git-aware and documentation-oriented tools
- stdio and HTTP transports

The active implementation lives in the Rust workspace under `crates/`.

## Current Architecture Snapshot

```text
crates/
├── contextro-core/
├── contextro-config/
├── contextro-parsing/
├── contextro-indexing/
├── contextro-engines/
├── contextro-memory/
├── contextro-git/
├── contextro-tools/
└── contextro-server/
```

The server entrypoint is the `contextro` binary in `crates/contextro-server`.

## Key Technical Facts

- Pure Rust runtime
- No Python interpreter or virtual environment required
- Local-first storage under `CTX_STORAGE_DIR`
- HTTP mode exposes `GET /health` and `POST /mcp`
- Test and release workflows are Cargo-based from `crates/`

## Workspace Responsibilities

| Crate | Responsibility |
|---|---|
| `contextro-core` | Shared domain models, traits, and errors |
| `contextro-config` | `CTX_` configuration and storage paths |
| `contextro-parsing` | tree-sitter parsing and supported-language detection |
| `contextro-indexing` | File scanning, symbol extraction pipeline, chunk creation |
| `contextro-engines` | BM25, graph, fusion, cache, sandbox |
| `contextro-memory` | Memory store and session/archive state |
| `contextro-git` | Git history and repo helpers |
| `contextro-tools` | User-facing MCP tool logic |
| `contextro-server` | Binary, stdio transport, HTTP transport |

## Tool Surface

Contextro exposes 35 MCP tools, including:

- indexing and search tools such as `index`, `search`, `find_symbol`, and `impact`
- project-analysis tools such as `overview`, `architecture`, and `dead_code`
- session and memory tools such as `remember`, `recall`, `compact`, and `restore`
- git and multi-repo tools such as `commit_search`, `commit_history`, and `repo_status`
- code-aware tools such as `code`, `docs_bundle`, and `sidecar_export`

## Performance Guidance

Per `AGENTS.md`, the intended profile is:

- cold start under roughly 50ms
- warm search latency under roughly 1ms
- indexing around 2 seconds for a 3,000-file project
- idle memory under roughly 50MB

## Supported Languages

Contextro indexes many languages through tree-sitter. Python source files remain a supported target language for indexing and search, but Python is not part of the runtime or installation model.

## Development Commands

Run from `crates/`:

```bash
cargo build
cargo test
cargo fmt --all
cargo clippy --workspace --all-targets
cargo build --release
```

## Running the Server

```bash
# stdio MCP transport
cargo run -p contextro-server --bin contextro

# HTTP transport
CTX_TRANSPORT=http cargo run -p contextro-server --bin contextro
```

## Related Documents

- `README.md` for installation, usage, and tool overview
- `docs/INSTALLATION.md` for binary install and MCP client setup
- `docs/DEVELOPER_GUIDE.md` for Rust workspace workflows
- `docs/ARCHITECTURE.md` for subsystem layout
