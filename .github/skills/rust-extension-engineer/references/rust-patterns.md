# Rust Patterns

Grounded in the Rust API Guidelines, Rust Performance Book, PyO3, and the repo's extension-
boundary needs.

## Core Shape

- Profile first.
- Keep APIs narrow, typed, and explicit.
- Prefer borrowing and zero-copy when the source data already exists.
- Use Rust for hot paths, correctness-critical internals, or a boundary that benefits from strong
  typing.
- Keep the fallback path and parity story intact when the architecture expects it.

## Ownership

- Prefer `&str`, `&[T]`, and `&T` over `String`, `Vec<T>`, and cloned values.
- Use `Cow<'a, T>` when data is usually borrowed but sometimes owned.
- Use `Arc<T>` for shared cross-thread ownership and `Rc<T>` for single-threaded sharing.
- Choose `RefCell`, `Mutex`, or `RwLock` only when the concurrency model requires it.
- Clone explicitly and only when ownership is truly needed.

## Errors

- Use `thiserror` for public library errors.
- Use `anyhow` for application-level aggregation and context.
- Return `Result` for expected failures.
- Preserve error chains with `#[from]` and `#[source]`.
- Avoid `.unwrap()` and `.expect()` in production code.
- Keep error messages short, lowercase, and specific.

## Memory

- Use `with_capacity()` when the size is known.
- Reuse allocations with `clear()`, `drain()`, and `clone_from()`.
- Consider `SmallVec`, `ArrayVec`, `Box<[T]>`, or `Bytes` when the shape is bounded or shared.
- Prefer zero-copy slicing and parsing when the original buffer can stay alive.
- Avoid `format!()` in hot paths when `write!()` or a literal works.

## API Design

- Use a builder for many optional parameters or complex construction.
- Use newtypes for IDs, validated values, and handles.
- Use typestate for compile-time state machines.
- Seal traits when you need to preserve invariants.
- Accept `impl Into<T>` or `impl AsRef<T>` at boundaries when flexibility helps.
- Use `#[must_use]` on values that should not be silently dropped.
- Use `#[non_exhaustive]` when future expansion matters.

## Async

- Use Tokio for production async.
- Never hold a lock across `.await`.
- Use `spawn_blocking` for CPU-heavy work.
- Use `tokio::fs` in async code.
- Use `CancellationToken` for shutdown and cancellation.
- Use `join!`, `try_join!`, `select!`, `JoinSet`, and bounded channels for flow control.
- Clone or extract state before awaiting.

## Performance

- Measure before tuning.
- Optimize the actual hot path, not the suspected one.
- Prefer iterators over indexing in hot loops.
- Use `entry()` for map insert/update flows.
- Avoid `collect()` as an intermediate step.
- Use release profiling, LTO, and PGO when warranted.
- Consider cache-friendly layouts and SIMD only when the workload justifies it.

## Testing

- Use unit tests for focused behavior and integration tests for cross-module behavior.
- Use `proptest` for invariants and round trips.
- Use `criterion` for benchmarks and `black_box` inside them.
- Keep doc examples executable.
- Use async test harnesses for async code.

## Documentation And Linting

- Document all public items.
- Include `# Examples`, `# Errors`, `# Panics`, and `# Safety` when relevant.
- Use `cargo fmt --check` and Clippy in CI.
- Enable missing-docs and unsafe-block documentation checks.

## FFI And PyO3

- Keep the Rust-Python boundary small and stable.
- Expose simple argument and return types.
- Use `#[repr(transparent)]` for single-field FFI newtypes.
- Avoid leaking internal Rust structs into Python.
- Surface meaningful errors instead of opaque Python failures.
- Keep Python fallback behavior aligned with Rust behavior.
- Treat wheel building, ABI, and portability as first-class design constraints.

## Anti-Patterns

- Porting code without profiling.
- Large unstable FFI surfaces.
- Ignoring build or portability friction.
- Locking across `.await`.
- Using `&String` or `&Vec<T>` in APIs.
- `unwrap()` in production paths.
- Premature micro-optimization.
- Overly clever generic APIs that hide simple ownership or boundary rules.
