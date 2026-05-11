# Contextro

Unified MCP server: hybrid search + code graph + semantic memory, implemented as a single compiled Rust binary.

## MANDATORY: Use Contextro Tools Before Built-in Tools

Contextro is registered as an MCP server (`contextro`). **You MUST use Contextro tools as your PRIMARY method for understanding and navigating code.** This is a BLOCKING REQUIREMENT — do not skip it.

### Required workflow for EVERY code task

1. **Start of session**: Run `mcp__contextro__status` to check index state. If not indexed, run `mcp__contextro__index`.
2. **Before reading any file**: Use `mcp__contextro__search` to find relevant code first. Do NOT jump straight to Read/Grep/Glob.
3. **Before exploring a symbol**: Use `mcp__contextro__find_symbol`, `mcp__contextro__explain`, `mcp__contextro__find_callers`, or `mcp__contextro__find_callees` instead of grepping for it.
4. **Before refactoring**: Use `mcp__contextro__impact` to assess blast radius.
5. **For project understanding**: Use `mcp__contextro__overview` or `mcp__contextro__architecture` instead of manually browsing directories.
6. **Only use Read/Grep/Glob AFTER** Contextro tools have identified the specific files/lines you need to examine or edit.

**Why this matters**: Contextro provides semantic search, call graph analysis, and code intelligence that built-in tools cannot. Using Read/Grep first means you miss semantic matches, don't understand call relationships, and waste time manually browsing.

### Workflow: Always index first

Before using any search/graph/analysis tools, index the codebase:

```
# Single directory
mcp__contextro__index(path="/path/to/codebase")
```

For subsequent sessions or after file changes, use incremental mode (auto-detected).

### When to use each tool

| Tool | When to use |
|------|-------------|
| `index` | **First thing** when working with a new or changed codebase. Re-run after significant file changes. |
| `status` | Check if a codebase is indexed, how many symbols/chunks exist, memory usage. |
| `health` | Verify the server and all engines are running before starting work. |
| `search` | **Primary tool** for finding relevant code. Use for any "where is...", "how does...", "find..." query. Supports `mode=hybrid` (default), `vector`, or `bm25`. Use language/type filters to narrow results. |
| `find_symbol` | Look up a specific symbol by name. Use `exact=False` for fuzzy matching when unsure of the name. |
| `find_callers` | Understand who calls a function — use before refactoring to assess blast radius. |
| `find_callees` | Understand what a function depends on — use to trace execution flow. |
| `analyze` | Get code quality metrics: complexity, code smells, dependencies, maintainability. Use when reviewing or improving code. |
| `impact` | **Before making changes**: assess transitive impact of modifying a symbol. Shows all affected symbols and files. |
| `explain` | Get a combined graph + vector + analysis explanation of any symbol. Use for onboarding to unfamiliar code. |
| `overview` | Get a high-level project overview: file counts, languages, symbol types, directory structure, quality metrics, top modules. Use when starting work on an unfamiliar project. |
| `architecture` | Document the project architecture: layers, module dependencies, class hierarchies, entry points, hub symbols, complexity hotspots. Use for understanding system design. |
| `remember` | Store project context, decisions, or notes as semantic memories with tags. Use to persist context across conversations. |
| `recall` | Retrieve stored memories by semantic similarity. Check for existing context before starting new work. |
| `forget` | Clean up outdated memories by ID, tags, or type. |

### Best practices

- **NEVER use Grep/Glob/Read as your first action for code exploration**: Always start with `search`, `find_symbol`, or `overview`. Only fall back to built-in tools for reading specific files identified by Contextro, or for files outside the indexed codebase.
- **Search before reading files**: Use `search` to find relevant code instead of manually browsing. It's faster and finds semantic matches.
- **Use `impact` before refactoring**: Always check change impact before modifying shared symbols.
- **Use `explain` for unfamiliar code**: Combines graph relationships, related code, and quality metrics in one call.
- **Store decisions with `remember`**: When making architectural decisions or noting important context, store it so future sessions have access.
- **Check `recall` at session start**: Query memories for existing project context before asking the user to repeat information.
- **Use `find_callers`/`find_callees` for dependency tracing**: These are more reliable than text search for understanding call graphs.
- **Use `overview` when starting a new project**: Get a quick summary of project structure, languages, and quality before diving in.
- **Use `architecture` for design understanding**: See layers, dependencies, entry points, and hub symbols to understand system design.
- **Use `analyze` for code reviews**: Get objective quality metrics to guide review feedback.
- **Re-index incrementally after changes**: Run `index` again after making significant edits — incremental mode only processes changed files.

## Structure (current)

```text
crates/
├── contextro-core/        # Domain types, graph models, shared traits
├── contextro-config/      # CTX_ configuration and defaults
├── contextro-parsing/     # tree-sitter parsing and language support
├── contextro-indexing/    # File scanner, indexing pipeline, chunking
├── contextro-engines/     # BM25, graph, fusion, cache, sandbox
├── contextro-memory/      # Memory store, archive, session tracking
├── contextro-git/         # Commit history and repo helpers
├── contextro-tools/       # MCP tool implementations
└── contextro-server/      # `contextro` binary, stdio and HTTP transports
```

## Commands

```bash
cd crates
cargo build
cargo test
cargo fmt --all
cargo clippy --workspace --all-targets
cargo run -p contextro-server --bin contextro
claude mcp add contextro -- contextro
```

## Key Decisions

- Single compiled Rust binary, no interpreter or Python runtime
- Workspace split across focused crates under `crates/`
- tree-sitter-based parsing for many languages, including Python source files
- HTTP transport exposes `GET /health` and `POST /mcp`
- Testing and release flows are Cargo-based from the Rust workspace

## Gotchas

1. The runtime entrypoint is the `contextro` binary in `crates/contextro-server`
2. The active workspace is `crates/`, not a Python `src/` tree
3. `status` and `health` work without an indexed codebase; most other tools require `index` first
4. HTTP mode requires `CTX_TRANSPORT=http` and serves `/health` and `/mcp`
5. Python is a supported indexed language, not an installation prerequisite

## Embedding Models

| Model | Key | Notes |
|---|---|---|
| Potion Code 16M | `potion-code-16m` | Default local embedding model |

### Changing the embedding model

Set the `CTX_EMBEDDING_MODEL` environment variable:

```bash
# Example
CTX_EMBEDDING_MODEL=potion-code-16m contextro

# Or set in your shell profile
export CTX_EMBEDDING_MODEL=potion-code-16m

# For Claude Code MCP config
claude mcp add contextro -e CTX_EMBEDDING_MODEL=potion-code-16m -- contextro
```

After changing the embedding model, re-index the codebase so stored vectors match the active model.
