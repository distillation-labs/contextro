# Release Candidate Testing

Use the local release binary as the thing you test. Do **not** use npm publish as the first time the MCP is exercised like a real product.

If your agent supports repo-local skills in this repository, load
`contextro-release-tester` to drive the same gate sequence consistently.

## One command to scaffold a local RC

From the repo root:

```bash
./scripts/release-candidate.sh --install-claude
```

That script:

1. Builds the Rust release binaries from `crates/`
2. Creates an isolated RC workspace under `scratch/release-candidate/`
3. Generates a wrapper script that pins `CTX_STORAGE_DIR` for restart/persistence testing
4. Generates a Claude helper script to register the local RC as an MCP entry
5. Generates a developer-gate checklist and a repo-list template

The wrapper is the important bit. It ensures your local Claude/agent client talks to the **local release binary** with a stable storage directory, instead of a published package or an ephemeral process state.

## External repo matrix

Create a repo list in `scratch/release-candidate/repos.txt`:

```text
/absolute/path/to/python-pytest-repo
/absolute/path/to/typescript-monorepo
/absolute/path/to/rust-repo
/absolute/path/to/messy-mixed-repo
```

Then run:

```bash
./scripts/release-candidate.sh --repos-file scratch/release-candidate/repos.txt
```

This keeps the local RC assets in place and also runs `contextro-study` against those repos, storing results under `scratch/release-candidate/study/`.

## Recommended release gates

### 1. Engine gate

```bash
cd crates
cargo test --quiet
./target/release/contextro-bench /path/to/repo
```

This checks the parser/index/search core. It is **not** enough by itself.

### 2. Real MCP gate

Use the generated wrapper with a real MCP client (for example Claude Code) and test the local RC binary directly.

### 3. Developer gate

For each external repo in the matrix, verify:

1. `index(path)` works
2. wrong-path calls return errors, not empty success payloads:
   - `focus(path=...)`
   - `analyze(path=...)`
   - `code(operation="get_document_symbols", path=...)`
3. restart-sensitive flows survive a restart:
   - `compact(...)` -> restart -> `retrieve(...)`
   - `repo_add(...)` -> restart -> `repo_status()`
4. `search_codebase_map(...)` returns useful results
5. search ranking looks sane for real code
6. `dead_code()` skips pytest fixtures on Python repos

If any tool returns a silent empty result, loses state across restart, or gives misleading output, block the release.

### 4. Packaging gate

Only after the local RC passes the developer gate should you do an npm package smoke test.

## Why this workflow exists

The Rust rewrite made the engine fast, but real failures showed up in the MCP product layer:

- stdio restart persistence
- path normalization across unrelated repos
- inconsistent parameter contracts
- output truthfulness vs happy-path correctness

Testing the local release binary through a real client catches those problems before you ship them.
