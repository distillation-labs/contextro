---
name: contextro-release-tester
description: >
  Use for pre-release Contextro MCP validation: run the local release-candidate
  workflow, exercise every public tool, verify restart persistence and external-repo
  behavior, and block a release on regressions. Trigger when the user asks to test
  the MCP before a tag or publish, run a release candidate, validate tool contracts,
  or confirm that a release only contains improvements. Do not use for implementing
  fixes, generic benchmarking without a release decision, or unrelated code changes.
when_to_use: >
  Especially useful after changing MCP tools, search or ranking, persistence,
  parameter schemas, packaging, or release workflows, and before tags or npm
  publishes.
metadata:
  version: "0.1.0"
  category: release-qa
  tags: [contextro, mcp, release, qa, regression, testing, external-repos]
license: Proprietary
---

# Contextro Release Tester

Test the local release candidate like a real developer would use it. Speed is a
bonus, not proof. A release passes only when tool behavior is at least as good as
the previous public version and no high-risk workflow regresses.

## Core Rules

- Test the **local release binary first**, not the published npm package.
- Run the release candidate through a **real MCP client**, not only unit tests or
  direct function calls.
- Exercise **every public tool** at least once, but front-load the high-risk
  surfaces: path validation, persistence across restart, search ranking, output
  truthfulness, and parameter compatibility.
- Use **external repos with different shapes**: Python/pytest, TypeScript or
  JavaScript monorepo, Rust, and one messy or mixed-language repo.
- Block the release on any **silent empty success-shaped response**, misleading
  summary, lost persisted state, or behavior that gets worse than the previous
  public version.
- Treat benchmark wins as meaningless if developer workflows regress.

## Use It For

- Pre-tag or pre-publish release validation
- RC testing after MCP tool, schema, search, memory, git, or packaging changes
- Running a regression pass against external repos
- Producing a go/block release decision with evidence
- Rechecking `evaluation.md` findings before another release

## Do Not Use It For

- Implementing the fixes themselves
- One-off performance experiments without a ship/no-ship decision
- Normal feature work or architecture exploration
- Replacing the regular Rust test suite

## Mandatory Workflow

### 1. Build the local RC and freeze the test surface

```text
1. ./scripts/release-candidate.sh --skip-study
2. Use scratch/release-candidate/contextro-rc.sh as the binary under test
3. Use the fixed CTX_STORAGE_DIR created by the RC workflow
4. Register the wrapper in a real MCP client before testing
```

### 2. Clear the release gate in order

```text
1. Engine gate: cargo test --quiet, contextro-bench on at least one repo
2. Real MCP gate: connect the wrapper through a real client
3. Developer gate: external-repo matrix + restart/persistence checks
4. Packaging gate: npm pack or publish flow only after all above pass
```

### 3. External-repo matrix

Minimum matrix:

- one Python repo with pytest fixtures
- one TypeScript or JavaScript monorepo
- one Rust repo
- one repo with imperfect layout, mixed languages, or messy paths

### 4. High-risk regression checks first

| Risk | What to prove |
|---|---|
| silent failures | wrong paths return explicit errors |
| persistence | `compact`/`retrieve`, `session_snapshot`, and `repo_add` survive restart |
| search trust | rankings stay meaningful and not flattened |
| output truth | `overview`, `explain`, `test_coverage_map`, and `dead_code` stay honest |
| parameter compatibility | canonical params work and legacy aliases do not regress |
| cross-repo behavior | path normalization works on external repos |

### 5. Tool coverage

Use the matrix in `references/tool-regression-matrix.md`. Do not claim the release
is clean unless every public tool or tool-family path has been exercised.

## Release-blocking failures

- Any tool returns empty or near-empty success output for invalid input
- Any persisted state disappears after a restart
- Any tool description or schema mismatch causes obvious client misuse
- A ranking or reporting tool is fast but misleading
- A new change makes a previously good workflow worse without a very explicit,
  intentional reason

## Output Format

Return results in this order:

1. `Release target`
2. `Repos tested`
3. `Gate status`
4. `Tool matrix coverage`
5. `Regressions found`
6. `Release decision`
7. `Next fixes`

## Anti-Patterns

- shipping because `cargo test` passed
- using npm publish as the first real-world test
- testing only the repo under development
- treating a benchmark number as proof that the MCP UX is good
- skipping restart tests after memory, repo, or archive changes
- checking only the "big" tools and not the lower-frequency admin or support tools
- marking a release good when the output is technically non-empty but still misleading

## References

- `references/release-gate-patterns.md`
- `references/tool-regression-matrix.md`
