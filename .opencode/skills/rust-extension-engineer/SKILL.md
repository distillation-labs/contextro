---
name: rust-extension-engineer
description: >
  Use for deciding when and how to implement or refactor Rust extensions that back the Python
  MCP codebase, especially PyO3 or maturin-style native modules for hot paths. Trigger when the
  user asks whether code should move to Rust, how to design a Python-Rust boundary, how to keep
  parity between Rust and Python fallback paths, how to structure a PyO3 module, or how to keep
  Rust acceleration maintainable and benchmark-driven. Do not use for pure Python refactors or
  FastMCP server-shape questions.
when_to_use: >
  Especially useful for performance hot paths, FFI boundaries, PyO3 modules, maturin packaging,
  parity testing, error handling across the boundary, and deciding whether Rust is justified.
metadata:
  version: "0.1.0"
  category: mcp-development
  tags: [rust, pyo3, maturin, extension, ffi, performance, parity]
license: Proprietary
---

# Rust Extension Engineer

Use Rust to buy measurable performance or robustness, not novelty.

## Core Rules

- Profile first.
- Port only hot paths or correctness-critical low-level routines.
- Keep the Python-Rust boundary narrow and stable.
- Preserve parity with a Python fallback when that is part of the architecture.
- Benchmark and test both sides.

## Rust/Python Guidance

### When Rust Is Worth It

- heavy scanning, hashing, parsing, graph, or batch compute work
- code called frequently enough that Python overhead matters
- logic that benefits from Rust crates or memory behavior

Do not move logic to Rust just because it feels more serious.

### Boundary Design

- expose a small API with simple argument and return types
- keep ownership and conversions explicit
- make import or load failure degrade gracefully when a fallback exists
- avoid spreading FFI knowledge across the Python codebase

### PyO3 And Packaging

- use PyO3 for Python extension modules
- use maturin-style build and packaging patterns when shipping native modules
- ensure crate/module naming and exported symbols align with Python import expectations
- document build and platform assumptions clearly

### Errors And Safety

- surface meaningful errors across the boundary
- do not let Rust internals disappear into ambiguous Python failures
- preserve deterministic behavior and behavioral parity with the fallback path

### Benchmarks And Parity

- compare before vs after, not just Rust vs intuition
- keep parity tests or golden behavior checks for Python and Rust paths
- treat portability and build friction as real costs

## Output Format

Return:

1. whether Rust is justified
2. proposed boundary shape
3. PyO3/maturin implications
4. parity and fallback plan
5. benchmark and test plan

## Anti-Patterns

- porting code without profiling
- large unstable FFI surfaces
- no fallback or parity story when the architecture expects one
- treating build friction as free
- micro-optimizing Rust code before validating the right boundary

## References

- `references/rust-patterns.md`
- `references/eval-rubric.md`
