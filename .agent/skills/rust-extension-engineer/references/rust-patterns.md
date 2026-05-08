# Rust Extension Patterns

Grounded in the Rust book, PyO3 guide, and the repo's `ctx_fast` architecture.

## Strong Practice

- use Rust for measured hot paths
- keep extension module boundaries small and typed
- use PyO3 for Python interop and maturin-style packaging patterns
- preserve a clean Python fallback when the product expects graceful degradation
- benchmark and parity-test the Rust and Python behaviors

## Weak Practice

- moving arbitrary business logic to Rust without evidence
- exposing a broad unstable FFI surface
- letting Python-side code become coupled to extension internals
- ignoring build, wheel, or portability costs
