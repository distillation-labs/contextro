# Developer Guide

## Development Setup

```bash
git clone <internal-contextro-repo-url>
cd contextro/crates

# Verify toolchain
cargo --version

# Build the workspace
cargo build

# Run tests
cargo test

# Format and lint
cargo fmt --all
cargo clippy --workspace --all-targets

# Run the MCP server over stdio
cargo run -p contextro-server --bin contextro

# Optional HTTP smoke test
CTX_TRANSPORT=http cargo run -p contextro-server --bin contextro
curl http://127.0.0.1:8000/health
```

## Workspace Structure

The real implementation lives under `crates/`.

```text
crates/
├── Cargo.toml              # Workspace manifest
├── contextro-core/         # Domain types, traits, errors
├── contextro-config/       # CTX_ environment configuration
├── contextro-parsing/      # tree-sitter parsing and language support
├── contextro-indexing/     # File scanning, indexing pipeline, chunking
├── contextro-engines/      # BM25, graph, fusion, cache, sandbox
├── contextro-memory/       # Memory store, archive, session tracking
├── contextro-git/          # Git indexing and repo registry helpers
├── contextro-tools/        # Tool handlers and user-facing tool docs
└── contextro-server/       # `contextro` binary and HTTP transport
```

## Core Commands

Run these from `contextro/crates`.

```bash
# Build everything
cargo build

# Fast development cycle
cargo test

# One crate
cargo test -p contextro-indexing

# Production build
cargo build --release

# Run the server binary
cargo run -p contextro-server --bin contextro

# Run the benchmark binary
cargo run -p contextro-server --bin contextro-bench
```

## Project Structure Notes

- `contextro-server` owns transport selection and exposes the `contextro` binary.
- `contextro-server/src/http.rs` serves `GET /health` and `POST /mcp` for HTTP deployments.
- `contextro-config` is the source of truth for `CTX_` settings and defaults.
- `contextro-tools` defines the user-facing tool surface.
- `contextro-indexing` builds searchable chunks from parsed symbols.
- `contextro-parsing` supports many target languages, including Python source files, via tree-sitter.

## Testing

The project uses Cargo-based testing from the Rust workspace.

```bash
# All tests
cargo test

# Single crate
cargo test -p contextro-tools

# Integration tests in the indexing crate
cargo test -p contextro-indexing --test bench_index
```

Current repository guidance from `AGENTS.md`:

- Run `cargo test` from `crates/`
- Unit tests live within crates
- Integration tests live in `contextro-indexing/tests/`
- Use `cargo build --release` for production binaries

## Formatting and Linting

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
```

## Running the Server

Stdio transport for MCP clients:

```bash
cargo run -p contextro-server --bin contextro
```

HTTP transport for container or team deployments:

```bash
CTX_TRANSPORT=http CTX_HTTP_HOST=0.0.0.0 CTX_HTTP_PORT=8000 \
  cargo run -p contextro-server --bin contextro
```

HTTP endpoints:

- `GET /health`
- `POST /mcp`

## Adding a New MCP Tool

1. Add the handler in `crates/contextro-server/src/main.rs`.
2. Implement the tool logic in the appropriate module under `crates/contextro-tools/src/`.
3. Add the tool definition to `ContextroServer::tool_definitions()`.
4. Add or update tests in the relevant crate.
5. Update user-facing docs such as `README.md` if the tool surface changes.

Prefer extending the existing crates instead of creating a new abstraction layer unless the logic clearly warrants it.

## Adding a New Engine or Subsystem

1. Add the implementation in the relevant crate under `crates/contextro-engines/`, `crates/contextro-indexing/`, or another focused crate.
2. Wire it into `crates/contextro-server/src/state.rs` or the indexing pipeline as needed.
3. Keep public types in `contextro-core` when they are shared across crates.
4. Add tests at the crate boundary that owns the behavior.

## Architecture Decision Records

Significant design decisions belong in `docs/adr/`.

1. Copy `docs/adr/ADR-000-template.md`
2. Number it sequentially
3. Document context, decision, alternatives, and consequences
4. Update high-level docs if the public architecture changed

## Debugging

```bash
CTX_LOG_LEVEL=DEBUG cargo run -p contextro-server --bin contextro
```

Useful checks:

- `cargo test -p <crate>` for a focused failure loop
- `curl http://127.0.0.1:8000/health` when running HTTP transport
- `contextro` `status` and `health` tools once the server is connected to a client

## Release Process

1. Update the workspace version in `crates/Cargo.toml` if needed.
2. Run `cargo fmt --all`.
3. Run `cargo clippy --workspace --all-targets`.
4. Run `cargo test`.
5. Build `cargo build --release`.
6. Validate the release binary exists and runs: `test -x ./target/release/contextro` and, for HTTP, `CTX_TRANSPORT=http ./target/release/contextro`.
