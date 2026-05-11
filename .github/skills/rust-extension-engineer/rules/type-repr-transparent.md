# type-repr-transparent

> Use `#[repr(transparent)]` for FFI newtypes.

## Why It Matters

`#[repr(transparent)]` guarantees the wrapper has the same ABI and layout as its inner field. That
matters for FFI handles and type-safe wrappers.

## Bad

```rust
struct Handle(u64);
extern "C" {
    fn process_handle(h: Handle);
}
```

## Good

```rust
#[repr(transparent)]
struct Handle(u64);

extern "C" {
    fn process_handle(h: Handle);
}
```

## Extra Checks

```rust
use std::mem::{align_of, size_of};
assert_eq!(size_of::<Handle>(), size_of::<u64>());
assert_eq!(align_of::<Handle>(), align_of::<u64>());
```

## See Also

- `api-newtype-safety.md`
- `type-phantom-marker.md`
- `api-sealed-trait.md`
