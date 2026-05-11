# Architecture

Contextro is a pure Rust MCP server built as a single compiled binary. It combines semantic retrieval, BM25 search, call-graph analysis, git-aware tools, semantic memory, and session recovery without a Python runtime.

## System Overview

```text
┌────────────────────────────────────────────────────┐
│                   MCP Client                       │
└──────────────────────┬─────────────────────────────┘
                       │ stdio or HTTP
┌──────────────────────┴─────────────────────────────┐
│             `contextro` Rust binary               │
│         (`crates/contextro-server`)               │
├────────────────────────────────────────────────────┤
│ Tool dispatch, transport, server state            │
├────────────────────────────────────────────────────┤
│ Search tools │ Graph tools │ Memory │ Git │ Code │
├────────────────────────────────────────────────────┤
│ BM25 │ Graph │ Fusion │ Cache │ Sandbox │ Memory │
├────────────────────────────────────────────────────┤
│ File scanning → tree-sitter parsing → chunking    │
├────────────────────────────────────────────────────┤
│ Local storage under `CTX_STORAGE_DIR`             │
└────────────────────────────────────────────────────┘
```

## Workspace Layout

```text
crates/
├── contextro-core/       # Shared models, graph/domain types, traits, errors
├── contextro-config/     # CTX_ config parsing and storage paths
├── contextro-parsing/    # tree-sitter parsing and language support
├── contextro-indexing/   # File scanner, pipeline, chunker
├── contextro-engines/    # BM25, graph, search fusion, cache, sandbox
├── contextro-memory/     # Memory store, archive, session tracker
├── contextro-git/        # Commit-history and repo helpers
├── contextro-tools/      # User-facing MCP tool implementations
└── contextro-server/     # Binary, stdio transport, HTTP transport
```

## Main Components

### `contextro-server`

- Defines the `contextro` binary.
- Selects stdio or HTTP transport from `CTX_TRANSPORT`.
- Exposes `GET /health` and `POST /mcp` in HTTP mode.
- Dispatches tool calls to handlers in `contextro-tools`.

### `contextro-config`

- Holds runtime defaults and `CTX_` environment parsing.
- Defines storage paths for the local data directory.

### `contextro-indexing`

- Scans files from the indexed repository.
- Parses supported source files with tree-sitter.
- Produces symbol-derived chunks for retrieval.

### `contextro-engines`

- BM25 search via Tantivy-backed indexing.
- In-memory code graph for callers, callees, and impact analysis.
- Result fusion, caching, and large-output sandboxing.

### `contextro-memory`

- Local memory store for `remember`, `recall`, and `forget`.
- Session archive and snapshot helpers.

### `contextro-tools`

- Implements the 35-tool user-facing MCP surface.
- Includes search, graph, analysis, git, session, and artifact tools.

## Data Flow

### Indexing

```text
Repository files
  -> file scanning
  -> tree-sitter symbol extraction
  -> chunk creation
  -> BM25 indexing
  -> graph construction
```

### Querying

```text
Tool call
  -> tool handler
  -> graph / BM25 / memory / git subsystem
  -> optional fusion, caching, sandboxing
  -> structured MCP response
```

## Runtime Model

- Single local process
- Single compiled Rust binary
- No Python interpreter, `venv`, `pip`, PyO3, or `maturin` required at runtime
- Local storage under `CTX_STORAGE_DIR`
- Default transport is stdio; HTTP is opt-in

## Language Support

Contextro parses many source languages using tree-sitter. Python is one of the supported indexed languages, alongside Rust and other common languages, but it is not part of the server runtime.

## Build and Test Model

All development workflows run from the Rust workspace in `crates/`:

```bash
cargo build
cargo test
cargo build --release
```
