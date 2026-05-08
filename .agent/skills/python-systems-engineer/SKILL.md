---
name: python-systems-engineer
description: >
  Use for Python-specific refactors and implementation decisions in Contextro: typing,
  dataclasses, exceptions, thread safety, interfaces, module structure, logging, validation,
  compatibility, and testability. Trigger when the user asks how to improve Python code quality,
  refactor Python modules safely, use typing well, reduce Any usage, improve maintainability, or
  encode good Python systems practices in the MCP codebase. Do not use for FastMCP-specific API
  questions or Rust extension boundary work.
when_to_use: >
  Especially useful for typed APIs, error handling, immutable models, concurrency boundaries,
  module organization, and keeping Python 3.10-3.12 compatible code clean and maintainable.
metadata:
  version: "1.0.0"
  category: mcp-development
  tags: [python, typing, refactor, dataclasses, exceptions, interfaces, maintainability]
license: MIT
---

# Python Systems Engineer

Write Python that is explicit, typed, testable, and easy for future agents to reason about.

## Core Rules

- Prefer precise types over `Any`.
- Use standard library typing features well.
- Make boundaries explicit: inputs, outputs, exceptions, and side effects.
- Keep concurrency and shared state visible.
- Refactor toward clarity, not abstraction for its own sake.

## Python Guidance

### Typing

- Use concrete container types and unions intentionally.
- Use `Protocol` for behavior-based interfaces where it improves decoupling.
- Use `TypedDict`, dataclasses, or well-typed models for structured payloads.
- Use `Self`, `Literal`, `Final`, and aliases when they clarify intent.
- Use `object` rather than `Any` when the value is unknown but should remain type-safe.

### Structure

- Keep modules cohesive.
- Keep top-level orchestration separate from domain logic.
- Extract helpers only when they clarify repeated logic or isolate complexity.
- Avoid giant god-modules that combine formatting, state, policies, and business logic.

### Exceptions And Validation

- Raise meaningful exceptions at the right layer.
- Validate at boundaries.
- Do not swallow errors that should be surfaced or logged.
- Use context managers and cleanup paths for resources.

### Concurrency And State

- Shared mutable state must be obvious and guarded.
- Use locks or safe ownership boundaries where concurrent tool calls exist.
- Document or encode invariants around caches, singletons, and background jobs.

### Compatibility And Tests

- Keep Python 3.10-3.12 compatibility in mind.
- Prefer standard typing forms supported by the repo target.
- Couple refactors to tests, lint, and clear behavioral equivalence.

## Output Format

Return:

1. Python code quality issue or opportunity
2. typing and structure recommendation
3. exception/state/concurrency notes
4. minimal refactor path
5. verification plan

## Anti-Patterns

- using `Any` as the default escape hatch
- refactoring into excessive abstraction
- hiding shared mutable state
- mixing boundary validation with unrelated business logic
- adding clever type machinery with no readability win

## References

- `references/python-patterns.md`
- `references/eval-rubric.md`
