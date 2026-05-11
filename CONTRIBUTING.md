# Contributing to Contextro

Contextro is a local code-intelligence MCP server implemented as a pure Rust workspace. Contributions should preserve the single-binary deployment model and the crate-based architecture under `crates/`.

## Quick Start

```bash
git clone <internal-contextro-repo-url>
cd contextro/crates

# Build
cargo build

# Test
cargo test

# Format and lint
cargo fmt --all
cargo clippy --workspace --all-targets

# Run the server
cargo run -p contextro-server --bin contextro
```

## Project Structure

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

User-facing docs live at the repository root and under `docs/`. The runtime implementation does not live under a Python `src/` package anymore.

## Development Workflow

Run all development commands from `crates/`.

```bash
# Full workspace tests
cargo test

# Single crate
cargo test -p contextro-tools

# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets

# Release build
cargo build --release
```

## Architecture Expectations

- Keep Contextro as a single compiled Rust binary.
- Keep the public entrypoint as the `contextro` binary.
- Prefer focused changes inside existing crates over adding new top-level subsystems.
- Preserve local-first behavior and the current HTTP endpoints: `GET /health` and `POST /mcp`.

## Adding a New Tool

1. Add the tool implementation in `crates/contextro-tools/src/`.
2. Wire dispatch in `crates/contextro-server/src/main.rs`.
3. Add the tool schema in `ContextroServer::tool_definitions()`.
4. Add tests in the owning crate.
5. Update user-facing documentation if the tool surface changes.

## Adding a New Language

1. Extend `crates/contextro-parsing` language support.
2. Update any file-extension or parser registration code there.
3. Add tests covering symbol extraction and indexing behavior.

Python remains a supported source language example, but language support work should not introduce a Python runtime dependency.

## Performance and Quality

- Run `cargo test` before submitting changes.
- Run `cargo fmt --all` and `cargo clippy --workspace --all-targets`.
- Keep memory-sensitive behavior in mind: idle target remains under roughly 50MB per `AGENTS.md`.
- Avoid design changes that reintroduce interpreter-based runtime requirements.

## Submitting a PR

1. Create a topic branch.
2. Make the smallest correct change.
3. Run format, lint, and tests from `crates/`.
4. Summarize the user-visible impact and any benchmark or performance effect.

PR checklist:

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets`
- [ ] `cargo test`
- [ ] Docs updated if public behavior changed

## Reporting Issues

Include:

- Contextro version or commit SHA
- Operating system
- Whether you ran `contextro` via stdio or HTTP transport
- Reproduction steps
- Relevant logs or error output

## License

Proprietary — contributions are accepted under Distillation Labs internal terms.
