# doc-all-public

> Document all public items with `///`.

## Why It Matters

Public items define the crate contract. Without docs, users have to inspect source to know how to
use the API safely.

## Bad

```rust
pub struct Config {
    pub timeout: std::time::Duration,
    pub retries: u32,
}
```

## Good

```rust
/// Configuration for establishing a connection to the service.
///
/// # Examples
///
/// ```
/// use my_crate::Config;
/// # use std::time::Duration;
/// let config = Config { timeout: Duration::from_secs(30), retries: 3 };
/// ```
pub struct Config {
    /// Maximum time to wait for a response.
    pub timeout: std::time::Duration,
    /// Number of retry attempts.
    pub retries: u32,
}
```

## Also Document

- functions with `# Errors`
- unsafe functions with `# Safety`
- panicking APIs with `# Panics`

## See Also

- `doc-examples-section.md`
- `doc-errors-section.md`
- `lint-missing-docs.md`
