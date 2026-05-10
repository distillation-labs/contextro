# anti-unwrap-abuse

> Do not use `.unwrap()` in production code.

## Why It Matters

`.unwrap()` panics on `None` or `Err`, turning recoverable failures into crashes.

## Bad

```rust
let content = std::fs::read_to_string("config.toml").unwrap();
let num: i32 = user_input.parse().unwrap();
```

## Good

```rust
fn load_config() -> Result<Config, Error> {
    let content = std::fs::read_to_string("config.toml")?;
    Ok(toml::from_str(&content)?)
}
```

## When It Can Be Acceptable

```rust
#[test]
fn parse_valid_input() {
    let result = parse("valid").unwrap();
    assert_eq!(result, expected());
}
```

## See Also

- `err-result-over-panic.md`
- `err-expect-bugs-only.md`
- `anti-expect-lazy.md`
