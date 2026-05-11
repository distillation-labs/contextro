# err-result-over-panic

> Return `Result` for expected failures instead of panicking.

## Why It Matters

Panic is a process-level failure. If the error is expected or recoverable, model it as `Result` so
the caller can decide what to do.

## Bad

```rust
fn load_config() -> Config {
    let content = std::fs::read_to_string("config.toml").unwrap();
    toml::from_str(&content).unwrap()
}
```

## Good

```rust
fn load_config() -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string("config.toml")?;
    Ok(toml::from_str(&content)?)
}
```

## When Panic Is Acceptable

```rust
// Tests, impossible invariants, or startup-only assumptions can justify panic.
assert!(!vec.is_empty());
let last = vec.pop().expect("checked non-empty above");
```

## See Also

- `err-no-unwrap-prod.md`
- `err-question-mark.md`
- `anti-panic-expected.md`
