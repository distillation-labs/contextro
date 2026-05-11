# api-builder-pattern

> Use a builder for complex construction.

## Why It Matters

Builders keep constructors readable when there are many optional fields or validation steps.

## Bad

```rust
let client = Client::new(
    "https://api.example.com",
    30,
    true,
    None,
    Some("auth_token"),
    false,
);
```

## Good

```rust
#[derive(Default)]
#[must_use]
pub struct ClientBuilder {
    base_url: Option<String>,
    timeout: Option<std::time::Duration>,
    max_retries: u32,
}

impl ClientBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    pub fn build(self) -> Result<Client, BuilderError> {
        let base_url = self.base_url.ok_or(BuilderError::MissingBaseUrl)?;
        Ok(Client { base_url, timeout: self.timeout.unwrap_or_default(), max_retries: self.max_retries })
    }
}
```

## See Also

- `api-builder-must-use.md`
- `api-typestate.md`
- `api-impl-into.md`
