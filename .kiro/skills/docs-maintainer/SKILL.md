---
name: docs-maintainer
description: Use for writing and maintaining product, benchmark, release, and skill documentation so Contextro's docs stay aligned with the real Rust workspace, public tool contracts, and retained benchmark evidence.
when_to_use: Especially useful for README, release notes, benchmark claims, installation and usage docs, skill docs, and cross-doc sync after behavior or contract changes.
metadata:
  version: 0.2.0
  category: documentation
  tags:
    - documentation
    - benchmark-claims
    - release-notes
    - readme
    - skills
    - contextro
---

# Docs Maintainer

Treat documentation as part of the product surface. For Contextro, stale docs are not cosmetic;
they create wrong tool choices, wrong benchmark expectations, and wrong release decisions.

## Use It To Produce

- accurate README and installation guidance
- benchmark and study claims that match retained evidence
- release notes and RC docs that match the actual workflow
- skill docs that reflect the current repo and tool contracts
- cross-doc sync after a contract, command, or architecture change

## Canonical Sources To Check First

Use code and retained benchmark evidence as the system of record:

- `README.md`
- `CHANGELOG.md`
- `docs/RELEASE_CANDIDATE_TESTING.md`
- `docs/EXPERIMENT_FRAMEWORK.md`
- `.agents/skills/README.md`
- `.agents/skills/dev-contextro-mcp/references/benchmark-results.md`
- `.agents/skills/dev-contextro-mcp/references/tool-decision-tree.md`
- `packages/skills/README.md`
- `crates/contextro-tools/src/tool_manifest/catalog.rs`
- `crates/contextro-server/src/bench.rs`
- `crates/contextro-server/src/study.rs`
- `scripts/release-candidate.sh`

When a README or skill conflicts with code, trust the code and update the docs.

## Contextro-Specific Drift Checks

Always verify:

- public tool count and tool names
- response-shape or routing changes
- benchmark commands and threshold values
- retained study numbers and token reductions
- release-candidate wrapper paths and generated artifacts
- active-repo and persistence semantics
- which skill directories are canonical versus derived

Do not repeat benchmark numbers from memory if `benchmark-results.md`, `bench.rs`, or
`study.rs` say otherwise.

## Method

### 1. Start From The Change Surface

Identify what changed:

- tool contract
- benchmark result
- architecture or runtime behavior
- release workflow
- skill guidance

Then find every doc surface that makes a promise about that behavior.

### 2. Update All Dependent Surfaces In One Pass

For Contextro, common linked surfaces are:

- `README.md`
- `CHANGELOG.md`
- `docs/*`
- `.agents/skills/*`
- derived `.github/skills/*` or `.opencode/skills/*`
- `packages/skills/README.md`

Do not fix one copy and leave the others stale.

### 3. Prefer Precise Claims Over Marketing Drift

Good docs state:

- the exact command
- the exact path
- the retained benchmark number and scope
- the condition under which the claim holds

Avoid vague claims like "very fast" when the repo already has defendable numbers.

### 4. Treat Benchmark Claims As Evidence-Bound

Before publishing a metric, confirm:

- which harness produced it
- whether it is retained/current or historical/older
- what task count, repo, or environment it refers to
- what guardrails were preserved

If the scope is ambiguous, clarify it instead of inflating the claim.

### 5. Preserve Reader Workflow

For docs that guide action, make the next step obvious:

- installation docs should end in a runnable command
- benchmark docs should point to the harness and result source
- release docs should show the exact wrapper or checklist path
- skill docs should route to the right adjacent skill when appropriate

## Output Format

Return work in this order:

1. `Doc surface`
2. `Authoritative source`
3. `Drift found`
4. `Update plan`
5. `Cross-doc sync`
6. `Result`

## Anti-Patterns

- updating a README without checking the tool manifest or benchmark code
- copying numbers from old studies after retained results changed
- fixing only `.github/skills` or only `.agents/skills`
- documenting a generated path without naming how it is produced
- writing release docs that do not match `release-candidate.sh`
