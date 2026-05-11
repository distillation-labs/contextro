# err-thiserror-lib

> Use `thiserror` for library error types.

## Why It Matters

Libraries should expose typed, matchable errors so callers can handle specific failure modes.
`thiserror` keeps that ergonomic without hand-writing boilerplate.

## Bad

```rust
fn parse(input: &str) -> Result<Data, String> {
    Err("parse error".to_string())
}

fn load(path: &std::path::Path) -> Result<Data, Box<dyn std::error::Error>> {
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found")))
}
```

## Good

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid syntax at line {line}: {message}")]
    Syntax { line: usize, message: String },

    #[error("unexpected end of file")]
    UnexpectedEof,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

## Library Vs Application

| Context | Crate | Why |
|---------|-------|-----|
| Library | `thiserror` | Typed errors callers can match |
| Application | `anyhow` | Easy propagation with context |
| Both | `thiserror` publicly, `anyhow` internally | Best of both |

## See Also

- `err-anyhow-app.md`
- `err-from-impl.md`
- `err-source-chain.md`
