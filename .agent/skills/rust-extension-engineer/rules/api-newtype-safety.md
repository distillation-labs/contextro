# api-newtype-safety

> Use newtypes for type-safe distinctions.

## Why It Matters

Newtypes turn raw primitives into named domain concepts, which prevents accidental mixing and makes
APIs easier to read.

## Bad

```rust
fn load_user(user_id: u64, env: &str) {}
```

## Good

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Environment(pub String);

fn load_user(user_id: UserId, env: &Environment) {}
```

## FFI Note

```rust
#[repr(transparent)]
pub struct Handle(u64);
```

## See Also

- `type-newtype-ids.md`
- `type-repr-transparent.md`
- `type-no-stringly.md`
