# Python Systems Patterns

Grounded in the official Python docs and this repo's engineering style.

## Strong Practice

- explicit types and boundary contracts
- `Protocol` or abstract interfaces when behavior matters more than inheritance
- `TypedDict` or dataclass-shaped payloads for structured data
- meaningful exceptions and cleanup paths
- careful handling of shared state and concurrency
- modules organized around cohesive responsibilities

## Weak Practice

- `Any` spreading through interfaces
- giant modules that accumulate unrelated concerns
- implicit shared mutable state
- type complexity that does not improve correctness or readability
