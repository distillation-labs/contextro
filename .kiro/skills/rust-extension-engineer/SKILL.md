---
name: rust-extension-engineer
description: >
  Comprehensive Rust best-practices guidance for writing, reviewing, and refactoring Rust code,
  especially PyO3/maturin extensions, FFI boundaries, hot paths, and Python fallback parity in the
  MCP codebase.
when_to_use: >
  Use for Rust ownership, errors, async, memory, API design, testing, docs, linting, performance,
  FFI, and extension-boundary decisions. Profile first, keep boundaries small, and prefer
  benchmarked, testable designs over novelty.
metadata:
  version: "2.0.0"
  category: mcp-development
  tags: [rust, pyo3, maturin, ffi, ownership, errors, async, memory, api, testing, performance, linting]
  sources:
    - Rust API Guidelines
    - Rust Performance Book
    - PyO3 Guide
    - rust-skills-master
license: MIT
---

# Rust Extension Engineer

Rust should buy measurable performance, correctness, or a cleaner boundary. Use these rules for
Rust code generally, and especially when Rust sits behind a Python or FFI interface.

## Decision Order

- Profile first.
- Port only hot paths or correctness-critical low-level routines.
- Keep the Python-Rust boundary narrow, typed, and stable.
- Preserve parity with a Python fallback when one exists.
- Treat build, wheel, and portability friction as real costs.

## Rule Categories by Priority

| Priority | Category | Impact | Prefix | Rules |
|----------|----------|--------|--------|-------|
| 1 | Ownership & Borrowing | CRITICAL | `own-` | 12 |
| 2 | Error Handling | CRITICAL | `err-` | 12 |
| 3 | Memory Optimization | CRITICAL | `mem-` | 15 |
| 4 | API Design | HIGH | `api-` | 15 |
| 5 | Async/Await | HIGH | `async-` | 15 |
| 6 | Compiler Optimization | HIGH | `opt-` | 12 |
| 7 | Naming Conventions | MEDIUM | `name-` | 16 |
| 8 | Type Safety | MEDIUM | `type-` | 10 |
| 9 | Testing | MEDIUM | `test-` | 13 |
| 10 | Documentation | MEDIUM | `doc-` | 11 |
| 11 | Performance Patterns | MEDIUM | `perf-` | 11 |
| 12 | Project Structure | LOW | `proj-` | 11 |
| 13 | Clippy & Linting | LOW | `lint-` | 11 |
| 14 | Anti-patterns | REFERENCE | `anti-` | 15 |

## Quick Reference

### Ownership & Borrowing

- Prefer `&T`, `&str`, and `&[T]` over owned inputs when possible.
- Use `Cow<'a, T>` when ownership is conditional.
- Use `Arc<T>` and `Rc<T>` deliberately; use `Mutex`, `RwLock`, or `RefCell` only when they fit the concurrency model.
- Derive `Copy` only for small, trivial types.
- Make `Clone` explicit; move large data instead of cloning it.
- Let lifetime elision work when it keeps signatures simpler.

### Error Handling

- Use `thiserror` for library error types and `anyhow` for application-level aggregation.
- Return `Result`, use `?`, and attach context.
- Use `#[from]` and `#[source]` to preserve error chains.
- Avoid `.unwrap()` and `.expect()` in production code.
- Keep error messages lowercase and concise.
- Document fallible APIs with `# Errors`.

### Memory Optimization

- Use `with_capacity()` when you know the size.
- Reuse allocations with `clear()`, `drain()`, and `clone_from()`.
- Consider `SmallVec`, `ArrayVec`, or `Box<[T]>` when the shape is bounded.
- Prefer zero-copy slices or `Bytes` when possible.
- Use `Cow` when data is usually borrowed but sometimes owned.
- Avoid `format!()` in hot paths; prefer `write!()` or direct string literals.

### API Design

- Use a builder when construction has many optional fields.
- Use newtypes for IDs, validated values, and FFI-safe handles.
- Use typestate for compile-time state machines.
- Seal traits when you need to preserve invariants.
- Accept `impl Into<T>` or `impl AsRef<T>` for flexible inputs.
- Use `#[must_use]`, `#[non_exhaustive]`, `From`, `Default`, and common traits intentionally.
- Parse at boundaries; do not leave stringly APIs exposed.

### Async/Await

- Use Tokio for production async.
- Never hold a lock across `.await`.
- Use `spawn_blocking` for CPU-heavy work.
- Prefer `tokio::fs` in async code.
- Use `CancellationToken` for shutdown/cancellation.
- Reach for `join!`, `try_join!`, `select!`, `JoinSet`, and bounded channels when they fit the flow.
- Clone or extract state before awaiting.

### Compiler Optimization

- Benchmark before tuning.
- Use `#[inline]` for tiny hot functions; `#[inline(always)]` sparingly.
- Use `#[inline(never)]` or `#[cold]` for cold/error paths.
- Consider `lto`, `codegen-units = 1`, and PGO for release builds.
- Prefer iterator patterns that avoid bounds checks in hot loops.
- Use cache-friendly layouts and portable SIMD only when it is justified.

### Naming Conventions

- Use `UpperCamelCase` for types and variants.
- Use `snake_case` for functions, methods, and modules.
- Use `SCREAMING_SNAKE_CASE` for constants.
- Prefer `is_`, `has_`, and `can_` for boolean methods.
- Avoid `get_` for simple getters.
- Treat acronyms as words: `Uuid`, not `UUID`.
- Prefer crate names without `-rs`.

### Type Safety

- Wrap IDs and validated values in newtypes.
- Use enums for mutually exclusive states.
- Use `Option<T>` and `Result<T, E>` instead of sentinel values.
- Use `PhantomData<T>` for type-level markers.
- Use `!` for functions that never return.
- Add trait bounds only where needed.
- Use `#[repr(transparent)]` for FFI newtypes.

### Testing

- Keep unit tests in `#[cfg(test)] mod tests {}` and integration tests in `tests/`.
- Use `proptest` for invariants and round-trip behavior.
- Use `criterion` for benchmarks and `black_box` inside them.
- Use `tokio::test` for async tests.
- Prefer doctests and runnable examples.
- Use traits and mocks when dependencies need isolation.
- Keep test structure clear: arrange, act, assert.

### Documentation

- Document every public item with `///`.
- Use `//!` for module-level docs.
- Include `# Examples`, `# Errors`, `# Panics`, and `# Safety` where relevant.
- Use `?` in doc examples instead of `.unwrap()`.
- Hide setup noise with `#`.
- Use intra-doc links and keep `Cargo.toml` metadata current.

### Performance Patterns

- Prefer iterators over manual indexing.
- Keep iterators lazy; collect only once.
- Use `entry()` for map insert-or-update flows.
- Reuse buffers with `drain()` and `extend()`.
- Avoid `chain()` in hot loops.
- Use `collect_into()` when reusing a container.
- Measure with `black_box` and release-profile benchmarks.

### Project Structure

- Keep `main.rs` thin; put logic in `lib.rs`.
- Organize modules by feature, not by syntax kind.
- Use `pub(crate)` and `pub(super)` to keep surfaces small.
- Re-export public APIs intentionally with `pub use`.
- Use a `prelude` for common imports when it helps.
- Use workspaces for larger projects and `src/bin/` for multiple binaries.

### Clippy & Linting

- Enable correctness, suspicious, style, complexity, and perf lints.
- Use `clippy::pedantic` selectively.
- Enable `missing_docs` and `undocumented_unsafe_blocks`.
- Run `cargo fmt --check` in CI.
- Prefer workspace-level lint configuration.

### Anti-patterns

- Do not use `.unwrap()` or `.expect()` for recoverable errors.
- Do not clone when borrowing works.
- Do not hold locks across `.await`.
- Do not accept `&String` or `&Vec<T>` when `&str` or `&[T]` works.
- Do not index when iterators fit better.
- Do not panic on expected failures.
- Do not over-abstract or optimize before profiling.
- Do not use `Box<dyn Trait>` when `impl Trait` is enough.
- Do not stringly-type structured data.
- Do not `collect()` intermediate iterators in hot paths.
- Do not use `format!()` in hot loops.

## Rust Extension And FFI

- Use PyO3 for Python interop and maturin-style build/publish patterns.
- Keep the Rust-Python boundary small, typed, and stable.
- Prefer simple borrowed arguments and owned return values only when needed.
- Avoid leaking nested internal Rust structs across the boundary.
- Surface errors explicitly and preserve source chains.
- Use `#[repr(transparent)]` for single-field FFI newtypes and other boundary handles.
- Keep import/load failures graceful when a Python fallback exists.
- Preserve deterministic behavior between Rust and Python paths.
- Treat wheel portability, ABI constraints, and platform support as part of the design.
- Consider `abi3` only when it matches the API and deployment needs.
- Benchmark before and after; do not port code just because it is "more serious."

## Output Format

Return:

1. whether Rust is justified or which Rust rule family dominates
2. proposed API or boundary shape
3. ownership, error, async, and memory implications
4. testing, benchmark, doc, and lint plan
5. extension-specific notes for PyO3, fallback parity, and portability

If no extension boundary is involved, omit item 5.

## References

- `rules/index.md`
- `rules/`
- `references/rust-patterns.md`
- `references/eval-rubric.md`
